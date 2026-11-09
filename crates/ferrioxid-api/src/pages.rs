//! Serverseitige HTML-Seiten mit Tera. Die Syntax folgt dem Jinja-Modell.

use axum::extract::Request;
use axum::response::{Html, IntoResponse, Response};
use axum::{routing::get, Router};
use ferrioxid_migrations::ProjectConfig;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tera::{Context, Tera};

#[derive(Debug)]
pub struct PageError(String);

impl PageError {
    pub(crate) fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl std::fmt::Display for PageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for PageError {}

pub struct Site {
    tera: Tera,
}

impl Site {
    pub fn open(directory: &Path) -> Result<Self, PageError> {
        let index = directory.join("index.html");
        if !index.is_file() {
            return Err(PageError(format!(
                "Die Startseite fehlt: {}",
                index.display()
            )));
        }
        let pattern = directory
            .join("**")
            .join("*.html")
            .to_string_lossy()
            .replace('\\', "/");
        let tera = Tera::new(&pattern).map_err(|error| {
            PageError(format!(
                "Die Vorlagen konnten nicht geladen werden: {error}"
            ))
        })?;
        Ok(Self { tera })
    }

    pub fn from_named(
        files: &[(String, String)],
        routes: &crate::urls::RouteMap,
    ) -> Result<Self, PageError> {
        let mut tera = Tera::default();
        tera.add_raw_templates(
            files
                .iter()
                .map(|(name, body)| (name.as_str(), body.as_str())),
        )
        .map_err(|error| PageError(format!("Invalid template catalog: {error}")))?;
        let mut site = Self { tera };
        site.set_routes(routes.clone());
        Ok(site)
    }

    pub fn set_routes(&mut self, routes: crate::urls::RouteMap) {
        let routes = Arc::new(routes);
        self.tera.register_function("url", UrlFunction { routes });
    }

    pub fn render(&self, name: &str, context: &Context) -> Result<String, PageError> {
        self.tera.render(name, context).map_err(|error| {
            PageError(format!(
                "Vorlage `{name}` konnte nicht gerendert werden: {}",
                error_chain(&error)
            ))
        })
    }
}

pub fn home_context(settings: &ProjectConfig) -> Context {
    home_context_for(settings, &negotiate_locale(None, None, settings))
}

pub fn home_context_for(settings: &ProjectConfig, locale: &str) -> Context {
    let mut context = Context::new();
    context.insert("locale", locale);
    context.insert(
        "project_name",
        settings
            .name
            .as_deref()
            .unwrap_or(settings.app_name.as_str()),
    );
    context.insert("app_name", &settings.app_name);
    context.insert("site_title", &settings.site_title);
    context.insert("primary_color", &settings.primary_color);
    context.insert("admin", &settings.admin_enabled);
    context.insert("debug", &settings.debug);
    context.insert("running", message(locale, "running"));
    context.insert("admin_link", message(locale, "admin_link"));
    context.insert("auth_hint", message(locale, "auth_hint"));
    context.insert("api_hint", message(locale, "api_hint"));
    context.insert("setup_hint", message(locale, "setup_hint"));
    context.insert("status_label", message(locale, "status_label"));
    context.insert("not_found", message(locale, "not_found"));
    context.insert("server_error", message(locale, "server_error"));
    context.insert("welcome", message(locale, "welcome"));
    context
}

/// Reihenfolge: gültiger `lang`-Parameter, unterstütztes `Accept-Language`, Projektstandard, Fallback.
/// Der Wert wird nie als Dateipfad verwendet.
pub fn negotiate_locale(
    explicit: Option<&str>,
    accept_language: Option<&str>,
    settings: &ProjectConfig,
) -> String {
    if let Some(locale) = explicit.and_then(supported_locale) {
        return locale;
    }
    if let Some(locale) = accept_language.and_then(accept_locale) {
        return locale;
    }
    supported_locale(&settings.default_locale)
        .or_else(|| supported_locale(&settings.fallback_locale))
        .unwrap_or_else(|| "en".to_owned())
}

/// Hängt Startseite und Fehlerseiten an den Router, wenn Vorlagen aktiv sind.
pub fn mount_pages(router: Router, settings: &ProjectConfig) -> Result<Router, PageError> {
    mount_pages_with(router, settings, &crate::urls::RouteMap::default())
}

/// Wie `mount_pages`, und die Projektvorlagen erhalten dieselbe `url`-Funktion wie App-Vorlagen.
pub fn mount_pages_with(
    router: Router,
    settings: &ProjectConfig,
    routes: &crate::urls::RouteMap,
) -> Result<Router, PageError> {
    if !settings.templates_enabled {
        return Ok(router);
    }
    let mut opened = Site::open(&settings.templates_directory())?;
    opened.set_routes(routes.clone());
    let site = Arc::new(opened);
    let settings = settings.clone();
    let index_site = site.clone();
    let index_settings = settings.clone();
    let method_site = site.clone();
    let method_settings = settings.clone();
    let missing_site = site;
    let missing_settings = settings;
    Ok(router
        .route(
            "/",
            get(move |request: Request| {
                let site = index_site.clone();
                let settings = index_settings.clone();
                async move {
                    let context = context_for_request(&settings, &request);
                    render_named(&site, "index.html", &context, axum::http::StatusCode::OK)
                }
            }),
        )
        .fallback(move |request: Request| {
            let site = missing_site.clone();
            let settings = missing_settings.clone();
            async move {
                let context = context_for_request(&settings, &request);
                render_named(
                    &site,
                    "errors/404.html",
                    &context,
                    axum::http::StatusCode::NOT_FOUND,
                )
            }
        })
        .method_not_allowed_fallback({
            let site = method_site;
            let settings = method_settings;
            move |request: Request| {
                let site = site.clone();
                let settings = settings.clone();
                async move {
                    let context = context_for_request(&settings, &request);
                    render_named(
                        &site,
                        "errors/405.html",
                        &context,
                        axum::http::StatusCode::METHOD_NOT_ALLOWED,
                    )
                }
            }
        }))
}

fn render_named(
    site: &Site,
    name: &str,
    context: &Context,
    status: axum::http::StatusCode,
) -> Response {
    match site.render(name, context) {
        Ok(body) => (status, Html(body)).into_response(),
        Err(_) if name.starts_with("errors/") => builtin_status(status, context),
        Err(_) => match site.render("errors/500.html", context) {
            Ok(body) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Html(body)).into_response(),
            Err(_) => builtin_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR, context),
        },
    }
}

