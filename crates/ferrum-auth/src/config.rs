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
    /// Stabiler HMAC-Schlüssel für sitzungsgebundene CSRF-Tokens. Ein Neustart darf ihn nicht neu würfeln.
    pub csrf_secret: [u8; 32],
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
            csrf_secret: *b"ferrum-dev-csrf-secret-32-bytes!",
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
        config.csrf_secret = load_csrf_secret();
        config.trust_proxy = matches!(
            std::env::var("FERRUM_TRUST_PROXY").ok().as_deref(),
            Some("1" | "true" | "TRUE")
        );
        if let Some(hours) = env_range("FERRUM_SESSION_HOURS", 1, 168) {
            config.session_ttl_secs = i64::from(hours) * 3600;
        }
        if let Some(minutes) = env_range("FERRUM_IDLE_MINUTES", 1, 1440) {
            config.idle_timeout_secs = i64::from(minutes) * 60;
        }
        if environment == Environment::Production && !config.cookie_secure {
            tracing::warn!(
                "FERRUM_COOKIE_SECURE ist in Produktion aus. Session-Cookies werden ohne Secure gesendet."
            );
        }
        config
    }

    /// Dateiwerte gelten nur, wenn die Umgebung sie nicht bereits gesetzt hat.
    pub fn apply_project_session(&mut self, session_hours: u32, idle_minutes: u32) {
        if std::env::var("FERRUM_SESSION_HOURS").is_err() {
            self.session_ttl_secs = i64::from(session_hours) * 3600;
        }
        if std::env::var("FERRUM_IDLE_MINUTES").is_err() {
            self.idle_timeout_secs = i64::from(idle_minutes) * 60;
        }
    }
}

fn env_range(name: &str, min: u32, max: u32) -> Option<u32> {
    let value = std::env::var(name).ok()?;
    match value.parse::<u32>() {
        Ok(number) if (min..=max).contains(&number) => Some(number),
        _ => {
            tracing::warn!("{name} liegt außerhalb von {min} bis {max} und wird ignoriert.");
            None
        }
    }
}

fn load_csrf_secret() -> [u8; 32] {
    if let Ok(value) = std::env::var("FERRUM_CSRF_SECRET") {
        if let Some(secret) = decode_hex_32(&value) {
            return secret;
        }
        tracing::warn!("FERRUM_CSRF_SECRET muss 64 Hex-Zeichen sein. Ferrum verwendet die gespeicherte Schlüsseldatei.");
    }
    let path = std::path::PathBuf::from(".ferrum/csrf.key");
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut secret = [0u8; 32];
            secret.copy_from_slice(&bytes);
            return secret;
        }
    }
    let mut secret = [0u8; 32];
    getrandom::getrandom(&mut secret).expect("csrf secret");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&path, secret).is_err() {
        tracing::warn!("CSRF-Schlüsseldatei konnte nicht geschrieben werden. Der Prozessschlüssel gilt nur bis zum Neustart.");
    }
    secret
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    let value = value.trim();
    if value.len() != 64 || !value.chars().all(|char| char.is_ascii_hexdigit()) {
        return None;
    }
    let mut secret = [0u8; 32];
    for (index, byte) in secret.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(secret)
}
