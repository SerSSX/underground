//! Payment configuration (admin-editable; defaults to test mode). Secrets are
//! never returned in GET — only a `*_set` boolean.

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::CurrentAdmin;
use crate::error::AppError;
use crate::state::AppState;

#[derive(sqlx::FromRow)]
pub struct PaymentSettings {
    pub mode: String,
    pub provider: String,
    pub ls_api_key: Option<String>,
    pub ls_store_id: Option<String>,
    pub ls_variant_id: Option<String>,
    pub ls_webhook_secret: Option<String>,
    pub platform_fee_bps: i32,
    pub clearing_window_days: i32,
    pub updated_at: DateTime<Utc>,
}

pub async fn load_settings(db: &PgPool) -> Result<PaymentSettings, sqlx::Error> {
    sqlx::query_as::<_, PaymentSettings>(
        "SELECT mode, provider, ls_api_key, ls_store_id, ls_variant_id, ls_webhook_secret, \
            platform_fee_bps, clearing_window_days, updated_at \
         FROM payment_settings WHERE id = TRUE",
    )
    .fetch_one(db)
    .await
}

#[derive(Serialize)]
pub struct SettingsView {
    pub mode: String,
    pub provider: String,
    pub ls_store_id: Option<String>,
    pub ls_variant_id: Option<String>,
    pub ls_api_key_set: bool,
    pub ls_webhook_secret_set: bool,
    pub platform_fee_bps: i32,
    pub clearing_window_days: i32,
    pub updated_at: DateTime<Utc>,
}

impl From<PaymentSettings> for SettingsView {
    fn from(s: PaymentSettings) -> Self {
        SettingsView {
            mode: s.mode,
            provider: s.provider,
            ls_store_id: s.ls_store_id,
            ls_variant_id: s.ls_variant_id,
            ls_api_key_set: s.ls_api_key.is_some(),
            ls_webhook_secret_set: s.ls_webhook_secret.is_some(),
            platform_fee_bps: s.platform_fee_bps,
            clearing_window_days: s.clearing_window_days,
            updated_at: s.updated_at,
        }
    }
}

/// `GET /api/admin/payment-settings` (admin only).
pub async fn get_settings(_admin: CurrentAdmin, State(st): State<AppState>) -> Result<Json<SettingsView>, AppError> {
    Ok(Json(load_settings(&st.db).await?.into()))
}

/// True if an optional string holds non-blank content.
fn present(o: &Option<String>) -> bool {
    o.as_deref().map(str::trim).is_some_and(|s| !s.is_empty())
}

/// Effective presence of a field after a partial update: the incoming value if supplied,
/// otherwise the stored one (mirrors the COALESCE in the UPDATE — `Some("")` clears it).
fn eff_present(incoming: &Option<String>, stored: &Option<String>) -> bool {
    match incoming {
        Some(v) => !v.trim().is_empty(),
        None => present(stored),
    }
}

#[derive(Serialize)]
pub struct ReadinessView {
    pub mode: String,
    pub provider: String,
    pub ls_store_id_set: bool,
    pub ls_variant_id_set: bool,
    pub ls_api_key_set: bool,
    pub ls_webhook_secret_set: bool,
    /// All four Lemon Squeezy fields present under the lemonsqueezy provider.
    pub ready_for_live: bool,
}

/// `GET /api/admin/payment-readiness` (admin only) — pre-launch checklist for live mode.
pub async fn readiness(_admin: CurrentAdmin, State(st): State<AppState>) -> Result<Json<ReadinessView>, AppError> {
    let s = load_settings(&st.db).await?;
    let store = present(&s.ls_store_id);
    let variant = present(&s.ls_variant_id);
    let api = present(&s.ls_api_key);
    let hook = present(&s.ls_webhook_secret);
    let ready_for_live = s.provider == "lemonsqueezy" && store && variant && api && hook;
    Ok(Json(ReadinessView {
        mode: s.mode,
        provider: s.provider,
        ls_store_id_set: store,
        ls_variant_id_set: variant,
        ls_api_key_set: api,
        ls_webhook_secret_set: hook,
        ready_for_live,
    }))
}

