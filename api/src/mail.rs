//! Account mail through Resend (TASK-304): password-reset links and email login codes.
//!
//! Switched at RUNTIME: mail is on only when both `RESEND_API_KEY` and `EMAIL_FROM` are set
//! (`GET /api/features` reports it, and the web offers "Forgot password?" and code login only
//! then). With mail off the endpoints answer exactly as before and nothing is sent.
//!
//! Delivery is a background task per mail ([`enqueue`]): the request that asked for it never
//! waits on Resend, so its answer and its timing do not depend on whether a mail went out (no
//! account oracle). Messages live only in memory until sent — a reset token or a login code is
//! never written anywhere in plain text (the database keeps their hashes only). A restart loses
//! what was in flight; the user asks again.
//!
//! Transport: `POST {RESEND_URL}/emails` with `Authorization: Bearer <key>`, a 5 s timeout, no
//! redirects, no proxy, an `Idempotency-Key` per mail (so a retry can never send twice), at most
//! two retries with backoff and jitter on 429/5xx/network errors. Logs carry the mail kind, the
//! HTTP status and Resend's message id — never the key, the recipient or the body.
//!
//! The order-confirmation and notification outboxes in `email.rs` are NOT sent through this
//! module (they stay logged-only until the owner asks for them).

use std::time::Duration;

use serde::Serialize;
use tokio::sync::Semaphore;

/// Resend's API host; tests point `RESEND_URL` at a stub on 127.0.0.1.
pub const DEFAULT_RESEND_URL: &str = "https://api.resend.com";
const TIMEOUT: Duration = Duration::from_secs(5);
/// Extra attempts after the first one (bounded, §7).
const RETRIES: u32 = 2;
/// Mails in flight at once; the rest wait their turn.
static IN_FLIGHT: Semaphore = Semaphore::const_new(4);

/// Mail settings from the environment, read on every use so a restart with a new `.env` is all
/// it takes to switch mail on or off.
#[derive(Clone, Debug)]
pub struct MailConfig {
    api_key: String,
    pub from: String,
    endpoint: reqwest::Url,
}

impl MailConfig {
    pub fn from_env() -> Option<Self> {
        Self::resolve(
            std::env::var("RESEND_API_KEY").ok(),
            std::env::var("EMAIL_FROM").ok(),
            std::env::var("RESEND_URL").ok(),
        )
    }

    /// `None` (mail off) unless the key and the from-address are both non-empty and the base URL
    /// is https, or plain http to a loopback host (the test stub). Pure, so it is unit-tested.
    pub fn resolve(api_key: Option<String>, from: Option<String>, base: Option<String>) -> Option<Self> {
        let api_key = api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty())?;
        let from = from.map(|f| f.trim().to_string()).filter(|f| f.contains('@'))?;
        if from.chars().any(char::is_control) || api_key.chars().any(|c| c.is_control() || c == ' ') {
            return None;
        }
        let base = base.map(|b| b.trim().to_string()).filter(|b| !b.is_empty());
        let base = base.as_deref().unwrap_or(DEFAULT_RESEND_URL);
        let mut url = reqwest::Url::parse(base).ok()?;
        let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        let ok = (url.scheme() == "https" || (url.scheme() == "http" && loopback))
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none();
        if !ok {
            return None;
        }
        url.path_segments_mut().ok()?.pop_if_empty().push("emails");
        Some(Self {
            api_key,
            from,
            endpoint: url,
        })
    }
}

/// True when account mail can be sent (`GET /api/features` → `email`).
pub fn enabled() -> bool {
    MailConfig::from_env().is_some()
}

/// One message: a plain-text part and an HTML part.
#[derive(Clone, Debug)]
pub struct Mail {
    /// A short label for logs ("password_reset", "login_code"); never personal data.
    pub kind: &'static str,
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

#[derive(Serialize)]
struct ResendBody<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: &'a str,
    html: &'a str,
}

/// Hand a mail to the background sender and return at once. With mail off it is dropped (logged
/// by kind only). Must be called inside a Tokio runtime (every handler is).
pub fn enqueue(mail: Mail) {
    let Some(cfg) = MailConfig::from_env() else {
        tracing::info!(target: "email", kind = mail.kind, "mail off (no RESEND_API_KEY/EMAIL_FROM): not sent");
        return;
    };
    tokio::spawn(async move {
        let _permit = IN_FLIGHT.acquire().await;
        match send(&cfg, &mail).await {
            Ok(id) => tracing::info!(target: "email", kind = mail.kind, id = %id, "mail sent"),
            Err(e) => tracing::warn!(target: "email", kind = mail.kind, error = %e, "mail not sent"),
        }
    });
}

/// Why a send failed, safe to log: a status code or an error class, nothing from the request.
#[derive(Debug)]
pub enum SendError {
    Status(u16),
    Network(&'static str),
    Client,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Status(s) => write!(f, "resend answered HTTP {s}"),
            SendError::Network(k) => write!(f, "network: {k}"),
            SendError::Client => write!(f, "http client could not be built"),
        }
    }
}

