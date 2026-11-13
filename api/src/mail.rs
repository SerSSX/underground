//! Account mail through Resend (TASK-304): password-reset links and email login codes.
//!
//! Switched at RUNTIME: mail is on only when both `RESEND_API_KEY` and `EMAIL_FROM` are set
//! (`GET /api/features` reports it, and the web offers "Forgot password?" and code login only
//! then). With mail off the endpoints answer exactly as before and nothing is sent.
//!
//! Delivery is a background task per mail ([`enqueue`]): the request that asked for it never
//! waits on Resend, so Resend's latency or failure does not show in the answer. The answer itself
//! is the same for any address; its timing still differs slightly (a reset writes a token row only
//! for a registered address), and account existence is not secret anyway: sign-up answers 409 for a
//! registered address (rate-limited). Messages live only in memory until sent — a reset token or a login code is
//! never written anywhere in plain text (the database keeps their hashes only). A restart loses
//! what was in flight; the user asks again.
//!
//! Transport: `POST {RESEND_URL}/emails` with `Authorization: Bearer <key>`, a 5 s timeout, no
//! redirects, no proxy, an `Idempotency-Key` per mail (so a retry can never send twice), at most
//! two retries with backoff and jitter on 429/5xx/network errors. Logs carry the mail kind, the
//! HTTP status and Resend's message id — never the key, the recipient or the body.
//!
//! Abuse bound (TASK-304 review): every mail first takes a slot from [`reserve`] — at most
//! `MAIL_DAILY_BUDGET` (default 80, under Resend's free 100/day) in any 24 h, at most
//! `MAIL_DAILY_PER_IP` (default 10) asked for by one client IP in 24 h, and at most
//! [`PER_RECIPIENT_HOURLY`] to one address in any hour (so a targeted lockout ends about an hour
//! after the attacker stops). Counted in the DATABASE (`mail_sends`, recipient and IP as SHA-256
//! hashes), not in process memory, so it survives restarts and holds with several API replicas.
//! Residual (owner's call, docs/decisions.md): many IPs can still spend the daily budget or keep
//! delaying one address. A spent budget is answered honestly (503 "mail temporarily unavailable"), never as
//! "sent"; the decision is taken before any account lookup, identically for every address.
//!
//! The order-confirmation and notification outboxes in `email.rs` are NOT sent through this
//! module (they stay logged-only until the owner asks for them).

use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::Semaphore;

use crate::error::AppError;

/// Resend's API host; tests point `RESEND_URL` at a stub on 127.0.0.1.
pub const DEFAULT_RESEND_URL: &str = "https://api.resend.com";
const TIMEOUT: Duration = Duration::from_secs(5);
/// Extra attempts after the first one (bounded, §7).
const RETRIES: u32 = 2;
/// Mails in flight at once; the rest wait their turn.
static IN_FLIGHT: Semaphore = Semaphore::const_new(4);

/// Mail settings from the environment, read on every use so a restart with a new `.env` is all
/// it takes to switch mail on or off.
#[derive(Clone)]
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
#[derive(Clone)]
pub struct Mail {
    /// A short label for logs ("password_reset", "login_code"); never personal data.
    pub kind: &'static str,
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

/// The key is never printed, not even by `{:?}`.
impl std::fmt::Debug for MailConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailConfig")
            .field("api_key", &"<redacted>")
            .field("from", &self.from)
            .field("endpoint", &self.endpoint.as_str())
            .finish()
    }
}

/// A mail carries a reset token or a login code and a person's address: `{:?}` shows the kind only.
impl std::fmt::Debug for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mail").field("kind", &self.kind).finish_non_exhaustive()
    }
}

/// One plain address: exactly one `@`, something on both sides, 3–254 characters, and none of
/// the characters that let one string name several recipients or a display name
/// (`,` `;` `<` `>` `"`, whitespace, control characters).
pub fn is_single_address(email: &str) -> bool {
    let n = email.chars().count();
    if !(3..=254).contains(&n) || email.matches('@').count() != 1 {
        return false;
    }
    if email
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, ',' | ';' | '<' | '>' | '"'))
    {
        return false;
    }
    let (local, domain) = email.split_once('@').unwrap_or_default();
    !local.is_empty() && !domain.is_empty()
}

