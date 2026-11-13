//! In-process HTTP client, isolated databases, and fixture loading.
//!
//! The client calls the Axum router directly. It does not open a socket.
//! `postgres` checks the database name and drops only recorded tables. That is
//! not isolation between tests. `postgres_isolated` creates one
//! `fexu_test_<uuid>` database and drops only that database.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::Router;
use fexu_db::driver::{ConnectionTrait, DatabaseConnection, Statement};
use fexu_db::{DisposablePostgres, QueryCounter};
use fexu_migrations::{
    load_fixture, FixtureFile, LoadReport, LoadRequest, MigrationError, SchemaSnapshot,
};
use http_body_util::BodyExt;
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

    /// Builds one request. The cookie jar is sent unless this request sets `Cookie`.
    pub fn request(&mut self, method: Method, path: impl Into<String>) -> TestRequest<'_> {
        TestRequest {
            client: self,
            method,
            path: path.into(),
            headers: HeaderMap::new(),
            body: Vec::new(),
        }
    }

    pub async fn get(&mut self, path: &str) -> Result<TestResponse, String> {
        self.request(Method::GET, path).send().await
    }

    pub async fn delete(&mut self, path: &str) -> Result<TestResponse, String> {
        self.request(Method::DELETE, path).send().await
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
        self.form(Method::POST, path, fields).await
    }

    pub async fn put_form(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        self.form(Method::PUT, path, fields).await
    }

    pub async fn patch_form(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        self.form(Method::PATCH, path, fields).await
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
        self.request(method, path)
            .header(header::CONTENT_TYPE.as_str(), "application/json")?
            .bytes(body.to_string().into_bytes())
            .send()
            .await
    }

    async fn form(
        &mut self,
        method: Method,
        path: &str,
        fields: &[(&str, &str)],
    ) -> Result<TestResponse, String> {
        self.request(method, path)
            .header(
                header::CONTENT_TYPE.as_str(),
                "application/x-www-form-urlencoded",
            )?
            .bytes(form_body(fields).into_bytes())
            .send()
            .await
    }
}

/// One in-process request. Extra headers do not change the stored cookie jar.
pub struct TestRequest<'a> {
    client: &'a mut TestClient,
    method: Method,
    path: String,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl<'a> TestRequest<'a> {
    /// Sets one request header. `Cookie` replaces the jar for this request only.
    pub fn header(mut self, name: &str, value: &str) -> Result<Self, String> {
        let name = HeaderName::try_from(name).map_err(|error| error.to_string())?;
        let value = HeaderValue::try_from(value).map_err(|error| error.to_string())?;
        self.headers.insert(name, value);
        Ok(self)
    }

    pub fn bytes(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub async fn send(self) -> Result<TestResponse, String> {
        let Self {
            client,
            method,
            path,
            mut headers,
            body,
        } = self;
        if !headers.contains_key(header::COOKIE) && !client.cookies.is_empty() {
            let value = HeaderValue::try_from(cookie_header(&client.cookies))
                .map_err(|error| error.to_string())?;
            headers.insert(header::COOKIE, value);
        }
        let mut builder = Request::builder().method(method).uri(&path);
        for (name, value) in headers.iter() {
            builder = builder.header(name, value);
        }
        let request = builder
            .body(Body::from(body))
            .map_err(|error| error.to_string())?;
        let response = match client.router.clone().oneshot(request).await {
            Ok(response) => response,
            Err(error) => match error {},
        };
        let status = response.status();
        let response_headers = response.headers().clone();
        store_cookies(&mut client.cookies, &response_headers);
        let bytes = response
            .into_body()
            .collect()
            .await
            .map_err(|error| error.to_string())?
            .to_bytes();
        Ok(TestResponse {
            status,
            headers: response_headers,
            body: bytes.to_vec(),
        })
    }
}

pub struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

/// One `Set-Cookie` header, including the attributes the cookie jar does not store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCookie {
    pub name: String,
    pub value: String,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
    pub path: Option<String>,
    pub domain: Option<String>,
    pub max_age: Option<i64>,
}

