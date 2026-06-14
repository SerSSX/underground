//! Runtime configuration, loaded once at startup from the environment.

/// The dev fallback secret — must never reach production (see [`Config::validate`]).
pub const DEV_JWT_SECRET: &str = "dev-insecure-secret-change-me";
/// Minimum acceptable JWT secret length in production (256 bits of entropy).
const MIN_JWT_SECRET_LEN: usize = 32;

/// Application configuration. Extended per milestone (billing, escrow).
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    /// Deployment environment (`development` default; `production` enables fail-fast checks).
    pub environment: String,
    /// Allowed CORS origins; empty means permissive (dev only).
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    /// Reads configuration from the process environment, applying dev defaults.
    /// Call after `dotenvy::dotenv()`.
    pub fn from_env() -> Self {
        Self::resolve(
            std::env::var("DATABASE_URL").ok(),
            std::env::var("PORT").ok(),
            std::env::var("JWT_SECRET").ok(),
            std::env::var("ENVIRONMENT").or_else(|_| std::env::var("APP_ENV")).ok(),
            std::env::var("CORS_ALLOWED_ORIGINS").ok(),
        )
    }

    /// Pure resolution of raw values to a `Config` (testable without touching env).
    fn resolve(
        database_url: Option<String>,
        port: Option<String>,
        jwt_secret: Option<String>,
        environment: Option<String>,
        cors_allowed_origins: Option<String>,
    ) -> Self {
        let database_url = database_url.unwrap_or_else(|| "postgres://localhost/underground_dev".to_string());
        let port = port.and_then(|p| p.parse().ok()).unwrap_or(8080);
        let jwt_secret = jwt_secret.unwrap_or_else(|| DEV_JWT_SECRET.to_string());
        let environment = environment.unwrap_or_else(|| "development".to_string());
        let cors_allowed_origins = cors_allowed_origins
            .map(|s| {
                s.split(',')
                    .map(|o| o.trim().to_string())
                    .filter(|o| !o.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Self {
            database_url,
            port,
            jwt_secret,
            environment,
            cors_allowed_origins,
        }
    }

    /// True when running as a production deployment.
    pub fn is_production(&self) -> bool {
        self.environment.eq_ignore_ascii_case("production")
    }

    /// Fail-fast guard: a money-handling API must never run with the dev/weak JWT secret in
    /// production, nor with a malformed CORS origin (which would otherwise be silently dropped,
    /// turning the allow-list into a surprise deny-all). Returns a human-readable reason.
    pub fn validate(&self) -> Result<(), String> {
        if self.is_production() {
            if self.jwt_secret == DEV_JWT_SECRET {
                return Err("JWT_SECRET must be changed from the dev default in production".into());
            }
            if self.jwt_secret.len() < MIN_JWT_SECRET_LEN {
                return Err(format!(
                    "JWT_SECRET must be at least {MIN_JWT_SECRET_LEN} bytes in production"
                ));
            }
        }
        for origin in &self.cors_allowed_origins {
            if axum::http::HeaderValue::from_str(origin).is_err() {
                return Err(format!("CORS_ALLOWED_ORIGINS contains an invalid origin: {origin:?}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_absent() {
        let c = Config::resolve(None, None, None, None, None);
        assert_eq!(c.port, 8080);
        assert!(c.database_url.contains("underground_dev"));
        assert!(!c.jwt_secret.is_empty());
        assert_eq!(c.environment, "development");
        assert!(c.cors_allowed_origins.is_empty());
        assert!(!c.is_production());
    }

    #[test]
    fn parses_overrides() {
        let c = Config::resolve(
            Some("postgres://x/y".to_string()),
            Some("9999".to_string()),
            Some("s3cret".to_string()),
            Some("production".to_string()),
            Some(" https://a.com , https://b.com ,, ".to_string()),
        );
        assert_eq!(c.port, 9999);
        assert_eq!(c.database_url, "postgres://x/y");
        assert_eq!(c.jwt_secret, "s3cret");
        assert!(c.is_production());
        assert_eq!(c.cors_allowed_origins, vec!["https://a.com", "https://b.com"]);
    }

    #[test]
    fn invalid_port_falls_back_to_default() {
        let c = Config::resolve(None, Some("not-a-number".to_string()), None, None, None);
        assert_eq!(c.port, 8080);
    }

    #[test]
    fn production_rejects_dev_and_weak_secrets() {
        // Dev default in production → rejected.
        let dev = Config::resolve(None, None, None, Some("production".to_string()), None);
        assert!(dev.validate().is_err());
        // Non-default but too short → rejected.
        let short = Config::resolve(
            None,
            None,
            Some("short".to_string()),
            Some("PRODUCTION".to_string()),
            None,
        );
        assert!(short.validate().is_err());
        // Strong secret in production → ok.
        let strong = Config::resolve(None, None, Some("x".repeat(32)), Some("production".to_string()), None);
        assert!(strong.validate().is_ok());
        // Outside production the dev default is allowed.
        let dev_ok = Config::resolve(None, None, None, None, None);
        assert!(dev_ok.validate().is_ok());
    }

    #[test]
    fn validate_checks_cors_origins() {
        // Valid origins pass (alongside the production secret checks).
        let ok = Config::resolve(
            None,
            None,
            Some("x".repeat(32)),
            Some("production".to_string()),
            Some("https://a.com,https://b.com".to_string()),
        );
        assert!(ok.validate().is_ok());
        // A malformed origin is rejected regardless of environment (would be silently dropped).
        let bad = Config::resolve(None, None, None, None, Some("https://ok.com,bad\nvalue".to_string()));
        assert!(bad.validate().is_err());
    }

    #[test]
    fn from_env_reads_process_environment() {
        let c = Config::from_env();
        assert!(!c.database_url.is_empty());
        assert!(c.port > 0);
        assert!(!c.jwt_secret.is_empty());
    }
}
