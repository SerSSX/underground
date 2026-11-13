//! Shared checks for user-supplied text (TASK-305 F1, F6, F7, F8, F10).
//!
//! - [`reject_nul`]: a router-wide middleware that answers 422 to any JSON body string, query
//!   string or path holding a NUL character, before any handler or the database sees it
//!   (Postgres refuses `\0` in text and the request used to end in a 500).
//! - [`clean_line`] / [`clean_multiline`]: strip C0/C1 control characters and the bidi
//!   embedding/override/isolate characters, keeping newlines and tabs in multi-line text.
//! - [`is_blank`]: whitespace and zero-width characters only.
//! - [`check_len`]: a cap counted in characters, not bytes.

use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::AppError;

/// The largest JSON body the middleware buffers: axum's own default `Json` limit, so no route
/// accepts less than before. A larger JSON body was already a 413 at the extractor.
const MAX_JSON_BODY: usize = 2 * 1024 * 1024;

/// The answer for a NUL anywhere in the input.
pub const NUL_REFUSED: &str = "текст містить недопустимий символ (NUL)";

/// Bidi embedding, override and isolate controls: they reorder how the text around them reads
/// (a file name or a price shown backwards). Marks such as U+200E/U+200F are left alone.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Zero-width characters that render as nothing.
fn is_zero_width(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}')
}

/// One-line text (names, titles, places): every control character and bidi control removed,
/// surrounding whitespace trimmed.
pub fn clean_line(s: &str) -> String {
    let kept: String = s.chars().filter(|c| !c.is_control() && !is_bidi_control(*c)).collect();
    kept.trim().to_string()
}

/// Multi-line text (descriptions, messages): as [`clean_line`] but newlines and tabs stay;
/// a carriage return is dropped (`\r\n` becomes `\n`).
pub fn clean_multiline(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|c| matches!(c, '\n' | '\t') || (!c.is_control() && !is_bidi_control(*c)))
        .collect();
    kept.trim().to_string()
}

/// True when nothing visible is left: only whitespace and zero-width characters.
pub fn is_blank(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace() || is_zero_width(c))
}

/// 422 when `s` is longer than `max` characters.
pub fn check_len(field: &str, s: &str, max: usize) -> Result<(), AppError> {
    if s.chars().count() > max {
        return Err(AppError::Validation(format!("{field}: не більше {max} символів")));
    }
    Ok(())
}

/// At most `max` characters, with an ellipsis when cut.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// True when a JSON body holds a NUL inside a string or key (TASK-305 F7, R7). A raw NUL byte is
/// invalid JSON and fails at the extractor; inside a string NUL can only arrive as the `\u0000`
/// escape. One pass over the bytes, tracking whether it is inside a string and skipping each
/// escaped character, so `\\u0000` (an escaped backslash, then text) is not NUL. Nothing is
/// allocated and nothing is parsed, so no size or nesting depth makes it slow or makes it give up
/// (the earlier `serde_json::Value` parse built the whole tree and returned "clean" past its
/// recursion limit). Only a string that closes counts: a body cut off inside its string is left
/// to the handler's own extractor (400), as before.
pub fn json_has_nul(bytes: &[u8]) -> bool {
    if !bytes.windows(6).any(|w| w == b"\\u0000") {
        return false;
    }
    let mut in_string = false;
    let mut nul_here = false;
    let mut i = 0;
    while i < bytes.len() {
        match (in_string, bytes[i]) {
            (false, b'"') => {
                in_string = true;
                nul_here = false;
                i += 1;
            }
            (false, _) => i += 1,
            (true, b'"') => {
                if nul_here {
                    return true;
                }
                in_string = false;
                i += 1;
            }
            (true, b'\\') => {
                if bytes.get(i + 1..i + 6) == Some(b"u0000".as_slice()) {
                    nul_here = true;
                    i += 6;
                } else {
                    // Any other escape: the backslash and the escaped byte (`\"` does not close).
                    i += 2;
                }
            }
            (true, _) => i += 1,
        }
    }
    false
}

fn uri_has_nul(uri: &axum::http::Uri) -> bool {
    let encoded = |s: &str| s.as_bytes().windows(3).any(|w| w == b"%00");
    encoded(uri.path()) || uri.query().is_some_and(encoded)
}

/// JSON, or no declared type at all (some handlers read the raw body: reserve, webhooks).
fn may_be_json(req: &Request) -> bool {
    match req.headers().get(axum::http::header::CONTENT_TYPE) {
        None => true,
        Some(v) => v.to_str().is_ok_and(|v| v.to_ascii_lowercase().contains("json")),
    }
}

