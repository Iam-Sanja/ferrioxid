# ferrum-macros – Compilerbasierte Modellgenerierung

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-macros/src/lib.rs`.

## Verantwortung und Verarbeitungsschritte (IST)

1. `#[derive(FerrumModel)]` empfängt `syn::DeriveInput`.
2. `#[ferrum(...)]` liest `table`, `api`, `allow_anonymous`, `admin`, `admin_label` und `admin_permission`. Felder lesen zusätzlich `#[admin(...)]`. Die Admin-Angaben erzeugen `admin_meta()` und keine zusätzliche Spalte. Details: [admin-customization.md](admin-customization.md).
3. Erlaubt sind nur Structs mit benannten Feldern.
4. Pro Feld wird `#[field(...)]` geprüft.
5. SQL-Datentyp, Constraints und generiertes Create-DTO werden bestimmt.
6. `quote!` erzeugt `FerrumModel`- und `ModelRepository`-Implementierung plus `ProductCreate`-artigen Typ.
7. Falsche Attribute erzeugen `syn::Error` als Compiler-Diagnose.

### Modellsyntax (IST, Pfad `examples/myshop/src/models/product.rs`)

```rust
#[derive(FerrumModel, ferrum::serde::Serialize, ferrum::serde::Deserialize)]
#[ferrum(table = "products", api = "/api/v1/products", allow_anonymous = true)]
pub struct Product {
    #[field(primary_key, auto)] pub id: Uuid,
    #[field(required, index, min_length = 3, max_length = 150)] pub name: String,
    #[field(min = 0)] pub price_cents: i64,
    #[field(default = true)] pub active: bool,
    #[field(foreign_key = "categories.id", on_delete = "restrict", belongs_to = "super::category::Category")]
    pub category_id: Option<Uuid>,
}
```

`pub struct ProductCreate` und `pub struct ProductUpdate` werden im Modul des Models erzeugt. Create besitzt kein `id`. Bei `#[field(default = true)]` ist `active` im Create-DTO `Option<bool>` mit `#[serde(default)]` und wird bei `None` durch `true` ersetzt. Update enthält kein `id` und keine `#[field(readonly)]`-Felder. Jedes änderbare Feld hat den Typ `ferrum::db::PatchField<T>`. Der `Product`-Typ selbst muss Serde-Serialize und Deserialize ableiten.

## Zulässige Typen (IST)

| Rust-Typ | SQL | Regeln | Create-DTO |
|---|---|---|---|
| `Uuid` | `UUID NOT NULL` | nur `id` mit `primary_key, auto` | weggelassen; nicht im Update |
| `String` | `TEXT NOT NULL` | `required`, `min_length`, `max_length` | `String`; Update `PatchField<String>` |
| `i64` | `BIGINT NOT NULL` | `min` als nichtnegatives Integer-Literal | `i64`; Update `PatchField<i64>` |
| `bool` | `BOOLEAN NOT NULL` | `default = true/false` | `Option<bool>` bei Default; Update `PatchField<bool>` |
| `Option<String/i64/bool/Uuid>` | Typ ohne `NOT NULL` | Länge und `min` nur bei vorhandenem Wert | `Option<T>` mit `#[serde(default)]`; Update `PatchField<T>` |

`id` darf nicht nullable sein. `required` und `default` dürfen nicht mit `Option<T>` kombiniert werden. `#[field(readonly)]` lässt das Feld im Create-DTO, entfernt es aus dem Update-DTO. Nicht unterstützt: `Decimal`, `DateTime`, Many-to-Many, benutzerdefinierte Felder, generische Models. `min = -5` ist kein Literal, das das Makro akzeptiert. Belongs-To und Has-Many sind implementiert und in [relations.md](relations.md) beschrieben. Unique, Index und Foreign Key sind in [constraints.md](constraints.md) beschrieben.

## Validierung (IST)

