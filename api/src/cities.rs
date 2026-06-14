//! City gazetteer (TASK-017 M2): worldwide name → country + coordinates.
//! Powers geo-aware discovery without any external geocoding service.

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct City {
    pub name: String,
    pub country: String,
    pub lat: f64,
    pub lng: f64,
}

/// Resolved location for an event/user, derived from the gazetteer by city name.
pub struct Resolved {
    pub country: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

/// Look up a city case-insensitively; returns all-`None` when it isn't in the gazetteer.
pub async fn resolve(db: &PgPool, city: Option<&str>) -> Result<Resolved, sqlx::Error> {
    let Some(city) = city.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(Resolved {
            country: None,
            lat: None,
            lng: None,
        });
    };
    let row: Option<City> = sqlx::query_as("SELECT name, country, lat, lng FROM cities WHERE lower(name) = lower($1)")
        .bind(city)
        .fetch_optional(db)
        .await?;
    Ok(match row {
        Some(c) => Resolved {
            country: Some(c.country),
            lat: Some(c.lat),
            lng: Some(c.lng),
        },
        None => Resolved {
            country: None,
            lat: None,
            lng: None,
        },
    })
}

#[derive(Deserialize)]
pub struct CityQuery {
    pub q: Option<String>,
}

/// `GET /api/cities?q=` — autocomplete for the city picker (public).
pub async fn search(State(st): State<AppState>, Query(query): Query<CityQuery>) -> Result<Json<Vec<City>>, AppError> {
    let q = query.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let cities: Vec<City> = sqlx::query_as(
        "SELECT name, country, lat, lng FROM cities \
         WHERE $1::text IS NULL OR name ILIKE $1 || '%' \
         ORDER BY name LIMIT 20",
    )
    .bind(q)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(cities))
}
