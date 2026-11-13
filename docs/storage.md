# Storage

**Milestone:** v0.12, implemented for local storage. Remote object storage is a checked configuration, not a client. The Cargo version remains `0.1.0`. See [storage-remote.md](storage-remote.md).

## Responsibility

`ferrioxid-storage` stores bytes under a root that the application sets. The client filename never becomes the path.

## Crates and files

- Trait and types: `crates/ferrioxid-storage/src/lib.rs`
- Local backend: `crates/ferrioxid-storage/src/local.rs`
- HTTP, only after explicit attachment: `crates/ferrioxid-storage/src/http.rs`

## Public API

`Storage` is asynchronous: `save`, `read`, `delete`, `exists`, `metadata`. `LocalStorage::open(root, max_bytes, allow)` creates the root. `detect_type` recognizes PNG, JPEG, GIF, WEBP, PDF and simple text based on the bytes. `routes` provides `POST /uploads`, `GET /uploads/{id}` and `DELETE /uploads/{id}`.

The local backend performs brief blocking `std::fs` operations. This is a known limitation of the current storage implementation.

## Data flow

`save` checks size and content, creates a UUID and writes `{root}/{owner}/{id}.partial`. Only then will the file be moved exclusively to `{id}`. Metadata is next to it in `{id}.json`. An error deletes the incomplete file. There is no new SQL migration. The existing checksums remain valid.

## Configuration

```toml
[storage]
backend = "local"
directory = "media"
max_upload_size_mb = 10
```

`backend` only accepts `local`. `directory` is relative without `..`. The size is between 1 and 100 MiB. `ferrioxid::web::prepare` sets the body limit at least to this size. myshop opens `examples/myshop/media` with 1 MiB and the allowlist `text/plain`, `image/png`, `image/jpeg`, `application/pdf`.

## Security

Paths are canonicalized and must stay inside the owner’s directory. A symlinked owner directory is refused, and a symlinked file is not opened. Cleanup unlinks those symlinks without deleting their targets. `create_new` prevents silent overwrites. The original filename is sanitized to ASCII letters, digits, periods, hyphens and underscores. A name that is only dots is rejected. When the name has an extension, it must match the detected media type. HTML, SVG, scripts, `MZ` and `ELF` are rejected as text uploads. Requests from another owner return 404. Downloads use `Content-Disposition: attachment`, `application/octet-stream`, `nosniff` and `Content-Security-Policy: sandbox; default-src 'none'`. `save_until` sets an expiry. `cleanup_orphans` removes partial files, mismatched sidecars and expired files without following symlinks. Quotas are per process. Signed URLs and the S3 plan are described in [storage-remote.md](storage-remote.md).

## Error cases

`upload.too_large` returns 413, `upload.type` returns 415 and `resource.not_found` returns 404. Storage failures return `request.rejected` with status 500 and expose neither paths nor system error text.

## Example

```rust
let stored = storage.save(user_id, "notes.txt", b"hello").await?;
let (_meta, bytes) = storage.read(user_id, stored.id).await?;
```

## Tests

`cargo test -p ferrioxid-storage` stores, reads, deletes, isolates owners, rejects size, HTML and path names, and checks anonymous and third-party HTTP accesses.

## Image checks

`detect_type` only recognizes a signature. `inspect_image` additionally checks PNG chunk framing or JPEG markers, the declared width and height, and `ImageLimits` (`max_bytes`, `max_width`, `max_height`, `max_pixels`). It does not inflate PNG pixels or decode a JPEG scan. A later decoder can still allocate more memory than the declared size. `LocalStorage::with_check` runs a `BeforeSave` hook before creating a file. `ImageHeaderCheck` uses `inspect_image`. Files that fail the hook are not written. Uploads without a hook keep the previous behavior.

## Limitations

S3 has no network client. See [storage-remote.md](storage-remote.md). Malware scanning is missing. Content recognition does not replace a virus scanner. Header checks do not decode image pixels. Metadata is Sidecar JSON, not a database table. `StoredFile` accepts older sidecars that omit `created_at`, `expires_at` and `sha256`.

## Extension

Other backends can implement the same `Storage` methods. A future metadata table would require a new migration; existing `0001` files must remain unchanged.
