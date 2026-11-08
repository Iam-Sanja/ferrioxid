# Routing

[Zur Übersicht](index.md) · Namen: [urls.md](urls.md) · **IST-Dateien:** `crates/ferrioxid-api/src/lib.rs`, `crates/ferrioxid-api/src/urls.rs`, `crates/ferrioxid-auth/src/routes.rs`.

## Aufgabe

Der Anwendungscode gruppiert Routen in Rust-Modulen. Ferrioxid bindet sie an denselben `AppState` und dieselbe Middleware wie Admin, Auth und die Modell-API. Es gibt keinen zweiten Router.

## Zustand

`AppState` bleibt `{ db, auth, mail }`. Settings und die Routenkarte sind Axum-Extensions, damit vorhandene `State<AppState>`-Literale gültig bleiben. `with_settings` legt `Extension<Arc<ProjectConfig>>`. `into_router` legt immer `Extension<Arc<RouteMap>>`, auch wenn sie leer ist.

Ein Handler, der Settings liest, braucht deshalb `with_settings`. Ohne diesen Aufruf schlägt der Extractor fehl.

## Reihenfolge

`App::into_router` hängt zuerst die Auth-Routen und danach den Admin an die Anwendungsrouten. `/healthz` kommt zuletzt. `pages::mount_pages` und `web::prepare` liegen außerhalb von `App` und bleiben die Stellen für die Startseite, Staticfiles und die Standard-Middleware. Die Request-ID ist `x-request-id`.

```rust
let router = ferrioxid::pages::mount_pages(app.into_router(), &settings)?;
let router = ferrioxid::web::prepare(router, &settings);
```

## Konflikte

`UrlSet` lehnt reservierte Framework-Pfade ab, bevor der Server startet. Zwei gleiche Namen oder zwei gleiche Pfade in den benannten Routen ebenfalls. `merge` prüft diese Namen nicht. Eine Route, die dort denselben Pfad wie Auth oder Admin belegt, bleibt ein Axum-Konflikt zur Laufzeit des Routers. Neue benannte Routen gehören in `UrlSet`, nicht in einen zweiten Aufbau.

`register` und `register_policy` für dasselbe Modell erzeugen weiterhin doppelte Modellrouten. Das ist der bestehende Vertrag und unabhängig von `UrlSet`.

## Zugriff

Views bekommen keine Admin-Rechte durch die Registrierung. `allow_anonymous`, Modellrechte und `ObjectPolicy` bleiben an `register` und `register_policy`. Eigene Seiten rufen `ferrioxid::views::require_user`, `require_staff`, `require_permission` oder `require_object_read` auf. Eine zweite Auth-Middleware gibt es nicht.

## Unbekannte Pfade

Ein Pfad ohne Route bleibt die Axum-404. `mount_pages` rendert dafür `templates/errors/404.html`, wenn die Vorlagen aktiv sind. `views::fail` mit Status 404 ist die Antwort, die eine View selbst erzeugt.

## Bestehende Projekte

`examples/myshop` und ältere `main.rs` nutzen `App::new`, `register`, `merge` und `discover_project`. Sie müssen `urls` und `with_settings` nicht aufrufen. Die leere Routenkarte wird trotzdem als Extension gesetzt.
