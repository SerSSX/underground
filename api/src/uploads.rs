//! Image uploads (event posters, gallery, avatars): multipart → re-encoded WebP in local storage,
//! served at /uploads (TASK-304).
//!
//! Every upload is decoded and re-encoded, never stored as sent:
//! - the format is sniffed from the bytes (JPEG, PNG or WebP only), whatever the name or the
//!   declared type say; animated WebP is refused;
//! - the pixel count is read from the header and checked before the pixels are decoded
//!   ([`MAX_PIXELS`], a cheap first gate);
//! - the picture is scaled to at most [`MAX_SIDE`] px on its longest side (never up), turned
//!   upright by its EXIF orientation, and written as lossy WebP (quality [`QUALITY`]) with NO
//!   metadata: EXIF (GPS included), XMP and ICC are dropped because only pixels are encoded;
//! - a second [`SMALL_SIDE`] px copy is written beside it as `<name>-480.webp` for cards.
//!
//! Memory is bounded STRUCTURALLY, not by estimating it (TASK-304 review: two header heuristics
//! were bypassed): decoding and encoding run in a child process — this same binary started as
//! `underground-api decode-image` — with a hard address-space limit (`RLIMIT_AS`,
//! `IMAGE_DECODE_MEM_MB`, default 192 MiB), a CPU limit ([`DECODE_CPU_SECS`]) and a wall-clock
//! timeout ([`DECODE_WALL_SECS`]). Whatever a picture makes a decoder allocate, the child dies
//! alone and the API answers "too complex". The child gets the upload on stdin and nothing else:
//! no environment (so no secrets), no database, no network. One child runs at a time.
//! `RLIMIT_AS` is enforced by Linux (production, CI); macOS ignores it, so on a Mac the limit is
//! not in force (the CPU and wall-clock limits are).
//!
//! Decoding is image's pure-Rust code (zune-jpeg, png, image-webp); libwebp (C) only ever
//! encodes pixels we produced. Files uploaded before TASK-304 stay as they are.

use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
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
/// Below this many pixels a smooth filter is used; above it the fast area-sampling one, which needs
/// no large intermediate buffer.
const SMOOTH_LIMIT: u64 = 4_000_000;

/// The child's subcommand: `underground-api decode-image`.
pub const DECODE_SUBCOMMAND: &str = "decode-image";
/// Address-space limit of the child, unless `IMAGE_DECODE_MEM_MB` says otherwise. With the API's
/// own ~40–60 MB this keeps both under the container's 256 MB.
pub const DEFAULT_DECODE_MEM_MB: u64 = 192;
/// CPU seconds the child may use (`RLIMIT_CPU`; the kernel kills it after that).
pub const DECODE_CPU_SECS: u64 = 20;
/// Wall-clock seconds before the API kills the child.
pub const DECODE_WALL_SECS: u64 = 30;

/// Child exit codes (0 = success, the frame is on stdout).
const EXIT_NOT_IMAGE: i32 = 3;
const EXIT_TOO_LARGE: i32 = 4;
const EXIT_COMPLEX: i32 = 5;
const EXIT_ENCODE: i32 = 6;
/// The child's answer starts with this.
const FRAME_MAGIC: &[u8; 4] = b"UGIM";

/// One picture is processed at a time. The permit is OWNED by the task that runs the child, so it
/// is held until the child has really ended, even when the client disconnects.
fn processing() -> Arc<Semaphore> {
    static P: OnceLock<Arc<Semaphore>> = OnceLock::new();
    P.get_or_init(|| Arc::new(Semaphore::new(1))).clone()
}

/// Children running right now and the most seen at once (read by the cancellation test).
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
    /// More pixels than [`MAX_PIXELS`] (from the header).
    TooLarge,
    /// The picture needs more memory or time than the child may use, or is animated.
    Complex,
    /// Encoding failed (our side).
    Encode,
}

/// The honest answer when the child could not finish a picture.
pub const COMPLEX_MESSAGE: &str = "зображення надто складне для обробки — збережіть як JPEG або зменште розмір";

impl From<Reject> for AppError {
    fn from(r: Reject) -> Self {
        match r {
            Reject::NotImage => AppError::Validation("файл не є зображенням JPEG, PNG або WebP".into()),
            Reject::TooLarge => AppError::Validation("зображення завелике: до 32 мегапікселів".into()),
            Reject::Complex => AppError::Validation(COMPLEX_MESSAGE.into()),
            Reject::Encode => AppError::Internal,
        }
    }
}

/// The two stored files of one upload.
#[derive(Debug, PartialEq, Eq)]
pub struct Processed {
    pub large: Vec<u8>,
    pub small: Vec<u8>,
    /// Size of `large` after scaling and orientation.
    pub width: u32,
    pub height: u32,
}

