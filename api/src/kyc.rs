//! Organiser payouts. Under direct charges the organiser is the merchant of record for their
//! own tickets, so their money never touches our balance — and we no longer need, or want, to
//! hold their bank details. They onboard on the PROVIDER's hosted pages; we keep only a mirror
//! of what the provider tells us about that account (PAY-5).
//!
//! `kyc_status` survives as OUR moderation verdict, which is a different thing from the
//! provider's capability flags: the provider says whether an account is valid, we say whether
//! we are willing to work with this organiser.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{CurrentAdmin, CurrentUser};
use crate::error::AppError;
use crate::state::AppState;

const KYC_STATUSES: [&str; 4] = ["none", "pending", "verified", "rejected"];

/// What the organiser's cabinet renders: the three states of "can I be paid yet".
#[derive(Serialize)]
pub struct PayoutStatus {
    /// Whether the provider will let this account take card payments yet.
    pub charges_enabled: bool,
    /// Whether money can actually leave the provider to their bank yet.
    pub payouts_enabled: bool,
    /// WHICH documents the provider is still waiting for. Empty when nothing is outstanding.
    pub requirements_due: Vec<String>,
    /// Whether onboarding has been started at all (an account exists).
    pub connected: bool,
    pub country: Option<String>,
    /// Our own moderation verdict, which is not the provider's business.
    pub kyc_status: String,
}

/// `GET /api/me/payouts/status` — the caller's OWN payout state. There is deliberately no
/// variant of this that takes a user id: one organiser has no business reading another's
/// onboarding progress.
pub async fn payouts_status(user: CurrentUser, State(st): State<AppState>) -> Result<Json<PayoutStatus>, AppError> {
    type Row = (
        Option<String>,
        bool,
        bool,
        Option<serde_json::Value>,
        Option<String>,
        String,
    );
    let row: Row = sqlx::query_as(
        "SELECT psp_account_id, psp_charges_enabled, psp_payouts_enabled, psp_requirements_due, \
            psp_country, kyc_status FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    let (account_id, charges_enabled, payouts_enabled, requirements, country, kyc_status) = row;
    Ok(Json(PayoutStatus {
        charges_enabled,
        payouts_enabled,
        requirements_due: requirements
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default(),
        connected: account_id.is_some(),
        country,
        kyc_status,
    }))
}

#[derive(Deserialize)]
pub struct ConnectReq {
    /// The organiser's own country, two-letter ISO. Their declaration, which the provider then
    /// verifies during onboarding — not something we can look up for them.
    pub country: String,
}

#[derive(Serialize)]
pub struct ConnectResp {
    /// Short-lived, single-use URL on the PROVIDER's pages. Never stored, never logged, never
    /// handed to anyone but the organiser who asked for it.
    pub onboarding_url: String,
}

/// `POST /api/me/payouts/connect` — start (or resume) hosted onboarding.
///
/// Called twice, this must not produce two accounts: the second call resumes the first. Two
/// defences, because one is not enough under concurrency — the provider call carries an
/// idempotency key derived from the organiser, and the database write is a compare-and-set that
/// loses gracefully if a racing call stored an account first.
pub async fn payouts_connect(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<ConnectReq>,
) -> Result<Json<ConnectResp>, AppError> {
    let country = req.country.trim().to_ascii_uppercase();
    if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::Validation("country must be a two-letter ISO code".into()));
    }
    let (email, existing, blocked): (String, Option<String>, bool) =
        sqlx::query_as("SELECT email, psp_account_id, blocked FROM users WHERE id = $1")
            .bind(user.id)
            .fetch_one(&st.db)
            .await?;
    if blocked {
        return Err(AppError::Forbidden);
    }

    let settings = crate::payments::load_settings(&st.db).await?;
    let provider = crate::provider::resolve(&settings).map_err(|e| {
        tracing::error!(error = %e, "payouts: provider unavailable");
        AppError::Conflict("платіжна система тимчасово недоступна, спробуйте за хвилину".into())
    })?;

    let account_ref = match existing {
        Some(a) => a,
        None => {
            let created = provider.create_seller_account(&country, &email).await.map_err(|e| {
                tracing::error!(error = %e, "payouts: could not create the seller account");
                AppError::Conflict("не вдалося створити акаунт для виплат, спробуйте за хвилину".into())
            })?;
            // Compare-and-set: only the first writer wins. A racing call that stored its own
            // account leaves us with rows_affected = 0, and we then use THEIRS — never a second
            // account, and never an id chosen by anyone but the provider.
            let stored = sqlx::query(
                "UPDATE users SET psp_account_id = $2, psp_country = $3 \
                 WHERE id = $1 AND psp_account_id IS NULL",
            )
            .bind(user.id)
            .bind(&created)
            .bind(&country)
            .execute(&st.db)
            .await?;
            if stored.rows_affected() > 0 {
                created
            } else {
                sqlx::query_scalar("SELECT psp_account_id FROM users WHERE id = $1")
                    .bind(user.id)
                    .fetch_one(&st.db)
                    .await?
            }
        }
    };

    let base = crate::passwords::web_base_url();
    let onboarding_url = provider
        .seller_onboarding_link(
            &account_ref,
            &format!("{base}/organizer/payouts?refresh=1"),
            &format!("{base}/organizer/payouts?done=1"),
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "payouts: could not create the onboarding link");
            AppError::Conflict("не вдалося відкрити форму підключення, спробуйте за хвилину".into())
        })?;
    Ok(Json(ConnectResp { onboarding_url }))
}

#[derive(Deserialize)]
pub struct KycDecisionReq {
    pub status: String,
}

/// `POST /api/admin/users/:id/kyc` — admin reviews and sets a KYC decision.
pub async fn admin_set_kyc(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<KycDecisionReq>,
) -> Result<StatusCode, AppError> {
    if !KYC_STATUSES.contains(&req.status.as_str()) {
        return Err(AppError::Validation(
            "status must be none|pending|verified|rejected".into(),
        ));
    }
    // `blocked = false` so an erased/blocked account can never be (re)verified for payouts.
    let mut tx = st.db.begin().await?;
    let res = sqlx::query("UPDATE users SET kyc_status = $2 WHERE id = $1 AND blocked = false")
        .bind(id)
        .bind(&req.status)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::audit::log(
        &mut *tx,
        admin.id,
        "kyc_set",
        "user",
        Some(id),
        serde_json::json!({ "status": req.status }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
