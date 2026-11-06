//! Laufzeitkonfiguration. `Secure` hängt nicht an `BIND_ADDRESS`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieSameSite {
    Lax,
    Strict,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub environment: Environment,
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub same_site: CookieSameSite,
    pub session_ttl_secs: i64,
    pub idle_timeout_secs: i64,
    pub allowed_origins: Vec<String>,
    pub trust_proxy: bool,
    pub login_max_attempts: u32,
    pub login_window_secs: i64,
}

impl AuthConfig {
    pub fn development() -> Self {
        Self {
            environment: Environment::Development,
            cookie_name: "ferrum_session".to_owned(),
            cookie_secure: false,
            same_site: CookieSameSite::Lax,
            session_ttl_secs: 12 * 60 * 60,
            idle_timeout_secs: 30 * 60,
            allowed_origins: vec![
                "http://127.0.0.1:3000".to_owned(),
                "http://localhost:3000".to_owned(),
            ],
            trust_proxy: false,
            login_max_attempts: 5,
            login_window_secs: 15 * 60,
        }
    }

    pub fn from_env() -> Self {
        let environment = match std::env::var("FERRUM_ENV").ok().as_deref() {
            Some("production") => Environment::Production,
            _ => Environment::Development,
        };
        let mut config = Self::development();
        config.environment = environment;
        config.cookie_secure = match std::env::var("FERRUM_COOKIE_SECURE") {
            Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE"),
            Err(_) => environment == Environment::Production,
        };
        if let Ok(origins) = std::env::var("FERRUM_ALLOWED_ORIGINS") {
            config.allowed_origins = origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect();
        } else if environment == Environment::Production {
            config.allowed_origins.clear();
        }
        config.trust_proxy = matches!(
            std::env::var("FERRUM_TRUST_PROXY").ok().as_deref(),
            Some("1" | "true" | "TRUE")
        );
        if environment == Environment::Production && !config.cookie_secure {
            tracing::warn!(
                "FERRUM_COOKIE_SECURE ist in Produktion aus. Session-Cookies werden ohne Secure gesendet."
            );
        }
        config
    }
}
