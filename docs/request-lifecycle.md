# Request lifecycle

**Milestone:** v0.12, Implemented. The Cargo version remains `0.1.0`.

## Responsibility

This document describes the order of an HTTP request after supplementing forms, static files, uploads and middleware.

## Crates and files

- Router construction: `crates/fexu-api/src/lib.rs`, `App::merge`, `App::into_router`
- Web integration: `crates/fexu-api/src/web.rs`
- Middleware: `crates/fexu-middleware/src/lib.rs`
- Pages: `crates/fexu-api/src/pages.rs`
- Auth: `crates/fexu-auth/src/service.rs`

## Public API

`App::merge` adds routes before `into_router` binds application state. `fexu::web::prepare` then mounts static files when `[static] enabled` is true and applies standard middleware.

## Data flow

```text
Body-Limit
  → Request-ID
  → Access-Log
  → Sicherheitsheader
  → Axum-Route
       → REST
       → Admin und eingebettete Assets
       → Tera-Seiten und Formulare
       → Staticfiles
       → ausdrücklich registrierte Uploads
  → Modell, Auth oder Storage
  → SQLite, PostgreSQL oder lokaler Speicher
```

Auth and CSRF run in the handler. A middleware cannot skip it because it does not replace the handler check.

## Configuration

Static files and storage are in `fexu.toml`. The body limit follows the upload size when `prepare` is used. myshop mounts static files and middleware explicitly because the workspace file omits static files.

## Security

First the size is limited, then a separate request ID is generated. Logs see the path without query. Forms check fields on the server side. Uploads check the owner. Staticfiles remain in the configured root.

## Error cases

413 from the body limit, 401 and 403 from Auth and CSRF, 404 from external files and missing static files, 415 from types, 422 from forms. Database texts remain generic.

## Example

A fullstack project from `fexu new` calls `mount_pages_with` and then `fexu::web::prepare`. `/contact` is merged before `into_router`. The cloned route map gives the project templates `url()`.

## Tests

The middleware order is in `fexu-middleware`. Auth, CSRF and CRUD tests remain the existing SQLite tests.

## Limitations

myshop and generated projects differ in the attachment point: myshop knows the workspace file and mounts its own `static/`. `prepare` reads the discovered `fexu.toml`.

## Extension

Further outer layers may lie in front of `prepare`. Inner auth middleware is not planned as long as the handler verification remains the source of the decision.
