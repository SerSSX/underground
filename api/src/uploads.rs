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
//! encodes pixels we produced. One image is processed at a time, and no decode may need more than
//! [`DECODE_BUDGET`] (estimated from the header: pixel buffer, plus the coefficients of a
//! progressive JPEG), so the worst case stays well under the API container's 256 MB.
//! Files uploaded before TASK-304 stay as they are; their URLs keep working.

use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

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
/// The decoder's own allocation cap (a second line of defence; `from_decoder` does not apply it to
/// the pixel buffer, which is why [`DECODE_BUDGET`] exists).
const MAX_ALLOC: u64 = 160 * 1024 * 1024;
/// Most bytes one upload's processing may need, estimated from the header BEFORE decoding
/// (TASK-304 review; see [`decode_cost`]). With the process's own ~6 MB the measured peak stays
/// under ~150 MiB for every accepted picture.
pub const DECODE_BUDGET: u64 = 140 * 1024 * 1024;
/// Below this many pixels a smooth filter is used; above it the fast area-sampling one, which needs
/// no large intermediate buffer.
const SMOOTH_LIMIT: u64 = 4_000_000;
/// One picture is decoded at a time. The permit is OWNED by the blocking task, so it is held until
/// the decode really ends, even when the client disconnects and the request future is dropped.
fn processing() -> Arc<Semaphore> {
    static P: OnceLock<Arc<Semaphore>> = OnceLock::new();
    P.get_or_init(|| Arc::new(Semaphore::new(1))).clone()
}

/// Decodes running right now and the most seen at once (read by the cancellation test).
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
#[doc(hidden)]
pub fn decode_stats() -> (usize, usize) {
    (IN_FLIGHT.load(Ordering::SeqCst), PEAK.load(Ordering::SeqCst))
}

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
    // The smooth filter works through a float copy (16 B/px): only for small 8-bit pictures.
    let eight_bit = matches!(
        img,
        DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
            | DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
    );
    if eight_bit && u64::from(img.width()) * u64::from(img.height()) <= SMOOTH_LIMIT {
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

/// For a progressive JPEG, the bytes its decoder holds for DCT coefficients: 2 bytes for each of
/// the 64 coefficients of every 8×8 block of every component, at its sampling. `Some(0)` for a
/// sequential (baseline) JPEG; `None` if no frame header is found before the scan data.
pub fn jpeg_coefficient_bytes(data: &[u8]) -> Option<u64> {
    if data.get(..2) != Some(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    while i + 1 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let m = data[i + 1];
        if m == 0xFF {
            i += 1; // fill byte
            continue;
        }
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            i += 2; // markers without a length
            continue;
        }
        if m == 0xD9 || m == 0xDA {
            return None; // end of image or scan data before any frame header
        }
        let len = usize::from(u16::from_be_bytes([*data.get(i + 2)?, *data.get(i + 3)?]));
        let seg = data.get(i + 4..i + 2 + len.max(2))?;
        // SOF0..SOF15 except DHT (C4), JPG (C8), DAC (CC).
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            if !matches!(m, 0xC2 | 0xC6 | 0xCA | 0xCE) {
                return Some(0);
            }
            let h = u64::from(u16::from_be_bytes([*seg.get(1)?, *seg.get(2)?]));
            let w = u64::from(u16::from_be_bytes([*seg.get(3)?, *seg.get(4)?]));
            let nf = usize::from(*seg.get(5)?);
            let comps: Vec<(u64, u64)> = (0..nf)
                .map(|c| {
                    seg.get(6 + 3 * c + 1)
                        .map(|hv| (u64::from(hv >> 4).max(1), u64::from(hv & 0x0F).max(1)))
                })
                .collect::<Option<_>>()?;
            let hmax = comps.iter().map(|c| c.0).max()?;
            let vmax = comps.iter().map(|c| c.1).max()?;
            // Blocks per component, padded to whole MCUs as decoders allocate them.
            let mcu_x = w.div_ceil(8 * hmax);
            let mcu_y = h.div_ceil(8 * vmax);
            let blocks: u64 = comps.iter().map(|(hs, vs)| mcu_x * hs * mcu_y * vs).sum();
            return Some(blocks * 64 * 2);
        }
        i += 2 + len;
    }
    None
}

