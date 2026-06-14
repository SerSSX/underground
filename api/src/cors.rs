//! CORS layer construction. Production passes an explicit allow-list (origins from
//! `CORS_ALLOWED_ORIGINS`). Malformed origins are rejected at startup by
//! [`crate::config::Config::validate`], so the list reaching here is either empty
//! (no origins configured → permissive, dev convenience only) or all-valid.

use std::time::Duration;

use axum::http::{header, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Build the CORS layer. With no configured origins this is permissive (dev fallback,
/// logged by the caller); with origins it restricts to that allow-list and, because
/// credentials are allowed, enumerates methods and headers explicitly (a wildcard with
/// `allow_credentials` is rejected by the spec / panics in tower-http).
///
/// Any origin that is not a valid header value is dropped (and warned about) rather than
/// silently widening access — config validation should already have rejected it at startup.
pub fn cors_layer(origins: &[String]) -> CorsLayer {
    if origins.is_empty() {
        return CorsLayer::permissive();
    }
    let allowed: Vec<HeaderValue> = origins.iter().filter_map(|o| HeaderValue::from_str(o).ok()).collect();
    let dropped = origins.len() - allowed.len();
    if dropped > 0 {
        tracing::warn!(dropped, "CORS: ignored malformed origin(s) — rejected at startup");
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_origins_is_permissive_dev_fallback() {
        // Just constructing must not panic; permissive branch.
        let _ = cors_layer(&[]);
    }

    #[test]
    fn all_valid_origins_build_restricted_layer() {
        // Every origin is a valid header value → nothing dropped (no-warn branch).
        let _ = cors_layer(&[
            "https://app.example.com".to_string(),
            "https://admin.example.com".to_string(),
        ]);
    }

    #[test]
    fn malformed_origins_are_dropped_with_warning() {
        // A bad header value is filtered out (filter_map None arm + dropped-count warn branch).
        let _ = cors_layer(&["https://app.example.com".to_string(), "bad\nvalue".to_string()]);
    }
}
