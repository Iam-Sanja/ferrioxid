# Apps

[Zur Übersicht](index.md) · Registry: [app-registry.md](app-registry.md) · Migrationen: [app-migrations.md](app-migrations.md) · Wiederverwendung: [reusable-apps.md](reusable-apps.md) · Generator: [create-app.md](create-app.md)

**IST.** Funktionsstand v0.25. Die Cargo-Version bleibt `0.1.0`. Die Template-Version neuer Projekte bleibt `0.24`. Eine App ist ein Rust-Typ, der Modelle, Routen, Admin, Migrationen, Vorlagen und Staticfiles an die vorhandenen Engines übergibt. Es gibt keine zweite Router-, Auth-, Model- oder Admin-Engine und kein Nachladen unbekannter Crates.

## Django und Ferrioxid

| Django | Ferrioxid |
|---|---|
| `INSTALLED_APPS` | `App::install` mit einem Typ, der `FerrioxidApplication` implementiert |
| `AppConfig` | `AppConfig::new("shop").label(...).prefix(...).depends_on(...)` |
| `apps.py` und dynamischer Import | Rust-Code im Binary. `ferrum.toml` lädt keinen Code |
| `shop.Product` und `inventory.Product` | getrennte Tabellennamen. Ein App-Name ändert weder SQL noch API-Pfade |
| `migrations/` je App | dasselbe Verzeichnis, Historien-ID `shop:0001_initial` |
| `{% url "shop:product_detail" id=product.id %}` | `{{ url(name="shop.detail", id=product.id) }}` |
| App-Templates | Dateien unter `{app}/`, zum Beispiel `shop/list.html` |
| App-Staticfiles | `/static/{app}/...` über die vorhandene Staticfiles-Auslieferung |

## Registrieren

```rust
use axum::routing::get;
use ferrioxid::apps::{AppConfig, AppError, FerrioxidApplication, Registration};
use ferrioxid::urls::{UrlError, UrlSet};

pub struct ShopApp;

impl FerrioxidApplication for ShopApp {
    fn config(&self) -> AppConfig {
        AppConfig::new("shop").label("Shop").prefix("/shop")
    }

    fn contribute(&self, registry: &mut Registration<'_>) -> Result<(), AppError> {
        registry.routes(routes()?)?;
        Ok(())
    }
}

pub fn routes() -> Result<UrlSet, UrlError> {
    UrlSet::new().route("shop.list", "/items", get(list))
}

async fn list() -> axum::response::Response {
    ferrioxid::views::json(&ferrioxid::serde_json::json!({"app": "shop"}))
}
```

```rust
let app = ferrioxid::App::new(database)
    .install(ShopApp)?
    .install(BlogApp)?
    .seal()?;
let router = app.into_router();
```

`install` prüft den Namen, lehnt Duplikate und reservierte Namen ab und ruft `contribute` auf. `seal` prüft den Migrationsgraphen, die Übereinstimmung mit `[app.<name>]` und, falls vorhanden, `AdminRegistry::seal`. `seal` wendet keine Migration an. `try_into_router` führt diese Prüfung aus, wenn sie noch fehlt. `into_router` bleibt ohne `Result` und startet bei einem Fehler nicht. Ohne Apps und ohne `[app.<name>]` ändert sich der Start nicht. Der Lebenszyklus steht in [app-lifecycle.md](app-lifecycle.md).

Ohne `install` bleiben `App::new`, `register`, `urls` und `merge` gültig. `FerrioxidApplication::register_jobs` hat eine leere Standardmethode. `install`, `seal` und `into_router` rufen sie nicht auf und starten keinen Worker. Das Worker-Binary der Anwendung ruft `register_jobs` auf derselben `JobRegistry` auf. Ein Beispiel ist `examples/notes`. Siehe [jobs.md](jobs.md) und [workers.md](workers.md).

## Was eine App anmelden kann

- `routes`: ein `UrlSet`. Der Namensraum vor dem Punkt muss der App-Name sein. Ein Präfix wird davorgeschrieben. `/` wird zum Präfix selbst.
- `model`: das bestehende `FerrioxidModel`-Repository. Eine schon belegte Tabelle ist `app.model.duplicate`.
- `admin_model`, `admin_view`, `admin_action`, `admin_filter`, `admin_widget`: die vorhandene `AdminRegistry`.
- `migrations`: ein relatives Verzeichnis ohne `..`.
- `templates`: HTML unter `{app}/`. `TemplateMode::Isolated` lehnt eine gleichnamige Projektdatei ab. `ProjectOverride` liest sie stattdessen.
- `static_files`: Dateien unter `/static/{app}`.
- `project`: die `ProjectConfig` aus `with_settings`. Eigene Werte kommen aus `[custom.shop]` über `custom_settings`.

Die Registrierung vergibt kein Staff, keinen Superuser und kein Modellrecht.

## Namen

Ein App-Name beginnt mit einem Kleinbuchstaben und enthält danach nur Kleinbuchstaben, Ziffern und `_`. Höchstens 32 Zeichen. `admin`, `auth`, `api`, `static`, `healthz` und `uploads` sind reserviert.

Ein URL-Präfix darf nicht `/admin` oder `/auth` sein und darf nicht aus dem Pfad ausbrechen.

## Lebenszyklus

1. `App::new` und optional `with_settings` sowie `with_admin`.
2. `install` je App, in der gewünschten Reihenfolge.
3. `contribute` meldet Modelle, Routen und die übrigen Bestandteile.
4. Konflikte bei Namen, Pfaden, Tabellen und Vorlagen beenden `install`.
5. `seal` prüft Abhängigkeiten und die Admin-Registry. Die Registry nimmt danach keine weitere App an.
6. `ferrioxid migrate up` wendet ausstehende Migrationen an. Das ist ein eigener Schritt.
7. `into_router` hängt Staticfiles, die Vorlagen und die `RouteMap` an.
8. Der Server startet.

`with_admin` ersetzt die Admin-Registry. Apps, die Admin-Erweiterungen anmelden, kommen danach oder die Erweiterungen werden nach `with_admin` erneut angemeldet.
