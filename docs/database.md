# ferrum-db – Typisierte Repository-Grenze und PostgreSQL

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-db/src/lib.rs`. Makro-Repository-Implementierung liegt in `crates/ferrum-macros/src/lib.rs`.

## Verantwortung (IST)

- `connect(url: &str)` ruft `sea_orm::Database::connect(url).await` auf.
- `ModelRepository` beschreibt typisierte `list`-, `get`-, `create`-, `update`- und `delete`-Operationen.
- `ListPage<T>` beschreibt die Listenantwort als generischen Rust-Typ.
- Die Makro-Crate implementiert das Trait für jedes Model.

```rust
#[async_trait::async_trait]
pub trait ModelRepository: FerrumModel + Serialize + DeserializeOwned {
    type Create: DeserializeOwned + Send;
    type Update: DeserializeOwned + Send;
    async fn list(db: &DatabaseConnection) -> Result<ListPage<Self>, FerrumError>;
    async fn get(db: &DatabaseConnection, id: uuid::Uuid) -> Result<Option<Self>, FerrumError>;
    async fn create(db: &DatabaseConnection, input: Self::Create) -> Result<Self, FerrumError>;
    async fn update(db: &DatabaseConnection, id: uuid::Uuid, input: Self::Update) -> Result<Self, FerrumError>;
    async fn delete(db: &DatabaseConnection, id: uuid::Uuid) -> Result<DeleteOutcome, FerrumError>;
}
```

`ListPage<T>` enthält `data: Vec<T>`, `limit: usize`, `truncated: bool`. `DeleteOutcome` ist `Deleted` oder `Missing`. `get` bleibt `Option`, die API macht daraus 404. `update` liefert das Model oder `FerrumError::NotFound`. Öffentliche Grenzen verwenden kein `serde_json::Value`.

## Abfrageverhalten (IST)

### Create

1. DTO aus HTTP-Payload oder direktem Rust-Aufruf.
2. Generierte `required`/Längen-/`min`-Prüfung.
3. UUID mit `Uuid::new_v4()` erstellen.
4. `Statement::from_sql_and_values(DbBackend::Postgres, insert_sql, values)` erzeugen.
5. `INSERT ... RETURNING to_jsonb(...) AS record`.
6. Rückgabe von `to_jsonb` intern zu `Self` deserialisieren; Fehler als `FerrumError::Database`.

### Update

1. `PatchField` auswerten: `Missing` lässt die Spalte stehen, `Null` setzt nur bei nullable Spalten SQL `NULL`, `Value` validiert und schreibt den Originalwert.
2. Eine einzelne `UPDATE ... SET "spalte" = CASE WHEN $n::boolean THEN $m::<typ> ELSE "spalte" END WHERE "id" = $id RETURNING to_jsonb(...)`.
3. Keine Zeile ergibt `NotFound`. PostgreSQL sperrt die betroffene Zeile für die Dauer der Anweisung. Ohne Versionsfeld gewinnt die zuletzt festgeschriebene Anweisung.

### Delete

`DELETE FROM "tabelle" WHERE "id" = $1`. `rows_affected() == 0` wird `DeleteOutcome::Missing`, sonst `Deleted`. Es gibt keine Löschung ohne diese WHERE-Bedingung.

### List

- `ORDER BY "id" LIMIT 101`.
- Maximal 100 Objekte zurückgeben; bei 101 Zeilen `truncated = true`.
- Keine Pagination und kein garantierter Snapshot zwischen konkurrierenden Änderungen.
- UUID-Sortierung ist stabil, aber nicht automatisch Erstellungszeitreihenfolge.

### Sicherheit

Bezeichner (Tabellen-/Spaltennamen) sind beim Kompilieren validiert und werden gequotet, Client-Werte werden parameterisiert. Keine SQL-Fragmente aus HTTP-Parametern zusammensetzen. Secrets der `DATABASE_URL` nicht loggen.

## SOLL: Entkopplung und Transaktionen

1. Connection-Konfiguration mit Pool-Limits und Timeout als explizite Options.
2. Eine Modelschema-Repräsentation ohne direkt eingebettete SQL-Texte.
3. Transaktionskontext für zusammenhängende Operationen; keine stillen globalen Transaktionen.
4. Beziehungen erst nach festgelegter FK- und Loading-Semantik.
5. Fehler- und Constraint-Mapping: Unique-Verletzung → definierter Anwendungscode, ohne sensible SQL-Details.
6. Gezielt indexierte Abfragen; keine unlimitierte Collection-API.

## Tests

- IST: Der optionale End-to-End-Test prüft Create, Read, PATCH, DELETE, 403 und Nullable-Felder gegen PostgreSQL. Ohne laufende Datenbank ist er ignoriert und kein Erfolgsnachweis.
- SOLL: Race Conditions, Constraint-Fehler, Transaktionen, Pool-Timeouts und temporäre Datenbankausfälle abdecken.
- Keine vermeintlichen CI-Postgres-Tests behaupten: `.github/workflows/rust.yml` startet aktuell keinen DB-Service.

Siehe [migrations.md](migrations.md) und [testing.md](testing.md).

## Einzelabruf

`ModelRepository::get` erzeugt `SELECT to_jsonb(t) AS record FROM "<tabelle>" t WHERE "id" = $1` und bindet die UUID. Keine Zeile ergibt `Ok(None)`. Die HTTP-Schicht macht daraus 404. SQL- oder JSON-Fehler bleiben `FerrumError::Database`.