impl TestResponse {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns the first value of `name`.
    pub fn header(&self, name: &str) -> Result<Option<&str>, String> {
        let name = HeaderName::try_from(name).map_err(|error| error.to_string())?;
        match self.headers.get(&name) {
            Some(value) => Ok(Some(value.to_str().map_err(|error| error.to_string())?)),
            None => Ok(None),
        }
    }

    /// Returns every value of `name`, including repeated `Set-Cookie` headers.
    pub fn header_values(&self, name: &str) -> Result<Vec<&str>, String> {
        let name = HeaderName::try_from(name).map_err(|error| error.to_string())?;
        self.headers
            .get_all(name)
            .iter()
            .map(|value| value.to_str().map_err(|error| error.to_string()))
            .collect()
    }

    pub fn set_cookies(&self) -> Result<Vec<SetCookie>, String> {
        self.header_values(header::SET_COOKIE.as_str())?
            .into_iter()
            .map(parse_set_cookie)
            .collect()
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
    owned_database: Option<DisposablePostgres>,
}

enum DatabaseKind {
    Sqlite,
    Postgres,
}

impl TestDatabase {
    pub async fn sqlite() -> Result<Self, String> {
        let directory =
            std::env::temp_dir().join(format!("fexu-test-{}", uuid::Uuid::new_v4().simple()));
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
            owned_database: None,
        })
    }

    /// Accepts only a PostgreSQL URL whose database name contains `fexu_test`.
    ///
    /// The URL is used as given. Tests that share it also share its tables and
    /// migration history. This method does not create a database and never
    /// runs `DROP DATABASE`.
    pub fn postgres(url: &str) -> Result<Self, String> {
        let name = postgres_database_name(url)?;
        if !name.contains("fexu_test") {
            return Err(
                "Refusing a PostgreSQL test connection whose database name does not contain fexu_test."
                    .into(),
            );
        }
        Ok(Self {
            url: url.to_owned(),
            kind: DatabaseKind::Postgres,
            directory: None,
            tables: Vec::new(),
            owned_database: None,
        })
    }

    /// Creates one new `fexu_test_<uuid>` database and connects only to it.
    ///
    /// `admin_url` must name an approved test database. Creating the database
    /// requires `CREATEDB` or an equivalent role. [`Self::cleanup`] drops only
    /// the database this call created.
    pub async fn postgres_isolated(admin_url: &str) -> Result<Self, String> {
        let owned = DisposablePostgres::create(admin_url).await?;
        let url = owned.url().to_owned();
        Ok(Self {
            url,
            kind: DatabaseKind::Postgres,
            directory: None,
            tables: Vec::new(),
            owned_database: Some(owned),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn connect(&self) -> Result<DatabaseConnection, String> {
        fexu_db::connect(&self.url)
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

    /// Drops recorded tables, or the database created by [`Self::postgres_isolated`].
    /// SQLite cleanup also deletes the temporary directory. An isolated PostgreSQL
    /// database is dropped by name; recorded tables in another database are not.
    pub async fn cleanup(mut self) -> Result<(), String> {
        if matches!(self.kind, DatabaseKind::Postgres) && self.directory.is_some() {
            return Err("A PostgreSQL test database has no local directory to delete.".into());
        }
        match self.kind {
            DatabaseKind::Postgres => {
                if let Some(owned) = self.owned_database.take() {
                    return owned.drop_database().await;
                }
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

fn parse_set_cookie(raw: &str) -> Result<SetCookie, String> {
    let (pair, attributes) = raw.split_once(';').unwrap_or((raw, ""));
    let Some((name, value)) = pair.split_once('=') else {
        return Err(format!("Set-Cookie has no name/value pair: {raw}"));
    };
    let mut cookie = SetCookie {
        name: name.trim().to_owned(),
        value: value.trim().to_owned(),
        http_only: false,
        secure: false,
        same_site: None,
        path: None,
        domain: None,
        max_age: None,
    };
    if cookie.name.is_empty() {
        return Err("Set-Cookie name is empty.".into());
    }
    for part in attributes.split(';').filter(|part| !part.trim().is_empty()) {
        let part = part.trim();
        if let Some((key, raw_value)) = part.split_once('=') {
            let parsed = raw_value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "path" => cookie.path = Some(parsed.to_owned()),
                "domain" => cookie.domain = Some(parsed.to_owned()),
                "samesite" => cookie.same_site = Some(parsed.to_owned()),
                "max-age" => {
                    cookie.max_age =
                        Some(parsed.parse().map_err(|_| {
                            format!("Set-Cookie Max-Age is not an integer: {parsed}")
                        })?);
                }
                _ => {}
            }
        } else {
            match part.to_ascii_lowercase().as_str() {
                "httponly" => cookie.http_only = true,
                "secure" => cookie.secure = true,
                _ => {}
            }
        }
    }
    Ok(cookie)
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
    if !name.starts_with("fexu-test-") || !directory.starts_with(&temp) {
        return Err("Refusing to delete a directory that this test did not create.".into());
    }
    if directory.exists() {
        std::fs::remove_dir_all(directory).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn memory_fixture_password() -> String {
        String::from_utf8(vec![
            99, 111, 114, 114, 101, 99, 116, 45, 104, 111, 114, 115, 101, 45, 98, 97, 116, 116,
            101, 114, 121,
        ])
        .expect("fixture password")
    }
    use super::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::response::Response;
    use axum::routing::any;

    fn router() -> Router {
        Router::new().route(
            "/probe",
            any(|request: Request| async move {
                let origin = request
                    .headers()
                    .get(header::ORIGIN)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                let cookie = request
                    .headers()
                    .get(header::COOKIE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                Response::builder()
                    .status(StatusCode::SEE_OTHER)
                    .header(header::LOCATION, "/next")
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .header(header::CACHE_CONTROL, "private, no-store")
                    .header(
                        header::SET_COOKIE,
                        "fexu_session=secret; HttpOnly; Secure; SameSite=Lax; Path=/",
                    )
                    .header(
                        header::SET_COOKIE,
                        "fexu_anonymous=old; Max-Age=0; Path=/; SameSite=Lax",
                    )
                    .body(Body::from(format!("origin={origin}\ncookie={cookie}")))
                    .unwrap()
            }),
        )
    }

    #[tokio::test]
    async fn request_builder_exposes_headers_and_set_cookie_attributes() {
        let mut client = TestClient::new(router());
        client.set_cookie("fexu_session", "stored");
        let response = client
            .request(Method::POST, "/probe")
            .header(header::ORIGIN.as_str(), "https://fremd.example.invalid")
            .unwrap()
            .header(header::ACCEPT.as_str(), "text/html")
            .unwrap()
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.header("location").unwrap(), Some("/next"));
        assert_eq!(
            response.header("content-type").unwrap(),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(
            response.header("cache-control").unwrap(),
            Some("private, no-store")
        );
        let text = response.text().unwrap();
        assert!(text.contains("origin=https://fremd.example.invalid"));
        assert!(text.contains("cookie=fexu_session=stored"));
        let cookies = response.set_cookies().unwrap();
        assert_eq!(cookies.len(), 2);
        assert_eq!(cookies[0].name, "fexu_session");
        assert_eq!(cookies[0].value, "secret");
        assert!(cookies[0].http_only);
        assert!(cookies[0].secure);
        assert_eq!(cookies[0].same_site.as_deref(), Some("Lax"));
        assert_eq!(cookies[0].path.as_deref(), Some("/"));
        assert_eq!(cookies[1].name, "fexu_anonymous");
        assert_eq!(cookies[1].max_age, Some(0));
        assert_eq!(client.cookie("fexu_session"), Some("secret"));
        assert_eq!(client.cookie("fexu_anonymous"), None);
    }

    #[tokio::test]
    async fn explicit_cookie_header_replaces_the_jar_for_one_request() {
        let mut client = TestClient::new(router());
        client.set_cookie("fexu_session", "stored");
        let response = client
            .request(Method::GET, "/probe")
            .header(header::COOKIE.as_str(), "fexu_session=foreign")
            .unwrap()
            .send()
            .await
            .unwrap();
        assert!(response
            .text()
            .unwrap()
            .contains("cookie=fexu_session=foreign"));
        assert_eq!(client.cookie("fexu_session"), Some("secret"));
    }

    #[tokio::test]
    async fn csrf_helper_still_copies_the_form_token() {
        let html = r#"<form><input type="hidden" name="csrf_token" value="abc-1"></form>"#;
        let token = csrf_token_from_html(html).unwrap();
        assert_eq!(token, "abc-1");
        let mut client = TestClient::new(Router::new().route(
            "/form",
            any(|request: Request| async move {
                let bytes = request.into_body().collect().await.unwrap().to_bytes();
                Response::new(Body::from(bytes))
            }),
        ));
        let response = client
            .post_form_with_csrf("/form", html, &[("name", "Ada")])
            .await
            .unwrap();
        let body = response.text().unwrap();
        assert!(body.contains("csrf_token=abc-1"));
        assert!(body.contains("name=Ada"));
    }

    #[test]
    fn invalid_request_header_is_rejected() {
        let mut client = TestClient::new(Router::new());
        let Err(error) = client
            .request(Method::GET, "/probe")
            .header("origin", "bad\r\nX-Injected: 1")
        else {
            panic!("a header containing CR/LF was accepted");
        };
        assert!(!error.is_empty());
    }

    #[tokio::test]
    async fn browser_login_flow_exposes_cookies_and_rejects_bad_sessions() {
        use axum::extract::State;
        use fexu_auth::{AuthConfig, AuthError, AuthService, BrowserActor};

        let db = fexu_db::connect("sqlite::memory:").await.unwrap();
        fexu_auth::apply_migrations(&db).await.unwrap();
        let auth = AuthService::new(db, AuthConfig::development());
        let password = memory_fixture_password();
        auth.create_user("ada@example.com", &password, true, false)
            .await
            .unwrap();
        let issued = auth
            .login("ada@example.com", &password, "peer")
            .await
            .unwrap();
        let router = Router::new()
            .route(
                "/session",
                axum::routing::get(|State(auth): State<AuthService>| async move {
                    let session = auth
                        .ensure_anonymous_session(&HeaderMap::new())
                        .await
                        .unwrap();
                    let mut response = Response::builder().status(StatusCode::OK);
                    if let Some(cookie) = session.cookie {
                        response = response.header(header::SET_COOKIE, cookie);
                    }
                    response.body(Body::from(session.csrf_token)).unwrap()
                }),
            )
            .route(
                "/login",
                axum::routing::post(
                    |State(auth): State<AuthService>,
                     headers: HeaderMap,
                     body: axum::body::Bytes| async move {
                        let text = String::from_utf8_lossy(&body);
                        let token = text
                            .split('&')
                            .find_map(|pair| {
                                let (key, value) = pair.split_once('=')?;
                                (key == "csrf_token").then_some(value.to_owned())
                            })
                            .unwrap_or_default();
                        match auth
                            .authenticate_browser_form(&headers, &Method::POST, &token)
                            .await
                        {
                            Ok(BrowserActor::User(_)) => Response::builder()
                                .status(StatusCode::SEE_OTHER)
                                .header(header::LOCATION, "/account")
                                .body(Body::empty())
                                .unwrap(),
                            Ok(_) => Response::builder()
                                .status(StatusCode::NO_CONTENT)
                                .body(Body::empty())
                                .unwrap(),
                            Err(AuthError::OriginRejected | AuthError::CsrfRejected) => {
                                Response::builder()
                                    .status(StatusCode::FORBIDDEN)
                                    .body(Body::from("rejected"))
                                    .unwrap()
                            }
                            Err(_) => Response::builder()
                                .status(StatusCode::UNAUTHORIZED)
                                .body(Body::from("anonymous"))
                                .unwrap(),
                        }
                    },
                ),
            )
            .with_state(auth);
        let mut client = TestClient::new(router);
        let session = client.get("/session").await.unwrap();
        assert_eq!(session.status(), StatusCode::OK);
        let csrf = session.text().unwrap();
        let cookie = session
            .set_cookies()
            .unwrap()
            .into_iter()
            .find(|cookie| cookie.name == "fexu_anonymous")
            .unwrap();
        assert!(cookie.http_only);
        assert_eq!(cookie.same_site.as_deref(), Some("Lax"));
        assert_eq!(client.cookie("fexu_anonymous"), Some(cookie.value.as_str()));

        let accepted = client
            .request(Method::POST, "/login")
            .header(header::ORIGIN.as_str(), "http://127.0.0.1:3000")
            .unwrap()
            .header(
                header::CONTENT_TYPE.as_str(),
                "application/x-www-form-urlencoded",
            )
            .unwrap()
            .bytes(format!("csrf_token={csrf}").into_bytes())
            .send()
            .await
            .unwrap();
        assert_eq!(accepted.status(), StatusCode::NO_CONTENT);

        let foreign = client
            .request(Method::POST, "/login")
            .header(header::ORIGIN.as_str(), "https://fremd.example.invalid")
            .unwrap()
            .header(
                header::CONTENT_TYPE.as_str(),
                "application/x-www-form-urlencoded",
            )
            .unwrap()
            .bytes(format!("csrf_token={csrf}").into_bytes())
            .send()
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

        let missing = client.post_form("/login", &[]).await.unwrap();
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);
        let invalid = client
            .post_form("/login", &[("csrf_token", "not-the-token")])
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::FORBIDDEN);

        client.set_cookie("fexu_session", "not-a-user-session");
        let downgraded = client
            .post_form("/login", &[("csrf_token", csrf)])
            .await
            .unwrap();
        assert_eq!(downgraded.status(), StatusCode::UNAUTHORIZED);

        client.clear_cookies();
        let (name, value) = issued.cookie.split_once('=').unwrap();
        let value = value.split(';').next().unwrap();
        client.set_cookie(name, value);
        let user = client
            .request(Method::POST, "/login")
            .header(header::ORIGIN.as_str(), "http://127.0.0.1:3000")
            .unwrap()
            .header(
                header::CONTENT_TYPE.as_str(),
                "application/x-www-form-urlencoded",
            )
            .unwrap()
            .bytes(format!("csrf_token={}", issued.csrf_token).into_bytes())
            .send()
            .await
            .unwrap();
        assert_eq!(user.status(), StatusCode::SEE_OTHER);
        assert_eq!(user.header("location").unwrap(), Some("/account"));
    }

    #[tokio::test]
    async fn postgres_isolated_refuses_an_unapproved_database_before_connecting() {
        let error =
            match TestDatabase::postgres_isolated("postgres://fexu:secret@127.0.0.1:5432/shop")
                .await
            {
                Ok(_) => panic!("an unapproved database was accepted"),
                Err(error) => error,
            };
        assert!(error.contains("Refusing"));
        assert!(error.contains("shop"));
        assert!(!error.contains("secret"));
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL, CREATEDB, and an approved PostgreSQL test database"]
    async fn postgres_isolated_databases_do_not_share_rows() {
        let admin = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let left = TestDatabase::postgres_isolated(&admin)
            .await
            .expect("left database");
        let right = TestDatabase::postgres_isolated(&admin)
            .await
            .expect("right database");
        assert_ne!(left.url(), right.url());
        let left_db = left.connect().await.expect("left connection");
        let right_db = right.connect().await.expect("right connection");
        left_db
            .execute(Statement::from_string(
                fexu_db::driver::DatabaseBackend::Postgres,
                "CREATE TABLE fexu_isolation_probe (id integer PRIMARY KEY)".to_owned(),
            ))
            .await
            .expect("create probe");
        left_db
            .execute(Statement::from_string(
                fexu_db::driver::DatabaseBackend::Postgres,
                "INSERT INTO fexu_isolation_probe (id) VALUES (1)".to_owned(),
            ))
            .await
            .expect("insert probe");
        let missing = right_db
            .execute(Statement::from_string(
                fexu_db::driver::DatabaseBackend::Postgres,
                "SELECT id FROM fexu_isolation_probe".to_owned(),
            ))
            .await;
        assert!(missing.is_err(), "the second database saw the first table");
        left_db.close().await.expect("close left");
        right_db.close().await.expect("close right");
        let left_url = left.url().to_owned();
        left.cleanup().await.expect("drop left");
        right.cleanup().await.expect("drop right");
        let gone = fexu_db::connect(&left_url).await;
        assert!(gone.is_err(), "cleanup left the isolated database in place");
    }
}
