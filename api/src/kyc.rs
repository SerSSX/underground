//! Organizer KYC (P1 #78). Before any payout an organizer must submit verified bank details
//! and reach `kyc_status = 'verified'`. Identity-provider verification (Sumsub/Onfido) plugs
//! into the pending->verified transition later; for now an admin reviews and verifies.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{CurrentAdmin, CurrentUser};
use crate::error::AppError;
use crate::state::AppState;

const KYC_STATUSES: [&str; 4] = ["none", "pending", "verified", "rejected"];

/// Loose IBAN sanity check (pre-stripped of whitespace): 15-34 chars, alphanumeric,
/// 2 leading country letters then 2 check digits. Not a full mod-97 validation.
fn valid_iban(s: &str) -> bool {
    let len = s.len();
    (15..=34).contains(&len)
        && s.chars().all(|c| c.is_ascii_alphanumeric())
        && s[..2].chars().all(|c| c.is_ascii_alphabetic())
        && s[2..4].chars().all(|c| c.is_ascii_digit())
}

/// SWIFT/BIC: 8 or 11 alphanumeric characters.
fn valid_swift(s: &str) -> bool {
    (s.len() == 8 || s.len() == 11) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[derive(Deserialize)]
pub struct KycReq {
    pub name: String,
    pub iban: String,
    pub swift: String,
}

/// `POST /api/me/kyc` — organizer submits payout identity + bank details (→ pending).
pub async fn submit_kyc(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<KycReq>,
) -> Result<StatusCode, AppError> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 200 {
        return Err(AppError::Validation("name is required (max 200 chars)".into()));
    }
    let iban: String = req.iban.split_whitespace().collect::<String>().to_uppercase();
    if !valid_iban(&iban) {
        return Err(AppError::Validation("invalid IBAN".into()));
    }
    let swift = req.swift.split_whitespace().collect::<String>().to_uppercase();
    if !valid_swift(&swift) {
        return Err(AppError::Validation("invalid SWIFT/BIC".into()));
    }
    sqlx::query(
        "UPDATE users SET payout_name = $2, payout_iban = $3, payout_swift = $4, kyc_status = 'pending' \
         WHERE id = $1",
    )
    .bind(user.id)
    .bind(name)
    .bind(&iban)
    .bind(&swift)
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct KycView {
    pub kyc_status: String,
    pub has_bank_details: bool,
}

/// `GET /api/me/kyc` — the caller's own KYC status (never returns the raw bank details).
pub async fn get_kyc(user: CurrentUser, State(st): State<AppState>) -> Result<Json<KycView>, AppError> {
    let view = sqlx::query_as::<_, KycView>(
        "SELECT kyc_status, payout_iban IS NOT NULL AS has_bank_details FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(view))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iban_validation() {
        assert!(valid_iban("UA903052992990004149123456789")); // UA IBAN
        assert!(valid_iban("DE89370400440532013000"));
        assert!(!valid_iban("DE8937")); // too short
        assert!(!valid_iban("1234567890123456")); // no country letters
        assert!(!valid_iban("DEXX370400440532013000")); // check digits not numeric
        assert!(!valid_iban("DE89-7040-0440-5320-13000!")); // bad chars
    }

    #[test]
    fn swift_validation() {
        assert!(valid_swift("PBANUA2X")); // 8
        assert!(valid_swift("PBANUA2XXXX")); // 11
        assert!(!valid_swift("SHORT")); // wrong length
        assert!(!valid_swift("PBANUA2X!!!")); // bad chars (11 len but non-alnum)
    }
}
