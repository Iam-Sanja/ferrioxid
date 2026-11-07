# Lokalisierung im Admin

[Zur Übersicht](index.md) · Architektur: [i18n.md](i18n.md)

## Formate

Der HTML-Admin formatiert Zahlen in `display_cell`. Deutsch gruppiert mit Punkt, Englisch mit Komma. Felder auf `_cents` werden mit der Projektwährung und zwei Nachkommastellen gezeigt. Der gespeicherte Wert bleibt die ganze Cent-Zahl. Datumswerte aus der Datenbank bleiben der ausgegebene Text. Es gibt keinen `Intl`-Aufruf im Server.

## Sprache der Startseite

`negotiate_locale` in `crates/ferrum-api/src/pages.rs` prüft in dieser Reihenfolge:

1. `?lang=` nur wenn der Wert `de` oder `en` ist, optional mit Region wie `de-DE`.
2. Den ersten unterstützten Eintrag von `Accept-Language`, gewichtet mit `q`.
3. `project.default_locale`.
4. `project.fallback_locale`, sonst `en`.

Der Wert wird in den Tera-Kontext als `locale` geschrieben. Er wird nicht an einen Dateipfad gehängt. Tera escaped die eingesetzten Texte. Generierte Projekte verlinken `/ ?lang=de` und `/?lang=en` fest.

## CLI

`FERRUM_LOCALE=en` oder `de` überschreibt `default_locale`. Übersetzt sind die Meldungen für erzeugtes Projekt, erzeugtes Modell und Start und Stopp des Entwicklungsservers. `ferrum dev` gibt die Zeile `Ferrum lauscht` des generierten Programms unverändert aus. Migrationsstatus, `Angewendet` und `Keine Schemaänderung` bleiben unverändert.

## Tests

`examples/myshop/tests/admin_html.rs` prüft die deutsche Anmeldung, `Accept-Language: en` und den Sprachwechsel auf Englisch. `crates/ferrum-api/src/pages.rs` prüft den englischen Fallback und eine ungültige Locale. Abgelehnte Branding-Farben prüft die Registry in `ferrum-admin`.
