//! Multi-currency event creation/update (TASK-017 M5): currency defaults from the
//! city's country and is validated against the supported set.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

fn build(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
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

async fn register(app: &Router, email: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"M"})),
        ),
    )
    .await;
    v["token"].as_str().unwrap().to_string()
}

fn ev(city: &str, currency: Option<&str>) -> Value {
    let mut v = json!({"title":"Gig","category":"Rock","city":city,"price":"10","tickets_total":5,"cost_type":"paid"});
    if let Some(c) = currency {
        v["currency"] = json!(c);
    }
    v
}

#[sqlx::test(migrations = "./migrations")]
async fn currency_defaults_from_city_and_validates(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "m@x.co").await;

    // City in the Eurozone → EUR; Kyiv (gazetteer is Cyrillic) → UAH; unknown city → UAH fallback.
    let (s, berlin) = send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(ev("Berlin", None))),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(berlin["currency"], "EUR");
    let (_, kyiv) = send(&app, build("POST", "/api/events", Some(&maker), Some(ev("Київ", None)))).await;
    assert_eq!(kyiv["currency"], "UAH");
    let (_, unknown) = send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(ev("Atlantis", None))),
    )
    .await;
    assert_eq!(
        unknown["currency"], "UAH",
        "unresolved country falls back to UAH (launch market)"
    );

    // An explicit, supported currency is honored.
    let (_, usd) = send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(ev("Kyiv", Some("USD")))),
    )
    .await;
    assert_eq!(usd["currency"], "USD");

    // An unsupported currency is rejected.
    let (bad, _) = send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(ev("Kyiv", Some("ZZZ")))),
    )
    .await;
    assert_eq!(bad, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn update_event_currency_validates(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "m@x.co").await;
    let (_, created) = send(&app, build("POST", "/api/events", Some(&maker), Some(ev("Kyiv", None)))).await;
    let id = created["id"].as_str().unwrap().to_string();

    // Owner switches the currency to a supported code.
    let (s, upd) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(json!({"currency":"GBP"})),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(upd["currency"], "GBP");

    // An unsupported currency is rejected.
    let (bad, _) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(json!({"currency":"ZZZ"})),
        ),
    )
    .await;
    assert_eq!(bad, StatusCode::UNPROCESSABLE_ENTITY);
}
