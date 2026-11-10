//! In-process HTTP client, isolated databases, and fixture loading.
//!
//! The client calls the Axum router directly. It does not open a socket.
//! PostgreSQL cleanup drops only tables recorded on a database whose name
//! contains `ferrioxid_test`. It never issues `DROP DATABASE`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::Router;
use ferrioxid_db::QueryCounter;
use ferrioxid_migrations::{
    load_fixture, FixtureFile, LoadReport, LoadRequest, MigrationError, SchemaSnapshot,
};
use http_body_util::BodyExt;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde_json::Value;
use tower::ServiceExt;

/// Cookie jar and oneshot client for one router.
#[derive(Clone)]
pub struct TestClient {
    router: Router,
    cookies: BTreeMap<String, String>,
}

impl TestClient {
    pub fn new(router: Router) -> Self {
        Self {
            router,
            cookies: BTreeMap::new(),
        }
    }

    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(String::as_str)
    }

    pub fn set_cookie(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.cookies.insert(name.into(), value.into());
    }

    pub fn clear_cookies(&mut self) {
        self.cookies.clear();
    }

    pub async fn get(&mut self, path: &str) -> Result<TestResponse, String> {
        self.request(Method::GET, path, None, None).await
    }

    pub async fn delete(&mut self, path: &str) -> Result<TestResponse, String> {
        self.request(Method::DELETE, path, None, None).await
    }

    pub async fn post_json(&mut self, path: &str, body: &Value) -> Result<TestResponse, String> {
        self.send_json(Method::POST, path, body).await
    }

    pub async fn put_json(&mut self, path: &str, body: &Value) -> Result<TestResponse, String> {
        self.send_json(Method::PUT, path, body).await
    }

    pub async fn patch_json(&mut self, path: &str, body: &Value) -> Result<TestResponse, String> {
        self.send_json(Method::PATCH, path, body).await
    }

    pub async fn post_form(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        self.request(
            Method::POST,
            path,
            Some("application/x-www-form-urlencoded"),
            Some(form_body(fields)),
        )
        .await
    }

    pub async fn put_form(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        self.request(
            Method::PUT,
            path,
            Some("application/x-www-form-urlencoded"),
            Some(form_body(fields)),
        )
        .await
    }

    pub async fn patch_form(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        self.request(
            Method::PATCH,
            path,
            Some("application/x-www-form-urlencoded"),
            Some(form_body(fields)),
        )
        .await
    }

    /// Reads `csrf_token` from an HTML form and posts the supplied fields with it.
    pub async fn post_form_with_csrf(
        &mut self,
        path: &str,
        html: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        let token = csrf_token_from_html(html)?;
        let mut owned = fields.to_vec();
        owned.push(("csrf_token", token.as_str()));
        self.post_form(path, &owned).await
    }

    async fn send_json(
        &mut self,
        method: Method,
        path: &str,
        body: &Value,
    ) -> Result<TestResponse, String> {
        self.request(
            method,
            path,
            Some("application/json"),
            Some(body.to_string()),
        )
        .await
    }

    async fn request(
        &mut self,
        method: Method,
        path: &str,
        content_type: Option<&str>,
        body: Option<String>,
    ) -> Result<TestResponse, String> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        if !self.cookies.is_empty() {
            builder = builder.header(header::COOKIE, cookie_header(&self.cookies));
        }
        let request = builder
            .body(Body::from(body.unwrap_or_default()))
            .map_err(|error| error.to_string())?;
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        store_cookies(&mut self.cookies, response.headers());
        let bytes = response
            .into_body()
            .collect()
            .await
            .map_err(|error| error.to_string())?
            .to_bytes();
        Ok(TestResponse {
            status,
            body: bytes.to_vec(),
        })
    }
}

pub struct TestResponse {
    status: StatusCode,
    body: Vec<u8>,
}

impl TestResponse {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn assert_status(self, status: StatusCode) -> Result<Self, String> {
        if self.status == status {
            Ok(self)
        } else {
            Err(format!(
                "status {} != {status}; body: {}",
                self.status,
                self.text().unwrap_or("<body is not utf-8>")
            ))
        }
    }

    pub fn text(&self) -> Result<&str, String> {
        std::str::from_utf8(&self.body).map_err(|error| error.to_string())
    }

    pub fn json(&self) -> Result<Value, String> {
        serde_json::from_slice(&self.body).map_err(|error| error.to_string())
    }

    pub fn assert_json(self, expected: &Value) -> Result<Self, String> {
        let actual = self.json()?;
        if &actual == expected {
            Ok(self)
        } else {
            Err(format!("json {actual} != {expected}"))
        }
    }
}

/// Reads the first `csrf_token` value from an HTML form.
pub fn csrf_token_from_html(html: &str) -> Result<String, String> {
    let needle = "name=\"csrf_token\"";
    let Some(start) = html.find(needle) else {
        return Err("The page has no csrf_token field.".into());
    };
    let rest = &html[start + needle.len()..];
    let value_key = "value=\"";
    let Some(value_at) = rest.find(value_key) else {
        return Err("The csrf_token field has no value.".into());
    };
    let rest = &rest[value_at + value_key.len()..];
    let end = rest
        .find('"')
        .ok_or_else(|| "The csrf_token value is not quoted.".to_owned())?;
    Ok(rest[..end].to_owned())
}

