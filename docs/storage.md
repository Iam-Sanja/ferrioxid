# Speicher

**Stand:** v0.12, IS for local memory. S3 is planned and not implemented. The cargo version remains `0.1.0`.

## Verantwortlichkeit

`ferrioxid-storage` stores bytes under a root that the application sets. The client filename never becomes the path.

## Crates and files

- Trait and types: `crates/ferrioxid-storage/src/lib.rs`
- Lokal: `crates/ferrioxid-storage/src/local.rs`
- HTTP, only after explicit attachment: `crates/ferrioxid-storage/src/http.rs`

## Public API

`Storage` is asynchronous: `save`, `read`, `delete`, `exists`, `metadata`. `LocalStorage::open(root, max_bytes, allow)` creates the root. `detect_type` recognizes PNG, JPEG, GIF, WEBP, PDF and simple text based on the bytes. `routes` provides `POST /uploads`, `GET /uploads/{id}` and `DELETE /uploads/{id}`.

The local implementation blocks briefly in `std::fs`. This is a known limit, not a second memory runtime system.

## Datenfluss

`save` checks size and content, creates a UUID and writes `{root}/{owner}/{id}.partial`. Only then will the file be moved exclusively to `{id}`. Metadata is next to it in `{id}.json`. An error deletes the incomplete file. There is no new SQL migration. The existing checksums remain valid.

## Konfiguration

```toml
[storage]
backend = "local"
directory = "media"
max_upload_size_mb = 10
```

`backend` only accepts `local`. `directory` is relative without `..`. The size is between 1 and 100 MiB. `ferrioxid::web::prepare` sets the body limit at least to this size. myshop opens `examples/myshop/media` with 1 MiB and the allowlist `text/plain`, `image/png`, `image/jpeg`, `application/pdf`.

## Sicherheit

Paths are canonized and must remain in the owner directory. `create_new` prevents silent overwriting. The original name shall be abbreviated to ASCII letters, numerals, point, hyphen and underscore. HTML, SVG, scripts, `MZ` and `ELF` are not accepted as text. Another owner gets 404, not 403. Downloads are `Content-Disposition: attachment`, `application/octet-stream` and `nosniff`.

## Error cases

`upload.too_large` is 413. `upload.type` is 415. `resource.not_found` is 404. Memory errors are `request.rejected` with 500, without path and without system text.

## Example

```rust
let stored = storage.save(user_id, "notes.txt", b"hello").await?;
let (_meta, bytes) = storage.read(user_id, stored.id).await?;
```

## Tests

`cargo test -p ferrioxid-storage` stores, reads, deletes, isolates owners, rejects size, HTML and path names, and checks anonymous and third-party HTTP accesses.

## Limitations

S3 is not implemented. Malware scanning is missing. Content recognition does not replace a virus scanner. Metadata is Sidecar JSON, not a database table.

## Erweiterung

Another `Storage` backend can implement the same methods. An optional table can be created later about a new migration. Existing `0001` files are not changed for this.
