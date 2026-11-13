//! TASK-304 review round 4 (F): uploads waiting for the single decode slot hold their bodies in API
//! memory, so only `UPLOAD_QUEUE` (3) are let in at once; one more gets 503 before its body is read,
//! and the API stays responsive. Own test binary: it holds the process-wide decode slot.

use std::io::Cursor;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use image::{DynamicImage, ImageFormat};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::uploads::{hold_decode_slot, upload_queue_free, BUSY_MESSAGE, UPLOAD_QUEUE};

// This binary keeps the production default (no UPLOAD_QUEUE override).

const BOUNDARY: &str = "XTESTBOUNDARY";

fn upload_req(token: &str, data: &[u8]) -> Request<Body> {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"p.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
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

#[sqlx::test(migrations = "./migrations")]
async fn a_full_waiting_room_answers_503_and_the_api_stays_up(pool: PgPool) {
    std::env::set_var("UPLOADS_DIR", "/tmp/underground-test-uploads");
    std::env::set_var("IMAGE_DECODER_BIN", env!("CARGO_BIN_EXE_underground-api"));
    std::env::remove_var("UPLOAD_QUEUE");
    let app: Router = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    });
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"queue@x.co","password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let v: Value = serde_json::from_slice(
        &to_bytes(app.clone().oneshot(req).await.unwrap().into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let token = v["token"].as_str().unwrap().to_string();
    let mut png = Cursor::new(Vec::new());
    DynamicImage::new_rgb8(16, 16)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();

    // A long decode holds the slot; UPLOAD_QUEUE uploads come in and wait for it.
    let slot = hold_decode_slot().await;
    let waiting: Vec<_> = (0..UPLOAD_QUEUE)
        .map(|_| tokio::spawn(app.clone().oneshot(upload_req(&token, &png))))
        .collect();
    for _ in 0..2000 {
        if upload_queue_free() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(upload_queue_free(), 0, "the waiting room is full");

    // One more: refused at once, not queued behind the others.
    let extra = tokio::time::timeout(Duration::from_secs(5), app.clone().oneshot(upload_req(&token, &png)))
        .await
        .expect("[SEC] the extra upload must be refused at once, not wait in memory")
        .unwrap();
    assert_eq!(extra.status(), StatusCode::SERVICE_UNAVAILABLE);
    let v: Value = serde_json::from_slice(&to_bytes(extra.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(v["error"], BUSY_MESSAGE);

    // The API answers other requests meanwhile.
    let health = tokio::time::timeout(
        Duration::from_secs(5),
        app.clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap()),
    )
    .await
    .expect("the API stays responsive")
    .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    // The slot frees: every waiting upload is processed, and the room empties.
    drop(slot);
    for w in waiting {
        let resp = tokio::time::timeout(Duration::from_secs(120), w)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    assert_eq!(upload_queue_free(), UPLOAD_QUEUE);
}
