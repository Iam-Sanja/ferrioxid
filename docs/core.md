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
pub trait FerrumModel: Send + Sync + 'static {
    fn descriptor() -> &'static ModelDescriptor;
    fn schema_sql() -> &'static str;
}
pub enum FerrumError {
    Validation(String),
    Forbidden,
    Database(Box<dyn Error + Send + Sync>),
}
```

Hinweis: Das obige `Error` ist zur Lesbarkeit abgekürzt; in der Quelldatei steht `std::error::Error` vollständig. Der Helfer `FerrumError::database(error)` konvertiert beliebige kompatible Fehler zu `Database`.

### Semantik

- `name`: Rust-Typname (via `stringify!`).
- `table`: bereits makroseitig validierter SQL-Tabellenname.
- `api_path`: statischer Pfad, z. B. `/api/v1/products`.
- `allow_anonymous`: explizite Freigabe für Demo-Routen; Standard `false`.
- `schema_sql`: SQL-Text aus der Modelldefinition; ausschließlich für expliziten Entwicklungs-Bootstrap.

Keine Feldmetadaten, Relationships, Registry-Container oder Policies in v0.1.

## Fehlersemantik (IST)

| Fehler | Bedeutung | HTTP-Mapping |
|---|---|---|
| `Validation` | ungültige Eingaben | 422 |
| `Forbidden` | Zugriff nicht freigegeben | 403 |
| `Database` | Datenbank, Query oder Daten-Deserialisierung | 500, internes Detail nur im Log |

Die Umwandlung zu HTTP findet **nicht** in `ferrum-core`, sondern ausschließlich in `ferrum-api` statt; siehe [api.md](api.md).

## Erweiterungsentwurf (SOLL)

1. `FieldDescriptor` mit eindeutigem Feldnamen, Typ, Nullbarkeit, Default, Constraints und API-Sichtbarkeit.
2. `RelationDescriptor` mit Relationstyp und FK-Referenz.
3. `ModelSchema` mit Tabelle, PK, Feldern, Indizes und Constraints.
4. `Registry` für **explizit registrierte** Modelle.
5. Deskriptor-Schema-Version zur stabilen Weiterentwicklung des Formats.

Designregeln: keine SQL-Strings als alleinige kanonische Repräsentation; keine dynamisch geleakten `&'static str`; Reihenfolge und Hashing deterministisch; deskriptorbasierte Erweiterungen versionierbar; Feldtypen müssen SQLite/MySQL nicht vorzeitig abstrahieren.

## Tests und Abnahme

- IST: Typen kompilieren im Workspace (vor Ort prüfen).
- SOLL: Unit-Tests für eindeutige Tabellen-/Feldnamen, stabile Registry-Reihenfolge und eindeutige Model-IDs.
- Für neue öffentliche Typen: SemVer-Auswirkung und Migration von alten Descriptor-Aufrufen dokumentieren.

**Nicht hier implementieren:** SQL-Ausführung, Axum-Handler, CLI-Parsing oder React-Code.

## Fehlererweiterung (implementiert, noch nicht lokal kompiliert)

`FerrumError::NotFound` stellt den domänenweiten Fehler für fehlende Ressourcen dar. Die Axum-Adapter liefern dafür `404` mit demselben JSON-Fehlerformat wie für `Validation` und `Forbidden`.
