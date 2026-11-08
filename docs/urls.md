# URLs

[Zur Übersicht](index.md) · Einbindung: [routing.md](routing.md) · Views: [views.md](views.md) · **IST-Datei:** `crates/ferrioxid-api/src/urls.rs`.

## Aufgabe

`UrlSet` sammelt Axum-Routen und stabile Namen. Axum bleibt der Router. Eine zweite Routing-Engine gibt es nicht. Namen haben die Form `bereich.name`, zum Beispiel `shop.list` oder `site.home`.

Das entspricht konzeptionell Djangos `urls.py` und `reverse`. Ferrioxid versteckt `Router<AppState>` dabei nicht.

Eine App hängt ihr `UrlSet` über `Registration::routes` ein. Der Namensraum muss der App-Name sein. Ein schon belegter Pfad, auch ein Modellpfad, ist `url.duplicate`. `App::merge` eines rohen Routers sieht diese Namen nicht.

In einer App-Vorlage löst Tera den Namen auf:

```text
<a href="{{ url(name="shop.detail", id=product.id) }}">Produkt</a>
```

Tera kodiert das Ergebnis. Ein Schrägstrich erscheint als `&#x2F;`. In einem `href` bleibt das ein relativer Pfad. Die Funktion ist nicht als HTML-sicher markiert. Fehlt die Route oder ein Parameter, nennt der Fehler nur den angeforderten Namen und nicht die übrigen Routen. Parameter bleiben bei Buchstaben, Ziffern, `-` und `_`.

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

`nest("/catalog", shop)` hängt die Kindpfade unter das Präfix. `/{id}` wird dadurch `/catalog/{id}`. Der Name bleibt `shop.detail`.

Die Anwendung übergibt das Ergebnis an `App::urls`:

```rust
let app = ferrioxid::App::new(db)
    .with_settings(settings.clone())
    .urls(routes()?)?;
```

`merge` bleibt für Routen ohne Namen gültig. Bestehende `main.rs`-Dateien, die nur `register` und `merge` nutzen, kompilieren weiter.

## Namen auflösen

Nach `into_router` liegt die Karte als `Extension<Arc<ferrioxid::urls::RouteMap>>`.

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

`path` ersetzt `{id}` nur durch 1 bis 80 Zeichen aus Buchstaben, Ziffern, `-` und `_`. `..` ist kein gültiger Parameter. Ein unbekannter Name ist `url.unknown`. Ein fehlender oder überzähliger Parameter ist `url.parameter`.

## Regeln

Pfade beginnen mit `/`, sind höchstens 256 Zeichen lang und enthalten kein `//`, `\`, `..`, `://`, `?`, `#`, Leerzeichen oder Steuerzeichen.

Diese Pfade und jeder Pfad unter `/admin` oder `/auth` sind `url.reserved`: `/healthz`, `/auth/login`, `/auth/logout`, `/auth/me`, `/auth/csrf`, `/auth/change-password`, `/auth/password-reset`, `/auth/password-reset/confirm`, `/auth/password-reset/choose`, `/admin`, `/admin/login`.

Ein zweiter Name oder ein zweiter Pfad in derselben `UrlSet` oder beim Zusammenführen in `App::urls` ist `url.duplicate`. `/` bleibt der Startseite von `mount_pages` vorbehalten, wenn Vorlagen aktiv sind. Die Fullstack-Vorlage legt die eigene Seite deshalb auf `/home`.

## Tests

`names_nests_and_reserved_paths_are_rejected` in `urls.rs`.

## Einschränkungen

Tera erhält die Namen nicht automatisch als Globale. Eine Vorlage ruft `reverse` nicht von selbst auf. Der Handler löst den Pfad auf und legt ihn in den Kontext. `require_user` prüft eine GET-Sitzung und kein CSRF. Schreibende Views rufen weiterhin `AuthService::authenticate_csrf` auf.
