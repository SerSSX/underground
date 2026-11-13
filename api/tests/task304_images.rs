//! TASK-304: uploaded pictures are re-encoded as WebP — bounded size, a 480 px card copy, no
//! metadata, upright by EXIF orientation — and anything that is not a picture, or claims too many
//! pixels, is refused.

use std::io::Cursor;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::uploads::{process, Reject};

const BOUNDARY: &str = "XTESTBOUNDARY";

fn make_app(pool: PgPool) -> Router {
    std::env::set_var("UPLOADS_DIR", "/tmp/underground-test-uploads");
    // TASK-304 review: uploads are decoded by a child process — this test build's API binary.
    std::env::set_var("IMAGE_DECODER_BIN", env!("CARGO_BIN_EXE_underground-api"));
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

async fn register_token(app: &Router) -> String {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"img@x.co","password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let v: Value = serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    v["token"].as_str().unwrap().to_string()
}

fn upload_req(token: &str, filename: &str, content_type: &str, data: &[u8]) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/api/uploads")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

async fn fetch(app: &Router, url: &str) -> Vec<u8> {
    let path = &url[url.find("/uploads/").unwrap()..];
    let resp = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "{path}");
    to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()
}

/// A picture with detail (so the encoder has something to keep), in `format`.
fn picture(w: u32, h: u32, format: ImageFormat) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x / 7 + y / 5) % 256) as u8])
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(img).write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn is_webp(b: &[u8]) -> bool {
    b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

fn dims(b: &[u8]) -> (u32, u32) {
    image::load_from_memory_with_format(b, ImageFormat::WebP)
        .unwrap()
        .dimensions()
}

/// A JPEG whose EXIF says "rotate 90°" (orientation 6) and carries a description we can look for.
fn jpeg_with_exif(w: u32, h: u32, marker: &str) -> Vec<u8> {
    let jpeg = picture(w, h, ImageFormat::Jpeg);
    let mut s = marker.as_bytes().to_vec();
    s.push(0);
    // TIFF, big-endian: header, IFD0 at 8 with 2 entries (ImageDescription, Orientation), then the string.
    let mut tiff = b"MM\x00\x2a\x00\x00\x00\x08".to_vec();
    tiff.extend_from_slice(&2u16.to_be_bytes());
    tiff.extend_from_slice(&[0x01, 0x0E, 0x00, 0x02]);
    tiff.extend_from_slice(&(s.len() as u32).to_be_bytes());
    tiff.extend_from_slice(&38u32.to_be_bytes());
    tiff.extend_from_slice(&[0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1, 0x00, 0x06, 0, 0]);
    tiff.extend_from_slice(&0u32.to_be_bytes());
    assert_eq!(tiff.len(), 38);
    tiff.extend_from_slice(&s);
    let mut app1 = b"Exif\x00\x00".to_vec();
    app1.extend_from_slice(&tiff);
    let mut out = jpeg[..2].to_vec(); // SOI
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A valid tiny RGB PNG whose header claims `w`×`h` pixels (a decompression bomb's shape).
fn png_claiming(w: u32, h: u32) -> Vec<u8> {
    png_claiming_as(w, h, 8, 2)
}

/// The same with a given bit depth and colour type (2 = RGB, 6 = RGBA).
fn png_claiming_as(w: u32, h: u32, depth: u8, color_type: u8) -> Vec<u8> {
    let mut png = picture(1, 1, ImageFormat::Png);
    assert_eq!(&png[12..16], b"IHDR");
    png[16..20].copy_from_slice(&w.to_be_bytes());
    png[20..24].copy_from_slice(&h.to_be_bytes());
    png[24] = depth;
    png[25] = color_type;
    let crc = crc32(&png[12..29]);
    png[29..33].copy_from_slice(&crc.to_be_bytes());
    png
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_is_webp_bounded_with_a_small_copy(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let src = picture(2400, 1800, ImageFormat::Png);
    let (status, v) = send(&app, upload_req(&token, "big.png", "image/png", &src)).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    let (url, small) = (v["url"].as_str().unwrap(), v["url_small"].as_str().unwrap());
    assert!(url.ends_with(".webp") && small.ends_with("-480.webp"), "{url} {small}");
    assert_eq!(
        small,
        url.replace(".webp", "-480.webp"),
        "the card copy sits beside the picture"
    );

    let large = fetch(&app, url).await;
    assert!(is_webp(&large));
    assert_eq!(dims(&large), (1600, 1200), "longest side 1600");
    let small_b = fetch(&app, small).await;
    assert!(is_webp(&small_b));
    assert_eq!(dims(&small_b), (480, 360), "card copy 480");
    assert!(small_b.len() < large.len());
}

#[sqlx::test(migrations = "./migrations")]
async fn small_pictures_are_not_upscaled(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let (status, v) = send(
        &app,
        upload_req(&token, "s.jpg", "image/jpeg", &picture(300, 200, ImageFormat::Jpeg)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(dims(&fetch(&app, v["url"].as_str().unwrap()).await), (300, 200));
    assert_eq!(dims(&fetch(&app, v["url_small"].as_str().unwrap()).await), (300, 200));
}

#[sqlx::test(migrations = "./migrations")]
async fn exif_is_stripped_and_orientation_applied(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let src = jpeg_with_exif(40, 20, "GPS-SECRET-48.85N");
    assert!(
        src.windows(17).any(|w| w == b"GPS-SECRET-48.85N"),
        "fixture carries the marker"
    );
    let (status, v) = send(&app, upload_req(&token, "phone.jpg", "image/jpeg", &src)).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    for url in [v["url"].as_str().unwrap(), v["url_small"].as_str().unwrap()] {
        let out = fetch(&app, url).await;
        assert!(is_webp(&out));
        assert!(!out.windows(10).any(|w| w == b"GPS-SECRET"), "[SEC] metadata text gone");
        assert!(
            !out.windows(4).any(|w| w == b"EXIF" || w == b"Exif"),
            "[SEC] no EXIF chunk"
        );
        assert!(!out.windows(4).any(|w| w == b"XMP "), "no XMP chunk");
        assert_eq!(dims(&out), (20, 40), "orientation 6 applied: upright portrait");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn non_images_are_refused_whatever_their_name(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    for (name, data) in [
        ("photo.jpg", b"<?php system($_GET['c']); ?>".to_vec()),
        ("photo.jpg", b"%PDF-1.7 not an image".to_vec()),
        ("photo.png", b"\x89PNG\r\n\x1a\n truncated".to_vec()),
        ("anim.jpg", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec()),
    ] {
        let (status, v) = send(&app, upload_req(&token, name, "image/jpeg", &data)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] {name}: {data:?}");
        assert_eq!(v["error"], "файл не є зображенням JPEG, PNG або WebP");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn too_many_pixels_are_refused_before_decoding(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    // 6000×6000 = 36 MP: over the 32 MP cap, refused from the header by the pixel check (in the
    // child), with the pixel message.
    let bomb = png_claiming(6000, 6000);
    assert!(bomb.len() < 200, "a tiny file");
    let (status, v) = send(&app, upload_req(&token, "bomb.png", "image/png", &bomb)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        v["error"], "зображення завелике: до 32 мегапікселів",
        "[SEC] the pixel cap refused it"
    );
    // A 100k × 100k claim gets the same honest answer (no decoder size limit answers first).
    assert_eq!(process(&png_claiming(100_000, 100_000)).err(), Some(Reject::TooLarge));
    // Exactly at the cap (8000 × 4000) passes the check (and then fails on the missing pixel data).
    assert_eq!(process(&png_claiming(8000, 4000)).err(), Some(Reject::NotImage));
}

#[sqlx::test(migrations = "./migrations")]
async fn uploads_are_rate_limited_per_user(pool: PgPool) {
    let app = make_app(pool.clone());
    let token = register_token(&app).await;
    let png = picture(10, 10, ImageFormat::Png);
    assert_eq!(
        send(&app, upload_req(&token, "a.png", "image/png", &png)).await.0,
        StatusCode::CREATED
    );
    // 119 more uploads this hour (the default cap is 120), recorded directly.
    sqlx::query(
        "INSERT INTO rate_events (bucket) SELECT 'upload:' || id FROM users, generate_series(1, 119) WHERE email = 'img@x.co'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        send(&app, upload_req(&token, "a.png", "image/png", &png)).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[test]
fn transparency_is_kept() {
    let img = image::RgbaImage::from_fn(64, 64, |x, _| image::Rgba([255, 0, 0, if x < 32 { 0 } else { 255 }]));
    let mut src = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img)
        .write_to(&mut src, ImageFormat::Png)
        .unwrap();
    let out = process(&src.into_inner()).unwrap();
    let back = image::load_from_memory_with_format(&out.large, ImageFormat::WebP).unwrap();
    assert!(back.color().has_alpha());
    assert!(back.get_pixel(5, 5)[3] < 10 && back.get_pixel(60, 5)[3] > 245);
}

const COMPLEX: &str = "зображення надто складне для обробки — збережіть як JPEG або зменште розмір";

/// Upload `data`; the answer must be a stored picture or the honest "too complex" refusal — and the
/// API must keep working afterwards (a small upload right after answers 201).
async fn processed_or_too_complex(app: &Router, token: &str, name: &str, ct: &str, data: &[u8]) -> StatusCode {
    let (status, v) = send(app, upload_req(token, name, ct, data)).await;
    match status {
        StatusCode::CREATED => {}
        StatusCode::UNPROCESSABLE_ENTITY => assert_eq!(v["error"], COMPLEX, "{name}: {v}"),
        other => panic!("{name}: unexpected {other} {v}"),
    }
    let (after, _) = send(
        app,
        upload_req(token, "ok.png", "image/png", &picture(16, 16, ImageFormat::Png)),
    )
    .await;
    assert_eq!(after, StatusCode::CREATED, "[SEC] the API survives {name}");
    status
}

/// Review round 3: worst cases the old header estimate let through (a decoy marker before a
/// progressive frame; lossless WebP; 16-bit RGBA at the pixel cap) and an animated WebP. Each is
/// either processed or refused as too complex, and the API process survives. (Was: refused as "too
/// large" by `decode_cost`; now: decided by the child's hard memory limit; why: the heuristic was
/// bypassed twice and is deleted.)
#[sqlx::test(migrations = "./migrations")]
async fn hostile_pictures_are_processed_or_refused_and_the_api_survives(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    for (name, ct, data) in [
        (
            "prog-decoy.jpg",
            "image/jpeg",
            &include_bytes!("fixtures/prog444_32mp_decoy_sof3.jpg")[..],
        ),
        (
            "lossless.webp",
            "image/webp",
            &include_bytes!("fixtures/lossless_27mp.webp")[..],
        ),
        (
            "deep.png",
            "image/png",
            &include_bytes!("fixtures/png16_rgba_5656.png")[..],
        ),
    ] {
        processed_or_too_complex(&app, &token, name, ct, data).await;
    }
    // Animated WebP is always refused (first frame only would silently drop the rest).
    let anim = include_bytes!("fixtures/animated.webp");
    assert!(underground_api::uploads::webp_is_animated(anim));
    assert_eq!(
        processed_or_too_complex(&app, &token, "anim.webp", "image/webp", anim).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

/// The proof that the memory limit is in force — Linux only, because macOS does not enforce
/// RLIMIT_AS (CI is Linux). Each of these needs far more than the child's 192 MiB (measured peaks
/// in-process: ~307, ~270 and ~311 MiB), so with the limit in place every one is refused as too
/// complex; without it they would be stored (201).
#[cfg(target_os = "linux")]
#[sqlx::test(migrations = "./migrations")]
async fn the_child_memory_limit_refuses_heavy_decodes(pool: PgPool) {
    std::env::remove_var("IMAGE_DECODE_MEM_MB");
    let app = make_app(pool);
    let token = register_token(&app).await;
    for (name, ct, data) in [
        (
            "prog-decoy.jpg",
            "image/jpeg",
            &include_bytes!("fixtures/prog444_32mp_decoy_sof3.jpg")[..],
        ),
        (
            "lossless.webp",
            "image/webp",
            &include_bytes!("fixtures/lossless_27mp.webp")[..],
        ),
        (
            "deep.png",
            "image/png",
            &include_bytes!("fixtures/png16_rgba_5656.png")[..],
        ),
    ] {
        assert_eq!(
            processed_or_too_complex(&app, &token, name, ct, data).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "[SEC] {name} must hit the child's memory limit"
        );
    }
}

/// Review round 3 (D): a stray 0x00 between JPEG segments is accepted, as every decoder does.
#[sqlx::test(migrations = "./migrations")]
async fn a_jpeg_with_a_stray_zero_between_segments_is_accepted(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let jpeg = picture(64, 48, ImageFormat::Jpeg);
    // After SOI comes the first segment (FF xx len); put a 0x00 after that segment.
    assert_eq!(&jpeg[0..2], &[0xFF, 0xD8]);
    let first_len = usize::from(u16::from_be_bytes([jpeg[4], jpeg[5]]));
    let cut = 4 + first_len;
    let mut odd = jpeg[..cut].to_vec();
    odd.push(0x00);
    odd.extend_from_slice(&jpeg[cut..]);
    let (status, v) = send(&app, upload_req(&token, "odd.jpg", "image/jpeg", &odd)).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
}

/// Peak-memory measurement (TASK-304 review), not part of the suite. Run the test binary under
/// `/usr/bin/time -l` with `UG_MEASURE_FILE=<picture>` and read "maximum resident set size" (this
/// decodes IN-process, without the child's limit, to see what a picture really needs).
#[test]
#[ignore]
fn measure_decode_peak() {
    let path = std::env::var("UG_MEASURE_FILE").expect("UG_MEASURE_FILE");
    let data = std::fs::read(path).unwrap();
    let out = process(&data);
    println!("MEASURE result: {:?}", out.map(|p| (p.width, p.height)));
}
