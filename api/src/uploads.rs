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
//! timeout ([`DECODE_WALL_SECS`]); no core dumps. At most [`UPLOAD_QUEUE`] uploads are let in at
//! once (one decoding, the rest waiting with their bodies in memory); one more gets 503 before its
//! body is read. Whatever a picture makes a decoder allocate, the child dies
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
/// CPU seconds the child may use (`RLIMIT_CPU`; the kernel kills it after that), unless
/// `IMAGE_DECODE_CPU_SECS` says otherwise.
pub const DECODE_CPU_SECS: u64 = 20;
/// Wall-clock seconds before the API kills the child, unless `IMAGE_DECODE_WALL_SECS` says otherwise.
pub const DECODE_WALL_SECS: u64 = 30;
/// Uploads let in at once: the one being decoded plus those waiting for it. Each holds its body
/// (≤ 5 MB) in API memory, so this bounds the waiting room; one more answers 503 at once, before its
/// body is read. `UPLOAD_QUEUE` overrides it (read once, at the first upload).
pub const UPLOAD_QUEUE: usize = 3;

/// The waiting-room size in force.
pub fn upload_queue_size() -> usize {
    static N: OnceLock<usize> = OnceLock::new();
    *N.get_or_init(|| env_u64("UPLOAD_QUEUE", UPLOAD_QUEUE as u64) as usize)
}
/// Most bytes read back from the child: the frame header plus two pictures of at most 16 MB.
const MAX_FRAME: usize = 16 + 4 + 2 * 16 * 1024 * 1024;

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

/// The waiting room (see [`UPLOAD_QUEUE`]).
fn upload_queue() -> Arc<Semaphore> {
    static Q: OnceLock<Arc<Semaphore>> = OnceLock::new();
    Q.get_or_init(|| Arc::new(Semaphore::new(upload_queue_size()))).clone()
}

/// Test hook: take the decode slot, as a long decode would.
#[doc(hidden)]
pub async fn hold_decode_slot() -> tokio::sync::OwnedSemaphorePermit {
    processing()
        .acquire_owned()
        .await
        .expect("the decode semaphore is never closed")
}

/// Test hook: free places in the waiting room.
#[doc(hidden)]
pub fn upload_queue_free() -> usize {
    upload_queue().available_permits()
}

/// The answer when the waiting room is full.
pub const BUSY_MESSAGE: &str = "забагато завантажень одночасно, спробуйте за хвилину";
/// The answer when the body did not arrive within [`UPLOAD_BODY_SECS`].
pub const SLOW_BODY_MESSAGE: &str = "завантаження триває надто довго — перевірте з'єднання і спробуйте ще раз";
/// Seconds a client has to send the whole upload body (`UPLOAD_BODY_SECS` overrides). A body that
/// trickles in would otherwise hold its waiting-room place for ever.
pub const UPLOAD_BODY_SECS: u64 = 60;

/// Users with an upload in the waiting room: one place per user, so one account cannot fill it.
fn users_in_room() -> &'static std::sync::Mutex<std::collections::HashSet<Uuid>> {
    static U: OnceLock<std::sync::Mutex<std::collections::HashSet<Uuid>>> = OnceLock::new();
    U.get_or_init(Default::default)
}

/// A user's place in the waiting room; dropping it frees the place.
struct UserPlace(Uuid);

impl UserPlace {
    fn take(user: Uuid) -> Option<Self> {
        // The lock is released BEFORE a guard can exist: `then_some(Self(..))` would build (and,
        // on refusal, drop) a guard while the lock is held — a self-deadlock that also frees the
        // place of the upload already in the room.
        let inserted = users_in_room().lock().unwrap_or_else(|p| p.into_inner()).insert(user);
        if inserted {
            Some(Self(user))
        } else {
            None
        }
    }
}

impl Drop for UserPlace {
    fn drop(&mut self) {
        users_in_room()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.0);
    }
}

/// Read the `file` field: its bytes, after the type and size checks.
async fn read_file(multipart: &mut Multipart) -> Result<axum::body::Bytes, AppError> {
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
        return Ok(data);
    }
    Err(AppError::Validation("missing 'file' field".into()))
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
            // The child could not encode: the same honest answer as any other child failure.
            Reject::Encode => AppError::Validation(COMPLEX_MESSAGE.into()),
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

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

/// The child's address-space limit in bytes (`IMAGE_DECODE_MEM_MB`, default 192).
pub fn decode_mem_bytes() -> u64 {
    env_u64("IMAGE_DECODE_MEM_MB", DEFAULT_DECODE_MEM_MB).saturating_mul(1024 * 1024)
}

