//! Benannte Anwendungsrouten auf dem bestehenden Axum-Router.

use crate::AppState;
use axum::routing::MethodRouter;
use axum::Router;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct UrlError {
    pub code: &'static str,
    pub detail: String,
}

impl std::fmt::Display for UrlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}", self.code, self.detail)
    }
}

impl std::error::Error for UrlError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteMap {
    entries: BTreeMap<String, String>,
}

impl RouteMap {
    pub fn path(&self, name: &str, params: &[(&str, &str)]) -> Result<String, UrlError> {
        let template = self.entries.get(name).ok_or_else(|| UrlError {
            code: "url.unknown",
            detail: format!("Die Route `{name}` ist nicht registriert."),
        })?;
        let mut path = template.clone();
        for (key, value) in params {
            let token = format!("{{{key}}}");
            if !path.contains(&token) {
                return Err(UrlError {
                    code: "url.parameter",
                    detail: format!("`{key}` gehört nicht zu `{name}`."),
                });
            }
            let encoded = encode_param(value).map_err(|_| UrlError {
                code: "url.parameter",
                detail: "Ein URL-Parameter enthält unzulässige Zeichen.".to_owned(),
            })?;
            path = path.replace(&token, &encoded);
        }
        if path.contains('{') {
            return Err(UrlError {
                code: "url.parameter",
                detail: format!("Für `{name}` fehlt ein Parameter."),
            });
        }
        Ok(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|(name, path)| (name.as_str(), path.as_str()))
    }

    pub(crate) fn merge(&mut self, other: RouteMap) -> Result<(), UrlError> {
        for (name, path) in &other.entries {
            if self.entries.contains_key(name) {
                return Err(UrlError {
                    code: "url.duplicate",
                    detail: format!("Der Name `{name}` ist bereits vergeben."),
                });
            }
            if self.entries.values().any(|existing| existing == path) {
                return Err(UrlError {
                    code: "url.duplicate",
                    detail: format!("Der Pfad `{path}` ist bereits vergeben."),
                });
            }
        }
        self.entries.extend(other.entries);
        Ok(())
    }
}

#[derive(Clone)]
pub struct UrlSet {
    router: Router<AppState>,
    names: RouteMap,
}

impl UrlSet {
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            names: RouteMap::default(),
        }
    }

    pub fn route(
        mut self,
        name: &str,
        path: &str,
        route: MethodRouter<AppState>,
    ) -> Result<Self, UrlError> {
        self.insert(name, path)?;
        self.router = self.router.route(path, route);
        Ok(self)
    }

    pub fn nest(mut self, prefix: &str, child: UrlSet) -> Result<Self, UrlError> {
        validate_path(prefix)?;
        if reserved_prefix(prefix) {
            return Err(reserved(prefix));
        }
        for (name, path) in child.names.entries {
            let full = join(prefix, &path);
            self.insert(&name, &full)?;
        }
        self.router = self.router.nest(prefix, child.router);
        Ok(self)
    }

    pub fn into_parts(self) -> (Router<AppState>, RouteMap) {
        (self.router, self.names)
    }

    pub fn names(&self) -> &RouteMap {
        &self.names
    }

    fn insert(&mut self, name: &str, path: &str) -> Result<(), UrlError> {
        validate_name(name)?;
        validate_path(path)?;
        if reserved_path(path) || reserved_prefix(path) {
            return Err(reserved(path));
        }
        if self.names.entries.contains_key(name) {
            return Err(UrlError {
                code: "url.duplicate",
                detail: format!("Der Name `{name}` ist bereits vergeben."),
            });
        }
        if self.names.entries.values().any(|existing| existing == path) {
            return Err(UrlError {
                code: "url.duplicate",
                detail: format!("Der Pfad `{path}` ist bereits vergeben."),
            });
        }
        self.names.entries.insert(name.to_owned(), path.to_owned());
        Ok(())
    }
}

impl Default for UrlSet {
    fn default() -> Self {
        Self::new()
    }
}

fn reserved(path: &str) -> UrlError {
    UrlError {
        code: "url.reserved",
        detail: format!("`{path}` ist eine Framework-Route."),
    }
}

