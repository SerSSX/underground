//! Filesystem failure paths for uploads (own process: mutates UPLOADS_DIR env).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;

const BOUNDARY: &str = "XTESTBOUNDARY";

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

async fn register_token(app: &Router) -> String {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"u@x.co","password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    v["token"].as_str().unwrap().to_string()
}

fn upload_req(token: &str) -> Request<Body> {
    // TASK-304: a real PNG (uploads are decoded now; fake bytes would be refused before the write).
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 4)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(&png.into_inner());
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/api/uploads")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_maps_fs_failures_to_500(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;

    // mkdir fails: a path under a non-directory can never be created.
    std::env::set_var("UPLOADS_DIR", "/dev/null/nope");
    let resp = app.clone().oneshot(upload_req(&token)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR, "mkdir failure → 500");

    // write fails: `/` exists but is not writable for the test user.
    std::env::set_var("UPLOADS_DIR", "/");
    let resp = app.clone().oneshot(upload_req(&token)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR, "write failure → 500");
}