pub fn render_status_template(
    site: &Site,
    name: &str,
    locale: &str,
    title: &str,
    detail: &str,
    status: u16,
) -> Option<String> {
    let mut context = Context::new();
    context.insert("locale", locale);
    context.insert("title", title);
    context.insert("detail", detail);
    context.insert("status", &status);
    site.render(name, &context).ok()
}

fn builtin_status(status: axum::http::StatusCode, context: &Context) -> Response {
    let locale = context
        .get("locale")
        .and_then(|value| value.as_str())
        .unwrap_or("de");
    let title = message(locale, status_key(status));
    let body = format!(
        "<!DOCTYPE html><html lang=\"{locale}\"><head><meta charset=\"utf-8\"><title>{}</title></head><body><h1>{}</h1><p>{}</p></body></html>",
        escape(title),
        escape(title),
        escape(title)
    );
    (status, Html(body)).into_response()
}

fn status_key(status: axum::http::StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        409 => "conflict",
        422 => "validation",
        _ => "server_error",
    }
}

fn escape(value: &str) -> String {
    let mut output = String::new();
    for char in value.chars() {
        match char {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(char),
        }
    }
    output
}

fn context_for_request(settings: &ProjectConfig, request: &Request) -> Context {
    let explicit = request.uri().query().and_then(query_lang);
    let accept = request
        .headers()
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok());
    home_context_for(settings, &negotiate_locale(explicit, accept, settings))
}

fn query_lang(query: &str) -> Option<&str> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "lang").then_some(value)
    })
}

fn supported_locale(value: &str) -> Option<String> {
    let token = value.trim().to_ascii_lowercase();
    let primary = token.split(['-', '_']).next().unwrap_or("");
    match primary {
        "de" => Some("de".to_owned()),
        "en" => Some("en".to_owned()),
        _ => None,
    }
}

fn accept_locale(header: &str) -> Option<String> {
    let mut ranked = Vec::new();
    for part in header.split(',') {
        let mut pieces = part.split(';');
        let tag = pieces.next()?.trim();
        let mut quality = 1.0_f32;
        for parameter in pieces {
            if let Some(value) = parameter.trim().strip_prefix("q=") {
                quality = value.parse().unwrap_or(0.0);
            }
        }
        if let Some(locale) = supported_locale(tag) {
            ranked.push((locale, quality));
        }
    }
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.into_iter().next().map(|(locale, _)| locale)
}

