# Nativer Admin

[Zur Übersicht](index.md) · Routen: [admin-routing.md](admin-routing.md) · Vorlagen: [admin-templates.md](admin-templates.md) · Gestaltung: [admin-design-system.md](admin-design-system.md)

**Stand:** v0.13.1. Die Cargo-Version bleibt `0.1.0`. Die Oberfläche ist serverseitiges HTML. React, Vite und Node.js werden dafür nicht gebraucht. Die Login-Origins folgen der Bind-Adresse aus `ferrum.toml` oder `BIND_ADDRESS`.

## Aufgabe

Der Admin zeigt registrierte Modelle, Formulare und das Benutzerverzeichnis. Der Browser sendet normale GET- und POST-Formulare. JavaScript in `admin.js` setzt nur den Fokus auf das erste ungültige Feld. Listen, Anlegen, Ändern, Löschen, Suche, Filter, Sortierung, Blättern, Login und Logout funktionieren ohne Script.

## Aufbau

```text
Browser
  → Axum-Route unter /admin
  → Sitzung, Staff, CSRF, Modellrecht, Query-Scope
  → AdminRegistry und ManagedModel
  → Tera
  → HTML
```

Die JSON-API unter `/admin/api` bleibt. Sie nutzt dieselben Modelle, Rechte und Scopes. HTML-Routen ersetzen sie nicht und überschreiben sie nicht.

## Zuständige Dateien

| Bereich | Datei |
|---|---|
| Routen und Formulare | `crates/ferrum-admin/src/html.rs` |
| JSON-API und gemeinsame Prüfungen | `crates/ferrum-admin/src/http.rs` |
| Vorlagen | `crates/ferrum-admin/src/rendering.rs`, `crates/ferrum-admin/templates/` |
| Texte | `crates/ferrum-admin/src/localization.rs` |
| CSS und Script | `crates/ferrum-admin/static/css/admin.css`, `crates/ferrum-admin/static/js/admin.js` |
| Auslieferung | `crates/ferrum-admin/src/assets.rs` |
| Modelle | `crates/ferrum-admin/src/registry.rs` |

`build.rs` ruft kein npm auf. CSS, Script und Vorlagen stecken über `include_str!` im Binary. Das Cargo-Feature `admin-ui` bleibt als leeres Kompatibilitätsflag erhalten. Es verlangt kein Frontend-Build mehr.

## Branding

`AdminBranding` liest `ferrum.toml`. Titel, Kopfzeile und Anwendungsname dürfen kein HTML enthalten. `primary_color` ist `#rgb` oder `#rrggbb`. Sprachen sind `de` und `en`. Die Währung ist ein Code aus drei Großbuchstaben. Die Farbe landet als CSS-Variable `--brand`. Tera escaped den Wert. Der Filter `safe` wird für Datenbankinhalte nicht verwendet.

## Sprache und Darstellung

`ferrum_locale` und `ferrum_theme` sind HttpOnly-Cookies. Sie entstehen über `POST /admin/preferences`. Erlaubte Sprachen sind `de` und `en`. Erlaubte Themes sind `light`, `dark` und `system`. `system` folgt `prefers-color-scheme`. Ohne Cookie gilt die Projektsprache und `system`. `next` muss mit `/admin` beginnen und darf kein Schema und keinen Host enthalten.

## Grenzen

Es gibt keine erfundenen Kennzahlen auf der Übersicht. Fremdschlüssel-Auswahlen laden höchstens 100 sichtbare Zeilen. Die HTML-Liste zeigt 25 Zeilen. Passwort-Hashes und sensible Feldnamen werden nicht ausgegeben. Ein Datensatz außerhalb der Objektregel ist 404, nicht 403.
