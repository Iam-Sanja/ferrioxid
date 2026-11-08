# Views

[For overview](index.md) · Routes: [urls.md](urls.md) · **actual file:** `crates/ferrioxid-api/src/views.rs`].

## Aufgabe

`ferrioxid::views` provides the answers that an application often needs: HTML, JSON, a local redirect and stable errors. The types remain Axum-`Response` and `HeaderMap`. Tera remains the template engine.

`require_mutation(auth, headers, token)` checks a POST request via `AuthService::authenticate_csrf`. If the Auth service is missing, the answer is `401` and `auth.not_configured`. There is no second CSRF mechanism. `require_user` remains a GET test. The app registration does not grant any permission.

## Django and ferrioxide

| Django | Ferrioxid |
|---|---|
| `render(request, "home.html", context)` | `views::render(site, "home.html", context, debug)` |
| `JsonResponse({"ok": True})` | `views::json(&ferrioxid::serde_json::json!({"ok": true}))` |
| `redirect("/home")` | `views::redirect_to("/home")` |
| `Http404` | `views::fail(..., StatusCode::NOT_FOUND, code, detail, debug)` |
| `PermissionDenied` |`views::fail(..., StatusCode::FORBIDDEN, ...)` or `require_staff`|
| `login_required` |`require_user` for an existing session|
| `staff_member_required` | `require_staff` |
| `permission_required("app.view")` | `require_permission(..., "app.view")` |
| `get_object_or_404` plus Policy |Existing repository and `require_object_read::<T, P>`|

The correspondence is the task, not the syntax. A ferrioxide view is an axum function. It gets the state as an argument and returns `Response`.

## HTML

```rust
use std::sync::Arc;

use axum::extract::Extension;
use ferrioxid::migrations::ProjectConfig;

pub async fn home(
    Extension(settings): Extension<Arc<ProjectConfig>>,
) -> axum::response::Response {
    let site = match ferrioxid::pages::Site::open(&settings.templates_directory()) {
        Ok(site) => site,
        Err(_) => {
            return ferrioxid::views::fail(
                &axum::http::HeaderMap::new(),
                None,
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "view.template",
                "Internal server error",
                settings.debug,
            )
        }
    };
    let context = ferrioxid::pages::home_context(&settings);
    ferrioxid::views::render(&site, "home.html", &context, settings.debug)
}
```

If the rendering fails, `render` responds with `view.template`. In production and at `debug = false` the text is `Internal server error`. The template itself is not copied into the response if the error calls a secret, a token or a stacktrace.

## JSON, Redirect, Formular

```rust
fn answers() -> Result<axum::response::Response, ferrioxid::views::ViewError> {
    let _response = ferrioxid::views::json(&ferrioxid::serde_json::json!({"ok": true}));
    let target = ferrioxid::views::redirect_to("/home")?;
    let _fields = ferrioxid::views::form_fields("name=Ada&city=Bern")?;
    Ok(target)
}
```

`json` takes `&serde_json::Value`. `redirect_to` allows a single local path: it starts with `/`, not `//`, and does not contain any schema, `\`, `..` or control characters. `https://…` is `view.redirect` with status 400. `form_fields` reads `application/x-www-form-urlencoded` up to 8 KiB. A larger or faulty pair is `request.rejected`.

## Fehler

`fail` and `ViewError` use the same categories:

| Status |Key of the HTML heading|
|---|---|
| 400 | `bad_request` |
| 401 | `unauthorized` |
| 403 | `forbidden` |
| 404 | `not_found` |
| 409 | `conflict` |
| 422 | `validation` |
| 500 | `server_error` |

Enthält `Accept` den Text `text/html` und nicht `application/json`, ist die Antwort HTML. Sonst bleibt `problem_json` mit `type`, `title`, `status`, `detail` und `code`. `validation` setzt das Detail auf `field:{name}: {detail}`, damit die bestehende Feldklassifikation gilt.

`Accept-Language`, das mit `en` beginnt, wählt die englische Überschrift. Sonst Deutsch. Ein Status 500 zeigt ohne Debug keinen inneren Text. Details mit `password`, `secret`, `token` oder `stack backtrace` werden `Request rejected`. Das Log enthält Request-ID, Code und Status, nicht das Detail.

Own pages can access `fail` with their own `code`. The project templates `templates/errors/404.html` and `500.html` remain the answers of `mount_pages`, not `fail`.

## Zugriff

```rust
async fn account(
    axum::extract::State(state): axum::extract::State<ferrioxid::auth::AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user = match ferrioxid::views::require_user(state.auth.as_ref(), &headers).await {
        Ok(user) => user,
        Err(error) => return error.into_response(),
    };
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"email": user.email}))
}
```

`require_user` calls `AuthService::authenticate` with GET. CSRF is not checked. `require_staff` requires additional active staff. `require_permission` uses `holds`. `require_object_read::<T, P>` uses `ObjectPolicy::can_read`. None of these functions sets Staff or Superuser.

A change is sent to CSRF via `authenticate_csrf`. View helpers do not replace this exam.

## Tests

`redirect_rejects_external_targets`, `responses_keep_problem_json_and_escape_html` and `staff_route_and_request_id_stay_closed` in `views.rs`. HTML is escaped. A 422 without `text/html` remains problem JSON. Staff will be closed without meeting. The request ID is in the response header if the standard middleware is in front of it.
