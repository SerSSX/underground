//! TASK-304 review item 3: one picture is decoded at a time, even when a client gives up mid-decode.
//! The request future is dropped then, but the blocking decode runs on; its permit must stay taken
//! until that decode ends. Own test binary: it reads process-wide decode counters.

use std::io::Cursor;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use image::{DynamicImage, ImageFormat, RgbImage};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::uploads::decode_stats;

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

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(img)
        .write_to(&mut out, ImageFormat::Jpeg)
        .unwrap();
    out.into_inner()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_dropped_upload_keeps_its_slot_until_its_decode_ends(pool: PgPool) {
    std::env::set_var("UPLOADS_DIR", "/tmp/underground-test-uploads");
    // TASK-304 review: uploads are decoded by a child process — this test build's API binary.
    std::env::set_var("IMAGE_DECODER_BIN", env!("CARGO_BIN_EXE_underground-api"));
    let app: Router = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    });
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"cancel@x.co","password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let v: Value = serde_json::from_slice(
        &to_bytes(app.clone().oneshot(req).await.unwrap().into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let token = v["token"].as_str().unwrap().to_string();

    // A: a picture that takes a while to decode and re-encode (debug build: seconds).
    let big = jpeg(4000, 3000);
    let a = tokio::spawn(app.clone().oneshot(upload_req(&token, &big)));
    // Wait until A's decode is running, then drop A (the client went away).
    let mut started = false;
    for _ in 0..2000 {
        if decode_stats().0 >= 1 {
            started = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(started, "A's decode never started");
    a.abort();
    let _ = a.await;
    assert_eq!(
        decode_stats().0,
        1,
        "A's decode is still running after the request was dropped"
    );

    // B arrives while A's orphaned decode still runs: it must wait for it, not run beside it.
    let resp = app.clone().oneshot(upload_req(&token, &jpeg(64, 48))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let (in_flight, peak) = decode_stats();
    assert_eq!(
        peak, 1,
        "[SEC] two decodes ran at once: the dropped request freed its slot early"
    );
    assert_eq!(in_flight, 0);
}