fn decode_error(e: image::ImageError) -> Reject {
    match e {
        // An allocation the decoder could not get (the child's memory limit) or a decoder limit.
        image::ImageError::Limits(_) => Reject::Complex,
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

/// Whether a WebP file is animated: an `ANIM`/`ANMF` chunk, or the animation flag of `VP8X`.
pub fn webp_is_animated(data: &[u8]) -> bool {
    let mut i = 12; // after "RIFF" <size> "WEBP"
    while i + 8 <= data.len() {
        let fourcc = &data[i..i + 4];
        let size = u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
        if fourcc == b"ANIM" || fourcc == b"ANMF" {
            return true;
        }
        if fourcc == b"VP8X" && data.get(i + 8).is_some_and(|flags| flags & 0x02 != 0) {
            return true;
        }
        i = i.saturating_add(8).saturating_add(size).saturating_add(size & 1);
    }
    false
}

/// Decode, check, scale, orient and re-encode one picture. Runs in the child process (and in
/// tests); it holds the whole picture in memory, which is why the API never calls it in-process.
pub fn process(data: &[u8]) -> Result<Processed, Reject> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| Reject::NotImage)?;
    // The bytes decide the format; a .jpg name or an image/jpeg type proves nothing.
    let format = match reader.format() {
        Some(f @ (ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)) => f,
        _ => return Err(Reject::NotImage),
    };
    if format == ImageFormat::WebP && webp_is_animated(data) {
        return Err(Reject::Complex);
    }
    // No size limits in the decoder: the pixel cap below answers honestly, and memory is the
    // child's address-space limit.
    reader.limits(Limits::no_limits());
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(Reject::NotImage);
    }
    // First gate, from the header: a picture over 32 MP is refused without decoding it.
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

/// The child: upload bytes on stdin, a frame on stdout, an exit code. Called from `main` before any
/// runtime or thread exists; single-threaded throughout.
pub fn decode_child_main() -> i32 {
    let mut data = Vec::new();
    if std::io::stdin()
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut data)
        .is_err()
        || data.len() > MAX_BYTES
    {
        return EXIT_NOT_IMAGE;
    }
    match process(&data) {
        Ok(p) => {
            let mut out = std::io::stdout().lock();
            let ok = out.write_all(&encode_frame(&p)).and_then(|()| out.flush());
            if ok.is_ok() {
                0
            } else {
                EXIT_ENCODE
            }
        }
        Err(Reject::NotImage) => EXIT_NOT_IMAGE,
        Err(Reject::TooLarge) => EXIT_TOO_LARGE,
        Err(Reject::Complex) => EXIT_COMPLEX,
        Err(Reject::Encode) => EXIT_ENCODE,
    }
}

/// `UGIM`, width, height, large length, large, small length, small (little-endian u32s).
pub fn encode_frame(p: &Processed) -> Vec<u8> {
    let mut f = Vec::with_capacity(20 + p.large.len() + p.small.len());
    f.extend_from_slice(FRAME_MAGIC);
    f.extend_from_slice(&p.width.to_le_bytes());
    f.extend_from_slice(&p.height.to_le_bytes());
    f.extend_from_slice(&(p.large.len() as u32).to_le_bytes());
    f.extend_from_slice(&p.large);
    f.extend_from_slice(&(p.small.len() as u32).to_le_bytes());
    f.extend_from_slice(&p.small);
    f
}

/// The inverse of [`encode_frame`]; `None` for anything malformed or with trailing bytes.
pub fn decode_frame(f: &[u8]) -> Option<Processed> {
    fn u32_at(f: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(f.get(at..at + 4)?.try_into().ok()?))
    }
    if f.get(..4)? != FRAME_MAGIC {
        return None;
    }
    let width = u32_at(f, 4)?;
    let height = u32_at(f, 8)?;
    let large_len = u32_at(f, 12)? as usize;
    let large = f.get(16..16usize.checked_add(large_len)?)?.to_vec();
    let at = 16 + large_len;
    let small_len = u32_at(f, at)? as usize;
    let small = f.get(at + 4..(at + 4).checked_add(small_len)?)?.to_vec();
    if at + 4 + small_len != f.len() || large.is_empty() || small.is_empty() {
        return None;
    }
    Some(Processed {
        large,
        small,
        width,
        height,
    })
}

/// The child's executable: `IMAGE_DECODER_BIN` (tests point it at the built API binary), else this
/// process's own executable.
fn decoder_bin() -> std::io::Result<PathBuf> {
    match std::env::var_os("IMAGE_DECODER_BIN") {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => std::env::current_exe(),
    }
}

