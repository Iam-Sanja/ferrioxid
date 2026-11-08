# Middleware

**Stand:** v0.12, IST. The cargo version remains `0.1.0`.

## Verantwortlichkeit

`ferrioxid-middleware` hangs tower layers on an Axum router. There is no second middleware runtime. Authentication, sessions and CSRF remain in the traders.

## Crates and files

- `crates/ferrioxid-middleware/src/lib.rs`
- Application: `ferrioxid::web::prepare` and `examples/myshop/src/main.rs`

## Public API

`Standard` carries `body_limit`, standard 2 MiB. `apply(router, standard)` attaches the layers. `RequestId` is in the request extensions. `logged_path` returns only the path. The incoming header `x-request-id` is ignored. The answer sets a new ID.

## Datenfluss

Tower runs the last layer added first. `apply` is therefore attached in this order: security header, access log, request ID, body limit. From the outside inwards:

1. Body-Limit
2. Request-ID
3. Access-Log
4. Sicherheitsheader
5. Route, then Auth, Session and CSRF in the trader

## Konfiguration

`Standard { body_limit }` is the only option. `prepare` uses at least the configured upload size. There is no TOML switch that turns off the security headers.

## Sicherheit

The log contains method, path, status, duration and request ID. Query, cookies, `Authorization`, CSRF, passwords and multipart content are not written. Response headers are `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY` and `Referrer-Policy: no-referrer`. A content security policy is not set so that the admin does not break. The middleware does not replace an authorization check.

The header `x-ferrioxid-trace-order` is only a test probe. He does not change the route.

## Error cases

A body above the limit is 413 before the route runs. A request ID that could not be represented as a header value is not set. UUIDs are displayable.

## Example

```rust
let router = ferrioxid::middleware::apply(router, ferrioxid::middleware::Standard::default());
```

## Tests

`cargo test -p ferrioxid-middleware` checks the request ID, the security headers, the order `request_id,access_log,security_headers`, the body limit and that query and cookie are not in the response stream of the log path.

## Limitations

Our own middleware is supplemented with the normal Axum and Tower layers. There is no declarative list in `ferrum.toml`. Logging writes about `tracing` and does not check the text a second time.

## Erweiterung

Additional layers can be placed before or after `apply`. Who puts it on the outside, sees the request before the body limit.
