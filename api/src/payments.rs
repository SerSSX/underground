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
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub platform_fee_bps: i32,
    pub clearing_window_days: i32,
    pub updated_at: DateTime<Utc>,
}

pub async fn load_settings(db: &PgPool) -> Result<PaymentSettings, sqlx::Error> {
    sqlx::query_as::<_, PaymentSettings>(
        "SELECT mode, provider, ls_api_key, ls_store_id, ls_variant_id, ls_webhook_secret, \
            stripe_secret_key, stripe_publishable_key, stripe_webhook_secret, \
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
    pub stripe_publishable_key: Option<String>,
    pub stripe_secret_key_set: bool,
    pub stripe_webhook_secret_set: bool,
    /// `sk_test_…` vs `sk_live_…`. Shown so the operator can see at a glance that the key
    /// matches the selected mode — a live key under test mode is a silent misconfiguration.
    pub stripe_key_kind: Option<String>,
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
            stripe_key_kind: key_kind(&s.stripe_secret_key),
            stripe_publishable_key: s.stripe_publishable_key,
            stripe_secret_key_set: present(&s.stripe_secret_key),
            stripe_webhook_secret_set: present(&s.stripe_webhook_secret),
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
    pub stripe_secret_key_set: bool,
    pub stripe_webhook_secret_set: bool,
    pub stripe_key_kind: Option<String>,
    /// The configured provider name is one the code actually knows how to drive. False here
    /// means checkout returns 409 for every buyer — surfaced so it is visible before a sale
    /// fails rather than after (PAY-1).
    pub provider_recognised: bool,
    /// Every credential the SELECTED provider needs for live mode is present.
    pub ready_for_live: bool,
}

/// `GET /api/admin/payment-readiness` (admin only) — pre-launch checklist for live mode.
pub async fn readiness(_admin: CurrentAdmin, State(st): State<AppState>) -> Result<Json<ReadinessView>, AppError> {
    let s = load_settings(&st.db).await?;
    let store = present(&s.ls_store_id);
    let variant = present(&s.ls_variant_id);
    let api = present(&s.ls_api_key);
    let hook = present(&s.ls_webhook_secret);
    let sk = present(&s.stripe_secret_key);
    let swh = present(&s.stripe_webhook_secret);
    let provider_recognised = crate::provider::resolve(&s).is_ok();
    let ready_for_live = match s.provider.as_str() {
        "lemonsqueezy" => store && variant && api && hook,
        // A webhook secret is not optional for Stripe: without it we cannot authenticate the
        // one message that tells us a buyer actually paid, so checkout would take money and
        // never issue a ticket.
        "stripe" => sk && swh,
        _ => false,
    };
    Ok(Json(ReadinessView {
        mode: s.mode,
        stripe_key_kind: key_kind(&s.stripe_secret_key),
        provider: s.provider,
        ls_store_id_set: store,
        ls_variant_id_set: variant,
        ls_api_key_set: api,
        ls_webhook_secret_set: hook,
        stripe_secret_key_set: sk,
        stripe_webhook_secret_set: swh,
        provider_recognised,
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
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub platform_fee_bps: Option<i32>,
    pub clearing_window_days: Option<i32>,
}

/// Which Stripe environment a secret key belongs to, from its documented prefix.
fn key_kind(key: &Option<String>) -> Option<String> {
    let k = key.as_deref().map(str::trim).filter(|s| !s.is_empty())?;
    Some(if k.starts_with("sk_live_") {
        "live".to_string()
    } else if k.starts_with("sk_test_") {
        "test".to_string()
    } else {
        "unknown".to_string()
    })
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
        if !matches!(p.as_str(), "test" | "lemonsqueezy" | "stripe" | "noop") {
            return Err(AppError::Validation(
                "provider must be test|noop|lemonsqueezy|stripe".into(),
            ));
        }
    }
    // Don't let an admin flip to live + lemonsqueezy without all four credentials in place
    // (effective = incoming-or-stored). Otherwise checkout would create unpayable orders.
    let cur = load_settings(&st.db).await?;
    let eff_mode = req.mode.clone().unwrap_or(cur.mode.clone());
    let eff_provider = req.provider.clone().unwrap_or(cur.provider.clone());
    if eff_mode == "live" && eff_provider == "stripe" {
        let mut missing = Vec::new();
        if !eff_present(&req.stripe_secret_key, &cur.stripe_secret_key) {
            missing.push("stripe_secret_key");
        }
        if !eff_present(&req.stripe_webhook_secret, &cur.stripe_webhook_secret) {
            missing.push("stripe_webhook_secret");
        }
        if !missing.is_empty() {
            return Err(AppError::Validation(format!(
                "live mode requires: {}",
                missing.join(", ")
            )));
        }
        // A test key in live mode charges nobody while every screen claims we are live.
        let eff_key = req.stripe_secret_key.clone().or(cur.stripe_secret_key.clone());
        if key_kind(&eff_key).as_deref() == Some("test") {
            return Err(AppError::Validation(
                "у live-режимі потрібен live-ключ Stripe (sk_live_…), а не тестовий".into(),
            ));
        }
    }
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
        "stripe_publishable_key": &req.stripe_publishable_key,
        "stripe_secret_key_changed": req.stripe_secret_key.is_some(),
        "stripe_webhook_secret_changed": req.stripe_webhook_secret.is_some(),
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
            stripe_secret_key = COALESCE($9, stripe_secret_key), \
            stripe_publishable_key = COALESCE($10, stripe_publishable_key), \
            stripe_webhook_secret = COALESCE($11, stripe_webhook_secret), \
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
    .bind(req.stripe_secret_key)
    .bind(req.stripe_publishable_key)
    .bind(req.stripe_webhook_secret)
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

#[derive(Serialize)]
pub struct ConnectionCheck {
    pub ok: bool,
    /// Stripe's own account id, so the operator sees WHICH account the key opens — not just
    /// that some key worked. A key pasted from the wrong project is otherwise invisible.
    pub account_id: Option<String>,
    pub message: String,
}

/// `POST /api/admin/stripe/test` (admin only) — call Stripe with the stored key and report
/// back. This is the one place a credential is proven before a buyer discovers it is wrong.
pub async fn stripe_test(_admin: CurrentAdmin, State(st): State<AppState>) -> Result<Json<ConnectionCheck>, AppError> {
    let s = load_settings(&st.db).await?;
    let Some(key) = s.stripe_secret_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) else {
        return Ok(Json(ConnectionCheck {
            ok: false,
            account_id: None,
            message: "секретний ключ Stripe не збережено".into(),
        }));
    };
    let provider = crate::stripe::StripeProvider::new(key, s.stripe_webhook_secret.clone(), crate::stripe::base_url());
    match provider.check_connection().await {
        Ok(id) => Ok(Json(ConnectionCheck {
            ok: true,
            message: format!("зв'язок є, акаунт {id}"),
            account_id: Some(id),
        })),
        // The provider's message is safe to show: it never contains our key.
        Err(e) => Ok(Json(ConnectionCheck {
            ok: false,
            account_id: None,
            message: e.to_string(),
        })),
    }
}
