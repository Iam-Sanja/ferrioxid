# Admin assets

[Overview](admin.md) · Native: [admin-native.md](admin-native.md)

## Purpose

CSS and the optional script are embedded in the Rust binary. Node.js is required neither at build time nor at runtime.

## Files

- `crates/ferrioxid-admin/static/css/admin.css`
- `crates/ferrioxid-admin/static/js/admin.js`
- `crates/ferrioxid-admin/src/assets.rs`
- `crates/ferrioxid-admin/build.rs`

`assets.rs` embeds both files with `include_str!` and serves only `admin.css` and `admin.js`. All other paths, including paths containing `..`, return 404. There is no directory override; `FERRUM_ADMIN_DIST` has no effect.

## Feature

The `admin-ui` feature on `ferrioxid` enables the empty compatibility feature `ferrioxid-admin/embedded-admin`. Existing fullstack projects that list it in `Cargo.toml` continue to compile. API projects also compile without it. Both serve embedded assets rather than a separate frontend build. `App::with_admin` determines whether admin routes are mounted.

## Test

`admin_assets_reject_traversal_and_keep_the_api` verifies that `FERRUM_ADMIN_DIST` does not affect responses, CSS and JavaScript come from the binary, unknown paths return 404, and `/admin/api/models` still returns JSON.
