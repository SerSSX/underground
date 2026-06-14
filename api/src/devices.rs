//! Device push-token registration (TASK-017 M4). Android uploads its FCM token
//! after login; the push outbox worker fans notifications out to these tokens.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterDeviceReq {
    pub token: String,
    pub platform: Option<String>,
}

/// `POST /api/me/devices` — register or refresh the caller's push token. A token
/// can move between users (shared device), so a conflict re-points it.
pub async fn register_device(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<RegisterDeviceReq>,
) -> Result<StatusCode, AppError> {
    let token = req.token.trim();
    if token.is_empty() {
        return Err(AppError::Validation("token is required".into()));
    }
    let platform = req.platform.as_deref().map(str::trim).unwrap_or("android");
    sqlx::query(
        "INSERT INTO device_tokens (token, user_id, platform) VALUES ($1, $2, $3) \
         ON CONFLICT (token) DO UPDATE \
            SET user_id = EXCLUDED.user_id, platform = EXCLUDED.platform, updated_at = now()",
    )
    .bind(token)
    .bind(user.id)
    .bind(platform)
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UnregisterDeviceReq {
    pub token: String,
}

/// `DELETE /api/me/devices` — drop a token on logout (only if it's the caller's).
pub async fn unregister_device(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<UnregisterDeviceReq>,
) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM device_tokens WHERE token = $1 AND user_id = $2")
        .bind(req.token.trim())
        .bind(user.id)
        .execute(&st.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
