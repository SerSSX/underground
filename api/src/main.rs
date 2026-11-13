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
/// Prompt attendees for a review of events that ended within this many days (TASK-199-D).
const REVIEW_PROMPT_RECENCY_DAYS: i32 = 14;
/// Nudge savers when a saved event's remaining stock drops to this fraction of its total (TASK-209).
const SELLING_FAST_THRESHOLD: f64 = 0.15;
/// Cap `selling_fast` nudges per user per rolling 24h so the trigger can't spam heavy savers (TASK-210).
const SELLING_FAST_DAILY_CAP: i64 = 2;
/// How often the reminder sweep runs.
const REMINDER_INTERVAL_SECS: u64 = 15 * 60;
/// How often the push outbox is drained.
const PUSH_INTERVAL_SECS: u64 = 30;
/// How often escrow is advanced (held → clearing → cleared).
const ESCROW_INTERVAL_SECS: u64 = 60 * 60;
/// How often the transactional-email outbox is drained.
const EMAIL_INTERVAL_SECS: u64 = 60;

/// `underground-api decode-image` is the image child process (TASK-304): it must start before any
/// runtime or thread exists, so it is dispatched here, ahead of Tokio, and never touches the
/// environment file, the database or the network.
fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some(underground_api::uploads::DECODE_SUBCOMMAND) {
        std::process::exit(underground_api::uploads::decode_child_main());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start the Tokio runtime")?
        .block_on(serve())
}

async fn serve() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Config::from_env();
    if let Err(e) = config.validate() {
        anyhow::bail!("invalid production configuration: {e}");
    }
    // TASK-305 R8: the login dummy hash is built before the first request, not during it.
    underground_api::auth::warm_dummy_hash();
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
    // Load the time-zone boundary data off the request path (TASK-303 G1).
    tokio::task::spawn_blocking(underground_api::tz::warm);
    tokio::spawn(reminder_worker(pool.clone()));
    tokio::spawn(escrow_worker(pool.clone()));
    match FcmConfig::from_env() {
        Some(cfg) => {
            tracing::info!(project = %cfg.project_id, "push: FCM configured — starting delivery worker");
            tokio::spawn(push_worker(pool.clone(), cfg));
        }
        None => tracing::info!("push: FCM not configured — OS push disabled (in-app notifications still active)"),
    }
    match underground_api::mail::MailConfig::from_env() {
        Some(cfg) => tracing::info!(from = %cfg.from, "mail: Resend configured — reset links and login codes are sent"),
        None => tracing::info!("mail: off (RESEND_API_KEY / EMAIL_FROM not set) — no reset or code emails"),
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
    let free_only = underground_api::events::free_events_only_from_env();
    underground_api::events::set_free_events_only(free_only);
    tracing::info!(free_only, "events: free-only mode (FREE_EVENTS_ONLY)");
    let app = routes::router(state)
        .layer(TraceLayer::new_for_http())
        .layer(underground_api::cors::cors_layer(&cors_origins));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!("underground-api listening on http://{addr}");

    // Connect info gives `ratelimit::ClientIp` the socket peer when no proxy header is present.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .context("server error")?;

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
        match reminders::review_prompt_sweep(&pool, REVIEW_PROMPT_RECENCY_DAYS).await {
            Ok(n) if n > 0 => tracing::info!(review_prompts = n, "review-prompt sweep emitted notifications"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "review-prompt sweep failed"),
        }
        match reminders::selling_fast_sweep(&pool, SELLING_FAST_THRESHOLD, SELLING_FAST_DAILY_CAP).await {
            Ok(n) if n > 0 => tracing::info!(selling_fast = n, "selling-fast sweep emitted notifications"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "selling-fast sweep failed"),
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
        // Order-confirmation emails.
        match email::pending(&pool, 100).await {
            Ok(batch) if !batch.is_empty() => {
                let ids: Vec<_> = batch.iter().map(|p| p.order_id).collect();
                for p in &batch {
                    let (subject, _body) = email::build_confirmation(p);
                    // TODO(sendgrid): POST https://api.sendgrid.com/v3/mail/send (Bearer cfg.api_key, from cfg.from).
                    // TASK-305 R10: no recipient address in the logs; the order id is enough to trace it.
                    tracing::info!(from = %cfg.from, order_id = %p.order_id, subject = %subject, "email.deliver");
                }
                if let Err(e) = email::mark_sent(&pool, &ids).await {
                    tracing::warn!(error = %e, "email: failed to watermark order outbox");
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "email: failed to read order outbox"),
        }
        // Retention notification emails (event_reminder, review_prompt) — TASK-199-D.
        match email::pending_notifications(&pool, 100).await {
            Ok(batch) if !batch.is_empty() => {
                let ids: Vec<_> = batch.iter().map(|p| p.notification_id).collect();
                for p in &batch {
                    let (subject, _body) = email::build_notification_email(p);
                    tracing::info!(
                        from = %cfg.from, notification_id = %p.notification_id, subject = %subject, kind = %p.kind,
                        "email.deliver"
                    );
                }
                if let Err(e) = email::mark_notifications_sent(&pool, &ids).await {
                    tracing::warn!(error = %e, "email: failed to watermark notification outbox");
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "email: failed to read notification outbox"),
        }
    }
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
