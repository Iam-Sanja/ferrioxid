# Admin-Assets

[For overview](admin.md) · Native: [admin-native.md](admin-native.md)]

## Aufgabe

CSS and the optional script are shipped with the Rust binary. Node.js does not start for construction or running time.

## Dateien

- `crates/ferrioxid-admin/static/css/admin.css`
- `crates/ferrioxid-admin/static/js/admin.js`
- `crates/ferrioxid-admin/src/assets.rs`
- `crates/ferrioxid-admin/build.rs`

`assets.rs` integrates the two files with `include_str!` and delivers only `admin.css` and `admin.js`. Every other path, even with `..`, is 404. There is no directory override and no `FERRUM_ADMIN_DIST`.

## Feature

`admin-ui` to `ferrioxid` switches `ferrioxid-admin/embedded-admin` on. This feature is empty. Existing fullstack projects, which call it `Cargo.toml`, continue to compile. An API project without the feature also compiles. The admin is not read from a frontend build in either case. Whether the routes exist, decides `App::with_admin`.

## Test

`admin_assets_reject_traversal_and_keep_the_api` checks that a set `FERRUM_ADMIN_DIST` does not change the answers, that CSS and script come from the binary, unknown paths are 404 and `/admin/api/models` JSON remains.
