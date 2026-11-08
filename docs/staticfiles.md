# Statische Dateien

**Stand:** v0.12, IST. The cargo version remains `0.1.0`.

## Verantwortlichkeit

`ferrioxid-static` provides CSS, JavaScript, images and fonts from an explicitly configured directory. Embedded admin assets under `/admin` remain separate.

## Crates and files

- `crates/ferrioxid-static/src/lib.rs`
- Install: `ferrioxid::web::prepare` in `crates/ferrioxid-api/src/web.rs`
- Example: `examples/myshop/static/`
- Generator: `static/css/app.css` and `static/js/app.js`

## Public API

`StaticOptions` und `mount`. Der Pfad ist `{url_prefix}/{*path}`. `serve` setzt `Content-Type`, `Content-Length`, `Cache-Control`, `ETag` und `X-Content-Type-Options: nosniff`. `If-None-Match` kann 304 liefern.

## Datenfluss

The route removes the prefix, checks each segment, and canonizes root and file. Only files whose canonical path begins in the root are read. The answer contains the bytes and the ETag from SHA-256.

## Konfiguration

```toml
[static]
enabled = true
url_prefix = "/static"
directory = "static"
cache_seconds = 3600
```

New projects set `enabled = true`. The existing workspace file without a section leaves `enabled` on `false`. `directory` is relative and must not contain `..`. `url_prefix` shall not overlap `/admin`, `/api`, `/auth`, `/healthz` and `/uploads`. `cache_seconds` is not more than 2592000. The maximum file size is 5 MiB.

## Sicherheit

Forbidden are empty segments, `\`, NUL, `.`, `..`, point files, `.env`, SQLite files and `.pem`. A symlink that is outside the root after canonization will not be delivered. Lists are not listed. HTML is not an allowed type. SVG from the static root is allowed, upload-SVG is not. The root must exist, otherwise the file is not reachable.

## Error cases

Missing, hidden or erupting paths are 404. Unknown endings are 415. Too large files are 413.

## Example

`GET /static/css/app.css` nach `ferrioxid::static_files::mount(router, StaticOptions::new("static"))`.

## Tests

`cargo test -p ferrioxid-static` checks CSS, JavaScript, PNG, MIME, 404, Cache, ETag, Traversal, hidden files and, if Windows is allowed to create the symlink, a symlink to the outside.

## Limitations

There are no manifest fingerprints, no Brotli and no CDN. The development and production modes use the same reader. Only `cache_seconds` distinguishes the cache duration.

## Erweiterung

A later build step can create hashed filenames. The route remains on the configured prefix.
