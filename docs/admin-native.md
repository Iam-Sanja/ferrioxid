# Nativer Admin

[Zur Übersicht](index.md) · Routen: [admin-routing.md](admin-routing.md) · Vorlagen: [admin-templates.md](admin-templates.md) · Gestaltung: [admin-design-system.md](admin-design-system.md)

**Stand:** v0.22. Die Cargo-Version bleibt `0.1.0`. Eigene Seiten liegen unter `/admin/pages/{key}` und nutzen dasselbe Layout. Die Oberfläche ist serverseitiges HTML. React, Vite und Node.js werden dafür nicht gebraucht. Die Login-Origins folgen der Bind-Adresse aus `ferrum.toml` oder `BIND_ADDRESS`. Formularentwürfe, Has-Many-Inlines, das gemeinsame Eltern-Kind-Formular, Sammelaktionen, die Historie und die Datumshierarchie laufen ohne JavaScript. `admin.js` ist `defer` und für das Absenden nicht erforderlich. Datumsfelder nutzen `type="date"`, `type="time"` und `type="datetime-local"` mit UTC-Hinweis.

## Aufgabe

Der Admin zeigt registrierte Modelle, Formulare und das Benutzerverzeichnis. Der Browser sendet normale GET- und POST-Formulare. JavaScript in `admin.js` setzt nur den Fokus auf das erste ungültige Feld. Listen, Anlegen, Ändern, Löschen, Suche, Filter, Sortierung, Blättern, Login und Logout funktionieren ohne Script, soweit die Seite sie überhaupt anzeigt.

Ein Nicht-Staff sieht eine 403-Seite ohne Seitenleiste und mit einem Abmeldeformular. Die Produktentscheidung bleibt: der Standard-Admin wird nicht wieder ein React-Build. Weitere Django-Funktionen sind Funktionsziele, keine Codekompatibilität: [admin-django-parity.md](admin-django-parity.md).

## Listenspalten

`listed_fields` in `crates/ferrioxid-admin/src/html.rs` wählt die Spalten:

1. Gibt es außer `id` mindestens ein Feld mit `#[admin(list_display)]`, gelten alle `list_display`-Felder in der Struct-Reihenfolge. `id` ist dabei, weil das Makro es immer als Listenspalte setzt.
2. Sonst das erste sichtbare Textfeld, danach `id`.
3. Sonst das erste andere sichtbare Feld, danach `id`.
4. Sonst nur `id`.

`hidden` und sensible Namen (`password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash`) fallen weg. Ein Modell muss kein Feld `name` haben. Schreibgeschützte Felder bleiben sichtbar. Außer `id` tragen sie den Hinweis „Nur lesen“. Sortierlinks gibt es nur für `sortable` und für `id`.

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
| Routen und Formulare | `crates/ferrioxid-admin/src/html.rs` |
| JSON-API und gemeinsame Prüfungen | `crates/ferrioxid-admin/src/http.rs` |
| Vorlagen | `crates/ferrioxid-admin/src/rendering.rs`, `crates/ferrioxid-admin/templates/` |
| Texte | `crates/ferrioxid-admin/src/localization.rs` |
| CSS und Script | `crates/ferrioxid-admin/static/css/admin.css`, `crates/ferrioxid-admin/static/js/admin.js` |
| Auslieferung | `crates/ferrioxid-admin/src/assets.rs` |
| Modelle | `crates/ferrioxid-admin/src/registry.rs` |

`build.rs` ruft kein npm auf. CSS, Script und Vorlagen stecken über `include_str!` im Binary. Das Cargo-Feature `admin-ui` bleibt als leeres Kompatibilitätsflag erhalten. Es verlangt kein Frontend-Build mehr.

## Branding

`AdminBranding` liest `ferrum.toml`. Titel, Kopfzeile und Anwendungsname dürfen kein HTML enthalten. `primary_color` ist `#rgb` oder `#rrggbb`. Sprachen sind `de` und `en`. Die Währung ist ein Code aus drei Großbuchstaben. Die HTML-Fläche setzt `--brand` nur, wenn weiße Schrift darauf mindestens 4,5:1 erreicht. Sonst bleibt `#0f172a`. Die dunkle Tür nutzt `--brand` nicht. Tera escaped den Wert. Der Filter `safe` wird für Datenbankinhalte nicht verwendet. Die JSON-API gibt die konfigurierte Farbe weiter.

## Sprache und Darstellung

`ferrioxid_locale` und `ferrioxid_theme` sind HttpOnly-Cookies. Sie entstehen über `POST /admin/preferences`. Erlaubte Sprachen sind `de` und `en`. Erlaubte Themes sind `light`, `dark` und `system`. `system` folgt `prefers-color-scheme`. Ohne Cookie gilt die Projektsprache und `system`. `next` muss mit `/admin` beginnen und darf kein Schema und keinen Host enthalten.

## Grenzen

Es gibt keine erfundenen Kennzahlen auf der Übersicht. Fremdschlüssel-Auswahlen laden höchstens 100 sichtbare Zeilen. Die HTML-Liste zeigt 25 Zeilen. Passwort-Hashes und sensible Feldnamen werden nicht ausgegeben. Ein Datensatz außerhalb der Objektregel ist 404, nicht 403.

## Fremdschlüssel und Filter

`display_column` wählt den Anzeigetext eines Zielmodells: das erste sichtbare Textfeld mit `list_display`, sonst ein durchsuchbares Textfeld, sonst das erste andere sichtbare Textfeld. `id`, verborgene und sensible Spalten scheiden aus. Ein Modell braucht kein Feld `name`.

Listen und Details lösen die IDs der aktuellen Zeilen mit `query_labels` auf. Pro Zieltabelle und Seite ist das eine Abfrage je 100 IDs, mit gebundenen Parametern und dem Query-Scope des Ziels. Fehlt das Zielmodell, die Leseberechtigung oder ein Textfeld, erscheint kein Name. Eine leere Referenz ist ein Gedankenstrich. Ein Superuser sieht bei einer unbekannten ID „Nicht vorhanden“. Jeder andere sieht „Nicht sichtbar“, ohne dass eine zweite Abfrage prüft, ob die Zeile existiert.

Bool-Filter sind `Alle`, `Ja` und `Nein`. Fremdschlüssel-Filter sind dieselbe begrenzte, berechtigte Auswahl. Zahlenfilter sind `type="number"`. Andere Typen bleiben ein Textfeld. Der Filterparser bleibt serverseitig. Sortierung bleibt als verborgenes Feld erhalten. Eine neue Filterung setzt die Seite zurück.
