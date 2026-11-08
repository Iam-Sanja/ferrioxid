# Uploads

**Milestone:** v0.12, Implemented for expressly registered routes. The Cargo version remains `0.1.0`.

## Responsibility

The upload routes are not mounted by default. An application calls `ferrioxid::storage::routes` and mounts the router with `App::merge`. Without auth service, the routes answer with 403.

## Crates and files

- `crates/ferrioxid-storage/src/http.rs`
- Example: `examples/myshop/src/main.rs`
- Storage details: [storage.md](storage.md)

## Public API

`POST /uploads` takes either the raw body with `x-original-name` and `x-csrf-token` or `multipart/form-data` with the fields `csrf_token` and `file`. The JSON response contains `id`, `original_name`, `content_type` and `size`. It does not contain a file system path. `GET /uploads/{id}` only loads its own file. `DELETE /uploads/{id}` requires session and CSRF headers.

## Data flow

The body is read until `max_bytes + 1` and discarded when exceeded. The content determines the type, not the client header. After that, `authenticate_csrf` reviews the existing session. `save` writes the UUID file. Download and delete filter via the user ID from the session.

## Configuration

The size comes from `[storage]`. The allowed types are set by the caller of `LocalStorage::open`. They are not additional TOML keys so that an allowlist does not accidentally open everything.

## Security

Deny by default. Authentication is required. The file ID is a UUID, not a path. Foreign IDs are 404. Deletion without CSRF is 403. Incomplete files are removed. Uploaded files are not executed and are not shipped as HTML or SVG. `media/` is in the project `.gitignore`.

## Error cases

Responses are 401 for a missing session, 403 for missing CSRF or auth service, 404 for another owner’s or missing files, 413 for oversized uploads, 415 for disallowed content and 500 for storage failures without system details.

## Example

```rust
let storage = ferrioxid::storage::LocalStorage::open("media", 1024 * 1024, &["text/plain"])?;
let app = app.merge(ferrioxid::storage::routes(std::sync::Arc::new(storage)));
```

## Tests

The HTTP test creates two users in SQLite, loads a text file, denies anonymous access, delivers a second user 404, and deletes only with the owner’s CSRF token.

## Limitations

There is no virus scanner, no image processing and no public upload URL. Multipart expects a quoted boundary and at most few parts.

## Extension

A virus scanner can be mounted after `detect_type` and before `save`. It is not included.
