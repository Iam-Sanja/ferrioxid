# Validierung – Regeln, Typen und Fehler

[Zur Übersicht](index.md) · **IST:** Regel-Generierung in `crates/ferrum-macros/src/lib.rs`; Fehler in `crates/ferrum-core/src/lib.rs`.

## Aktuelle Regeln (IST)

| Attribut | Typ | Prüfung |
|---|---|---|
| `required` | `String` | `trim().is_empty()` wird abgelehnt |
| `min_length` | `String` | Mindestzahl von `trim().chars().count()` |
| `max_length` | `String` | Höchstzahl von `trim().chars().count()` |
| `min` | `i64` | Feldwert muss ≥ Integer-Literal sein |
| `default = true/false` | `bool` | Fehlendes Feld und JSON-`null` im Create-DTO werden zum Default, zusätzlich SQL `DEFAULT` |
| `readonly` | jedes unterstützte Feld | Create setzt den Wert; PATCH kann ihn nicht senden |
| `unique` | jedes unterstützte Feld außer dem Primärschlüssel | Datenbank-Unique-Index; Duplikat wird 422 |
| `unique_together` | mindestens zwei Felder am Modell | zusammengesetzter Unique Index |
| `index`, `index_together` | Feld oder Feldgruppe | sekundärer Index, keine Wertprüfung |
| `foreign_key`, `on_delete` | eine Spalte | Datenbank-Foreign-Key; ungültige Referenz und `restrict` werden 422 |
| `Option<T>` | `String`, `i64`, `bool`, `Uuid` | Create: fehlend oder `null` speichert SQL `NULL`. PATCH: siehe `PatchField` |

Keine allgemeine Validator-Registry, keine Regex- und keine E-Mail-Prüfung. Rust prüft Länge, Pflicht und `min` in `create` und `update`. Unique und Foreign Key prüft die Datenbank. Die Repository-Schicht übersetzt die sqlx-Fehlerart in `Validation`. Mehrere `NULL` bleiben in einem Unique Index erlaubt. Details stehen in [constraints.md](constraints.md).

### PATCH-Null-Semantik

`ferrum::db::PatchField<T>` steht auf jedem änderbaren Update-Feld, mit `#[serde(default)]`.

| JSON | Zustand | `NOT NULL`-Spalte | nullable Spalte (`Option<T>`) |
|---|---|---|---|
| Schlüssel fehlt | `Missing` | bisheriger Wert bleibt | bisheriger Wert bleibt |
| `null` | `Null` | HTTP 422, Detail `{feld} cannot be null` | SQL `NULL` |
| Wert | `Value` | Validierung, dann Originalwert speichern | Validierung, dann Originalwert speichern |

`{}` ist der Fall, in dem jedes Feld `Missing` ist. Die Antwort ist HTTP 200 mit dem unveränderten Datensatz, sofern die UUID existiert. Strings werden nicht getrimmt gespeichert. `required` lehnt im `Value`-Arm leere und reine Whitespace-Texte ab.

## Beispiel-Testfälle (IST)

```json
{"name":"   ","price_cents":10}
{"name":"ok","price_cents":10}
{"name":"Laptop","price_cents":-1}
```

Für das aktuelle `Product` sind die drei Create-Körper 422, weil Blank, Name kürzer als 3 beziehungsweise negativer Preis. Im Create darf `active` fehlen; `active: null` wird dort ebenfalls zu `true`. Im PATCH ist `{"active": null}` dagegen 422, weil die Spalte `NOT NULL` ist. Ein String mit führenden oder folgenden Leerzeichen wird geprüft, aber nicht getrimmt gespeichert.

## SOLL: Gemeinsame Validator-Schnittstelle

1. Deklarative Feldregeln vom Makro in neutralen Metadaten ablegen.
2. Compile-Time-Typprüfungen (z. B. `min` nur auf Zahlen).
3. Laufzeitprüfungen für create/update mit **klarer PATCH-Semantik**: nicht vorhanden vs. explizit `null` vs. Wert.
4. Einheitliche strukturierte Fehler mit Feldpfaden und Maschinen-Codes.
5. Datenbank-Unique-Constraints als endgültige Autorität: Prechecks sind kein Schutz gegen Race Conditions.
6. Cross-Field-Validatoren erst nach Festlegung der Reihenfolge und Transaktionsstrategie.
7. Serde-Parsingfehler sauber von Domain-Validierungsfehlern unterscheiden.

## Testmatrix

`required` ohne `min_length`; Unicode, Leerzeichen, leere Strings; bool-Default DDL/DTO; Typkombinationen Compile-Fail; SQL-Constraints. Zu `422` siehe [api.md](api.md).