pub fn message(locale: &str, key: &str) -> &'static str {
    let locale = if locale == "de" { "de" } else { "en" };
    match (locale, key) {
        ("de", "running") => "Ferrioxid läuft. Die lokale Datenbank ist SQLite.",
        ("de", "admin_link") => "Admin öffnen",
        ("de", "auth_hint") => "Benutzer legt die Kommandozeile an. Es gibt kein Standardpasswort.",
        ("de", "api_hint") => {
            "Die Beispiel-API liegt unter /api/v1/products. Der Zustand liegt unter /healthz."
        }
        ("de", "setup_hint") => "Vor dem ersten Schreibzugriff ferrioxid migrate up ausführen.",
        ("de", "status_label") => "Datenbank",
        ("de", "not_found") => "Diese Seite existiert nicht.",
        ("de", "server_error") => "Die Seite konnte nicht erzeugt werden.",
        ("de", "forbidden") => "Zugriff verweigert.",
        ("de", "unauthorized") => "Anmeldung erforderlich.",
        ("de", "validation") => "Die Eingabe ist ungültig.",
        ("de", "bad_request") => "Die Anfrage ist ungültig.",
        ("de", "method_not_allowed") => "Diese Methode ist hier nicht erlaubt.",
        ("de", "conflict") => "Der Stand hat sich geändert.",
        ("de", "welcome") => "Willkommen",
        ("en", "running") => "Ferrioxid is running. The local database is SQLite.",
        ("en", "admin_link") => "Open admin",
        ("en", "auth_hint") => "Create users with the command line. There is no default password.",
        ("en", "api_hint") => "The sample API is /api/v1/products. Health is /healthz.",
        ("en", "setup_hint") => "Run ferrioxid migrate up before the first write.",
        ("en", "status_label") => "Database",
        ("en", "not_found") => "This page does not exist.",
        ("en", "server_error") => "The page could not be rendered.",
        ("en", "forbidden") => "Access denied.",
        ("en", "unauthorized") => "Authentication required.",
        ("en", "validation") => "The input is invalid.",
        ("en", "bad_request") => "The request is invalid.",
        ("en", "method_not_allowed") => "This method is not allowed.",
        ("en", "conflict") => "The record changed.",
        ("en", "welcome") => "Welcome",
        _ => "missing message",
    }
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut detail = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        detail.push_str(": ");
        detail.push_str(&inner.to_string());
        source = inner.source();
    }
    detail
}

struct UrlFunction {
    routes: Arc<crate::urls::RouteMap>,
}

