use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use underground_api::config::Config;
use underground_api::email::{self, EmailConfig};
use underground_api::push::{self, FcmConfig};
use underground_api::state::AppState;
use underground_api::{db, escrow, payments, reminders, routes};

/// Remind going/interested users about events starting within this many hours.
const REMINDER_WINDOW_HOURS: i32 = 24;
/// How often the reminder sweep runs.
const REMINDER_INTERVAL_SECS: u64 = 15 * 60;
/// How often the push outbox is drained.
const PUSH_INTERVAL_SECS: u64 = 30;
/// How often escrow is advanced (held → clearing → cleared).
const ESCROW_INTERVAL_SECS: u64 = 60 * 60;
/// How often the transactional-email outbox is drained.
const EMAIL_INTERVAL_SECS: u64 = 60;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Config::from_env();
    if let Err(e) = config.validate() {
        anyhow::bail!("invalid production configuration: {e}");
    }
    let port = config.port;
    let cors_origins = config.cors_allowed_origins.clone();
    if cors_origins.is_empty() {
        tracing::warn!("CORS: no CORS_ALLOWED_ORIGINS set — using permissive policy (dev only)");
    } else {
        tracing::info!(origins = ?cors_origins, "CORS: restricting to configured allow-list");
    }
    let pool = db::connect(&config.database_url)
        .await
        .context("failed to connect to database / run migrations")?;

    // Background workers share the pool. The reminder sweep always runs; the push
    // outbox worker only starts when FCM credentials are configured.
    tokio::spawn(reminder_worker(pool.clone()));
    tokio::spawn(escrow_worker(pool.clone()));
    match FcmConfig::from_env() {
        Some(cfg) => {
            tracing::info!(project = %cfg.project_id, "push: FCM configured — starting delivery worker");
            tokio::spawn(push_worker(pool.clone(), cfg));
        }
        None => tracing::info!("push: FCM not configured — OS push disabled (in-app notifications still active)"),
    }
    match EmailConfig::from_env() {
        Some(cfg) => {
            tracing::info!(from = %cfg.from, "email: SendGrid configured — starting delivery worker");
            tokio::spawn(email_worker(pool.clone(), cfg));
        }
        None => tracing::info!("email: SendGrid not configured — order-confirmation emails disabled"),
    }

    let state = AppState {
        db: pool,
        jwt_secret: config.jwt_secret,
    };
    let app = routes::router(state)
        .layer(TraceLayer::new_for_http())
        .layer(underground_api::cors::cors_layer(&cors_origins));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!("underground-api listening on http://{addr}");

    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}

/// Periodically fans out reminders for events starting within the next window.
async fn reminder_worker(pool: sqlx::PgPool) {
    let mut tick = tokio::time::interval(Duration::from_secs(REMINDER_INTERVAL_SECS));
    loop {
        tick.tick().await;
        match reminders::sweep(&pool, REMINDER_WINDOW_HOURS).await {
            Ok(n) if n > 0 => tracing::info!(reminders = n, "reminder sweep emitted notifications"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "reminder sweep failed"),
        }
    }
}

/// Periodically advances escrow: held → clearing once an event is over, then → cleared
/// once the configured clearing window has elapsed. Frozen (disputed) orders are skipped.
async fn escrow_worker(pool: sqlx::PgPool) {
    let mut tick = tokio::time::interval(Duration::from_secs(ESCROW_INTERVAL_SECS));
    loop {
        tick.tick().await;
        let days = match payments::load_settings(&pool).await {
            Ok(s) => s.clearing_window_days,
            Err(e) => {
                tracing::warn!(error = %e, "escrow: failed to load settings");
                continue;
            }
        };
        match escrow::sweep(&pool, days).await {
            Ok((clearing, cleared)) if clearing > 0 || cleared > 0 => {
                tracing::info!(clearing, cleared, "escrow sweep advanced funds")
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "escrow sweep failed"),
        }
    }
}

/// Drains the push outbox: every un-pushed notification is delivered to each of the
/// recipient's registered devices, then marked. The FCM HTTP transport is a follow-up
/// (needs an HTTP client + a Firebase project); until it lands, deliveries are logged
/// so the whole pipeline — selection, dedup, watermarking — is observable end-to-end.
async fn push_worker(pool: sqlx::PgPool, cfg: FcmConfig) {
    let mut tick = tokio::time::interval(Duration::from_secs(PUSH_INTERVAL_SECS));
    loop {
        tick.tick().await;
        let batch = match push::pending(&pool, 100).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "push: failed to read outbox");
                continue;
            }
        };
        if batch.is_empty() {
            continue;
        }
        let ids: Vec<_> = batch.iter().map(|p| p.notification_id).collect();
        for p in &batch {
            // TODO(fcm): POST https://fcm.googleapis.com/v1/projects/{project}/messages:send
            // with an OAuth2 access token minted from the service-account key (FCM_SA_KEY).
            tracing::info!(
                project = %cfg.project_id, token = %p.token, kind = %p.kind, title = %p.title,
                "push.deliver"
            );
        }
        if let Err(e) = push::mark_pushed(&pool, &ids).await {
            tracing::warn!(error = %e, "push: failed to mark delivered");
        }
    }
}

/// Drains the order-confirmation email outbox: each paid, not-yet-emailed order is sent and
/// watermarked. The SendGrid HTTP transport is a follow-up; until it lands, deliveries are
/// logged so the whole pipeline (selection, template, watermark) is observable end-to-end.
async fn email_worker(pool: sqlx::PgPool, cfg: EmailConfig) {
    let mut tick = tokio::time::interval(Duration::from_secs(EMAIL_INTERVAL_SECS));
    loop {
        tick.tick().await;
        let batch = match email::pending(&pool, 100).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "email: failed to read outbox");
                continue;
            }
        };
        if batch.is_empty() {
            continue;
        }
        let ids: Vec<_> = batch.iter().map(|p| p.order_id).collect();
        for p in &batch {
            let (subject, _body) = email::build_confirmation(p);
            // TODO(sendgrid): POST https://api.sendgrid.com/v3/mail/send (Bearer cfg.api_key, from cfg.from).
            tracing::info!(from = %cfg.from, to = %p.to_email, subject = %subject, "email.deliver");
        }
        if let Err(e) = email::mark_sent(&pool, &ids).await {
            tracing::warn!(error = %e, "email: failed to watermark outbox");
        }
    }
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