/// Router-wide NUL guard (F7). JSON bodies are buffered (up to [`MAX_JSON_BODY`]) and passed on
/// unchanged when clean.
pub async fn reject_nul(req: Request, next: Next) -> Response {
    if uri_has_nul(req.uri()) {
        return AppError::Validation(NUL_REFUSED.into()).into_response();
    }
    if !may_be_json(&req) {
        return next.run(req).await;
    }
    let (parts, body) = req.into_parts();
    let Ok(bytes) = to_bytes(body, MAX_JSON_BODY).await else {
        return axum::http::StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    if json_has_nul(&bytes) {
        return AppError::Validation(NUL_REFUSED.into()).into_response();
    }
    next.run(Request::from_parts(parts, Body::from(bytes))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_line_strips_controls_and_bidi_but_not_letters() {
        assert_eq!(clean_line(" Ab\u{202E}c\u{0007}d\u{0085}e\u{2066}f\n "), "Abcdef");
        assert_eq!(clean_line("Київ 👨\u{200D}👩"), "Київ 👨\u{200D}👩", "ZWJ emoji kept");
    }

    #[test]
    fn clean_multiline_keeps_newlines_and_tabs() {
        assert_eq!(clean_multiline("a\r\nb\tc\u{202D}d\u{0000}e"), "a\nb\tcde");
    }

    #[test]
    fn blank_means_whitespace_or_zero_width() {
        assert!(is_blank(" \u{200B}\u{200C}\u{200D}\u{2060}\u{FEFF}\n"));
        assert!(!is_blank("\u{200B}a"));
    }

    #[test]
    fn length_counts_characters() {
        assert!(check_len("name", &"ї".repeat(80), 80).is_ok());
        assert!(check_len("name", &"ї".repeat(81), 80).is_err());
        assert_eq!(truncate_chars("абвгд", 3), "аб…");
        assert_eq!(truncate_chars("абв", 3), "абв");
    }

    #[test]
    fn json_nul_detection_is_exact() {
        assert!(json_has_nul(br#"{"a":["x\u0000y"]}"#));
        assert!(json_has_nul(br#"{"k\u0000":1}"#));
        assert!(
            !json_has_nul(br#"{"a":"literal \\u0000 text"}"#),
            "an escaped backslash is not NUL"
        );
        assert!(!json_has_nul(br#"{"a":"fine"}"#));
        assert!(!json_has_nul(b"{\"a\":\"\\u0000"), "unparseable: left to the handler");
    }

    /// R7: the scan tracks escapes, so an escaped quote does not end the string early and an
    /// escaped backslash does not start an escape.
    #[test]
    fn json_nul_scan_follows_escapes() {
        assert!(
            json_has_nul(br#"{"a":"q\"\u0000"}"#),
            "after an escaped quote, still inside the string"
        );
        assert!(
            json_has_nul(br#"["x\\", "\u0000"]"#),
            "a string ending in an escaped backslash closes"
        );
        assert!(
            !json_has_nul(br#"{"a":"\\\\u0000"}"#),
            "two escaped backslashes, then text"
        );
        assert!(json_has_nul(br#"{"a":"\\\u0000"}"#), "an escaped backslash, then NUL");
        assert!(
            !json_has_nul(br#"{"a":1} \u0000"#),
            "outside any string: not JSON text, left alone"
        );
    }

    /// R7: a 2 MB array nested far past serde_json's recursion limit (128) is still scanned to
    /// the end. The previous `serde_json::Value` parse gave up on it and let the NUL through.
    #[test]
    fn json_nul_scan_handles_a_2mb_deep_array_without_parsing() {
        let depth = 1_000_000;
        let mut body = Vec::with_capacity(2 * depth + 16);
        body.extend(std::iter::repeat_n(b'[', depth));
        body.extend_from_slice(br#""a\u0000b""#);
        body.extend(std::iter::repeat_n(b']', depth));
        assert!(body.len() > 2_000_000);
        assert!(
            serde_json::from_slice::<serde_json::Value>(&body).is_err(),
            "too deep for a Value parse"
        );
        let started = std::time::Instant::now();
        assert!(json_has_nul(&body), "[SEC] the NUL at depth 1 000 000 is found");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        let clean: Vec<u8> = body.iter().map(|&b| if b == b'\\' { b'x' } else { b }).collect();
        assert!(!json_has_nul(&clean));
    }
}
