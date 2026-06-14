//! Transactional email (P1 #75). Order confirmations are delivered via an outbox: paid
//! orders carry `email_sent = false` until an email worker drains them. The testable logic
//! (config resolution, selection, watermarking, template) lives here; the SendGrid HTTP
//! transport is a follow-up in `main.rs` (env-gated, like FCM) so deliveries are observable
//! end-to-end before the network call is wired.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// SendGrid settings; both the worker and the gate read these from the environment.
pub struct EmailConfig {
    pub api_key: String,
    pub from: String,
}

impl EmailConfig {
    /// Read email settings from the process environment (call after dotenv).
    pub fn from_env() -> Option<Self> {
        Self::resolve(std::env::var("SENDGRID_API_KEY").ok(), std::env::var("EMAIL_FROM").ok())
    }

    /// Pure resolution (testable without touching env). An empty/absent API key disables
    /// delivery; the from-address falls back to a default.
    fn resolve(api_key: Option<String>, from: Option<String>) -> Option<Self> {
        let api_key = api_key.filter(|s| !s.is_empty())?;
        let from = from
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "tickets@underground.app".to_string());
        Some(Self { api_key, from })
    }
}

/// A paid order awaiting its confirmation email.
#[derive(sqlx::FromRow)]
pub struct PendingEmail {
    pub order_id: Uuid,
    pub to_email: String,
    pub event_title: String,
    pub amount: Decimal,
    pub currency: String,
}

/// Paid orders that have not been emailed yet (oldest first).
pub async fn pending(db: &PgPool, limit: i64) -> Result<Vec<PendingEmail>, sqlx::Error> {
    sqlx::query_as::<_, PendingEmail>(
        "SELECT o.id AS order_id, u.email AS to_email, e.title AS event_title, o.amount, o.currency \
         FROM orders o JOIN users u ON u.id = o.buyer_id JOIN events e ON e.id = o.event_id \
         WHERE o.status = 'paid' AND o.email_sent = false AND u.blocked = false \
         ORDER BY o.created_at ASC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(db)
    .await
}

/// Watermark the given orders as emailed (idempotent re-send guard).
pub async fn mark_sent(db: &PgPool, order_ids: &[Uuid]) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE orders SET email_sent = true WHERE id = ANY($1)")
        .bind(order_ids)
        .execute(db)
        .await?;
    Ok(())
}

/// Build the (subject, body) of an order-confirmation email (pure → unit-testable).
pub fn build_confirmation(p: &PendingEmail) -> (String, String) {
    let subject = format!("Ваш квиток: {}", p.event_title);
    let body = format!(
        "Дякуємо за покупку!\n\nПодія: {}\nСума: {} {}\nЗамовлення: {}\n\n\
         Квиток доступний у застосунку underground.",
        p.event_title, p.amount, p.currency, p.order_id
    );
    (subject, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_resolves_when_key_present() {
        let c = EmailConfig::resolve(Some("sg_key".into()), Some("a@b.co".into())).expect("present → Some");
        assert_eq!(c.api_key, "sg_key");
        assert_eq!(c.from, "a@b.co");
        // Default from-address when unset.
        let c = EmailConfig::resolve(Some("sg_key".into()), None).expect("key present → Some");
        assert_eq!(c.from, "tickets@underground.app");
    }

    #[test]
    fn config_disabled_without_key() {
        assert!(EmailConfig::resolve(None, Some("a@b.co".into())).is_none());
        assert!(EmailConfig::resolve(Some(String::new()), None).is_none());
    }

    #[test]
    fn from_env_smoke() {
        let _ = EmailConfig::from_env();
    }

    #[test]
    fn confirmation_template_includes_details() {
        let p = PendingEmail {
            order_id: Uuid::nil(),
            to_email: "b@x.co".into(),
            event_title: "Gig".into(),
            amount: Decimal::new(8000, 2),
            currency: "UAH".into(),
        };
        let (subject, body) = build_confirmation(&p);
        assert!(subject.contains("Gig"));
        assert!(body.contains("80.00"));
        assert!(body.contains("UAH"));
    }
}
