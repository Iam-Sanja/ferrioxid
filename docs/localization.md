# Lokalisierung im Admin

[Zur Übersicht](index.md) · Architektur: [i18n.md](i18n.md)

## Formate

`formatNumber`, `formatDecimal`, `formatDate`, `formatTime`, `formatDateTime` und `formatCurrency` liegen in `apps/admin-ui/src/i18n/format.ts`. Deutsch nutzt `de-DE`, Englisch `en-US`.

Die Währung kommt aus `branding.currency` und nie aus der Sprache. Der Default ist `EUR`. Felder, deren Name auf `_cents` endet, werden aus der ganzen Cent-Zahl angezeigt. Der API-Wert bleibt diese ganze Zahl. Andere Integer-Felder werden nur als Zahl formatiert. ISO-Texte in Feldern auf `_at` werden als Datum mit Uhrzeit gezeigt.

## Sprache der Startseite

`negotiate_locale` in `crates/ferrum-api/src/pages.rs` prüft in dieser Reihenfolge:

1. `?lang=` nur wenn der Wert `de` oder `en` ist, optional mit Region wie `de-DE`.
2. Den ersten unterstützten Eintrag von `Accept-Language`, gewichtet mit `q`.
3. `project.default_locale`.
4. `project.fallback_locale`, sonst `en`.

Der Wert wird in den Tera-Kontext als `locale` geschrieben. Er wird nicht an einen Dateipfad gehängt. Tera escaped die eingesetzten Texte. Generierte Projekte verlinken `/ ?lang=de` und `/?lang=en` fest.

## CLI

`FERRUM_LOCALE=en` oder `de` überschreibt `default_locale`. Übersetzt sind die Meldungen für erzeugtes Projekt, erzeugtes Modell, Start und Stopp des Entwicklungsservers und der Hinweis auf fehlende Admin-Assets. `ferrum dev` gibt die Zeile `Ferrum lauscht` des generierten Programms unverändert aus. Migrationsstatus, `Angewendet` und `Keine Schemaänderung` bleiben unverändert.

## Tests

`apps/admin-ui/src/admin.test.tsx` vergleicht die Katalogschlüssel, Interpolation, Währung, Datum, den Sprachwechsel und eine abgelehnte Farbe. `crates/ferrum-api/src/pages.rs` prüft den englischen Fallback und eine ungültige Locale.
