# Projektstruktur

[Zur Übersicht](index.md) · Vorlagen: [project-templates.md](project-templates.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · **IST-Datei:** `crates/ferrum-cli/src/scaffold.rs`.

## Aufgabe

`ferrum new` legt ein Projekt außerhalb des Ferrum-Repositories an. Die Cargo-Version der Abhängigkeit bleibt die des Workspaces. Die Template-Version im Manifest ist `0.24`.

## Fullstack

```text
meinprojekt/
├── Cargo.toml
├── ferrum.toml
├── .gitignore
├── .env.example
├── README.md
├── .ferrum/project.json
├── src/main.rs
├── src/lib.rs
├── src/settings.rs
├── src/urls.rs
├── src/views/mod.rs
├── src/views/home.rs
├── src/models/mod.rs
├── src/models/product.rs
├── templates/base.html
├── templates/index.html
├── templates/home.html
├── templates/errors/404.html
├── templates/errors/500.html
├── static/css/app.css
├── static/js/app.js
├── media/.gitkeep
├── migrations/0001_initial.json
└── tests/api.rs
```

`main.rs` lädt `settings::load`, bindet `bind_address` und hängt `.with_settings` sowie `.urls(urls::routes()?)?` an die bestehende Admin-, Auth- und Kontaktfolge. `/` bleibt die eingebaute Startseite. Die benannte Route `site.home` liegt auf `/home` und rendert `templates/home.html`.

## API

Die API-Vorlage hat dieselben Modelle, Migrationen, Staticfiles und HTML-Startvorlagen. Sie hat keinen Admin, keine Auth-Registrierung und kein `src/views/`. `src/urls.rs` registriert `api.ping` auf `/api/v1/ping` und antwortet mit JSON `{"ok": true}`.

Die HTML-Dateien bleiben, weil `templates.enabled` in beiden Vorlagen wahr ist und die bestehende Startseite sie erwartet. Eine API-Vorlage ohne jede HTML-Datei ist nicht der generierte Stand.

## Was bewusst fehlt

Es gibt kein zweites Settings-Modul neben `settings.rs`. Dieses Modul ruft nur `Settings::discover` auf. Es gibt keine zusätzliche Beispieltabelle neben `products`. Kontakt bleibt eine Funktion in `main.rs`, keine eigene View-Datei.

## Legacy

Ein Projekt, dessen `main.rs` `discover_project`, `App::new`, `register` und `merge` nutzt, muss diese Datei nicht umschreiben. `ferrum.toml` ohne `[custom]` bleibt gültig. Vorhandene Migrationen werden nicht verändert. `admin-ui` bleibt ein akzeptiertes Feature.

`ferrum upgrade` ersetzt keine benutzerdefinierte Datei. Liegt die Template-Version nicht auf `0.24`, meldet der Plan `project.template.outdated` und lässt die Dateien stehen. Eine geänderte, im Manifest geführte Datei bleibt `project.file.modified`. Die Anpassung auf `src/settings.rs` und `src/urls.rs` ist manuell, wenn ein altes Projekt sie übernehmen will.

Das Verzeichnis `D:\04_Projekte_Arbeit\Projekte\testprojekt` wird von dieser Vorlage nicht neu erzeugt und nicht verändert. Tests legen eigene temporäre Kopien an.
