# Request-Lebenszyklus

**Stand:** v0.12, IST. The cargo version remains `0.1.0`.

## Verantwortlichkeit

This document describes the order of an HTTP request after supplementing forms, static files, uploads and middleware.

## Crates and files

- Routeraufbau: `crates/ferrioxid-api/src/lib.rs`, `App::merge`, `App::into_router`
- Case: `crates/ferrioxid-api/src/web.rs`
- Middleware: `crates/ferrioxid-middleware/src/lib.rs`
- Seiten: `crates/ferrioxid-api/src/pages.rs`
- Auth: `crates/ferrioxid-auth/src/service.rs`

## Public API

`App::merge` attaches conditioned routes before `into_router` binds the state. `ferrioxid::web::prepare` then assembles static files if `[static] enabled` is, and the standard middleware.

## Datenfluss

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

Auth and CSRF run in the trader. A middleware cannot skip it because it does not replace the trader check.

## Konfiguration

Static files and storage are in `ferrum.toml`. The body limit follows the upload size when `prepare` is used. myshop mounts static files and middleware explicitly because the workspace file omits static files.

## Sicherheit

First the size is limited, then a separate request ID is generated. Logs see the path without query. Forms check fields on the server side. Uploads check the owner. Staticfiles remain in the configured root.

## Error cases

413 from the body limit, 401 and 403 from Auth and CSRF, 404 from external files and missing static files, 415 from types, 422 from forms. Database texts remain generic.

## Example

A fullstack project from `ferrioxid new` calls `mount_pages` and then `ferrioxid::web::prepare`. `/contact` is merged before `into_router`.

## Tests

The middleware order is in `ferrioxid-middleware`. Auth, CSRF and CRUD tests remain the existing SQLite tests.

## Limitations

myshop and generated projects differ in the attachment point: myshop knows the workspace file and mounts its own `static/`. `prepare` reads the discovered `ferrum.toml`.

## Erweiterung

Further outer layers may lie in front of `prepare`. Inner auth middleware is not planned as long as the trader verification remains the source of the decision.