- `required` auf `String`: `.trim().is_empty()` → `Validation`.
- `min_length`/`max_length`: `.trim().chars().count()`; Anzahl Unicode-Skalarwerte, **keine** Graphem-Cluster und **keine** Bytes.
- `min` auf `i64`: numerischer Vergleich, nicht automatisch SQL-CHECK-Constraint.
- `required` auf `Uuid`/`i64`/`bool`: Compile-Fehler.
- `default` für bool: Datenbank-DDL `DEFAULT TRUE`/`FALSE` und DTO-Fallback `unwrap_or(...)`.
- `null` bei `active` wird als `None` behandelt und verwendet denselben Default wie ein fehlendes Feld.
- `trim()` prüft die Eingabe, **speichert aber keine getrimmte Version** der Zeichenkette.
- `min_length` darf nicht größer als `max_length` sein.
- PATCH: `PatchField::Missing` überspringt die Spalte. `PatchField::Null` ist bei `NOT NULL` ein Validierungsfehler `cannot be null` und bei nullable Spalten ein SQL-`NULL`. `PatchField::Value` durchläuft dieselben String- und `min`-Regeln.

## SQL-Generierung (IST)

- Identifikatoren: ASCII-Buchstaben/Unterstrich zuerst, danach Buchstaben/Ziffern/Unterstrich; SQL-Namen sind mit `"` gequotet.
- API-Pfad: statische Abschnitte mit denselben Identifier-Regeln; keine Platzhalter, `*` oder `?`.
- DDL für den Legacy-Bootstrap: `schema_sql(DatabaseKind)` ruft `render_schema` auf. Migrationen verwenden dieselben Spaltenmetadaten, aber die Crate `ferrum-migrations`.
- INSERT, LIST, GET, UPDATE und DELETE rufen `ferrum::db` auf. SeaQuery bindet Werte und quotet Bezeichner.
- LIST liest 101 Zeilen, gibt höchstens 100 zurück und setzt `truncated`.
- UPDATE schreibt nur Spalten, deren `PatchField` nicht `Missing` ist. Ein leeres Patch liest die Zeile.
- DELETE filtert über die gebundene UUID und wertet `rows_affected` aus.
- Die Datenbankschicht baut intern JSON und deserialisiert es in den Model-Typ. `to_jsonb` wird nicht verwendet.

## Grenzen und bekannte Risiken (IST)

- Der vom Makro generierte SQL-Text wird nicht auf die konkrete Datenbank-Schema-Version abgeglichen.
- Validation läuft in `create` und `update`, nicht bei beliebigen direkten SQL-Zugriffen.
- Serde-JSON-Fehler beim HTTP-Parsing stammen von Axum, nicht `FerrumError`.
- Ohne Versionsfeld gibt es keine Erkennung verlorener paralleler Updates.
- Strenge Namensprüfung schließt viele Sonderzeichen aus, nicht ein SQL-Audit.

## Erweiterungsstrategie (SOLL)

1. Parse in neutrales internes `ParsedModel` statt SQL direkt beim Parsen zusammenzusetzen.
2. Normierte Metadaten erzeugen (Felder, SQL, API-Sichtbarkeit, Validation).
3. Codegen getrennt für ORM, Repository, DTO und Schema.
4. Die Attribute versionieren, damit zukünftige Defaults oder Beziehungen keine stillen Bedeutungswechsel auslösen.
5. Compile-Fail-Tests (`trybuild` nur nach bewusster Dependency-Entscheidung) für ungültige Kombinationen.
6. Die drei PATCH-Zustände sind implementiert. Offen bleibt eine gemeinsame Metadatenstruktur statt direkt generiertem SQL.
7. `syn::Error` an exakten Quellspans ausgeben, nicht `panic!`.

## Testfälle und Abnahme

- `String` mit nur `required`: `""`, `"   "` → 422.
- String-Länge anhand getrimmter Zeichen; Unicode-Tests.
- `default = true/false`: SQL und POST-Rückgabe prüfen.
- unbekannte Attribute, mehrfacher PK, fehlendes `id`, ungültige SQL-Namen → Compile-Fehler.
- DTO-Serialisierungsgrenze: Type-Correctness ohne `Value` in der öffentlichen Repository-Signatur.

**Nicht hier implementieren:** Netzwerk-Routing, Datenbankverbindungen, Admin-Oberflächen.
