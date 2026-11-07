# Formularvalidierung

**Stand:** v0.19, IST. Die Cargo-Version bleibt `0.1.0`. Zusätzliche Codes: `validation.date`, `validation.time`, `validation.datetime`, `validation.datetime_offset`.

## Verantwortlichkeit

`ferrum-core::validation` ist der gemeinsame Regelkern für Formularfelder. `FerrumModel` behält die bisher generierten Prüfungen, damit bestehende CRUD-Antworten unverändert bleiben.

## Crates und Dateien

- `crates/ferrum-core/src/validation.rs`
- Aufrufer: `crates/ferrum-forms/src/lib.rs`
- Modellmakro, unverändert: `crates/ferrum-macros/src/lib.rs`

## Öffentliche API

`text_code`, `parse_i64`, `parse_bool`, `parse_uuid`, `is_email` und `is_http_url`. Rückgaben sind stabile Codes, keine übersetzten Sätze. Übersetzung macht `ferrum::forms::message`.

## Datenfluss

```text
FerrumModel-Makro → eigene Meldungen (unverändert)
ferrum-forms → text_code / parse_* → Feldcode → message(locale, code)
```

Optionale Textfelder behandeln einen leeren Wert als `None`, bevor die Pflichtregel greift. Zahlen und UUIDs ohne Wert bleiben `validation.required`, wenn die Required-Hilfe verwendet wird.

## Konfiguration

Keine. Grenzen stehen an der Aufrufstelle, zum Beispiel `TextSpec.min_length`.

## Sicherheit

Regeln sehen nur den bereits dekodierten Feldwert. Sie führen keine SQL- oder Shell-Befehle aus. E-Mail- und URL-Prüfungen sind bewusst eng: ASCII, ein `@`, eine Domain mit Punkt, `http` oder `https`.

## Fehlerfälle

| Code | Bedeutung |
|---|---|
| `validation.required` | leerer Pflichtwert |
| `validation.min_length` | zu kurz |
| `validation.max_length` | zu lang |
| `validation.min_value` | Zahl zu klein |
| `validation.max_value` | Zahl zu groß |
| `validation.email` | keine einfache E-Mail |
| `validation.url` | kein `http`- oder `https`-URL |
| `validation.invalid_type` | keine Zahl oder kein Bool |
| `validation.uuid` | keine UUID |

## Beispiel

```rust
ferrum::core::text_code("a@b", true, None, Some(254), true, false);
```

## Tests

`ferrum-core` prüft die Codes. `ferrum-forms` prüft die Zuordnung zu Feldern.

## Einschränkungen

Das Modellmakro dupliziert die Regeln noch. Internationale E-Mail-Adressen und freie URL-Schemata werden abgelehnt.

## Erweiterung

Das Makro kann später `text_code` aufrufen, ohne die öffentlichen Fehlertexte der REST-API zu ändern.
