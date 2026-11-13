//! TASK-304 review round 5 (N): a body that trickles in must not hold a waiting-room place for
//! ever, and one account holds at most one place. Own test binary: it sets UPLOAD_BODY_SECS=2 and
//! reads the process-wide waiting room.

use std::io::Cursor;
use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body, Bytes};
use axum::http::{Request, StatusCode};
use axum::Router;
use futures_util::stream::{self, StreamExt};
use image::{DynamicImage, ImageFormat};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::uploads::{upload_queue_free, SLOW_BODY_MESSAGE, UPLOAD_QUEUE};

const BOUNDARY: &str = "XTESTBOUNDARY";

fn head() -> String {
    format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"p.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
    )
}

fn upload_req(token: &str, data: &[u8]) -> Request<Body> {
    let mut body = head().into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/api/uploads")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

/// Headers and the start of a multipart body, then silence: the client never sends the rest.
fn stalled_req(token: &str) -> Request<Body> {
    let first: Result<Bytes, std::io::Error> = Ok(Bytes::from(format!("{}\u{89}PNG", head())));
    let body = stream::iter([first]).chain(stream::pending());
    Request::post("/api/uploads")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from_stream(body))
        .unwrap()
}

async fn register(app: &Router, email: &str) -> String {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": "password1", "name": "U"}).to_string(),
        ))
        .unwrap();
    let v: Value = serde_json::from_slice(
        &to_bytes(app.clone().oneshot(req).await.unwrap().into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    v["token"].as_str().unwrap().to_string()
}

fn png() -> Vec<u8> {
    let mut png = Cursor::new(Vec::new());
    DynamicImage::new_rgb8(16, 16)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    png.into_inner()
}

async fn wait_for_room_free(n: usize) {
    for _ in 0..2000 {
        if upload_queue_free() == n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Both checks in one test: they share the process-wide waiting room.
#[sqlx::test(migrations = "./migrations")]
async fn a_stalled_body_times_out_and_one_user_holds_one_place(pool: PgPool) {
    std::env::set_var("UPLOADS_DIR", "/tmp/underground-test-uploads");
    std::env::set_var("IMAGE_DECODER_BIN", env!("CARGO_BIN_EXE_underground-api"));
    std::env::set_var("UPLOAD_BODY_SECS", "2");
    std::env::remove_var("UPLOAD_QUEUE");
    let app: Router = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    });
    let alice = register(&app, "alice@x.co").await;
    let bob = register(&app, "bob@x.co").await;

    // 1. A stalled body is answered 408 within the deadline, and its place is freed.
    let started = Instant::now();
    let resp = tokio::time::timeout(Duration::from_secs(10), app.clone().oneshot(stalled_req(&alice)))
        .await
        .expect("[SEC] a stalled upload must be cut off, not hold its place for ever")
        .unwrap();
    let took = started.elapsed();
    assert_eq!(resp.status(), StatusCode::REQUEST_TIMEOUT);
    let v: Value = serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(v["error"], SLOW_BODY_MESSAGE);
    assert!(
        took >= Duration::from_secs(2) && took < Duration::from_secs(6),
        "took {took:?}"
    );
    assert_eq!(upload_queue_free(), UPLOAD_QUEUE, "the place is free again");
    let (st, _) = {
        let r = app.clone().oneshot(upload_req(&alice, &png())).await.unwrap();
        (r.status(), ())
    };
    assert_eq!(st, StatusCode::CREATED, "the same user can upload right after");

    // 2. While Alice has an upload in the room, her second one is refused; Bob still gets in.
    let alice_first = tokio::spawn(app.clone().oneshot(stalled_req(&alice)));
    wait_for_room_free(UPLOAD_QUEUE - 1).await;
    assert_eq!(
        upload_queue_free(),
        UPLOAD_QUEUE - 1,
        "Alice's first upload is in the room"
    );
    let second = tokio::time::timeout(Duration::from_secs(5), app.clone().oneshot(upload_req(&alice, &png())))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        second.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] one account holds at most one place in the room"
    );
    let bob_resp = app.clone().oneshot(upload_req(&bob, &png())).await.unwrap();
    assert_eq!(bob_resp.status(), StatusCode::CREATED, "another user still gets in");
    let first = alice_first.await.unwrap().unwrap();
    assert_eq!(first.status(), StatusCode::REQUEST_TIMEOUT);
    wait_for_room_free(UPLOAD_QUEUE).await;
    assert_eq!(upload_queue_free(), UPLOAD_QUEUE);
    std::env::remove_var("UPLOAD_BODY_SECS");
}
