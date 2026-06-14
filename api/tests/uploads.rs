//! Integration tests for image uploads (multipart → local storage → /uploads).

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
    std::env::set_var("UPLOADS_DIR", "/tmp/underground-test-uploads");
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

fn multipart_body(field: &str, filename: &str, content_type: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\
             Content-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn upload_req(token: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut builder =
        Request::post("/api/uploads").header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"));
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::from(body)).unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let body = multipart_body("file", "a.png", "image/png", b"fakepng");
    let (status, _) = send(&app, upload_req(None, body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] upload without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_then_serve(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let body = multipart_body("file", "poster.png", "image/png", b"\x89PNG fake image bytes");
    let (status, v) = send(&app, upload_req(Some(&token), body)).await;
    assert_eq!(status, StatusCode::CREATED);
    let url = v["url"].as_str().unwrap();
    assert!(
        url.contains("/uploads/") && url.ends_with(".png"),
        "absolute url: {url}"
    );

    // The stored file is served back by the same app at /uploads/<name>.
    let path = &url[url.find("/uploads/").unwrap()..];
    let resp = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], b"\x89PNG fake image bytes");
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_accepts_jpeg_and_webp(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    for (content_type, ext) in [("image/jpeg", ".jpg"), ("image/webp", ".webp")] {
        let body = multipart_body("file", "x", content_type, b"image-bytes");
        let (status, v) = send(&app, upload_req(Some(&token), body)).await;
        assert_eq!(status, StatusCode::CREATED, "{content_type} accepted");
        assert!(
            v["url"].as_str().unwrap().ends_with(ext),
            "extension {ext} for {content_type}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_rejects_bad_input(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;

    let pdf = multipart_body("file", "doc.pdf", "application/pdf", b"%PDF-");
    let (status, _) = send(&app, upload_req(Some(&token), pdf)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "non-image rejected");

    let empty = multipart_body("file", "a.png", "image/png", b"");
    let (status, _) = send(&app, upload_req(Some(&token), empty)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "empty file rejected");

    let wrong_field = multipart_body("avatar", "a.png", "image/png", b"data");
    let (status, _) = send(&app, upload_req(Some(&token), wrong_field)).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "missing 'file' field rejected"
    );

    let huge = multipart_body("file", "big.png", "image/png", &vec![0u8; 5 * 1024 * 1024 + 1]);
    let (status, _) = send(&app, upload_req(Some(&token), huge)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "oversize rejected");
}

#[sqlx::test(migrations = "./migrations")]
async fn upload_rejects_malformed_multipart(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;

    // The declared boundary never appears in the body → the first field read fails.
    let req = Request::post("/api/uploads")
        .header("content-type", "multipart/form-data; boundary=SOMETHING_ELSE")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(multipart_body("file", "a.png", "image/png", b"data")))
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "bad multipart rejected");

    // Stream cut before the closing boundary → reading the field's bytes fails.
    let mut truncated = multipart_body("file", "a.png", "image/png", b"data");
    truncated.truncate(truncated.len() - (BOUNDARY.len() + 8));
    let (status, _) = send(&app, upload_req(Some(&token), truncated)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "interrupted upload rejected");
}