/// A database created for one test. Cleanup never targets an unrelated server database.
pub struct TestDatabase {
    url: String,
    kind: DatabaseKind,
    directory: Option<PathBuf>,
    tables: Vec<String>,
}

enum DatabaseKind {
    Sqlite,
    Postgres,
}

impl TestDatabase {
    pub async fn sqlite() -> Result<Self, String> {
        let directory =
            std::env::temp_dir().join(format!("ferrioxid-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let file = directory.join("test.sqlite");
        std::fs::File::create(&file).map_err(|error| error.to_string())?;
        let url = format!(
            "sqlite://{}?mode=rwc",
            file.to_string_lossy().replace('\\', "/")
        );
        Ok(Self {
            url,
            kind: DatabaseKind::Sqlite,
            directory: Some(directory),
            tables: Vec::new(),
        })
    }

    /// Accepts only a PostgreSQL URL whose database name contains `ferrioxid_test`.
    pub fn postgres(url: &str) -> Result<Self, String> {
        let name = postgres_database_name(url)?;
        if !name.contains("ferrioxid_test") {
            return Err(
                "Refusing a PostgreSQL test connection whose database name does not contain ferrioxid_test."
                    .into(),
            );
        }
        Ok(Self {
            url: url.to_owned(),
            kind: DatabaseKind::Postgres,
            directory: None,
            tables: Vec::new(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn connect(&self) -> Result<DatabaseConnection, String> {
        ferrioxid_db::connect(&self.url)
            .await
            .map_err(|error| error.to_string())
    }

    pub fn record_table(&mut self, name: &str) -> Result<(), String> {
        if !is_identifier(name) {
            return Err(format!("Refusing to record table `{name}`."));
        }
        if !self.tables.iter().any(|existing| existing == name) {
            self.tables.push(name.to_owned());
        }
        Ok(())
    }

    /// Drops recorded tables. SQLite cleanup also deletes the temporary directory.
    pub async fn cleanup(mut self) -> Result<(), String> {
        if matches!(self.kind, DatabaseKind::Postgres) && self.directory.is_some() {
            return Err("A PostgreSQL test database has no local directory to delete.".into());
        }
        match self.kind {
            DatabaseKind::Postgres => {
                if !self.tables.is_empty() {
                    let db = self.connect().await?;
                    for table in &self.tables {
                        let sql = format!("DROP TABLE IF EXISTS \"{table}\"");
                        db.execute(Statement::from_string(db.get_database_backend(), sql))
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                }
            }
            DatabaseKind::Sqlite => {
                if let Some(directory) = self.directory.take() {
                    remove_owned_directory(&directory)?;
                }
            }
        }
        Ok(())
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Some(directory) = self.directory.take() {
            let _ = remove_owned_directory(&directory);
        }
    }
}

pub async fn apply_fixture(
    db: &DatabaseConnection,
    schema: &SchemaSnapshot,
    file: &FixtureFile,
    request: &LoadRequest,
) -> Result<LoadReport, MigrationError> {
    load_fixture(db, schema, file, request).await
}

pub fn install_query_counter() -> QueryCounter {
    QueryCounter::install()
}

pub fn assert_query_budget(actual: usize, limit: usize) -> Result<(), String> {
    if actual <= limit {
        Ok(())
    } else {
        Err(format!(
            "The request issued {actual} statements, above the budget of {limit}."
        ))
    }
}

fn form_body(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{}={}", encode(name), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn cookie_header(cookies: &BTreeMap<String, String>) -> String {
    cookies
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn store_cookies(cookies: &mut BTreeMap<String, String>, headers: &HeaderMap) {
    for value in headers.get_all(header::SET_COOKIE) {
        let Ok(text) = value.to_str() else {
            continue;
        };
        let Some((pair, attributes)) = text.split_once(';').or(Some((text, ""))) else {
            continue;
        };
        let Some((name, cookie_value)) = pair.split_once('=') else {
            continue;
        };
        let expired = attributes.split(';').any(|part| {
            let part = part.trim();
            let Some((key, raw)) = part.split_once('=') else {
                return false;
            };
            key.trim().eq_ignore_ascii_case("max-age") && raw.trim() == "0"
        });
        if expired {
            cookies.remove(name.trim());
        } else {
            cookies.insert(name.trim().to_owned(), cookie_value.trim().to_owned());
        }
    }
}

fn postgres_database_name(url: &str) -> Result<String, String> {
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err("The controlled PostgreSQL helper accepts only a postgres URL.".into());
    }
    let without_query = url.split('?').next().unwrap_or(url);
    let name = without_query
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && !name.contains('@'))
        .ok_or_else(|| "The PostgreSQL URL has no database name.".to_owned())?;
    Ok(name.to_owned())
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(char) if char.is_ascii_alphabetic() || char == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn remove_owned_directory(directory: &Path) -> Result<(), String> {
    let temp = std::env::temp_dir();
    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !name.starts_with("ferrioxid-test-") || !directory.starts_with(&temp) {
        return Err("Refusing to delete a directory that this test did not create.".into());
    }
    if directory.exists() {
        std::fs::remove_dir_all(directory).map_err(|error| error.to_string())?;
    }
    Ok(())
}
