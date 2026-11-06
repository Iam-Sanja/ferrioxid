# Authentifizierung

[Zur Übersicht](index.md) · Sitzungen: [sessions.md](sessions.md) · Rechte: [permissions.md](permissions.md) · Objektregeln: [policies.md](policies.md)

## Zweck

Ferrum prüft Benutzer, Passwörter und Sitzungen serverseitig. Die Crate `ferrum-auth` hängt nicht am React-Admin. Generierte CRUD-Routen nutzen dieselbe Prüfung.

## Architektur

`ferrum-auth` hängt von `ferrum-core`, `ferrum-db` und `ferrum-migrations` ab, nicht von `ferrum-api`. `ferrum-api` hängt von `ferrum-auth` und registriert die Auth-Routen zusammen mit den Modellrouten. Die CLI ruft dieselben Service-Methoden auf.

Passwörter werden mit Argon2id gehasht und müssen 12 bis 128 Zeichen haben. Die CLI liest sie verdeckt oder aus `FERRUM_PASSWORD`. Sitzungstoken werden nur als SHA-256-Hex gespeichert. Objektregeln prüfen Create-Eingaben über `can_create`, bevor eine Zeile entsteht. Das Setzen der Besitzer-ID aus der Sitzung ist geplant. Das CSRF-Token wird bei Bedarf aus dem Schlüssel und der Sitzung berechnet; gespeichert wird ebenfalls nur der Hash. Rollen und Berechtigungen liest jede Prüfung erneut aus der Datenbank.

## Zuständige Dateien

| Datei | Aufgabe |
|---|---|
| `crates/ferrum-auth/src/password.rs` | Argon2id, PHC-String, Prüfung |
| `crates/ferrum-auth/src/service.rs` | Benutzer, Login, Rechte, Rate-Limit |
| `crates/ferrum-auth/src/store.rs` | SeaQuery gegen die Auth-Tabellen |
| `crates/ferrum-auth/src/schema.rs` | Snapshot und Migration |
| `crates/ferrum-auth/src/routes.rs` | `/auth/*` |
| `crates/ferrum-auth/migrations/0001_auth_schema.json` | Versiegeltes Schema |
| `crates/ferrum-api/src/lib.rs` | Guard vor generierten Routen |

## Öffentliche API

- `AuthConfig`, `AuthConfig::development`, `AuthConfig::from_env`
- `AuthService::create_user`, `list_users`, `set_user_active`
- `login`, `logout`, `authenticate_token`, `change_password`, `rotate_session`
- `authorize`, `ensure_role`, `grant_permission`, `revoke_permission`, `assign_role`, `revoke_role`
- `apply_migrations`
- `App::with_auth`, `App::register`, `App::register_policy`

`PublicUser` enthält `id`, `email`, `is_active`, `is_staff` und `is_superuser`. Kein Hash und kein Token.

## Datenmodell

Siehe [sessions.md](sessions.md) und [permissions.md](permissions.md). Zeitstempel sind Unix-Sekunden (`i64`), weil Ferrum keinen eigenen Datumstyp hat.

E-Mail wird getrimmt und in Kleinbuchstaben gespeichert. Sie ist eindeutig, höchstens 254 Zeichen lang und braucht genau ein `@` sowie einen Punkt in der Domain.

Ein neues Passwort hat 12 bis 128 Zeichen. Der Hash nutzt Argon2id, Version `0x13`, 19 456 KiB, zwei Iterationen und einen Thread. Salt und Parameter stehen im PHC-String.

## Sicherheitsmechanismen

- Unbekannte E-Mail, falsches Passwort und deaktivierter Benutzer liefern dieselbe Meldung `Invalid email or password`.
- Fehlt der Benutzer, wird ein festes Dummy-Hash geprüft, damit die Argon2-Arbeit nicht entfällt.
- Deaktivierte Benutzer können sich nicht anmelden. Bestehende Sitzungen enden bei der nächsten Prüfung.
- Eine Passwortänderung widerruft alle Sitzungen des Benutzers.
- Login ist pro Prozess auf fünf Fehlversuche in 15 Minuten begrenzt. Der Schlüssel ist Peer plus normalisierte E-Mail.
- `X-Forwarded-For` gilt nur, wenn `FERRUM_TRUST_PROXY` ausdrücklich `1` oder `true` ist. Gewertet wird der erste Hop.
- Sicherheitslogs nennen keine Passwörter und keine Session-Tokens.

## Fehlerfälle

| Situation | HTTP |
|---|---|
| Falsche Zugangsdaten oder deaktivierter Benutzer | 401, `Invalid email or password` |
| Fehlende oder ungültige Sitzung | 401, `Authentication required` |
| Fehlende Berechtigung | 403, `Access denied` |
| Zu viele Login-Versuche | 429, `Too many login attempts` |
| Ungültige E-Mail oder zu kurzes neues Passwort | 422 |

## Beispiel

```powershell
cargo run -p ferrum-cli -- migrate up
cargo run -p ferrum-cli -- admin create-user --email admin@example.com --superuser
cargo run -p ferrum-cli -- auth list-users
```

`create-user` fragt das Passwort verdeckt ab und bestätigt es. Es gibt kein `--password` und kein voreingestelltes Admin-Passwort.

```http
POST /auth/login
{"email":"admin@example.com","password":"example-password"}
```

Die Antwort setzt `Set-Cookie: ferrum_session=...` und liefert `user` plus `csrf_token`. Das Session-Token steht nicht im JSON.

## Tests

`crates/ferrum-auth/src/password.rs`, `crates/ferrum-auth/src/sqlite_tests.rs` und `examples/myshop/tests/auth.rs` laufen mit SQLite. PostgreSQL ist dafür nicht erforderlich.

## Einschränkungen

- Das Rate-Limit liegt im Prozess. Mehrere Prozesse teilen es nicht. Die Map kann später ersetzt werden.
- Es gibt kein E-Mail-Reset, kein MFA, kein OAuth und kein JWT für externe APIs.
- `is_staff` ist ein Kennzeichen und umgeht keine Rechte.
- Ein aktiver Superuser umgeht Rollen und Objektregeln. Ein deaktivierter Superuser kann sich nicht anmelden.

## Geplante Erweiterungen

Mehrprozess-Rate-Limit, Passwort-Reset, MFA und OAuth bleiben späteren Versionen vorbehalten.

## Admin und CSRF-Bootstrap

`GET /auth/csrf` liefert das zur Sitzung gehörende Token erneut. Die Sitzung selbst bleibt, und andere Tabs bleiben gültig. Details stehen in [csrf.md](csrf.md). Der Admin verlangt zusätzlich `is_staff`. Das Verzeichnis unter `/admin/api/directory` ist Superusern vorbehalten und kann `is_superuser` nicht setzen. Siehe [admin-security.md](admin-security.md). Der SQLite-Test `sqlite_admin_security_crud_and_csrf_bootstrap` prüft den Bootstrap.