/// Send one mail, retrying transient failures. Returns Resend's message id (or "" when absent).
pub async fn send(cfg: &MailConfig, mail: &Mail) -> Result<String, SendError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(TIMEOUT)
        .timeout(TIMEOUT)
        .build()
        .map_err(|_| SendError::Client)?;
    let body = serde_json::to_vec(&ResendBody {
        from: &cfg.from,
        to: [&mail.to],
        subject: &mail.subject,
        text: &mail.text,
        html: &mail.html,
    })
    .map_err(|_| SendError::Client)?;
    // One key for every attempt: Resend sends a mail at most once per key (24 h).
    let idem = uuid::Uuid::new_v4().to_string();
    let mut attempt = 0;
    loop {
        let res = client
            .post(cfg.endpoint.clone())
            .bearer_auth(&cfg.api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("Idempotency-Key", &idem)
            .body(body.clone())
            .send()
            .await;
        let err = match res {
            Ok(r) if r.status().is_success() => {
                let id = r
                    .bytes()
                    .await
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .and_then(|v| v["id"].as_str().map(str::to_string))
                    .unwrap_or_default();
                return Ok(id);
            }
            Ok(r) => SendError::Status(r.status().as_u16()),
            Err(e) if e.is_timeout() => SendError::Network("timeout"),
            Err(e) if e.is_connect() => SendError::Network("connect"),
            Err(_) => SendError::Network("request"),
        };
        let transient = match err {
            SendError::Status(s) => s == 429 || s >= 500,
            SendError::Network(_) => true,
            SendError::Client => false,
        };
        if !transient || attempt >= RETRIES {
            return Err(err);
        }
        attempt += 1;
        tokio::time::sleep(backoff(attempt)).await;
    }
}

/// 1 s, 3 s … plus up to 500 ms of jitter.
fn backoff(attempt: u32) -> Duration {
    let base = 1000 * (3u64.pow(attempt - 1));
    let jitter = u64::from(uuid::Uuid::new_v4().as_bytes()[0]) * 2;
    Duration::from_millis(base + jitter)
}

/// Mail language: Ukrainian for `uk`, English for every other locale (and for none).
pub fn mail_lang(locale: Option<&str>) -> &'static str {
    match locale.map(str::trim) {
        Some("uk") => "uk",
        _ => "en",
    }
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// A small HTML body: paragraphs, and an optional button (label, href) after the first one.
fn html_page(lang: &str, paragraphs: &[String], button: Option<(&str, &str)>) -> String {
    let mut body = String::new();
    for (i, p) in paragraphs.iter().enumerate() {
        body.push_str(&format!(
            "<p style=\"margin:0 0 16px;font-size:15px;line-height:1.5\">{}</p>",
            escape_html(p)
        ));
        if i == 0 {
            if let Some((label, href)) = button {
                body.push_str(&format!(
                    "<p style=\"margin:0 0 20px\"><a href=\"{h}\" style=\"display:inline-block;background:#6d5cff;\
                     color:#ffffff;text-decoration:none;font-weight:700;padding:12px 20px;border-radius:10px\">{l}</a></p>\
                     <p style=\"margin:0 0 16px;font-size:13px;color:#555;word-break:break-all\">{h}</p>",
                    h = escape_html(href),
                    l = escape_html(label),
                ));
            }
        }
    }
    format!(
        "<!doctype html><html lang=\"{lang}\"><body style=\"margin:0;padding:24px;background:#f4f4f6;\
         font-family:-apple-system,Segoe UI,Roboto,Arial,sans-serif;color:#111\"><div style=\"max-width:520px;\
         margin:0 auto;background:#fff;border-radius:14px;padding:28px\"><p style=\"margin:0 0 20px;font-weight:900;\
         letter-spacing:.3em;font-size:13px\">UNDERGROUND</p>{body}</div></body></html>"
    )
}

/// Minutes a password-reset link stays valid (see `passwords::RESET_TTL_MINS`).
pub fn reset_mail(locale: Option<&str>, to: &str, link: &str, ttl_mins: i64) -> Mail {
    let lang = mail_lang(locale);
    let (subject, lede, button, ignore) = if lang == "uk" {
        (
            "Скидання пароля Underground".to_string(),
            format!("Ви попросили скинути пароль до акаунта Underground. Відкрийте посилання протягом {ttl_mins} хвилин, щоб задати новий пароль."),
            "Задати новий пароль",
            "Якщо це були не ви, просто проігноруйте цей лист: пароль не зміниться.".to_string(),
        )
    } else {
        (
            "Reset your Underground password".to_string(),
            format!("You asked to reset the password of your Underground account. Open the link within {ttl_mins} minutes to set a new password."),
            "Set a new password",
            "If this wasn't you, just ignore this email: your password stays the same.".to_string(),
        )
    };
    let text = format!("{lede}\n\n{link}\n\n{ignore}\n");
    let html = html_page(lang, &[lede, ignore], Some((button, link)));
    Mail {
        kind: "password_reset",
        to: to.to_string(),
        subject,
        text,
        html,
    }
}