/// Runs in the forked child between fork and exec: only async-signal-safe calls (setrlimit).
fn limit_child(mem_bytes: u64, cpu_secs: u64) -> std::io::Result<()> {
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
    set(libc::RLIMIT_CPU as libc::c_int, cpu_secs)?;
    // No core dump: it would hold the uploader's picture, on the box's disk.
    set(libc::RLIMIT_CORE as libc::c_int, 0)?;
    // Linux enforces the address-space limit; macOS accepts or refuses it but never enforces it,
    // so it is not set there.
    #[cfg(target_os = "linux")]
    set(libc::RLIMIT_AS as libc::c_int, mem_bytes)?;
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
    let cpu = env_u64("IMAGE_DECODE_CPU_SECS", DECODE_CPU_SECS);
    let wall = Duration::from_secs(env_u64("IMAGE_DECODE_WALL_SECS", DECODE_WALL_SECS));
    let mut cmd = tokio::process::Command::new(bin);
    cmd.arg(DECODE_SUBCOMMAND)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // A backstop only: every path below reaps the child itself.
        .kill_on_drop(true);
    // SAFETY: the closure only calls setrlimit (async-signal-safe) and allocates nothing.
    unsafe {
        cmd.pre_exec(move || limit_child(mem, cpu));
    }
    let mut child = cmd.spawn().map_err(|e| {
        tracing::error!(error = %e, "uploads: cannot start the decoder child");
        AppError::Internal
    })?;
    let mut stdin = child.stdin.take().ok_or(AppError::Internal)?;
    let stdout = child.stdout.take().ok_or(AppError::Internal)?;
    // The child stays OUTSIDE the timed future, so a timeout can kill it and wait for it: the slot
    // is released only after the child has been reaped.
    let io = async {
        // A child that dies early closes the pipe: the write error is irrelevant, its status says why.
        let _ = stdin.write_all(&data).await;
        drop(stdin);
        let mut out = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(
            &mut tokio::io::AsyncReadExt::take(stdout, MAX_FRAME as u64 + 1),
            &mut out,
        )
        .await
        .map(|_| out)
    };
    let out = match tokio::time::timeout(wall, io).await {
        Ok(Ok(out)) if out.len() <= MAX_FRAME => out,
        Ok(Ok(_)) => {
            tracing::warn!("uploads: decoder child answered more than the frame cap");
            let _ = child.kill().await;
            return Err(Reject::Complex.into());
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "uploads: decoder child I/O failed");
            let _ = child.kill().await;
            return Err(Reject::Complex.into());
        }
        Err(_) => {
            tracing::warn!("uploads: decoder child timed out; killing it");
            let _ = child.kill().await; // kill + wait: reaped before the slot is freed
            return Err(Reject::Complex.into());
        }
    };
    // stdout reached EOF: the child is exiting; reap it (bounded, in case it lingers).
    let status = match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
        Ok(Ok(status)) => status,
        _ => {
            let _ = child.kill().await;
            return Err(Reject::Complex.into());
        }
    };
    match status.code() {
        Some(0) => decode_frame(&out).ok_or_else(|| {
            tracing::error!("uploads: decoder child answered a malformed frame");
            AppError::Internal
        }),
        Some(EXIT_NOT_IMAGE) => Err(Reject::NotImage.into()),
        Some(EXIT_TOO_LARGE) => Err(Reject::TooLarge.into()),
        Some(EXIT_COMPLEX | EXIT_ENCODE) => Err(Reject::Complex.into()),
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
    // The waiting room, BEFORE any of the body is read: a full room answers 503 at once.
    let queued = upload_queue()
        .try_acquire_owned()
        .map_err(|_| AppError::Unavailable(BUSY_MESSAGE.into()))?;
    // One place per user: a second upload of the same account while one is in the room → 429.
    let user_place = UserPlace::take(user.id).ok_or(AppError::RateLimited)?;
    crate::ratelimit::hit(
        &st.db,
        &format!("upload:{}", user.id),
        crate::ratelimit::env_limit("UPLOAD_LIMIT_PER_HOUR", 120),
        3600,
    )
    .await?;
    // The whole body within the deadline; a trickling client loses its place with 408.
    let deadline = Duration::from_secs(env_u64("UPLOAD_BODY_SECS", UPLOAD_BODY_SECS));
    let data = tokio::time::timeout(deadline, read_file(&mut multipart))
        .await
        .map_err(|_| AppError::RequestTimeout(SLOW_BODY_MESSAGE.into()))??;
    // Both permits move INTO a task of its own: dropping this request (client gone) neither
    // frees them nor stops the child. The decode slot is released when the child has been
    // reaped (`decode_in_child` waits for it on every path, timeout included); the waiting-room
    // place right after.
    let permit = processing().acquire_owned().await.map_err(|_| AppError::Internal)?;
    let processed = tokio::spawn(async move {
        let _in_the_room = (queued, user_place);
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
    Ok((
        StatusCode::CREATED,
        Json(UploadResp {
            url: format!("{base}/uploads/{name}"),
            url_small: format!("{base}/uploads/{small_name}"),
        }),
    ))
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
    fn a_refused_second_place_leaves_the_first_in_place() {
        let u = Uuid::new_v4();
        let first = UserPlace::take(u).expect("the first place");
        assert!(UserPlace::take(u).is_none(), "one place per user");
        assert!(UserPlace::take(u).is_none(), "the refusal did not free the first place");
        drop(first);
        let again = UserPlace::take(u);
        assert!(again.is_some(), "free again once the first ends");
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
