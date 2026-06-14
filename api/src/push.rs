//! Push-notification outbox (TASK-017 M4). Every in-app notification is also a
//! push candidate: this module selects un-pushed notifications fanned out to each
//! recipient device, and marks them delivered. The actual FCM HTTP call lives in
//! the binary's worker (`main.rs`) — intentionally outside coverage, since it needs
//! live Firebase credentials — while the selection/marking logic here is tested.

use sqlx::PgPool;
use uuid::Uuid;

/// A single pending delivery: one notification destined for one device token.
#[derive(Debug, sqlx::FromRow)]
pub struct PendingPush {
    pub notification_id: Uuid,
    pub token: String,
    pub platform: String,
    pub title: String,
    pub body: Option<String>,
    pub kind: String,
    pub event_id: Option<Uuid>,
}

/// Notifications not yet pushed, fanned out to every registered device of each
/// recipient, oldest first. `limit` bounds the number of NOTIFICATIONS (not rows):
/// each selected notification fans out to ALL its devices, so the watermark the worker
/// applies per notification can never strand undelivered devices beyond a batch edge.
pub async fn pending(db: &PgPool, limit: i64) -> Result<Vec<PendingPush>, sqlx::Error> {
    sqlx::query_as(
        "SELECT n.id AS notification_id, d.token, d.platform, n.title, n.body, n.kind, n.event_id \
         FROM ( \
            SELECT id, user_id, title, body, kind, event_id, created_at \
            FROM notifications WHERE pushed_at IS NULL \
            ORDER BY created_at ASC LIMIT $1 \
         ) n \
         JOIN device_tokens d ON d.user_id = n.user_id \
         ORDER BY n.created_at ASC",
    )
    .bind(limit)
    .fetch_all(db)
    .await
}

/// Mark notifications delivered so the outbox does not re-push them.
pub async fn mark_pushed(db: &PgPool, ids: &[Uuid]) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("UPDATE notifications SET pushed_at = now() WHERE id = ANY($1)")
        .bind(ids)
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

/// FCM credentials resolved from the environment. `None` means push delivery is
/// disabled — the worker logs delivery intent instead of calling Firebase. This
/// mirrors the test-mode payments seam: the plumbing is always present; live
/// delivery switches on when `FCM_PROJECT_ID` + `FCM_SA_KEY` are configured.
#[derive(Clone, Debug)]
pub struct FcmConfig {
    /// Firebase project id (the `messages:send` URL embeds it).
    pub project_id: String,
    /// Path to the service-account JSON key used to mint OAuth2 access tokens.
    pub service_account_path: String,
}

impl FcmConfig {
    /// Read FCM settings from the process environment (call after dotenv).
    pub fn from_env() -> Option<Self> {
        Self::resolve(std::env::var("FCM_PROJECT_ID").ok(), std::env::var("FCM_SA_KEY").ok())
    }

    /// Pure resolution (testable without touching env). Both values must be
    /// present and non-empty for delivery to be enabled.
    fn resolve(project_id: Option<String>, service_account_path: Option<String>) -> Option<Self> {
        match (project_id, service_account_path) {
            (Some(project_id), Some(service_account_path))
                if !project_id.is_empty() && !service_account_path.is_empty() =>
            {
                Some(Self {
                    project_id,
                    service_account_path,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_resolves_when_both_present() {
        let c = FcmConfig::resolve(Some("proj".into()), Some("/key.json".into()));
        let c = c.expect("both present → Some");
        assert_eq!(c.project_id, "proj");
        assert_eq!(c.service_account_path, "/key.json");
    }

    #[test]
    fn config_disabled_when_missing_or_empty() {
        assert!(FcmConfig::resolve(None, Some("/k".into())).is_none());
        assert!(FcmConfig::resolve(Some("p".into()), None).is_none());
        assert!(FcmConfig::resolve(None, None).is_none());
        // present-but-empty counts as disabled
        assert!(FcmConfig::resolve(Some(String::new()), Some("/k".into())).is_none());
        assert!(FcmConfig::resolve(Some("p".into()), Some(String::new())).is_none());
    }

    #[test]
    fn from_env_smoke() {
        // Just exercises the env-reading path; result depends on the environment.
        let _ = FcmConfig::from_env();
    }
}
