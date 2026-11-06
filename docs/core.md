# ferrum-core – Verträge und Modellmetadaten

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-core/src/lib.rs`.

## Verantwortung

`ferrum-core` enthält die gemeinsamen, von HTTP und ORM unabhängigen Vertragstypen. Die Crate darf **keine** Abhängigkeit auf `ferrum-api`, Axum oder eine konkrete Datenbankanbindung annehmen.

## Tatsächliche öffentliche Typen (IST)

```rust
pub struct ModelDescriptor {
    pub name: &'static str,
    pub table: &'static str,
    pub api_path: &'static str,
    pub allow_anonymous: bool,
}
pub enum DatabaseKind { Sqlite, Postgres }
pub enum ColumnKind { Uuid, Text, BigInt, Boolean }
pub struct ColumnSpec {
    pub name: &'static str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
}
pub trait FerrumModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn columns() -> &'static [ColumnSpec];
    fn schema_sql(kind: DatabaseKind) -> String;
}
pub enum FerrumError {
    Validation(String),
    Forbidden,
    NotFound,
    Database(Box<dyn Error + Send + Sync>),
}
```

Hinweis: Das obige `Error` ist zur Lesbarkeit abgekürzt; in der Quelldatei steht `std::error::Error` vollständig. Der Helfer `FerrumError::database(error)` konvertiert beliebige kompatible Fehler zu `Database`.

### Semantik

- `name`: Rust-Typname (via `stringify!`).
- `table`: bereits makroseitig validierter SQL-Tabellenname.
- `api_path`: statischer Pfad, z. B. `/api/v1/products`.
- `allow_anonymous`: explizite Freigabe für Demo-Routen; Standard `false`.
- `columns`: dialektunabhängige Spaltenmetadaten aus dem Makro.
- `schema_sql(kind)`: Bootstrap-DDL für SQLite oder PostgreSQL. Der Text kommt aus SeaQuery, nicht aus einem gespeicherten SQL-String.

Beziehungen, Unique-Constraints, Indizes und Policies sind keine `ColumnSpec`-Felder. `DateTime` ist kein `ColumnKind`. Migrationen bauen ihren Snapshot aus diesen Metadaten und erfinden keine zweite Spaltenbeschreibung.

## Fehlersemantik (IST)

| Fehler | Bedeutung | HTTP-Mapping |
|---|---|---|
| `Validation` | ungültige Eingaben | 422 |
| `Forbidden` | Zugriff nicht freigegeben | 403 |
| `NotFound` | Zeile fehlt | 404 |
| `Database` | Datenbank, Query oder Daten-Deserialisierung | 500, internes Detail nur im Log |

Die Umwandlung zu HTTP findet **nicht** in `ferrum-core`, sondern ausschließlich in `ferrum-api` statt; siehe [api.md](api.md).

## Erweiterungsentwurf (SOLL)

1. Reichere Feldmetadaten über `ColumnSpec` hinaus: Constraints, API-Sichtbarkeit und Defaults außer Bool.
2. `RelationDescriptor` mit Relationstyp und FK-Referenz.
3. `ModelSchema` mit Tabelle, PK, Feldern, Indizes und Constraints.
4. `Registry` für **explizit registrierte** Modelle.
5. Deskriptor-Schema-Version zur stabilen Weiterentwicklung des Formats.

Designregeln: Spaltenmetadaten bleiben die kanonische Darstellung. SQL entsteht erst für einen konkreten `DatabaseKind`. MySQL ist nicht vorgesehen, solange es keine eigene Abbildung und Tests gibt.

## Tests und Abnahme

- IST: Typen kompilieren im Workspace (vor Ort prüfen).
- SOLL: Unit-Tests für eindeutige Tabellen-/Feldnamen, stabile Registry-Reihenfolge und eindeutige Model-IDs.
- Für neue öffentliche Typen: SemVer-Auswirkung und Migration von alten Descriptor-Aufrufen dokumentieren.

**Nicht hier implementieren:** SQL-Ausführung, Axum-Handler, CLI-Parsing oder React-Code.

## NotFound (IST)

`FerrumError::NotFound` ist der Fehler für fehlende Ressourcen. Die Axum-Adapter liefern dafür `404` mit demselben JSON-Format wie für `Validation` und `Forbidden`.
