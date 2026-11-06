# Changelog

## Unreleased — CRUD mit PATCH und DELETE

- Jedes registrierte Modell erhält `PATCH /{id}` und `DELETE /{id}` zusätzlich zu Liste, Einzelabruf und POST.
- Das Makro erzeugt `{Model}Update`. Jedes änderbare Feld ist ein `PatchField<T>` mit den Zuständen `Missing`, `Null` und `Value`.
- Fehlende PATCH-Schlüssel behalten den gespeicherten Wert. `{}` liefert den Datensatz unverändert mit HTTP 200.
- JSON-`null` auf einer `NOT NULL`-Spalte ergibt HTTP 422. JSON-`null` auf `Option<T>` schreibt SQL `NULL`.
- `#[field(readonly)]` und der Primärschlüssel sind nicht im Update-DTO. Unbekannte Felder lehnt Serde ab.
- `DELETE` ist an `WHERE "id" = $1` gebunden. HTTP 204 hat keinen Körper. Eine zweite Löschung derselben UUID ergibt HTTP 404.
- Nullable Spalten sind `Option<String>`, `Option<i64>`, `Option<bool>` oder `Option<Uuid>`. `id` bleibt Pflicht und nicht null.
- Breaking Change: `ModelRepository` verlangt jetzt die zugehörigen Typen `Update` sowie die Methoden `update` und `delete`.
- Der PostgreSQL-HTTP-Test deckt PATCH, DELETE, 403 und Nullable-Felder ab, bleibt `#[ignore]` und wurde hier nicht gegen eine Datenbank ausgeführt.

## Unreleased — API detail endpoint and documentation

- Added generated, typed `ModelRepository::get(UUID)` and `GET /{id}` routes.
- Missing records return HTTP 404 with the shared Ferrum error body.
- Detail routes remain denied (403) for models without `allow_anonymous = true`.
- Tightened the PostgreSQL integration-test precondition; no silent success without `DATABASE_URL`.
- Documented the actual required Serde derives on model structs.

## 0.1.0 — source prototype

- Cargo workspace, FerrumModel macro, SeaORM PostgreSQL repository, Axum GET/POST routes.
- Typed create DTOs and typed model reads; 100-row list cap with `truncated` indicator.
- Required and trimmed string-length validation and Boolean SQL `DEFAULT TRUE/FALSE`.
- Shared error response for Ferrum validation and permission errors.
- Optional PostgreSQL HTTP integration test, demo app, Docker Compose, and modular documentation.
- Compilation and runtime verification of subsequent changes remain outstanding in this environment.
