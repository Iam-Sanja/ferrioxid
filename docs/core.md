# ferrum-core – Verträge und Modellmetadaten

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-core/src/lib.rs`.

## Authentifizierung (v0.7)

`permission` ist `None`, `"authenticated"` oder ein Ressourcenname. `requires_object_policy` markiert Modelle, deren Einzelzugriff eine gebundene `ObjectPolicy` braucht. `FerrumError` hat zusätzlich `Denied(String)` und `Unauthorized(String)`. Die HTTP-Zuordnung steht in [api.md](api.md).

## Verantwortung

`ferrum-core` enthält die gemeinsamen, von HTTP und ORM unabhängigen Vertragstypen. Die Crate darf **keine** Abhängigkeit auf `ferrum-api`, Axum oder eine konkrete Datenbankanbindung annehmen.

## Tatsächliche öffentliche Typen (IST)

```rust
pub struct ModelDescriptor {
    pub name: &'static str,
    pub table: &'static str,
    pub api_path: &'static str,
    pub allow_anonymous: bool,
    pub permission: Option<&'static str>,
    pub requires_object_policy: bool,
}
pub enum DatabaseKind { Sqlite, Postgres }
pub enum ColumnKind { Uuid, Text, BigInt, Boolean }
pub enum ReferentialAction { Restrict, Cascade, SetNull }
pub struct ColumnSpec {
    pub name: &'static str,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_bool: Option<bool>,
    pub unique: bool,
    pub indexed: bool,
    pub foreign_table: Option<&'static str>,
    pub foreign_column: Option<&'static str>,
    pub on_delete: Option<ReferentialAction>,
}
pub struct TableConstraints {
    pub unique_groups: &'static [&'static [&'static str]],
    pub index_groups: &'static [&'static [&'static str]],
}
pub trait FerrumModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn columns() -> &'static [ColumnSpec];
    fn constraints() -> &'static TableConstraints;
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

`unique`, `indexed` und die Foreign-Key-Felder stehen in `ColumnSpec`. Zusammengesetzte Gruppen stehen in `TableConstraints`. `constraints()` liefert für alte Modelle leere Gruppen. `DateTime` ist kein `ColumnKind`. Policies sind nicht enthalten. Migrationen bauen den Snapshot aus diesen Metadaten. Die Namenshelfer `unique_index_name`, `secondary_index_name` und `foreign_key_name` liegen in derselben Crate. Details stehen in [constraints.md](constraints.md) und [relations.md](relations.md).

## Fehlersemantik (IST)

| Fehler | Bedeutung | HTTP-Mapping |
|---|---|---|
| `Validation` | ungültige Eingaben, Unique- oder Foreign-Key-Verletzung | 422 |
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

## Admin-Metadaten

`FerrumModel::admin_meta` liefert standardmäßig `None`. Das Makro überschreibt die Methode nur bei `admin = true`. `AdminModelMeta` und `AdminFieldMeta` stehen neben `ModelDescriptor` und ändern keine Spalte. Sensible Felder bleiben über den Namen erkennbar und werden von der Admin-API entfernt. Tests: `examples/myshop/tests/admin_http.rs`. Grenze: die Metadaten beschreiben die Oberfläche, nicht einen zweiten Schema-Snapshot.