/// Most mails to one address in any rolling hour.
pub const PER_RECIPIENT_HOURLY: i64 = 5;
const DEFAULT_DAILY_BUDGET: i64 = 80;
const DEFAULT_DAILY_PER_IP: i64 = 10;
/// Advisory-lock namespace for the budget (count + insert are serialised).
const BUDGET_LOCK: i32 = 0x4d41_494c; // "MAIL"

/// How `mail_sends` stores a recipient: SHA-256 hex of the normalised address (no plain PII).
pub fn recipient_hash(email: &str) -> String {
    hex::encode(Sha256::digest(email.trim().to_lowercase().as_bytes()))
}

/// How `mail_sends` stores the client IP: SHA-256 hex (no plain IP at rest).
pub fn ip_hash(ip: &str) -> String {
    hex::encode(Sha256::digest(ip.trim().as_bytes()))
}

fn daily_budget() -> i64 {
    crate::ratelimit::env_limit("MAIL_DAILY_BUDGET", DEFAULT_DAILY_BUDGET)
}

/// Take one slot of the mail budget for `to`, asked for from `ip`. Callers run this BEFORE they look
/// the address up, for every address alike, so the outcome never depends on whether an account
/// exists. Spent global budget → 503 "mail temporarily unavailable"; this IP's daily cap or this
/// address's hourly cap → 429. Logs one WARN when the slot taken is the last one of the budget.
pub async fn reserve(db: &PgPool, to: &str, ip: &str) -> Result<(), AppError> {
    let budget = daily_budget();
    let per_ip = crate::ratelimit::env_limit("MAIL_DAILY_PER_IP", DEFAULT_DAILY_PER_IP);
    let hash = recipient_hash(to);
    let ip_h = ip_hash(ip);
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1, 0)")
        .bind(BUDGET_LOCK)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM mail_sends WHERE sent_at <= now() - interval '1 day'")
        .execute(&mut *tx)
        .await?;
    let (total, from_ip, to_addr): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE ip_hash = $2), \
                count(*) FILTER (WHERE to_hash = $1 AND sent_at > now() - interval '1 hour') \
         FROM mail_sends WHERE sent_at > now() - interval '1 day'",
    )
    .bind(&hash)
    .bind(&ip_h)
    .fetch_one(&mut *tx)
    .await?;
    if total >= budget {
        return Err(AppError::Unavailable(
            "пошта тимчасово недоступна, спробуйте пізніше".into(),
        ));
    }
    if from_ip >= per_ip || to_addr >= PER_RECIPIENT_HOURLY {
        return Err(AppError::RateLimited);
    }
    sqlx::query("INSERT INTO mail_sends (to_hash, ip_hash) VALUES ($1, $2)")
        .bind(&hash)
        .bind(&ip_h)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if total + 1 == budget {
        tracing::warn!(target: "email", budget, "mail: the daily budget is used up; mail is refused until slots free up");
    }
    Ok(())
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
    if !is_single_address(&mail.to) {
        tracing::warn!(target: "email", kind = mail.kind, "mail refused: recipient is not one plain address");
        return;
    }
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
    fn single_address_only() {
        assert!(is_single_address("a@b.co"));
        assert!(is_single_address("first.last+tag@sub.example.org"));
        for bad in [
            "a@b.co,c@d.co",
            "a@b.co;c@d.co",
            "A <a@b.co>",
            "a@b.co c@d.co",
            "a@@b.co",
            "a@b@c.co",
            "@b.co",
            "a@",
            "nope",
            "a\"@b.co",
            "a@b.co\n",
        ] {
            assert!(!is_single_address(bad), "{bad:?}");
        }
    }

    #[test]
    fn debug_never_shows_secrets() {
        let c = MailConfig::resolve(s("re_secret_value"), s("a@b.co"), None).unwrap();
        assert!(!format!("{c:?}").contains("re_secret_value"));
        let m = code_mail(Some("en"), "victim@x.co", "123456", 10);
        let d = format!("{m:?}");
        assert!(!d.contains("123456") && !d.contains("victim@x.co"), "{d}");
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
