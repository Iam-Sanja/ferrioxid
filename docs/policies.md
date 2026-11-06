# Objektregeln

[Zur Übersicht](index.md) · [permissions.md](permissions.md)

## Zweck

Eine Objektregel entscheidet anhand von Benutzer und Datensatz. Sie ergänzt die Rollenprüfung und ersetzt sie nicht.

## Architektur

```rust
pub trait ObjectPolicy<T> {
    fn can_read(user: &AuthUser, object: &T) -> bool;
    fn can_update(user: &AuthUser, object: &T) -> bool;
    fn can_delete(user: &AuthUser, object: &T) -> bool;
    fn read_scope(user: &AuthUser) -> QueryScope { QueryScope::None }
    fn write_scope(user: &AuthUser) -> QueryScope { Self::read_scope(user) }
}
```

Das Modell setzt `object_policy = true` und ein `permissions`-Attribut. Die Anwendung registriert die Regel mit `register_policy::<Model, Policy>()` statt mit `register`. `register` allein antwortet auf diesem Modell mit 403 und dem Hinweis, dass die Regel nicht gebunden ist.

Die Regel läuft nach der Sitzungs- und Berechtigungsprüfung. Ein aktiver Superuser überspringt `can_read`, `can_update` und `can_delete`.

## Zuständige Dateien

- `crates/ferrum-auth/src/policy.rs`
- `crates/ferrum-api/src/lib.rs`
- `crates/ferrum-macros/src/lib.rs`

## Öffentliche API

`ferrum::auth::ObjectPolicy` und `App::register_policy`.

Für GET, PATCH und DELETE lädt der Handler den Datensatz. Schlägt die Regel fehl, ist die Antwort 404, auch wenn die Zeile existiert. So bestätigt die Schnittstelle die Kennung nicht.

POST prüft nur das Create-Recht. Die Trait-Methode dafür gibt es nicht.

## Datenmodell

Die Regel hat keine eigene Tabelle. Sie sieht das bereits geladene Modell.

## Sicherheitsmechanismen

Listen, Zähler und Schreibzugriffe verwenden `QueryScope`. Der Standard ist `None`, also keine Zeile. `All` muss die Regel ausdrücklich liefern. Ein aktiver Superuser erhält `All`. Die SQL-Form steht in [query-scopes.md](query-scopes.md).

`register` und `register_policy` für denselben Pfad dürfen nicht beide aufgerufen werden. Axum behält sonst beide Routen.

## Fehlerfälle

| Fall | Antwort |
|---|---|
| Regel nicht gebunden | 403, `object policy is not bound` |
| Scope ist `None` oder die Zeile gehört nicht dazu | leere Liste beziehungsweise 404 |
| Datensatz fehlt oder `can_*` lehnt ab | 404 |
| Create-Recht fehlt | 403 vor dem Schreiben |

## Beispiel

```rust
struct OwnerPolicy;

impl ferrum::auth::ObjectPolicy<SecretNote> for OwnerPolicy {
    fn can_read(user: &ferrum::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_update(user: &ferrum::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_delete(user: &ferrum::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
}

let app = App::new(db).with_auth(auth).register_policy::<SecretNote, OwnerPolicy>();
```

myshop selbst bindet keine Objektregel. `InternalNote` nutzt nur Rollen, damit Liste und CRUD demonstrativ funktionieren.

## Tests

`examples/myshop/tests/auth.rs` legt zwei Geheimnisse an, erlaubt dem Eigentümer den Einzelabruf, antwortet beim fremden Datensatz mit 404, blockiert die Liste und lässt den Superuser die Liste laden. Dieselbe Datei prüft das ungebundene Modell.

## Einschränkungen

Der Scope kennt `All`, `None` und einen Gleichheitsvergleich. Joins und Oder-Verknüpfungen sind nicht umgesetzt. Eine Regel kann die Create-Entscheidung nicht am neuen Inhalt treffen. Feldweises Ausblenden fehlt.

## Geplante Erweiterungen

Scopes mit mehreren Spalten oder Joins sind geplant und nicht implementiert. Ein abgelehnter Einzeldatensatz bleibt 404. Superuser erhalten `All` und überspringen `can_*`. Test: `auth.rs` und `admin_http.rs`.