impl tera::Function for UrlFunction {
    fn call(&self, args: &HashMap<String, serde_json::Value>) -> tera::Result<serde_json::Value> {
        let name = args
            .get("name")
            .and_then(|value| value.as_str())
            .ok_or_else(|| tera::Error::msg("url braucht name."))?;
        let mut params = Vec::new();
        for (key, value) in args {
            if key == "name" {
                continue;
            }
            let text = match value {
                serde_json::Value::String(text) => text.clone(),
                serde_json::Value::Number(number) => number.to_string(),
                serde_json::Value::Bool(flag) => flag.to_string(),
                _ => {
                    return Err(tera::Error::msg(
                        "Ein URL-Parameter muss Text, Zahl oder Wahrheitswert sein.",
                    ))
                }
            };
            params.push((key.clone(), text));
        }
        let pairs: Vec<(&str, &str)> = params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        match self.routes.path(name, &pairs) {
            Ok(path) => Ok(serde_json::Value::String(path)),
            Err(error) => Err(tera::Error::msg(error.detail)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{home_context, message, negotiate_locale, render_named, Site};
    use ferrioxid_migrations::discover_project;
    use std::fs;

    #[test]
    fn messages_fall_back_to_english() {
        assert!(message("de", "running").contains("SQLite"));
        assert!(message("fr", "running").contains("SQLite"));
        assert!(message("en", "welcome").contains("Welcome"));
        assert_eq!(message("en", "unknown"), "missing message");
        let settings = discover_project(std::path::Path::new(".")).ok();
        if let Some(settings) = settings {
            assert_eq!(negotiate_locale(Some("en"), Some("de"), &settings), "en");
            assert_eq!(
                negotiate_locale(Some("../de"), Some("en-US,de;q=0.2"), &settings),
                "en"
            );
            assert_eq!(negotiate_locale(Some("fr"), None, &settings), "de");
        }
    }

    #[test]
    fn named_url_renders_without_listing_the_map() {
        let routes = crate::urls::UrlSet::new()
            .route(
                "shop.detail",
                "/shop/items/{id}",
                axum::routing::get(|| async { "ok" }),
            )
            .unwrap()
            .into_parts()
            .1;
        let site = Site::from_named(
            &[(
                "shop/item.html".to_owned(),
                "{{ url(name=\"shop.detail\", id=id) }}".to_owned(),
            )],
            &routes,
        )
        .unwrap();
        let mut context = tera::Context::new();
        context.insert("id", "sku-1");
        assert_eq!(
            site.render("shop/item.html", &context).unwrap(),
            "&#x2F;shop&#x2F;items&#x2F;sku-1"
        );
        let missing = Site::from_named(
            &[(
                "shop/item.html".to_owned(),
                "{{ url(name=\"missing\") }}".to_owned(),
            )],
            &routes,
        )
        .unwrap();
        let error = missing
            .render("shop/item.html", &tera::Context::new())
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing"), "{error}");
        assert!(!error.contains("shop.detail"), "{error}");
        let routes = crate::urls::UrlSet::new()
            .nest(
                "/shop",
                crate::urls::UrlSet::new()
                    .route(
                        "shop.detail",
                        "/{id}",
                        axum::routing::get(|| async { "ok" }),
                    )
                    .unwrap(),
            )
            .unwrap()
            .into_parts()
            .1;
        let home = Site::from_named(
            &[(
                "index.html".to_owned(),
                "{{ url(name=\"shop.detail\", id=sku) }}".to_owned(),
            )],
            &routes,
        )
        .unwrap();
        let mut home_context = tera::Context::new();
        home_context.insert("sku", "a b");
        assert_eq!(
            home.render("index.html", &home_context).unwrap(),
            "&#x2F;shop&#x2F;a%20b"
        );
    }

    #[tokio::test]
    async fn error_pages_use_templates_or_a_safe_fallback() {
        use http_body_util::BodyExt;
        let mut context = tera::Context::new();
        context.insert("locale", "en");
        let missing = Site::from_named(
            &[("index.html".to_owned(), "home".to_owned())],
            &crate::urls::RouteMap::default(),
        )
        .unwrap();
        let response = render_named(
            &missing,
            "errors/404.html",
            &context,
            axum::http::StatusCode::NOT_FOUND,
        );
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains("does not exist"), "{body}");
        assert!(!body.contains("Vorlage"), "{body}");
        assert!(!body.contains("stack"), "{body}");
        let custom = Site::from_named(
            &[
                (
                    "errors/404.html".to_owned(),
                    "<p>custom-missing</p>".to_owned(),
                ),
                (
                    "errors/500.html".to_owned(),
                    "<p>custom-down</p>".to_owned(),
                ),
                (
                    "index.html".to_owned(),
                    "{{ url(name=\"missing.route\") }}".to_owned(),
                ),
            ],
            &crate::urls::RouteMap::default(),
        )
        .unwrap();
        let found = render_named(
            &custom,
            "errors/404.html",
            &context,
            axum::http::StatusCode::NOT_FOUND,
        );
        let found_body = String::from_utf8(
            found
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(found_body.contains("custom-missing"));
        let broken = render_named(&custom, "index.html", &context, axum::http::StatusCode::OK);
        assert_eq!(
            broken.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let broken_body = String::from_utf8(
            broken
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(broken_body.contains("custom-down"), "{broken_body}");
        assert!(!broken_body.contains("missing.route"), "{broken_body}");
        assert!(!broken_body.to_ascii_lowercase().contains("stack"));
    }

    #[test]
    fn index_template_renders_the_project_name() {
        let directory =
            std::env::temp_dir().join(format!("ferrioxid-pages-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(directory.join("templates/errors")).unwrap();
        fs::write(
            directory.join("ferrioxid.toml"),
            "migrations_dir = \"migrations\"\n[project]\nname = \"Demo\"\n[templates]\nenabled = true\n[branding]\napp_name = \"Demo\"\n",
        )
        .unwrap();
        fs::create_dir_all(directory.join("migrations")).unwrap();
        fs::write(
            directory.join("templates/base.html"),
            "<html><body>{% block content %}{% endblock %}</body></html>\n",
        )
        .unwrap();
        fs::write(
            directory.join("templates/index.html"),
            "{% extends \"base.html\" %}{% block content %}<h1>{{ project_name }}</h1><p>{{ running }}</p>{% if admin %}<a href=\"/admin\">{{ admin_link }}</a>{% endif %}{% endblock %}\n",
        )
        .unwrap();
        fs::write(
            directory.join("templates/errors/404.html"),
            "{% extends \"base.html\" %}{% block content %}<p>{{ not_found }}</p>{% endblock %}\n",
        )
        .unwrap();
        fs::write(
            directory.join("templates/errors/500.html"),
            "{% extends \"base.html\" %}{% block content %}<p>500</p>{% endblock %}\n",
        )
        .unwrap();
        let settings = discover_project(&directory).expect("settings");
        let html = Site::open(directory.join("templates").as_path())
            .expect("site")
            .render("index.html", &home_context(&settings))
            .expect("html");
        assert!(html.contains("Demo"));
        assert!(html.contains("SQLite"));
        assert!(!html.contains("/admin"));
        let _ = fs::remove_dir_all(&directory);
    }
}
