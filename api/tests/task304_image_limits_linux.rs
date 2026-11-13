//! TASK-304 review round 4 (H): both sides of the child's memory limit — Linux only, because macOS
//! does not enforce RLIMIT_AS (CI is Linux). Own test binary: it changes IMAGE_DECODE_MEM_MB.
//!
//! - an ordinary 12 MP phone photo (4032 × 3024 baseline JPEG) is stored with the DEFAULT limit;
//! - the decoy-SOF3 progressive 32 MP JPEG is refused as "too complex" with the default limit and
//!   stored with a 4 GiB limit — so the refusal is the memory limit, not the CPU or wall-clock one.
//!
//! Debug decodes on a CI runner are slow, so this binary raises the CPU and wall-clock limits
//! (IMAGE_DECODE_CPU_SECS / IMAGE_DECODE_WALL_SECS = 300) for itself only; production keeps 20 s / 30 s.
#![cfg(target_os = "linux")]

use std::io::Cursor;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use image::{DynamicImage, ImageFormat, RgbImage};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::uploads::COMPLEX_MESSAGE;

const BOUNDARY: &str = "XTESTBOUNDARY";

fn upload_req(token: &str, data: &[u8]) -> Request<Body> {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"p.jpg\"\r\n\
         Content-Type: image/jpeg\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/api/uploads")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

async fn upload(app: &Router, token: &str, data: &[u8]) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(upload_req(token, data)).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[sqlx::test(migrations = "./migrations")]
async fn the_memory_limit_passes_a_phone_photo_and_stops_only_memory_hogs(pool: PgPool) {
    std::env::set_var("UPLOADS_DIR", "/tmp/underground-test-uploads");
    std::env::set_var("IMAGE_DECODER_BIN", env!("CARGO_BIN_EXE_underground-api"));
    std::env::set_var("IMAGE_DECODE_CPU_SECS", "300");
    std::env::set_var("IMAGE_DECODE_WALL_SECS", "300");
    std::env::remove_var("IMAGE_DECODE_MEM_MB");
    let app: Router = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    });
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"lim@x.co","password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let v: Value = serde_json::from_slice(
        &to_bytes(app.clone().oneshot(req).await.unwrap().into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let token = v["token"].as_str().unwrap().to_string();

    // A 12 MP phone photo, default limit: stored.
    let photo = RgbImage::from_fn(4032, 3024, |x, y| {
        image::Rgb([(x / 16) as u8, (y / 12) as u8, ((x + y) / 28) as u8])
    });
    let mut jpeg = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(photo)
        .write_to(&mut jpeg, ImageFormat::Jpeg)
        .unwrap();
    let jpeg = jpeg.into_inner();
    assert!(
        jpeg.len() <= 5 * 1024 * 1024,
        "fixture fits the 5 MB upload limit ({} B)",
        jpeg.len()
    );
    let (st, v) = upload(&app, &token, &jpeg).await;
    assert_eq!(st, StatusCode::CREATED, "a 12 MP photo must fit the default limit: {v}");

    // The memory hog, default limit: refused as too complex.
    let decoy = include_bytes!("fixtures/prog444_32mp_decoy_sof3.jpg");
    let (st, v) = upload(&app, &token, decoy).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["error"], COMPLEX_MESSAGE, "[SEC] the default memory limit stops it");

    // The same file with a 4 GiB limit: stored — the refusal above was memory, not time.
    std::env::set_var("IMAGE_DECODE_MEM_MB", "4096");
    let (st, v) = upload(&app, &token, decoy).await;
    std::env::remove_var("IMAGE_DECODE_MEM_MB");
    assert_eq!(st, StatusCode::CREATED, "with room to spare it decodes: {v}");
}
