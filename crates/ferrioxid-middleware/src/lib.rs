//! Standard-Middleware für Axum. Die zuletzt hinzugefügte Schicht sieht die Anfrage zuerst.
//!
//! Reihenfolge von außen nach innen: Body-Limit, Request-ID, Access-Log, Sicherheitsheader, Route.
//! Diese Crate ersetzt weder Authentifizierung noch Autorisierung. Den Cookie-CSRF-Schutz setzt
//! `App::into_router`, nicht diese Schicht.

use axum::extract::Request;
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use axum::Router;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RequestId(pub String);

#[derive(Clone, Debug)]
pub struct Standard {
    pub body_limit: usize,
}

impl Default for Standard {
    fn default() -> Self {
        Self {
            body_limit: 2 * 1024 * 1024,
        }
    }
}

pub fn apply<S>(router: Router<S>, standard: Standard) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(access_log))
        .layer(axum::middleware::from_fn(request_id))
        .layer(axum::extract::DefaultBodyLimit::max(standard.body_limit))
}

async fn request_id(mut request: Request, next: Next) -> Response {
    let id = Uuid::new_v4().simple().to_string();
    request.extensions_mut().insert(RequestId(id.clone()));
    note(&mut request, "request_id");
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

async fn access_log(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = logged_path(request.uri());
    let started = Instant::now();
    let id = request
        .extensions()
        .get::<RequestId>()
        .map(|value| value.0.clone())
        .unwrap_or_default();
    let mut request = request;
    note(&mut request, "access_log");
    let response = next.run(request).await;
    tracing::info!(
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        request_id = %id,
        "http request"
    );
    response
}

async fn security_headers(mut request: Request, next: Next) -> Response {
    note(&mut request, "security_headers");
    let order = request.extensions().get::<LayerOrder>().cloned();
    let mut response = next.run(request).await;
    if let Some(order) = order {
        if let Ok(value) = HeaderValue::from_str(&order.0.join(",")) {
            response
                .headers_mut()
                .insert("x-ferrioxid-middleware", value);
        }
    }
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

/// Nur der Pfad. Query-Werte wie Passwörter oder Tokens werden nicht protokolliert.
pub fn logged_path(uri: &axum::http::Uri) -> String {
    uri.path().to_owned()
}

#[derive(Clone, Default)]
struct LayerOrder(Vec<String>);

fn note(request: &mut Request, name: &str) {
    if request.headers().get("x-ferrioxid-trace-order").is_none() {
        return;
    }
    if let Some(order) = request.extensions_mut().get_mut::<LayerOrder>() {
        order.0.push(name.to_owned());
    } else {
        request
            .extensions_mut()
            .insert(LayerOrder(vec![name.to_owned()]));
    }
}

#[cfg(test)]
mod tests {
    use super::{apply, logged_path, Standard};
    use axum::routing::get;
    use axum::Router;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    #[test]
    fn query_secrets_stay_out_of_the_log_path() {
        let uri = "/login?password=secret&token=abc".parse().unwrap();
        assert_eq!(logged_path(&uri), "/login");
    }

    #[test]
    fn request_id_and_security_headers_are_set() {
        let app = apply(
            Router::new().route("/health", get(|| async { "ok" })),
            Standard::default(),
        );
        let body = exchange(app, "GET /health?password=secret HTTP/1.1\r\nHost: localhost\r\nCookie: ferrioxid_session=secret\r\nConnection: close\r\n\r\n");
        let lower = body.to_ascii_lowercase();
        assert!(lower.contains("x-request-id:"), "{body}");
        assert!(lower.contains("x-content-type-options: nosniff"));
        assert!(lower.contains("x-frame-options: deny"));
        assert!(!lower.contains("password=secret"));
        assert!(!lower.contains("ferrioxid_session=secret"));
    }

    #[test]
    fn oversized_body_is_rejected() {
        let app = apply(
            Router::new().route(
                "/echo",
                axum::routing::post(|_body: axum::body::Bytes| async { "ok" }),
            ),
            Standard { body_limit: 8 },
        );
        let body = exchange(app, "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 20\r\nConnection: close\r\n\r\n01234567890123456789");
        assert!(body.contains("413"), "{body}");
    }

    #[test]
    fn middleware_runs_request_id_before_logging_and_headers() {
        let app = apply(
            Router::new().route("/health", get(|| async { "ok" })),
            Standard::default(),
        );
        let body = exchange(app, "GET /health HTTP/1.1\r\nHost: localhost\r\nx-ferrioxid-trace-order: 1\r\nConnection: close\r\n\r\n");
        let lower = body.to_ascii_lowercase();
        assert!(
            lower.contains("x-ferrioxid-middleware: request_id,access_log,security_headers"),
            "{body}"
        );
    }

    fn exchange(app: Router, request: &str) -> String {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let address = listener.local_addr().unwrap();
        runtime.spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut body = String::new();
        let _ = stream.read_to_string(&mut body);
        body
    }
}
