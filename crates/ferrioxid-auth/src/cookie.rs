//! HttpOnly-Session-Cookie. Der Token steht nicht in der JSON-Antwort.

use crate::config::{AuthConfig, CookieSameSite};

pub fn set_cookie(config: &AuthConfig, token: &str, max_age: i64) -> String {
    format!(
        "{}={token}; HttpOnly; Path=/; SameSite={}; Max-Age={max_age}{}",
        config.cookie_name,
        same_site(config.same_site),
        secure(config)
    )
}

pub fn clear_cookie(config: &AuthConfig) -> String {
    format!(
        "{}=; HttpOnly; Path=/; SameSite={}; Max-Age=0{}",
        config.cookie_name,
        same_site(config.same_site),
        secure(config)
    )
}

pub fn read_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in header.split(';') {
        let mut pair = part.trim().splitn(2, '=');
        if pair.next()?.trim() == name {
            return Some(pair.next()?.trim().to_owned());
        }
    }
    None
}

fn same_site(value: CookieSameSite) -> &'static str {
    match value {
        CookieSameSite::Lax => "Lax",
        CookieSameSite::Strict => "Strict",
    }
}

fn secure(config: &AuthConfig) -> &'static str {
    if config.cookie_secure {
        "; Secure"
    } else {
        ""
    }
}
