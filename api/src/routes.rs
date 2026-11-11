use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tower_http::services::ServeDir;

use crate::state::AppState;
use crate::{
    admin, audit, auth, cities, devices, discover, emailcode, events, eventsocial, gdpr, kyc, messages, notifications,
    orders, organizer, passwords, payments, payouts, posts, prefs, promos, reports, reviews, social, staff, telemetry,
    tickets, uploads, webhooks,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/version", get(version))
        .route("/api/events", get(events::list_events).post(events::create_event))
        .route(
            "/api/events/:id",
            get(events::get_event)
                .put(events::update_event)
                .delete(events::delete_event),
        )
        .route("/api/me/events", get(events::my_events))
        .route("/api/discover", get(discover::discover))
        .route("/api/discover/sections", get(discover::discover_sections))
        .route("/api/cities", get(cities::search))
        .route("/api/me/saves", get(discover::my_saves))
        .route(
            "/api/events/:id/save",
            post(discover::save_event).delete(discover::unsave_event),
        )
        .route("/api/events/:id/reserve", post(tickets::reserve))
        .route("/api/events/:id/checkout", post(orders::checkout))
        .route("/api/orders/:id/pay-test", post(orders::pay_test))
        .route("/api/orders/:id/dispute", post(orders::dispute))
        .route("/api/webhooks/lemonsqueezy", post(webhooks::lemonsqueezy))
        .route("/api/me/tickets", get(tickets::my_tickets))
        .route("/api/me/prefs", get(prefs::get_prefs).put(prefs::put_prefs))
        .route("/api/me/unread", get(notifications::unread_counts))
        .route("/api/me/notifications", get(notifications::my_notifications))
        .route("/api/me/notifications/read", post(notifications::mark_all_read))
        .route(
            "/api/me/devices",
            post(devices::register_device).delete(devices::unregister_device),
        )
        .route("/api/telemetry", post(telemetry::log_event))
        .route("/api/reviews", post(reviews::post_review))
        .route("/api/events/:id/reviews", get(reviews::event_reviews))
        .route("/api/events/:id/social", get(eventsocial::social))
        .route("/api/events/:id/similar", get(eventsocial::similar))
        .route("/api/users/:id", get(social::get_profile))
        .route("/api/users/:id/events", get(social::user_events))
        .route("/api/users/:id/follow", post(social::follow).delete(social::unfollow))
        .route("/api/users/:id/followers", get(social::followers))
        .route("/api/users/:id/following", get(social::following))
        .route("/api/users/:id/posts", get(posts::user_posts))
        .route("/api/feed", get(posts::feed))
        .route("/api/posts", post(posts::create_post))
        .route("/api/posts/:id", axum::routing::delete(posts::delete_post))
        .route("/api/posts/:id/like", post(posts::like).delete(posts::unlike))
        .route(
            "/api/posts/:id/comments",
            get(posts::comments).post(posts::create_comment),
        )
        .route("/api/reports", post(reports::create_report))
        .route("/api/admin/reports", get(reports::list_reports))
        .route("/api/admin/reports/:id", axum::routing::patch(reports::resolve_report))
        .route("/api/messages", post(messages::send_message))
        .route("/api/messages/:user_id", get(messages::thread))
        .route("/api/me/conversations", get(messages::conversations))
        .route(
            "/api/uploads",
            // Body cap slightly above the 5 MB file limit (multipart overhead).
            post(uploads::upload).layer(axum::extract::DefaultBodyLimit::max(6 * 1024 * 1024)),
        )
        .nest_service("/uploads", ServeDir::new(uploads::uploads_dir()))
        .route("/api/tickets/validate", post(tickets::validate))
        .route("/api/tickets/:id/return", post(tickets::return_ticket))
        .route("/api/tickets/:id/transfer", post(tickets::transfer_ticket))
        .route("/api/tickets/:id/qr", get(tickets::ticket_qr))
        .route(
            "/api/tickets/:id/gift",
            post(tickets::gift_ticket).delete(tickets::revoke_gift),
        )
        .route("/api/tickets/claim", post(tickets::claim_ticket))
        .route("/api/gifts/:token", get(tickets::gift_preview))
        .route("/api/events/:id/staff/invite", post(staff::invite_staff))
        .route("/api/staff/accept", post(staff::accept_invite))
        .route("/api/events/:id/scan", post(staff::scan))
        .route("/api/me/staff/events", get(staff::my_staff_events))
        .route("/api/organizer/stats", get(organizer::stats))
        .route("/api/organizer/analytics", get(organizer::analytics))
        .route("/api/organizer/payouts", get(organizer::my_payouts))
        .route("/api/events/:id/tickets", get(organizer::event_tickets))
        .route(
            "/api/events/:id/promos",
            get(promos::list_promos).post(promos::create_promo),
        )
        .route("/api/promos/:id", axum::routing::delete(promos::delete_promo))
        .route(
            "/api/admin/payment-settings",
            get(payments::get_settings).put(payments::update_settings),
        )
        .route("/api/admin/payment-readiness", get(payments::readiness))
        .route("/api/admin/stats", get(admin::stats))
        .route("/api/admin/actions", get(audit::list_actions))
        .route("/api/admin/telemetry", get(telemetry::admin_telemetry))
        .route("/api/admin/analytics/funnel", get(telemetry::analytics_funnel))
        .route("/api/admin/users", get(admin::users))
        .route("/api/admin/users/:id", axum::routing::patch(admin::patch_user))
        .route("/api/admin/events", get(admin::events))
        .route("/api/admin/events/:id", axum::routing::patch(admin::patch_event))
        .route("/api/admin/orders", get(admin::orders))
        .route("/api/admin/orders/:id/escrow", post(admin::escrow_action))
        .route("/api/admin/posts/:id", axum::routing::delete(admin::delete_post))
        .route("/api/admin/reviews/:id", axum::routing::delete(admin::delete_review))
        .route("/api/admin/payouts", get(payouts::list_payouts))
        .route("/api/admin/payouts/:id", axum::routing::patch(payouts::update_payout))
        .route("/api/admin/organizers/:id/payout", post(payouts::create_payout))
        .route("/api/me/kyc", get(kyc::get_kyc).post(kyc::submit_kyc))
        .route("/api/admin/users/:id/kyc", post(kyc::admin_set_kyc))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/email/request-code", post(emailcode::request_code))
        .route("/api/auth/email/verify-code", post(emailcode::verify_code))
        .route("/api/auth/forgot-password", post(passwords::forgot_password))
        .route("/api/auth/reset-password", post(passwords::reset_password))
        .route("/api/me", get(auth::me).patch(auth::patch_me).delete(gdpr::delete_me))
        .route("/api/me/export", get(gdpr::export_me))
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[derive(Serialize)]
struct Version {
    name: &'static str,
    version: &'static str,
    /// Git commit the binary was built from (CI passes it as a Docker build arg). The DEV/prod smoke
    /// compares it with the commit it just deployed, so a stale container cannot pass as the new one.
    build: &'static str,
}

async fn version() -> Json<Version> {
    Json(Version {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        build: option_env!("UG_BUILD_SHA").unwrap_or("local"),
    })
}