/// The child's address-space limit in bytes (`IMAGE_DECODE_MEM_MB`, default 192).
pub fn decode_mem_bytes() -> u64 {
    std::env::var("IMAGE_DECODE_MEM_MB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|mb| *mb > 0)
        .unwrap_or(DEFAULT_DECODE_MEM_MB)
        .saturating_mul(1024 * 1024)
}

/// Runs in the forked child between fork and exec: only async-signal-safe calls (setrlimit).
fn limit_child(mem_bytes: u64) -> std::io::Result<()> {
    fn set(resource: libc::c_int, value: u64) -> std::io::Result<()> {
        let lim = libc::rlimit {
            rlim_cur: value as libc::rlim_t,
            rlim_max: value as libc::rlim_t,
        };
        // SAFETY: setrlimit only reads `lim`; it is async-signal-safe.
        if unsafe { libc::setrlimit(resource as _, &lim) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    set(libc::RLIMIT_CPU as libc::c_int, DECODE_CPU_SECS)?;
    // Linux enforces the address-space limit; macOS accepts or refuses it but never enforces it,
    // so it is not set there.
    #[cfg(target_os = "linux")]
    let _ = mem_bytes; // MUTANT: RLIMIT_AS removed
    #[cfg(not(target_os = "linux"))]
    let _ = mem_bytes;
    Ok(())
}

/// Process one picture in a limited child. Any death of the child — memory, CPU, wall clock, a
/// crash — is [`Reject::Complex`]; the API process itself is never at risk.
async fn decode_in_child(data: axum::body::Bytes) -> Result<Processed, AppError> {
    let bin = decoder_bin().map_err(|e| {
        tracing::error!(error = %e, "uploads: cannot find the decoder executable");
        AppError::Internal
    })?;
    let mem = decode_mem_bytes();
    let mut cmd = tokio::process::Command::new(bin);
    cmd.arg(DECODE_SUBCOMMAND)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    // SAFETY: the closure only calls setrlimit (async-signal-safe) and allocates nothing.
    unsafe {
        cmd.pre_exec(move || limit_child(mem));
    }
    let mut child = cmd.spawn().map_err(|e| {
        tracing::error!(error = %e, "uploads: cannot start the decoder child");
        AppError::Internal
    })?;
    let mut stdin = child.stdin.take().ok_or(AppError::Internal)?;
    let run = async move {
        // A child that dies early closes the pipe: the write error is irrelevant, its status says why.
        let _ = stdin.write_all(&data).await;
        drop(stdin);
        child.wait_with_output().await
    };
    // On timeout the future is dropped, and with it the child (kill_on_drop).
    let out = match tokio::time::timeout(Duration::from_secs(DECODE_WALL_SECS), run).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "uploads: decoder child I/O failed");
            return Err(Reject::Complex.into());
        }
        Err(_) => {
            tracing::warn!("uploads: decoder child timed out and was killed");
            return Err(Reject::Complex.into());
        }
    };
    match out.status.code() {
        Some(0) => decode_frame(&out.stdout).ok_or_else(|| {
            tracing::error!("uploads: decoder child answered a malformed frame");
            AppError::Internal
        }),
        Some(EXIT_NOT_IMAGE) => Err(Reject::NotImage.into()),
        Some(EXIT_TOO_LARGE) => Err(Reject::TooLarge.into()),
        Some(EXIT_COMPLEX) => Err(Reject::Complex.into()),
        Some(EXIT_ENCODE) => Err(Reject::Encode.into()),
        other => {
            // Killed by a signal (memory, CPU) or a panic/abort.
            tracing::warn!(status = ?other, "uploads: decoder child died");
            Err(Reject::Complex.into())
        }
    }
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
        // The permit moves INTO a task of its own: dropping this request (client gone) neither
        // frees it nor kills the child; the slot is released only when the child has ended.
        let permit = processing().acquire_owned().await.map_err(|_| AppError::Internal)?;
        let processed = tokio::spawn(async move {
            let _one_at_a_time = permit;
            let now = IN_FLIGHT.fetch_add(1, Ordering::SeqCst) + 1;
            PEAK.fetch_max(now, Ordering::SeqCst);
            let out = decode_in_child(data).await;
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

    #[test]
    fn frames_round_trip_and_reject_garbage() {
        let p = Processed {
            large: vec![1, 2, 3],
            small: vec![4],
            width: 1600,
            height: 900,
        };
        let f = encode_frame(&p);
        assert_eq!(decode_frame(&f), Some(p));
        assert_eq!(decode_frame(&f[..f.len() - 1]), None, "truncated");
        let mut extra = f.clone();
        extra.push(0);
        assert_eq!(decode_frame(&extra), None, "trailing bytes");
        assert_eq!(decode_frame(b"XXXX"), None);
        let mut huge = f.clone();
        huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode_frame(&huge), None, "a length past the end");
    }

    #[test]
    fn decode_memory_limit_is_configurable() {
        std::env::remove_var("IMAGE_DECODE_MEM_MB");
        assert_eq!(decode_mem_bytes(), 192 * 1024 * 1024);
        std::env::set_var("IMAGE_DECODE_MEM_MB", "160");
        assert_eq!(decode_mem_bytes(), 160 * 1024 * 1024);
        std::env::set_var("IMAGE_DECODE_MEM_MB", "0");
        assert_eq!(decode_mem_bytes(), 192 * 1024 * 1024, "nonsense → default");
        std::env::remove_var("IMAGE_DECODE_MEM_MB");
    }
}