#[derive(Deserialize)]
pub struct UpdateSettingsReq {
    pub mode: Option<String>,
    pub provider: Option<String>,
    pub ls_api_key: Option<String>,
    pub ls_store_id: Option<String>,
    pub ls_variant_id: Option<String>,
    pub ls_webhook_secret: Option<String>,
    pub platform_fee_bps: Option<i32>,
    pub clearing_window_days: Option<i32>,
}

/// `PUT /api/admin/payment-settings` (admin only). Partial update via COALESCE.
pub async fn update_settings(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Json(req): Json<UpdateSettingsReq>,
) -> Result<Json<SettingsView>, AppError> {
    if let Some(m) = &req.mode {
        if m != "test" && m != "live" {
            return Err(AppError::Validation("mode must be test|live".into()));
        }
    }
    if let Some(p) = &req.provider {
        if p != "test" && p != "lemonsqueezy" {
            return Err(AppError::Validation("provider must be test|lemonsqueezy".into()));
        }
    }
    // Don't let an admin flip to live + lemonsqueezy without all four credentials in place
    // (effective = incoming-or-stored). Otherwise checkout would create unpayable orders.
    let cur = load_settings(&st.db).await?;
    let eff_mode = req.mode.clone().unwrap_or(cur.mode.clone());
    let eff_provider = req.provider.clone().unwrap_or(cur.provider.clone());
    if eff_mode == "live" && eff_provider == "lemonsqueezy" {
        let mut missing = Vec::new();
        if !eff_present(&req.ls_store_id, &cur.ls_store_id) {
            missing.push("ls_store_id");
        }
        if !eff_present(&req.ls_variant_id, &cur.ls_variant_id) {
            missing.push("ls_variant_id");
        }
        if !eff_present(&req.ls_api_key, &cur.ls_api_key) {
            missing.push("ls_api_key");
        }
        if !eff_present(&req.ls_webhook_secret, &cur.ls_webhook_secret) {
            missing.push("ls_webhook_secret");
        }
        if !missing.is_empty() {
            return Err(AppError::Validation(format!(
                "live mode requires: {}",
                missing.join(", ")
            )));
        }
    }
    // Audit detail records WHAT changed but never the secret values — only booleans for whether
    // the API key / webhook secret were rotated (the keys themselves never enter the trail).
    let detail = serde_json::json!({
        "mode": &req.mode,
        "provider": &req.provider,
        "platform_fee_bps": req.platform_fee_bps,
        "clearing_window_days": req.clearing_window_days,
        "ls_store_id": &req.ls_store_id,
        "ls_variant_id": &req.ls_variant_id,
        "ls_api_key_changed": req.ls_api_key.is_some(),
        "ls_webhook_secret_changed": req.ls_webhook_secret.is_some(),
    });
    let mut tx = st.db.begin().await?;
    sqlx::query(
        "UPDATE payment_settings SET \
            mode = COALESCE($1, mode), \
            provider = COALESCE($2, provider), \
            ls_api_key = COALESCE($3, ls_api_key), \
            ls_store_id = COALESCE($4, ls_store_id), \
            ls_variant_id = COALESCE($5, ls_variant_id), \
            platform_fee_bps = COALESCE($6, platform_fee_bps), \
            ls_webhook_secret = COALESCE($7, ls_webhook_secret), \
            clearing_window_days = COALESCE($8, clearing_window_days), \
            updated_at = now() \
         WHERE id = TRUE",
    )
    .bind(req.mode)
    .bind(req.provider)
    .bind(req.ls_api_key)
    .bind(req.ls_store_id)
    .bind(req.ls_variant_id)
    .bind(req.platform_fee_bps)
    .bind(req.ls_webhook_secret)
    .bind(req.clearing_window_days)
    .execute(&mut *tx)
    .await?;
    crate::audit::log(
        &mut *tx,
        admin.id,
        "payment_settings_update",
        "payment_settings",
        None,
        detail,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(load_settings(&st.db).await?.into()))
}
