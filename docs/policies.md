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

Listen sind ohne SQL-Filter nicht sicher: eine Seite könnte fremde Zeilen enthalten oder durch die Filterung Lücken und falsche Längen erzeugen. Ferrum lädt solche Listen deshalb nicht. Normale Benutzer erhalten 403 mit dem Text, dass ein Query-Scope fehlt. Ein aktiver Superuser darf die ungefilterte Liste laden, weil die Objektregel für ihn keine Zeile ausschließt.

`register` und `register_policy` für denselben Pfad dürfen nicht beide aufgerufen werden. Axum behält sonst beide Routen.

## Fehlerfälle

| Fall | Antwort |
|---|---|
| Regel nicht gebunden | 403, `object policy is not bound` |
| Liste ohne Superuser | 403, Query-Scope fehlt |
| Datensatz fehlt oder Regel lehnt ab | 404 |
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

Es gibt keinen SQL-Query-Scope. Listen mit Objektregeln sind deshalb nicht allgemein freigegeben. Eine Regel kann die Create-Entscheidung nicht am neuen Inhalt treffen. Feldweises Ausblenden fehlt.

## Geplante Erweiterungen

Ein expliziter Query-Scope, der Pagination und Berechtigung in derselben Abfrage hält, ist die Voraussetzung für sichere Listen. Bis dahin antwortet auch der Admin auf Listen und Fremdschlüssel-Auswahlen mit 403. Ein abgelehnter Einzeldatensatz bleibt 404. Superuser überspringen die Objektprüfung beim Einzeldatensatz, nicht die Listensperre. Test: `admin_http.rs`.
