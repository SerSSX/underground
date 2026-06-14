//! User preferences (onboarding): interest categories + home city.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

const MAX_CATEGORIES: usize = 20;
/// Locales the platform localizes server content for (TASK-017 M5).
const SUPPORTED_LOCALES: [&str; 2] = ["uk", "en"];
const DEFAULT_LOCALE: &str = "uk";

/// Normalize a requested locale to a supported one (unknown → default).
fn normalize_locale(locale: Option<&str>) -> &str {
    match locale {
        Some(l) if SUPPORTED_LOCALES.contains(&l) => l,
        _ => DEFAULT_LOCALE,
    }
}

#[derive(Serialize)]
pub struct PrefsResp {
    pub categories: Vec<String>,
    pub city: Option<String>,
    /// Auto-derived from the city gazetteer (TASK-017 M2) — drives geo discovery.
    pub country: Option<String>,
    /// UI + notification language (TASK-017 M5): `uk` | `en`.
    pub locale: String,
    /// `false` until the user saves prefs once — the client uses this to show onboarding.
    pub onboarded: bool,
}

#[derive(Deserialize)]
pub struct PutPrefsReq {
    pub categories: Vec<String>,
    pub city: Option<String>,
    pub locale: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PrefsRow {
    categories: Vec<String>,
    city: Option<String>,
    country: Option<String>,
    locale: String,
}

async fn fetch_prefs(db: &PgPool, user_id: Uuid) -> Result<Option<PrefsRow>, sqlx::Error> {
    sqlx::query_as("SELECT categories, city, country, locale FROM user_prefs WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await
}

/// `GET /api/me/prefs` — saved prefs, or an empty default with `onboarded: false`.
pub async fn get_prefs(user: CurrentUser, State(st): State<AppState>) -> Result<Json<PrefsResp>, AppError> {
    let resp = match fetch_prefs(&st.db, user.id).await? {
        Some(r) => PrefsResp {
            categories: r.categories,
            city: r.city,
            country: r.country,
            locale: r.locale,
            onboarded: true,
        },
        None => PrefsResp {
            categories: vec![],
            city: None,
            country: None,
            locale: DEFAULT_LOCALE.to_string(),
            onboarded: false,
        },
    };
    Ok(Json(resp))
}

/// `PUT /api/me/prefs` — upsert the caller's prefs; country is auto-derived from the city.
pub async fn put_prefs(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<PutPrefsReq>,
) -> Result<Json<PrefsResp>, AppError> {
    let categories: Vec<String> = req
        .categories
        .iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    if categories.len() > MAX_CATEGORIES {
        return Err(AppError::Validation(format!("at most {MAX_CATEGORIES} categories")));
    }
    let city = req.city.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let country = crate::cities::resolve(&st.db, city).await?.country;
    let locale = normalize_locale(req.locale.as_deref());
    sqlx::query(
        "INSERT INTO user_prefs (user_id, categories, city, country, locale) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id) DO UPDATE \
            SET categories = $2, city = $3, country = $4, locale = $5, updated_at = now()",
    )
    .bind(user.id)
    .bind(&categories)
    .bind(city)
    .bind(&country)
    .bind(locale)
    .execute(&st.db)
    .await?;
    Ok(Json(PrefsResp {
        categories,
        city: city.map(str::to_string),
        country,
        locale: locale.to_string(),
        onboarded: true,
    }))
}
