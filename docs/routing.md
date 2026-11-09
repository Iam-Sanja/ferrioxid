# Routing

[Overview](index.md) · Names: [urls.md](urls.md) · **Implemented files:** `crates/ferrioxid-api/src/lib.rs`, `crates/ferrioxid-api/src/urls.rs`, `crates/ferrioxid-auth/src/routes.rs`.

## Purpose

The application code groups routes into Rust modules. Ferrioxid binds them to the same `AppState` and the same middleware as Admin, Auth and the model API. There is no second router.

## Status

`AppState` remains `{ db, auth, mail }`. Settings and the route map are Axum extensions so that existing `State<AppState>` literals remain valid. `with_settings` is `Extension<Arc<ProjectConfig>>`. `into_router` always puts `Extension<Arc<RouteMap>>`, even if it is empty.

A handler who reads settings therefore needs `with_settings`. Without this call, the extractor fails.

## Order

`App::into_router` first attaches the Auth routes and then the admin to the application routes. `/healthz` comes last. `pages::mount_pages` and `web::prepare` are outside of `App` and remain the positions for the homepage, static files and standard middleware. The request ID is `x-request-id`.

```rust
let names = app.route_names().clone();
let router = ferrioxid::pages::mount_pages_with(app.into_router(), &settings, &names)?;
let router = ferrioxid::web::prepare(router, &settings);
```

## Conflicts

`UrlSet` rejects reserved framework paths before the server starts. Two identical names or two identical paths in the named routes as well. `merge` does not check these names. A route that occupies the same path as Auth or Admin remains an Axum conflict at the runtime of the router. New named routes belong in `UrlSet`, not in a second setup.

`register` and `register_policy` for the same model continue to generate duplicate model routes. This is the existing contract and independent of `UrlSet`.

## Access

Views do not receive admin permissions through registration. `allow_anonymous`, model permissions and `ObjectPolicy` remain with `register` and `register_policy`. Custom pages call `ferrioxid::views::require_user`, `require_staff`, `require_permission` or `require_object_read`. There is no second auth middleware.

## Unknown paths

A path without a route remains the Axum 404. `mount_pages` renders `templates/errors/404.html` when that template exists, and a built-in 404 page when it does not. A matched path with the wrong method uses `templates/errors/405.html` the same way. `views::fail_in` uses the status template for a view-generated error.

## Existing projects

`examples/myshop` and older `main.rs` use `App::new`, `register`, `merge` and `discover_project`. You do not need to call `urls` and `with_settings`. The empty route map is still set as an extension.
