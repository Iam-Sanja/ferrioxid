# URLs

[For overview](index.md) · Integration: [routing.md](routing.md) · Views: [views.md](views.md) · **IST file:** `crates/ferrioxid-api/src/urls.rs`].

## Aufgabe

`UrlSet` collects Axum routes and stable names. Axum remains the router. There is no second routing engine. Names have the form `bereich.name`, for example `shop.list` or `site.home`.

This corresponds conceptually to Django’s `urls.py` and `reverse`. Ferrioxide does not hide `Router<AppState>`.

An app mounts your `UrlSet` via `Registration::routes`. The namespace must be the app name. An already occupied path, also a model path, is `url.duplicate`. `App::merge` of a raw router does not see these names.

In an app template, Tera resolves the name:

```text
<a href="{{ url(name="shop.detail", id=product.id) }}">Produkt</a>
```

Tera codes the result. A slash appears as `&#x2F;`. In a `href`, this remains a relative path. The function is not marked as HTML secure. If the route or a parameter is missing, the error only mentions the requested name and not the other routes. Parameters remain with letters, numbers, `-` and `_`.

## Registrieren

```rust
use axum::routing::get;
use ferrioxid::urls::{UrlError, UrlSet};

pub fn routes() -> Result<UrlSet, UrlError> {
    let shop = UrlSet::new().route("shop.detail", "/{id}", get(detail))?;
    UrlSet::new()
        .route("shop.list", "/shop", get(list))?
        .nest("/catalog", shop)
}

async fn list() -> axum::response::Response {
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"ok": true}))
}

async fn detail() -> axum::response::Response {
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"ok": true}))
}
```

`nest("/catalog", shop)` hangs the child paths under the prefix. `/{id}` thus becomes `/catalog/{id}`. The name remains `shop.detail`.

The application passes the result to `App::urls`:

```rust
let app = ferrioxid::App::new(db)
    .with_settings(settings.clone())
    .urls(routes()?)?;
```

`merge` remains valid for routes without names. Existing `main.rs` files that only use `register` and `merge` continue to compile.

## Dissolve Names

After `into_router`, the card is `Extension<Arc<ferrioxid::urls::RouteMap>>`.

```rust
use std::sync::Arc;

use axum::extract::Extension;
use ferrioxid::urls::RouteMap;

async fn go(Extension(names): Extension<Arc<RouteMap>>) -> axum::response::Response {
    match names.path("shop.detail", &[("id", "sku-1")]) {
        Ok(path) => match ferrioxid::views::redirect_to(&path) {
            Ok(response) => response,
            Err(error) => error.into_response(),
        },
        Err(error) => ferrioxid::views::fail(
            &axum::http::HeaderMap::new(),
            None,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error.code,
            &error.detail,
            false,
        ),
    }
}
```

`path` replaces `{id}` with only 1 to 80 characters of letters, numbers, `-` and `_`. `..` is not a valid parameter. An unknown name is `url.unknown`. A missing or excess parameter is `url.parameter`.

## Regeln

Paths start with `/`, are at most 256 characters long and do not contain `//`, `\`, `..`, `://`, `?`, `#`, spaces or control characters.

Diese Pfade und jeder Pfad unter `/admin` oder `/auth` sind `url.reserved`: `/healthz`, `/auth/login`, `/auth/logout`, `/auth/me`, `/auth/csrf`, `/auth/change-password`, `/auth/password-reset`, `/auth/password-reset/confirm`, `/auth/password-reset/choose`, `/admin`, `/admin/login`.

A second name or path in the same `UrlSet` or when merging into `App::urls` is `url.duplicate`. `/` is reserved for the start page of `mount_pages` when templates are active. The fullstack template therefore places its own page on `/home`.

## Tests

`names_nests_and_reserved_paths_are_rejected` in `urls.rs`.

## Limitations

Tera does not automatically receive the names as global. A template does not call `reverse` by itself. The trader dissolves the path and puts it in context. `require_user` checks a GET session and not a CSRF. Writing views continue to call `AuthService::authenticate_csrf`.
