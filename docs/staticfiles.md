# Static files

**Milestone:** v0.12, Implemented. The Cargo version remains `0.1.0`.

## Responsibility

`ferrioxid-static` provides CSS, JavaScript, images and fonts from an explicitly configured directory. Embedded admin assets under `/admin` remain separate.

## Crates and files

- `crates/ferrioxid-static/src/lib.rs`
- Install: `ferrioxid::web::prepare` in `crates/ferrioxid-api/src/web.rs`
- Example: `examples/myshop/static/`
- Generator: `static/css/app.css` and `static/js/app.js`

## Public API

The public API consists of `StaticOptions` and `mount`. The route is `{url_prefix}/{*path}`. `serve` sets `Content-Type`, `Content-Length`, `Cache-Control`, `ETag` and `X-Content-Type-Options: nosniff`. A matching `If-None-Match` may produce 304.

## Data flow

The route strips its prefix, validates every path segment and canonicalizes the root and file paths. It serves only files inside the canonical root and derives the ETag from SHA-256.

## Configuration

```toml
[static]
enabled = true
url_prefix = "/static"
directory = "static"
cache_seconds = 3600
```

New projects set `enabled = true`. The existing workspace file without a section leaves `enabled` on `false`. `directory` is relative and must not contain `..`. `url_prefix` shall not overlap `/admin`, `/api`, `/auth`, `/healthz` and `/uploads`. `cache_seconds` is not more than 2592000. The maximum file size is 5 MiB.

## Security

Empty path segments, `\`, NUL, `.`, `..`, dotfiles, `.env`, SQLite files and `.pem` files are rejected. Symlinks resolving outside the root are not served, and directories have no listings. HTML is disallowed. SVG is allowed in the trusted static root but not as an upload. The root must already exist.

## Error cases

Missing, hidden or escaping paths return 404. Unknown file extensions return 415; oversized files return 413.

## Example

After mounting with `ferrioxid::static_files::mount(router, StaticOptions::new("static"))`, request `GET /static/css/app.css`.

## Tests

`cargo test -p ferrioxid-static` checks CSS, JavaScript, PNG, MIME, 404, Cache, ETag, Traversal, hidden files and, if Windows is allowed to create the symlink, a symlink to the outside.

## Limitations

There are no manifest fingerprints, no Brotli and no CDN. The development and production modes use the same reader. Only `cache_seconds` distinguishes the cache duration.

## Extension

A later build step can create hashed filenames. The route remains on the configured prefix.
