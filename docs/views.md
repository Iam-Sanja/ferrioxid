# Views

[Overview](index.md) · Routes: [urls.md](urls.md) · **actual file:** `crates/fexu-api/src/views.rs`.

## Purpose

`fexu::views` provides the answers that an application often needs: HTML, JSON, a local redirect and stable errors. The types remain Axum-`Response` and `HeaderMap`. Tera remains the template engine.

`require_mutation(auth, headers, token)` checks a POST request via `AuthService::authenticate_csrf`. If the Auth service is missing, the answer is `401` and `auth.not_configured`. Anonymous forms use the same HMAC through `authenticate_browser_form`. That helper is separate because it does not return a user. `require_user` remains a GET test. The app registration does not grant any permission.

## Django and Fexu

| Django | Fexu |
|---|---|
| `render(request, "home.html", context)` | `views::render(site, "home.html", context, debug)` |
| `JsonResponse({"ok": True})` | `views::json(&fexu::serde_json::json!({"ok": true}))` |
| `redirect("/home")` | `views::redirect_to("/home")` |
| `Http404` | `views::fail(..., StatusCode::NOT_FOUND, code, detail, debug)`. HTML uses `templates/errors/404.html` when `mount_pages` installed the project templates. |
| `PermissionDenied` |`views::fail(..., StatusCode::FORBIDDEN, ...)` or `require_staff`|
| `login_required` |`require_user` for an existing session|
| `staff_member_required` | `require_staff` |
| `permission_required("app.view")` | `require_permission(..., "app.view")` |
| `get_object_or_404` plus Policy |Existing repository and `require_object_read::<T, P>`|

The correspondence is the task, not the syntax. A Fexu view is an axum function. It gets the state as an argument and returns `Response`.

## HTML

```rust
use std::sync::Arc;

use axum::extract::Extension;
use fexu::migrations::ProjectConfig;

pub async fn home(
    Extension(settings): Extension<Arc<ProjectConfig>>,
) -> axum::response::Response {
    let site = match fexu::pages::Site::open(&settings.templates_directory()) {
        Ok(site) => site,
        Err(_) => {
            return fexu::views::fail(
                &axum::http::HeaderMap::new(),
                None,
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "view.template",
                "Internal server error",
                settings.debug,
            )
        }
    };
    let context = fexu::pages::home_context(&settings);
    fexu::views::render(&site, "home.html", &context, settings.debug)
}
```

If the rendering fails, `render` responds with `view.template`. In production and at `debug = false` the text is `Internal server error`. The template itself is not copied into the response if the error calls a secret, a token or a stacktrace.

## JSON, redirects and forms

```rust
fn answers() -> Result<axum::response::Response, fexu::views::ViewError> {
    let _response = fexu::views::json(&fexu::serde_json::json!({"ok": true}));
    let target = fexu::views::redirect_to("/home")?;
    let _fields = fexu::views::form_fields("name=Ada&city=Bern")?;
    Ok(target)
}
```

`json` accepts `&serde_json::Value`. `redirect_to` permits local paths beginning with a single `/` and containing no URL scheme, `\`, `..` or control characters. An external URL such as `https://…` returns `view.redirect` with status 400. `form_fields` reads at most 8 KiB of `application/x-www-form-urlencoded` data; oversized or malformed input returns `request.rejected`.

## Errors

`fail` and `ViewError` use the same categories:

| Status |Key of the HTML heading|
|---|---|
| 400 | `bad_request` |
| 401 | `unauthorized` |
| 403 | `forbidden` |
| 404 | `not_found` |
| 409 | `conflict` |
| 405 | `method_not_allowed` |
| 422 | `validation` |
| 500 | `server_error` |

If `Accept` contains `text/html` and not `application/json`, the response is HTML. Otherwise, it remains `problem_json` with `type`, `title`, `status`, `detail` and `code`. For `validation`, the detail is `field:{name}: {detail}` so existing field-error classification continues to work.

An `Accept-Language` value beginning with `en` selects the English heading; otherwise the heading is German. Without debug mode, a 500 response hides internal details. Details containing `password`, `secret`, `token` or `stack backtrace` become `Request rejected`. Logs include the request ID, code and status, not the detail.

`fail` builds the safe HTML or JSON answer. `fail_in` uses the same answer and, for HTML, first tries `templates/errors/{status}.html` for 400, 401, 403, 404, 405, 409, 422 and 500. A missing template stays on the original status and uses the built-in page. The built-in page and the template context contain the localized title and detail, not a stack trace, SQL text or the template error chain. `mount_pages` uses the same 404 and 405 pages. JSON requests keep `problem_json`, the status and `application/json`.

## Access

```rust
async fn account(
    axum::extract::State(state): axum::extract::State<fexu::auth::AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user = match fexu::views::require_user(state.auth.as_ref(), &headers).await {
        Ok(user) => user,
        Err(error) => return error.into_response(),
    };
    fexu::views::json(&fexu::serde_json::json!({"email": user.email}))
}
```

`require_user` calls `AuthService::authenticate` with GET. CSRF is not checked. `require_staff` requires additional active staff. `require_permission` uses `holds`. `require_object_read::<T, P>` uses `ObjectPolicy::can_read`. None of these functions sets Staff or Superuser.

State-changing requests must pass `authenticate_csrf`. View helpers do not replace that check.

## Lists, notices and flash messages

`page_window` accepts pages from 1 and a size from 1 to 100. `pagination_href` stays on one local path and rejects `scope`, `csrf` and `token` query keys. `require_scope` rejects `QueryScope::None` before a read. `scoped_list` and `scoped_get` pass that scope into the existing query functions. They do not build a scope from the request.

`create_flash_table` creates `_fexu_flashes`. `push_flash` stores one line, at most 240 characters, for a session id. `take_flashes` returns those rows and deletes them in the same transaction. A second take is empty. `render_flashes` escapes the text. `fexu dev` does not create this table.

```rust
fexu::views::push_flash(&db, user.session_id, fexu::views::FlashLevel::Success, "Saved").await?;
let messages = fexu::views::take_flashes(&db, user.session_id).await?;
let html = fexu::views::render_flashes(&messages);
```

## Tests

`redirect_rejects_external_targets`, `responses_keep_problem_json_and_escape_html` and `staff_route_and_request_id_stay_closed` in `views.rs` cover redirects, responses and staff access. HTML is escaped. A 422 response without `text/html` remains problem JSON. Staff routes reject requests without a session. The response includes a request-ID header when standard middleware is installed.
