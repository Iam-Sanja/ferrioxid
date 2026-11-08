# Views

[Zur Übersicht](index.md) · Routen: [urls.md](urls.md) · **IST-Datei:** `crates/ferrioxid-api/src/views.rs`.

## Aufgabe

`ferrioxid::views` liefert die Antworten, die eine Anwendung häufig braucht: HTML, JSON, einen lokalen Redirect und stabile Fehler. Die Typen bleiben Axum-`Response` und `HeaderMap`. Tera bleibt die Vorlagen-Engine.

`require_mutation(auth, headers, token)` prüft eine POST-Anfrage über `AuthService::authenticate_csrf`. Fehlt der Auth-Dienst, ist die Antwort `401` und `auth.not_configured`. Es gibt keinen zweiten CSRF-Mechanismus. `require_user` bleibt eine GET-Prüfung. Die App-Registrierung vergibt dadurch keine Berechtigung.

## Django und Ferrioxid

| Django | Ferrioxid |
|---|---|
| `render(request, "home.html", context)` | `views::render(site, "home.html", context, debug)` |
| `JsonResponse({"ok": True})` | `views::json(&ferrioxid::serde_json::json!({"ok": true}))` |
| `redirect("/home")` | `views::redirect_to("/home")` |
| `Http404` | `views::fail(..., StatusCode::NOT_FOUND, code, detail, debug)` |
| `PermissionDenied` | `views::fail(..., StatusCode::FORBIDDEN, ...)` oder `require_staff` |
| `login_required` | `require_user` für eine vorhandene Sitzung |
| `staff_member_required` | `require_staff` |
| `permission_required("app.view")` | `require_permission(..., "app.view")` |
| `get_object_or_404` plus Policy | vorhandenes Repository und `require_object_read::<T, P>` |

Die Entsprechung ist die Aufgabe, nicht die Syntax. Eine Ferrioxid-View ist eine Axum-Funktion. Sie erhält den Zustand als Argument und gibt `Response` zurück.

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

Schlägt das Rendern fehl, antwortet `render` mit `view.template`. In Produktion und bei `debug = false` lautet der Text `Internal server error`. Die Vorlage selbst wird nicht in die Antwort kopiert, wenn der Fehler ein Secret, ein Token oder einen Stacktrace nennt.

## JSON, Redirect, Formular

```rust
fn answers() -> Result<axum::response::Response, ferrioxid::views::ViewError> {
    let _response = ferrioxid::views::json(&ferrioxid::serde_json::json!({"ok": true}));
    let target = ferrioxid::views::redirect_to("/home")?;
    let _fields = ferrioxid::views::form_fields("name=Ada&city=Bern")?;
    Ok(target)
}
```

`json` nimmt `&serde_json::Value`. `redirect_to` erlaubt einen einzelnen lokalen Pfad: er beginnt mit `/`, nicht mit `//`, und enthält kein Schema, `\`, `..` oder Steuerzeichen. `https://…` ist `view.redirect` mit Status 400. `form_fields` liest `application/x-www-form-urlencoded` bis 8 KiB. Ein größeres oder fehlerhaftes Paar ist `request.rejected`.

## Fehler

`fail` und `ViewError` nutzen dieselben Kategorien:

| Status | Schlüssel der HTML-Überschrift |
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

Eigene Seiten können `fail` mit einem eigenen `code` aufrufen. Die Projektvorlagen `templates/errors/404.html` und `500.html` bleiben die Antworten von `mount_pages`, nicht von `fail`.

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

`require_user` ruft `AuthService::authenticate` mit GET auf. CSRF wird dabei nicht geprüft. `require_staff` verlangt zusätzlich aktives Staff. `require_permission` nutzt `holds`. `require_object_read::<T, P>` nutzt `ObjectPolicy::can_read`. Keine dieser Funktionen setzt Staff oder Superuser.

Eine Änderung sendet CSRF weiter über `authenticate_csrf`. Die View-Helfer ersetzen diese Prüfung nicht.

## Tests

`redirect_rejects_external_targets`, `responses_keep_problem_json_and_escape_html` und `staff_route_and_request_id_stay_closed` in `views.rs`. HTML wird escaped. Ein 422 ohne `text/html` bleibt Problem-JSON. Ohne Sitzung bleibt Staff geschlossen. Die Request-ID steht im Antwort-Header, wenn die Standard-Middleware davor liegt.
