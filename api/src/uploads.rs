//! Image uploads (event posters, gallery, avatars): multipart → re-encoded WebP in local storage,
//! served at /uploads (TASK-304).
//!
//! Every upload is decoded and re-encoded, never stored as sent:
//! - the format is sniffed from the bytes (JPEG, PNG or WebP only), whatever the name or the
//!   declared type say;
//! - the pixel count is read from the header and checked BEFORE the pixels are decoded
//!   ([`MAX_PIXELS`]), and the decoder runs under an allocation cap, so a small file that claims
//!   to be huge (a decompression bomb) is refused without allocating its buffer;
//! - the picture is scaled to at most [`MAX_SIDE`] px on its longest side (never up), turned
//!   upright by its EXIF orientation, and written as lossy WebP (quality [`QUALITY`]) with NO
//!   metadata: EXIF (GPS included), XMP and ICC are dropped because only pixels are encoded;
//! - a second [`SMALL_SIDE`] px copy is written beside it as `<name>-480.webp` for cards.
//!
//! Decoding is image's pure-Rust code (zune-jpeg, png, image-webp); libwebp (C) only ever
//! encodes pixels we produced. One image is processed at a time (memory: the API container has
//! 256 MB; a 32 MP RGB picture is ~96 MB while it is being scaled).
//! Files uploaded before TASK-304 stay as they are; their URLs keep working.

use std::io::Cursor;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use serde::Serialize;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

const MAX_BYTES: usize = 5 * 1024 * 1024;
/// Largest picture accepted, in pixels (width × height), checked from the header before decoding.
pub const MAX_PIXELS: u64 = 32_000_000;
/// Longest side of the stored picture.
pub const MAX_SIDE: u32 = 1600;
/// Longest side of the card copy.
pub const SMALL_SIDE: u32 = 480;
/// Lossy WebP quality (0–100).
const QUALITY: f32 = 80.0;
/// The decoder may not allocate more than this (a 32 MP RGBA picture is 128 MB).
const MAX_ALLOC: u64 = 160 * 1024 * 1024;
/// Below this many pixels a smooth filter is used; above it the fast area-sampling one, which needs
/// no large intermediate buffer.
const SMOOTH_LIMIT: u64 = 4_000_000;
/// One picture is decoded at a time.
static PROCESSING: Semaphore = Semaphore::const_new(1);

/// Local directory for stored files (configurable, doctrine §10).
pub fn uploads_dir() -> String {
    std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".into())
}

/// Absolute base URL for stored files (configurable, doctrine §10).
pub fn public_base_url() -> String {
    std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

fn allowed_content_type(content_type: &str) -> bool {
    matches!(content_type, "image/jpeg" | "image/png" | "image/webp")
}

#[derive(Serialize)]
pub struct UploadResp {
    /// The stored picture (≤ 1600 px, WebP).
    pub url: String,
    /// The card copy (≤ 480 px, WebP).
    pub url_small: String,
}

/// Why a picture was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Reject {
    /// Not a JPEG, PNG or WebP picture we can decode.
    NotImage,
    /// More pixels than [`MAX_PIXELS`], or over the decoder's limits.
    TooLarge,
    /// Encoding failed (our side).
    Encode,
}

impl From<Reject> for AppError {
    fn from(r: Reject) -> Self {
        match r {
            Reject::NotImage => AppError::Validation("файл не є зображенням JPEG, PNG або WebP".into()),
            Reject::TooLarge => AppError::Validation("зображення завелике: до 32 мегапікселів".into()),
            Reject::Encode => AppError::Internal,
        }
    }
}

/// The two stored files of one upload.
pub struct Processed {
    pub large: Vec<u8>,
    pub small: Vec<u8>,
    /// Size of `large` after scaling and orientation.
    pub width: u32,
    pub height: u32,
}

fn decode_error(e: image::ImageError) -> Reject {
    match e {
        image::ImageError::Limits(_) => Reject::TooLarge,
        _ => Reject::NotImage,
    }
}

