# Rollen und Berechtigungen

[Zur Übersicht](index.md) · [auth.md](auth.md) · [policies.md](policies.md)

## Zweck

Rechte gelten serverseitig. Eine Änderung an Rolle oder Berechtigung wirkt bei der nächsten Anfrage. Ferrum cached diese Rechte nicht in der Sitzung.

## Architektur

Ein Benutzer kann mehrere Rollen haben. Eine Rolle kann mehrere Berechtigungen haben. Der Code hat die Form `ressource.aktion`. Die Aktion ist `read`, `create`, `update` oder `delete`.

`authorize` prüft bei jedem Aufruf die Tabellen `_ferrum_user_roles`, `_ferrum_role_permissions` und `_ferrum_permissions`.

## Zuständige Dateien

- `crates/ferrum-auth/src/service.rs`
- `crates/ferrum-auth/src/store.rs`
- `crates/ferrum-macros/src/lib.rs` für `permissions`
- `crates/ferrum-api/src/lib.rs` für die Zuordnung von HTTP-Methode zu Aktion

## Öffentliche API

```rust
auth.ensure_role("editor").await?;
auth.grant_permission("editor", "notes.read").await?;
auth.assign_role("editor@example.com", "editor").await?;
auth.revoke_permission("editor", "notes.read").await?;
auth.revoke_role("editor@example.com", "editor").await?;
```

Dieselben Schritte stellt die CLI bereit: `auth create-role`, `auth grant-permission`, `auth revoke-permission`, `auth assign-role`, `auth revoke-role`.

## Datenmodell

| Tabelle | Inhalt |
|---|---|
| `_ferrum_roles` | `id`, eindeutiger `name` |
| `_ferrum_permissions` | `id`, eindeutiger `code` |
| `_ferrum_user_roles` | eigene UUID, `user_id`, `role_id`, eindeutig als Paar |
| `_ferrum_role_permissions` | eigene UUID, `role_id`, `permission_id`, eindeutig als Paar |

Foreign Keys löschen Zuordnungen per `CASCADE`.

## Modellattribut

```rust
#[ferrum(table = "internal_notes", api = "/api/v1/internal_notes", permissions = "notes")]
```

| Attribut | Wirkung |
|---|---|
| keines, `allow_anonymous = false` | jede Methode 403 |
| `allow_anonymous = true` | öffentlich, ohne Cookie-Prüfung |
| `permissions = "authenticated"` | jede aktive Sitzung |
| `permissions = "notes"` | `notes.read`, `notes.create`, `notes.update`, `notes.delete` je nach Methode |

`allow_anonymous` zusammen mit `permissions` oder `object_policy` ist ein Compile-Fehler. Der Standard bleibt Verweigerung.

GET-Liste und GET-Detail prüfen `read`. POST prüft `create`. PATCH prüft `update`. DELETE prüft `delete`.

Ein aktiver Superuser besteht die Rollenprüfung. `is_staff` tut das nicht.

## Sicherheitsmechanismen

Es gibt keine Berechtigung ohne Eintrag. `grant_permission` legt Rolle und Code an, wenn sie fehlen. `revoke_permission` legt nichts an und ist ohne Treffer folgenlos.

## Fehlerfälle

Fehlende Rechte sind HTTP 403 `Access denied`. Eine fehlende Sitzung ist HTTP 401 und wird vor der Rechteprüfung zurückgegeben.

## Beispiel

```powershell
cargo run -p ferrum-cli -- auth create-role editor
cargo run -p ferrum-cli -- auth grant-permission notes.read --role editor
cargo run -p ferrum-cli -- auth assign-role --email editor@example.com --role editor
```

myshop schützt `InternalNote` mit `permissions = "notes"`. `Product` und `Category` bleiben `allow_anonymous = true` und sind Demo-Endpunkte.

## Tests

`sqlite_tests` vergibt und entzieht `notes.*` und prüft den Superuser. `examples/myshop/tests/auth.rs` prüft die geschützten HTTP-Methoden und die anonyme Demo-Route.

## Einschränkungen

Feldrechte gibt es nicht. Berechtigungen sind globale Codes, keine Zeilenfilter. Dafür sind Objektregeln zuständig. Listen verwenden denselben `QueryScope` wie die REST-API. Siehe [policies.md](policies.md) und [query-scopes.md](query-scopes.md).

## Geplante Erweiterungen

Feldrechte, Gruppen jenseits von Rollen und ein Zwischenspeicher mit gezielter Invalidierung sind nicht umgesetzt.

## Admin

Staff allein vergibt kein Modellrecht. Ein Superuser überspringt die Rollenprüfung, braucht für den Admin aber `is_staff`. Eine leere Admin-Berechtigung bedeutet Superuser-only. `permissions = "authenticated"` lässt nach der Staff-Prüfung jeden Staff-Benutzer zu. Das Verzeichnis prüft Superuser getrennt von den Modellrechten.