/// What processing a picture will hold at its peak, from the header alone. `total_bytes` is the
/// decoded pixel buffer (width × height × bytes per pixel, so 16-bit and alpha count); each decoder
/// holds a multiple of it at its peak (copies, row and plane buffers), and a progressive JPEG also
/// keeps the DCT coefficients of the whole picture. Multipliers are the measured peak RSS ÷ buffer
/// on worst cases (release build, 2026-11-13, table in docs/backlog/TASK-304-…md), rounded up:
/// JPEG ×1.3 (measured 1.27) and coefficients ×1.2 (1.24); PNG ×1.4 (1.28), with alpha ×1.9
/// (1.77), 16-bit ×2 + 80 MiB; WebP ×1.8 (1.67), with alpha ×4.2 (3.75–4.0).
pub fn decode_cost(
    format: ImageFormat,
    total_bytes: u64,
    sixteen_bit: bool,
    alpha: bool,
    jpeg_coefficients: u64,
) -> u64 {
    const MIB: u64 = 1024 * 1024;
    // Buffer and coefficient multipliers in tenths, plus a fixed part.
    let (buffer, coefficients, fixed) = match format {
        ImageFormat::Jpeg => (13, 12, 0),
        // 16-bit: the peak grew ~1.9× the buffer plus ~75 MiB however small the picture
        // (5.3 MP → 151 MiB, 7 MP → 175 MiB), so it gets that fixed part.
        ImageFormat::Png if sixteen_bit => (20, 0, 80 * MIB),
        ImageFormat::Png if alpha => (19, 0, 0),
        ImageFormat::Png => (14, 0, 0),
        ImageFormat::WebP if alpha => (42, 0, 0),
        _ => (18, 0, 0),
    };
    (total_bytes
        .saturating_mul(buffer)
        .saturating_add(jpeg_coefficients.saturating_mul(coefficients))
        / 10)
        .saturating_add(fixed)
}

/// Decode, check, scale, orient and re-encode one picture (blocking: CPU-bound).
pub fn process(data: &[u8]) -> Result<Processed, Reject> {
    process_with_budget(data, DECODE_BUDGET)
}

/// [`process`] with another decode budget — for the memory measurement only.
#[doc(hidden)]
pub fn process_with_budget(data: &[u8], budget: u64) -> Result<Processed, Reject> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| Reject::NotImage)?;
    // The bytes decide the format; a .jpg name or an image/jpeg type proves nothing.
    let reader_format = match reader.format() {
        Some(f @ (ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)) => f,
        _ => return Err(Reject::NotImage),
    };
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
    // Memory guard: what the decode will hold, from the header, before it allocates anything.
    let coefficients = if reader_format == ImageFormat::Jpeg {
        jpeg_coefficient_bytes(data).ok_or(Reject::NotImage)?
    } else {
        0
    };
    let color = decoder.color_type();
    let sixteen_bit = color.bytes_per_pixel() > color.channel_count();
    let cost = decode_cost(
        reader_format,
        decoder.total_bytes(),
        sixteen_bit,
        color.has_alpha(),
        coefficients,
    );
    if cost > budget {
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
        // The permit moves INTO the blocking task: dropping this request (client gone) does not
        // free it while the decode is still running.
        let permit = processing().acquire_owned().await.map_err(|_| AppError::Internal)?;
        let processed = tokio::task::spawn_blocking(move || {
            let _one_at_a_time = permit;
            let now = IN_FLIGHT.fetch_add(1, Ordering::SeqCst) + 1;
            PEAK.fetch_max(now, Ordering::SeqCst);
            let out = process(&data);
            IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
            out
        })
        .await
        .map_err(|_| AppError::Internal)??;
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