/// Scale so the longest side is at most `side` (never up).
fn fit(img: DynamicImage, side: u32) -> DynamicImage {
    if img.width().max(img.height()) <= side {
        return img;
    }
    if u64::from(img.width()) * u64::from(img.height()) <= SMOOTH_LIMIT {
        img.resize(side, side, image::imageops::FilterType::CatmullRom)
    } else {
        img.thumbnail(side, side)
    }
}

/// 8-bit RGB, or RGBA when the picture has transparency.
fn to_8bit(img: DynamicImage) -> DynamicImage {
    if img.color().has_alpha() {
        DynamicImage::ImageRgba8(img.into_rgba8())
    } else {
        DynamicImage::ImageRgb8(img.into_rgb8())
    }
}

fn encode_webp(img: &DynamicImage) -> Result<Vec<u8>, Reject> {
    let (w, h) = (img.width(), img.height());
    let enc = match img {
        DynamicImage::ImageRgb8(b) => webp::Encoder::from_rgb(b.as_raw(), w, h),
        DynamicImage::ImageRgba8(b) => webp::Encoder::from_rgba(b.as_raw(), w, h),
        _ => return Err(Reject::Encode),
    };
    let out = enc.encode_simple(false, QUALITY).map_err(|_| Reject::Encode)?;
    Ok(out.to_vec())
}

/// Decode, check, scale, orient and re-encode one picture (blocking: CPU-bound).
pub fn process(data: &[u8]) -> Result<Processed, Reject> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| Reject::NotImage)?;
    // The bytes decide the format; a .jpg name or an image/jpeg type proves nothing.
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err(Reject::NotImage);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(Reject::NotImage);
    }
    // Decompression-bomb guard: from the header, before a single pixel is decoded.
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(Reject::TooLarge);
    }
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let img = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    // Scale first, then rotate: rotating the full-size picture would copy it.
    let mut large = fit(img, MAX_SIDE);
    large.apply_orientation(orientation);
    let large = to_8bit(large);
    let small = fit(large.clone(), SMALL_SIDE);
    Ok(Processed {
        large: encode_webp(&large)?,
        small: encode_webp(&small)?,
        width: large.width(),
        height: large.height(),
    })
}

/// `POST /api/uploads` — single `file` field; a JPEG, PNG or WebP picture up to 5 MB and 32 MP.
/// Answers 201 `{url, url_small}`; both are WebP.
pub async fn upload(
    user: CurrentUser,
    State(st): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResp>), AppError> {
    crate::ratelimit::hit(
        &st.db,
        &format!("upload:{}", user.id),
        crate::ratelimit::env_limit("UPLOAD_LIMIT_PER_HOUR", 120),
        3600,
    )
    .await?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("bad multipart".into()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let content_type = field.content_type().unwrap_or("").to_string();
        if !allowed_content_type(&content_type) {
            return Err(AppError::Validation("only jpeg/png/webp images are allowed".into()));
        }
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
        let processed = {
            let _one_at_a_time = PROCESSING.acquire().await.map_err(|_| AppError::Internal)?;
            tokio::task::spawn_blocking(move || process(&data))
                .await
                .map_err(|_| AppError::Internal)??
        };
        let dir = uploads_dir();
        tokio::fs::create_dir_all(&dir).await.map_err(|_| AppError::Internal)?;
        let id = Uuid::new_v4();
        let (name, small_name) = (format!("{id}.webp"), format!("{id}-480.webp"));
        tokio::fs::write(format!("{dir}/{name}"), &processed.large)
            .await
            .map_err(|_| AppError::Internal)?;
        tokio::fs::write(format!("{dir}/{small_name}"), &processed.small)
            .await
            .map_err(|_| AppError::Internal)?;
        let base = public_base_url();
        return Ok((
            StatusCode::CREATED,
            Json(UploadResp {
                url: format!("{base}/uploads/{name}"),
                url_small: format!("{base}/uploads/{small_name}"),
            }),
        ));
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

    #[test]
    fn fit_never_upscales_and_bounds_the_long_side() {
        let img = DynamicImage::new_rgb8(300, 200);
        assert_eq!(fit(img, 480).width(), 300);
        let img = DynamicImage::new_rgb8(3000, 1000);
        let out = fit(img, 1600);
        assert_eq!((out.width(), out.height()), (1600, 533));
    }
}