/// The one-time login code mail.
pub fn code_mail(locale: Option<&str>, to: &str, code: &str, ttl_mins: i64) -> Mail {
    let lang = mail_lang(locale);
    let (subject, lede, ignore) = if lang == "uk" {
        (
            format!("Код входу в Underground: {code}"),
            format!("Ваш код для входу в Underground: {code}. Він діє {ttl_mins} хвилин."),
            "Якщо ви не намагалися увійти, просто проігноруйте цей лист.".to_string(),
        )
    } else {
        (
            format!("Your Underground sign-in code: {code}"),
            format!("Your code to sign in to Underground: {code}. It is valid for {ttl_mins} minutes."),
            "If you didn't try to sign in, just ignore this email.".to_string(),
        )
    };
    let text = format!("{lede}\n\n{ignore}\n");
    let html = html_page(lang, &[lede, ignore], None);
    Mail {
        kind: "login_code",
        to: to.to_string(),
        subject,
        text,
        html,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn config_needs_key_and_from() {
        assert!(MailConfig::resolve(s("re_x"), s("U <a@b.co>"), None).is_some());
        assert!(MailConfig::resolve(None, s("a@b.co"), None).is_none(), "no key → off");
        assert!(
            MailConfig::resolve(s("  "), s("a@b.co"), None).is_none(),
            "blank key → off"
        );
        assert!(MailConfig::resolve(s("re_x"), None, None).is_none(), "no from → off");
        assert!(
            MailConfig::resolve(s("re_x"), s(""), None).is_none(),
            "empty from → off"
        );
        assert!(MailConfig::resolve(s("re_x"), s("no-at-sign"), None).is_none());
        assert!(
            MailConfig::resolve(s("re_x"), s("a@b.co\r\nBcc: x@y.z"), None).is_none(),
            "control chars → off"
        );
    }

    #[test]
    fn endpoint_defaults_to_resend_and_only_https_or_loopback_http() {
        let c = MailConfig::resolve(s("k"), s("a@b.co"), None).unwrap();
        assert_eq!(c.endpoint.as_str(), "https://api.resend.com/emails");
        let c = MailConfig::resolve(s("k"), s("a@b.co"), s("http://127.0.0.1:4010")).unwrap();
        assert_eq!(c.endpoint.as_str(), "http://127.0.0.1:4010/emails");
        let c = MailConfig::resolve(s("k"), s("a@b.co"), s("https://mail.example/v1/")).unwrap();
        assert_eq!(c.endpoint.as_str(), "https://mail.example/v1/emails");
        assert!(
            MailConfig::resolve(s("k"), s("a@b.co"), s("http://api.resend.com")).is_none(),
            "[SEC] no plain http off-box"
        );
        assert!(MailConfig::resolve(s("k"), s("a@b.co"), s("https://u:p@x.co")).is_none());
        assert!(MailConfig::resolve(s("k"), s("a@b.co"), s("ftp://x.co")).is_none());
    }

    #[test]
    fn mail_language_is_uk_or_en() {
        assert_eq!(mail_lang(Some("uk")), "uk");
        assert_eq!(mail_lang(Some("en")), "en");
        assert_eq!(mail_lang(Some("ja")), "en");
        assert_eq!(mail_lang(None), "en");
    }

    #[test]
    fn reset_mail_has_link_in_both_parts_and_is_localised() {
        let m = reset_mail(Some("uk"), "a@b.co", "https://x.co/reset-password?token=abc", 30);
        assert!(m.subject.contains("Скидання"));
        assert!(m.text.contains("https://x.co/reset-password?token=abc"));
        assert!(m.text.contains("30"));
        assert!(m.html.contains("https://x.co/reset-password?token=abc"));
        let m = reset_mail(Some("de"), "a@b.co", "https://x.co/r?token=abc", 30);
        assert!(m.subject.starts_with("Reset"), "other languages get English");
        assert_eq!(m.kind, "password_reset");
    }

    #[test]
    fn code_mail_carries_the_code() {
        let m = code_mail(Some("en"), "a@b.co", "123456", 10);
        assert!(m.subject.contains("123456") && m.text.contains("123456") && m.html.contains("123456"));
        let m = code_mail(Some("uk"), "a@b.co", "654321", 10);
        assert!(m.text.contains("654321") && m.text.contains("10 хвилин"));
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(
            escape_html("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        let h = html_page("en", &["<script>".into()], Some(("go", "https://x.co/?a=1&b=\"2\"")));
        assert!(!h.contains("<script>"));
        assert!(h.contains("a=1&amp;b=&quot;2&quot;"));
    }
}