fn reserved_path(path: &str) -> bool {
    matches!(
        path,
        "/healthz"
            | "/readyz"
            | "/auth/login"
            | "/auth/logout"
            | "/auth/me"
            | "/auth/csrf"
            | "/auth/change-password"
            | "/auth/password-reset"
            | "/auth/password-reset/confirm"
            | "/auth/password-reset/choose"
            | "/admin"
            | "/admin/login"
    )
}

fn reserved_prefix(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    path == "/admin" || path.starts_with("/admin/") || path == "/auth" || path.starts_with("/auth/")
}

fn validate_name(name: &str) -> Result<(), UrlError> {
    let mut parts = name.split('.');
    let first = parts.next().unwrap_or("");
    let second = parts.next().unwrap_or("");
    if parts.next().is_some() || !ident(first) || !ident(second) {
        return Err(UrlError {
            code: "url.name",
            detail: "Ein Routenname hat die Form bereich.name.".to_owned(),
        });
    }
    Ok(())
}

fn ident(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && value.len() <= 32
}

fn validate_path(path: &str) -> Result<(), UrlError> {
    if !path.starts_with('/')
        || path.contains("//")
        || path.contains('\\')
        || path.contains("..")
        || path.contains("://")
        || path.contains('?')
        || path.contains('#')
        || path
            .chars()
            .any(|char| char.is_control() || char.is_whitespace())
        || path.len() > 256
    {
        return Err(UrlError {
            code: "url.path",
            detail: "Der Pfad muss relativ, absolut im Projekt und ohne Query sein.".to_owned(),
        });
    }
    Ok(())
}

fn join(prefix: &str, path: &str) -> String {
    format!(
        "{}{}",
        prefix.trim_end_matches('/'),
        if path == "/" { "" } else { path }
    )
}

fn encode_param(value: &str) -> Result<String, ()> {
    if value.is_empty()
        || value.len() > 80
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
        || value.contains('?')
        || value.contains('#')
        || value.chars().any(|char| char.is_control())
    {
        return Err(());
    }
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::UrlSet;
    use axum::routing::get;
    use axum::Router;

    async fn ok() -> &'static str {
        "ok"
    }

    #[test]
    fn names_nests_and_reserved_paths_are_rejected() {
        let set = UrlSet::new()
            .route("shop.list", "/shop", get(ok))
            .unwrap()
            .nest(
                "/catalog",
                UrlSet::new()
                    .route("shop.detail", "/{id}", get(ok))
                    .unwrap(),
            )
            .unwrap();
        let (_router, names) = set.into_parts();
        assert_eq!(
            names.path("shop.detail", &[("id", "sku-1")]).unwrap(),
            "/catalog/sku-1"
        );
        assert!(names.path("missing.route", &[]).is_err());
        assert!(names.path("shop.detail", &[("id", "../x")]).is_err());
        assert_eq!(
            names.path("shop.detail", &[("id", "a b")]).unwrap(),
            "/catalog/a%20b"
        );
        assert!(names.path("shop.detail", &[("id", "a/b")]).is_err());
        let nested = UrlSet::new()
            .nest(
                "/shop",
                UrlSet::new()
                    .nest(
                        "/items",
                        UrlSet::new().route("shop.item", "/{id}", get(ok)).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            nested
                .into_parts()
                .1
                .path("shop.item", &[("id", "a b")])
                .unwrap(),
            "/shop/items/a%20b"
        );
        let duplicate_name = UrlSet::new()
            .route("shop.detail", "/shop", get(ok))
            .unwrap()
            .nest(
                "/catalog",
                UrlSet::new()
                    .route("shop.detail", "/{id}", get(ok))
                    .unwrap(),
            );
        assert!(matches!(duplicate_name, Err(error) if error.code == "url.duplicate"));
        let reserved = UrlSet::new().route("auth.login", "/auth/login", get(ok));
        assert!(matches!(reserved, Err(error) if error.code == "url.reserved"));
        let duplicate = UrlSet::new()
            .route("shop.list", "/shop", get(ok))
            .unwrap()
            .route("shop.other", "/shop", get(ok));
        assert!(matches!(duplicate, Err(error) if error.code == "url.duplicate"));
        let _typed: Router<crate::AppState> = UrlSet::new().into_parts().0;
    }
}
