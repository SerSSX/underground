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
/// The UI languages (TASK-303). Stored as given; server-written texts (notifications) exist in
/// Ukrainian and English, and every language other than `uk` gets the English ones.
pub const SUPPORTED_LOCALES: [&str; 10] = ["uk", "en", "es", "pt", "fr", "de", "it", "pl", "tr", "ja"];
const DEFAULT_LOCALE: &str = "uk";

/// Normalize a requested locale to a supported one (unknown → default).
fn normalize_locale(locale: Option<&str>) -> &str {
    match locale {
        Some(l) if SUPPORTED_LOCALES.contains(&l) => l,
        _ => DEFAULT_LOCALE,
    }
}

/// A supported locale exactly as sent, or `None` (absent, empty or unknown).
pub fn supported_locale(locale: Option<&str>) -> Option<&'static str> {
    let l = locale?.trim();
    SUPPORTED_LOCALES.iter().copied().find(|s| *s == l)
}

/// The language chosen at sign-up (TASK-303 item 7): a prefs row with that locale and nothing
/// else, NOT onboarded, so the onboarding screen still shows. Never overwrites an existing row.
pub async fn seed_locale(db: &PgPool, user_id: Uuid, locale: Option<&str>) -> Result<(), sqlx::Error> {
    let Some(locale) = supported_locale(locale) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO user_prefs (user_id, locale, onboarded) VALUES ($1, $2, false) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(locale)
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Serialize)]
pub struct PrefsResp {
    pub categories: Vec<String>,
    pub city: Option<String>,
    /// Auto-derived from the city gazetteer (TASK-017 M2) — drives geo discovery.
    pub country: Option<String>,
    /// ISO code of the home city's country (TASK-303), as sent or from the gazetteer.
    pub country_code: Option<String>,
    /// UI + notification language: one of [`SUPPORTED_LOCALES`].
    pub locale: String,
    /// `false` until the user saves prefs once — the client uses this to show onboarding.
    pub onboarded: bool,
}

#[derive(Deserialize)]
pub struct PutPrefsReq {
    pub categories: Vec<String>,
    pub city: Option<String>,
    /// Absent keeps the stored locale (uk for a new row); unknown → uk.
    pub locale: Option<String>,
    /// The chosen city suggestion's ISO code (TASK-303): picks that country's city of this name.
    pub country_code: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PrefsRow {
    categories: Vec<String>,
    city: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    locale: String,
    onboarded: bool,
}

async fn fetch_prefs(db: &PgPool, user_id: Uuid) -> Result<Option<PrefsRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT categories, city, country, country_code, locale, onboarded FROM user_prefs WHERE user_id = $1",
    )
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
            country_code: r.country_code,
            locale: r.locale,
            onboarded: r.onboarded,
        },
        None => PrefsResp {
            categories: vec![],
            city: None,
            country: None,
            country_code: None,
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
    let city_text = req.city.as_deref().map(crate::geo::strip_invisible);
    let city = city_text.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let cc_in = req.country_code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    if cc_in.is_some_and(|c| !(c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))) {
        return Err(AppError::Validation("country_code must be two letters".into()));
    }
    let cc_in = cc_in.map(str::to_ascii_uppercase);
    let row = crate::cities::resolve(&st.db, city, cc_in.as_deref(), None).await?;
    let country_code = cc_in.or(row.country_code).filter(|_| city.is_some());
    let locale = req.locale.as_deref().map(|l| normalize_locale(Some(l)));
    let saved: (String,) = sqlx::query_as(
        "INSERT INTO user_prefs (user_id, categories, city, country, country_code, locale, onboarded) \
         VALUES ($1, $2, $3, $4, $5, COALESCE($6, 'uk'), true) \
         ON CONFLICT (user_id) DO UPDATE \
            SET categories = $2, city = $3, country = $4, country_code = $5, \
                locale = COALESCE($6, user_prefs.locale), onboarded = true, updated_at = now() \
         RETURNING locale",
    )
    .bind(user.id)
    .bind(&categories)
    .bind(city)
    .bind(&row.country)
    .bind(&country_code)
    .bind(locale)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(PrefsResp {
        categories,
        city: city.map(str::to_string),
        country: row.country,
        country_code,
        locale: saved.0,
        onboarded: true,
    }))
}
