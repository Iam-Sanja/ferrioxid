# Middleware

**Milestone:** v0.12, Implemented. The Cargo version remains `0.1.0`.

## Responsibility

`ferrioxid-middleware` applies Tower layers to an Axum router. Authentication, anonymous sessions and CSRF checks remain in the handlers. There is no global layer that creates an anonymous session or rejects every POST. A route opts in by calling `authenticate_browser_form` or `guard_cookie_write`.

## Crates and files

- `crates/ferrioxid-middleware/src/lib.rs`
- Application: `ferrioxid::web::prepare` and `examples/myshop/src/main.rs`

## Public API

`Standard` carries `body_limit`, standard 2 MiB. `apply(router, standard)` attaches the layers. `RequestId` is in the request extensions. `logged_path` returns only the path. The incoming header `x-request-id` is ignored. The answer sets a new ID.

## Data flow

Tower runs the last layer added first. `apply` is therefore attached in this order: security header, access log, request ID, body limit. From the outside inwards:

1. Body-Limit
2. Request-ID
3. Access-Log
4. Security headers
5. Route, then Auth, Session and CSRF in the handler

## Configuration

`Standard { body_limit }` is the only option. `prepare` uses at least the configured upload size. There is no TOML switch that turns off the security headers.

## Security

The log contains method, path, status, duration and request ID. Query, cookies, `Authorization`, CSRF, passwords and multipart content are not written. Response headers are `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY` and `Referrer-Policy: no-referrer`. A content security policy is not set so that the admin does not break. The middleware does not replace an authorization check.

The `x-ferrioxid-trace-order` header is a test probe and does not affect routing.

## Error cases

A body above the limit is 413 before the route runs. A request ID that could not be represented as a header value is not set. UUIDs are displayable.

## Example

```rust
let router = ferrioxid::middleware::apply(router, ferrioxid::middleware::Standard::default());
```

## Tests

`cargo test -p ferrioxid-middleware` checks the request ID, the security headers, the order `request_id,access_log,security_headers`, the body limit and that query and cookie are not in the response stream of the log path.

## Limitations

Standard Axum and Tower layers can supplement the built-in middleware. There is no declarative middleware list in `ferrioxid.toml`. Logging uses `tracing` without an additional text-filtering pass.

## Extension

Additional layers can run before or after `apply`. Outer layers receive the request before the body limit is checked.
