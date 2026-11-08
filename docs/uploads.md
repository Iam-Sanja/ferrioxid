# Uploads

**Stand:** v0.12, IS for expressly registered routes. The cargo version remains `0.1.0`.

## Verantwortlichkeit

The upload routes are not mounted by default. An application calls `ferrioxid::storage::routes` and mounts the router with `App::merge`. Without auth service, the routes answer with 403.

## Crates and files

- `crates/ferrioxid-storage/src/http.rs`
- Example: `examples/myshop/src/main.rs`
- Memory details: [storage.md](storage.md)]

## Public API

`POST /uploads` takes either the raw body with `x-original-name` and `x-csrf-token` or `multipart/form-data` with the fields `csrf_token` and `file`. The JSON response contains `id`, `original_name`, `content_type` and `size`. It does not contain a file system path. `GET /uploads/{id}` only loads its own file. `DELETE /uploads/{id}` requires session and CSRF headers.

## Datenfluss

The body is read until `max_bytes + 1` and discarded when exceeded. The content determines the type, not the client header. After that, `authenticate_csrf` reviews the existing session. `save` writes the UUID file. Download and delete filter via the user ID from the session.

## Konfiguration

The size comes from `[storage]`. The allowed types are set by the caller of `LocalStorage::open`. They are not additional TOML keys so that an allowlist does not accidentally open everything.

## Sicherheit

Deny by default. Authentication is required. The file ID is a UUID, not a path. Foreign IDs are 404. Deletion without CSRF is 403. Incomplete files are removed. Uploaded files are not executed and are not shipped as HTML or SVG. `media/` is in the project `.gitignore`.

## Error cases

401 without session, 403 without CSRF or without authentic service, 404 for foreign or missing files, 413 above the limit, 415 for unauthorized content, 500 for a memory error without system detail.

## Example

```rust
let storage = ferrioxid::storage::LocalStorage::open("media", 1024 * 1024, &["text/plain"])?;
let app = app.merge(ferrioxid::storage::routes(std::sync::Arc::new(storage)));
```

## Tests

The HTTP test creates two users in SQLite, loads a text file, denies anonymous access, delivers a second user 404, and deletes only with the owner’s CSRF token.

## Limitations

There is no virus scanner, no image processing and no public upload URL. Multipart expects a quoted boundary and at most few parts.

## Erweiterung

A virus scanner can be mounted after `detect_type` and before `save`. It is not included.
