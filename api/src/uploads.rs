//! Image uploads (event posters/gallery): multipart → local storage, served at /uploads.

use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;

const MAX_BYTES: usize = 5 * 1024 * 1024;

/// Local directory for stored files (configurable, doctrine §10).
pub fn uploads_dir() -> String {
    std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".into())
}

/// Absolute base URL for stored files (configurable, doctrine §10).
pub fn public_base_url() -> String {
    std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

fn ext_for(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

#[derive(Serialize)]
pub struct UploadResp {
    pub url: String,
}

/// `POST /api/uploads` — single `file` field; jpeg/png/webp up to 5 MB.
pub async fn upload(_user: CurrentUser, mut multipart: Multipart) -> Result<(StatusCode, Json<UploadResp>), AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("bad multipart".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or("").to_string();
        let Some(ext) = ext_for(&content_type) else {
            return Err(AppError::Validation("only jpeg/png/webp images are allowed".into()));
        };
        let data = field
            .bytes()
            .await
            .map_err(|_| AppError::Validation("upload too large or interrupted".into()))?;
        if data.is_empty() {
            return Err(AppError::Validation("empty file".into()));
        }
        if data.len() > MAX_BYTES {
            return Err(AppError::Validation("file must be at most 5 MB".into()));
        }
        let dir = uploads_dir();
        tokio::fs::create_dir_all(&dir).await.map_err(|_| AppError::Internal)?;
        let name = format!("{}.{ext}", Uuid::new_v4());
        tokio::fs::write(format!("{dir}/{name}"), &data)
            .await
            .map_err(|_| AppError::Internal)?;
        let url = format!("{}/uploads/{name}", public_base_url());
        return Ok((StatusCode::CREATED, Json(UploadResp { url })));
    }
    Err(AppError::Validation("missing 'file' field".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministically exercise both branches of the env-configurable paths
    // (integration tests always set the env, so the defaults need their own cover).
    #[test]
    fn env_paths_default_and_override() {
        std::env::remove_var("UPLOADS_DIR");
        std::env::remove_var("PUBLIC_BASE_URL");
        assert_eq!(uploads_dir(), "./uploads");
        assert_eq!(public_base_url(), "http://localhost:8080");

        std::env::set_var("UPLOADS_DIR", "/data/up");
        std::env::set_var("PUBLIC_BASE_URL", "https://cdn.example");
        assert_eq!(uploads_dir(), "/data/up");
        assert_eq!(public_base_url(), "https://cdn.example");

        std::env::remove_var("UPLOADS_DIR");
        std::env::remove_var("PUBLIC_BASE_URL");
    }
}
