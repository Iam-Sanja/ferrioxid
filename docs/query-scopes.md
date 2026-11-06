# Query-Scopes für Objektregeln

[Zur Übersicht](index.md) · [policies.md](policies.md) · [database.md](database.md)

## Zweck

Eine Objektregel soll Listen in der Datenbank einschränken. Ferrum lädt nicht alle Zeilen, um sie danach im Prozess zu filtern.

## Architektur

`QueryScope` liegt in `ferrum-core`, damit Datenbank, API und Admin dieselbe Darstellung nutzen.

```rust
pub enum QueryScope {
    All,
    None,
    Equals { column: &'static str, value: ScopeValue },
}
```

`ScopeValue` ist `Text`, `Uuid`, `BigInt` oder `Bool`. Die Standardmethode `ObjectPolicy::read_scope` liefert `None`. `write_scope` übernimmt `read_scope`, solange die Regel nichts anderes sagt. Ein aktiver Superuser erhält `All`.

## Verantwortliche Dateien

- `crates/ferrum-core/src/lib.rs`
- `crates/ferrum-auth/src/policy.rs`
- `crates/ferrum-db/src/sql.rs`
- `crates/ferrum-macros/src/lib.rs`
- `crates/ferrum-api/src/lib.rs`
- `crates/ferrum-admin/src/registry.rs`
- `crates/ferrum-admin/src/http.rs`

## Öffentliche APIs

- `ObjectPolicy::read_scope` und `write_scope`
- `ModelRepository::list_scoped`, `get_scoped`, `update_scoped`, `delete_scoped`
- `list`, `get`, `update` und `delete` bleiben und verwenden `QueryScope::All`
- Admin-Listen antworten zusätzlich mit `total`

## Datenfluss

REST und Admin rufen dieselben SeaQuery-Helfer auf. Der Scope wird vor Suche, Sortierung, Offset und Limit als parametrisierte Bedingung angehängt. `None` wird zu `0 = 1`. `Equals` prüft den Spaltennamen gegen die Modellmetadaten und bindet den Wert als Parameter. `total` zählt mit denselben Filtern und demselben Scope, ohne Limit.

PATCH und DELETE tragen den Scope in der `WHERE`-Klausel. Ein vorheriger SELECT bleibt als zweite Prüfung über `can_update` und `can_delete`.

## Sicherheitsregeln

- Spaltennamen kommen nur aus vertrauenswürdigen Modellmetadaten.
- Werte aus der HTTP-Anfrage werden nie als Scope-Ausdruck übernommen.
- Eine fehlende Implementierung von `read_scope` ergibt `None` und damit keine Zeile.
- `All` entsteht nur durch eine ausdrückliche Policy oder durch die Superuser-Regel.
- Eine ungebundene Objektregel bleibt 403 mit `object policy is not bound`.
- Sortier- und Filterfelder bleiben eine Whitelist. Ein unbekannter Name ist 422.

## Fehlerfälle

| Fall | Antwort |
|---|---|
| Regel nicht gebunden | 403 |
| Zeile liegt außerhalb des Scopes | 404 |
| Spalte des Scopes gehört nicht zum Modell | 422 |
| Superuser | voller Zugriff, weiterhin Staff-Pflicht im Admin |

## Beispiel

```rust
fn read_scope(user: &AuthUser) -> QueryScope {
    QueryScope::Equals {
        column: "owner",
        value: ScopeValue::Text(user.id.to_string()),
    }
}
```

## Tests

`examples/myshop/tests/auth.rs` prüft, dass ein Benutzer nur die eigene Zeile in der REST-Liste sieht und der Superuser beide sieht. `examples/myshop/tests/admin_http.rs` prüft dieselbe Liste im Admin, `total`, einen fremden GET, PATCH und DELETE.

## Einschränkungen

Es gibt kein `OR`, keine Joins und keine Ausdrücke aus mehreren Spalten. Eine `can_*`-Methode, die strenger ist als `write_scope`, bleibt eine zweite Prüfung nach dem SELECT. `write_scope` muss deshalb mindestens so eng sein wie `can_update` und `can_delete`. Create prüft den neuen Inhalt nicht gegen den Scope.
