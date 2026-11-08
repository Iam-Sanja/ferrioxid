# Authentifizierung

[Zur Übersicht](index.md) · Sitzungen: [sessions.md](sessions.md) · Rechte: [permissions.md](permissions.md) · Objektregeln: [policies.md](policies.md)

## Zweck

Ferrioxid prüft Benutzer, Passwörter und Sitzungen serverseitig. Die Crate `ferrioxid-auth` hängt nicht an der HTML-Oberfläche. Der Admin ruft dieselben Methoden auf. Generierte CRUD-Routen nutzen dieselbe Prüfung. Auth-Fehler laufen über `problem_json`. `Invalid email or password` bleibt für unbekannte Benutzer und falsche Passwörter gleich und trägt `auth.invalid_credentials`. Das Verzeichnis hängt Rollennamen an, ohne `PublicUser` von `/auth/me` zu verändern.

Admin-Änderungen an Benutzern, Rollen und Rechten laufen über `*_on` in einer Transaktion mit dem Audit. Sitzungswiderruf bei Passwort und Deaktivierung gehört dazu. Die CLI-Methoden bleiben ohne dieses Audit. Siehe [admin-directory-audit.md](admin-directory-audit.md). Ein vergessenes Passwort setzt der Benutzer selbst zurück: [password-reset.md](password-reset.md). Der Versand liegt in [mail.md](mail.md).

## Architektur

`ferrioxid-auth` hängt von `ferrioxid-core`, `ferrioxid-db` und `ferrioxid-migrations` ab, nicht von `ferrioxid-api`. `ferrioxid-api` hängt von `ferrioxid-auth` und registriert die Auth-Routen zusammen mit den Modellrouten. Die CLI ruft dieselben Service-Methoden auf.

Passwörter werden mit Argon2id gehasht und müssen 12 bis 128 Zeichen haben. Die CLI liest sie verdeckt oder aus `FERRUM_PASSWORD`. Sitzungstoken werden nur als SHA-256-Hex gespeichert. Objektregeln prüfen Create-Eingaben über `can_create`, bevor eine Zeile entsteht. Das Setzen der Besitzer-ID aus der Sitzung ist geplant. Das CSRF-Token wird bei Bedarf aus dem Schlüssel und der Sitzung berechnet; gespeichert wird ebenfalls nur der Hash. HTML-Formulare übergeben dasselbe Token als Feld `csrf_token` an `authenticate_csrf`. Ein zweiter CSRF-Mechanismus existiert nicht. Rollen und Berechtigungen liest jede Prüfung erneut aus der Datenbank.

## Zuständige Dateien

| Datei | Aufgabe |
|---|---|
| `crates/ferrioxid-auth/src/password.rs` | Argon2id, PHC-String, Prüfung |
| `crates/ferrioxid-auth/src/service.rs` | Benutzer, Login, Rechte, Rate-Limit |
| `crates/ferrioxid-auth/src/store.rs` | SeaQuery gegen die Auth-Tabellen |
| `crates/ferrioxid-auth/src/schema.rs` | Snapshot und Migration |
| `crates/ferrioxid-auth/src/routes.rs` | `/auth/*` |
| `crates/ferrioxid-auth/src/recovery.rs` | Reset-Anforderung, Austausch und Abschluss |
| `crates/ferrioxid-auth/migrations/0004_password_reset.json` | Token-Hash, Austausch-Hash, Outbox |
| `crates/ferrioxid-auth/migrations/0001_auth_schema.json` | Versiegeltes Schema |
| `crates/ferrioxid-api/src/lib.rs` | Guard vor generierten Routen |

## Öffentliche API

- `AuthConfig`, `AuthConfig::development`, `AuthConfig::from_env`, `AuthConfig::apply_bound_address`
- `AuthService::create_user`, `list_users`, `set_user_active`
- `login`, `logout`, `authenticate_token`, `change_password`, `rotate_session`
- `authorize`, `ensure_role`, `grant_permission`, `revoke_permission`, `assign_role`, `revoke_role`
- `request_reset`, `exchange_token`, `complete_reset`
- `apply_migrations`
- `App::with_auth`, `App::register`, `App::register_policy`

`PublicUser` enthält `id`, `email`, `is_active`, `is_staff` und `is_superuser`. Kein Hash und kein Token.

## Datenmodell

Siehe [sessions.md](sessions.md) und [permissions.md](permissions.md). Zeitstempel sind Unix-Sekunden (`i64`), weil Ferrioxid keinen eigenen Datumstyp hat.

E-Mail wird getrimmt und in Kleinbuchstaben gespeichert. Sie ist eindeutig, höchstens 254 Zeichen lang und braucht genau ein `@` sowie einen Punkt in der Domain.

Ein neues Passwort hat 12 bis 128 Zeichen. Der Hash nutzt Argon2id, Version `0x13`, 19 456 KiB, zwei Iterationen und einen Thread. Salt und Parameter stehen im PHC-String.

## Sicherheitsmechanismen

- Unbekannte E-Mail, falsches Passwort und deaktivierter Benutzer liefern dieselbe Meldung `Invalid email or password`.
- Fehlt der Benutzer, wird ein festes Dummy-Hash geprüft, damit die Argon2-Arbeit nicht entfällt.
- Deaktivierte Benutzer können sich nicht anmelden. Deaktivierung über CLI oder Admin widerruft die gespeicherten Sitzungen sofort.
- Eine Passwortänderung über CLI oder Admin widerruft alle Sitzungen des Benutzers. Der Admin-Passwortweg nutzt `validate_new_password` und `hash_password`, zeigt den Hash nicht und legt das Passwort nicht in die Weiterleitung.
- Staff- und Rollenänderungen schreibt der Admin in dieselbe Benutzerzeile. Die nächste `authenticate_token`-Prüfung liest die Flags neu. Dafür wird die Sitzung nicht zusätzlich gelöscht.
- Login ist pro Prozess auf fünf Fehlversuche in 15 Minuten begrenzt. Der Schlüssel ist Peer plus normalisierte E-Mail.
- `X-Forwarded-For` gilt nur, wenn `FERRUM_TRUST_PROXY` ausdrücklich `1` oder `true` ist. Gewertet wird der erste Hop.
- Sicherheitslogs nennen keine Passwörter, Session-Tokens, Login-CSRF-Cookies oder Formular-Tokens.
- Liegt `FERRUM_ALLOWED_ORIGINS` nicht vor, ersetzt `AuthConfig::apply_bound_address` die Entwicklungsliste durch die Loopback-Origins der Adresse, an die der Prozess bindet. Öffentliche Hosts und der Host-Header einer Anfrage werden nicht in diese Liste übernommen. `Origin: null` bleibt ungültig. Eine Anfrage ohne Origin und ohne Referer bleibt erlaubt, das Login-CSRF-Cookie aber nicht.

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
cargo run -p ferrioxid-cli -- migrate up
cargo run -p ferrioxid-cli -- admin create-user --email admin@example.com --superuser
cargo run -p ferrioxid-cli -- auth list-users
```

`create-user` fragt das Passwort verdeckt ab und bestätigt es. Es gibt kein `--password` und kein voreingestelltes Admin-Passwort.

```http
POST /auth/login
{"email":"admin@example.com","password":"example-password"}
```

Die Antwort setzt `Set-Cookie: ferrioxid_session=...` und liefert `user` plus `csrf_token`. Das Session-Token steht nicht im JSON.

## Tests

`crates/ferrioxid-auth/src/password.rs`, `crates/ferrioxid-auth/src/sqlite_tests.rs` und `examples/myshop/tests/auth.rs` laufen mit SQLite. PostgreSQL ist dafür nicht erforderlich.

## Einschränkungen

- Das Rate-Limit liegt im Prozess. Mehrere Prozesse teilen es nicht. Die Map kann später ersetzt werden. Der Reset nutzt eigene Schlüssel und dasselbe Fenster.
- Es gibt kein MFA, kein OAuth und kein JWT für externe APIs. Der E-Mail-Reset ist in v0.23 vorhanden und garantiert die Zustellung nicht.
- `is_active` ist nötig für eine Sitzung. Ein inaktives Konto wird bei der Anmeldung abgelehnt.
- `is_staff` öffnet die Verwaltung und umgeht keine Modellrechte.
- `is_superuser` umgeht Rollen und Objektregeln, ersetzt `is_staff` aber nicht. Ein Superuser ohne Staff erhält im Admin 403 und kann sich dort abmelden.
- `ferrioxid admin create-user` setzt nur die übergebenen Flags und druckt Aktiv, Staff und Superuser. `--superuser` ohne `--staff` weist auf den geschlossenen Admin hin.

## Geplante Erweiterungen

Mehrprozess-Rate-Limit, MFA und OAuth bleiben späteren Versionen vorbehalten. Der Passwort-Reset ist umgesetzt.

## Admin und CSRF-Bootstrap

`GET /auth/csrf` liefert das zur Sitzung gehörende Token erneut. Die Sitzung selbst bleibt, und andere Tabs bleiben gültig. Details stehen in [csrf.md](csrf.md). Der Admin verlangt zusätzlich `is_staff`. Das Verzeichnis unter `/admin/api/directory` und die HTML-Seiten unter `/admin/directory/users` sind Staff-Superusern vorbehalten und können `is_superuser` nicht setzen. Siehe [admin-directory.md](admin-directory.md) und [admin-security.md](admin-security.md). Der SQLite-Test `sqlite_admin_security_crud_and_csrf_bootstrap` prüft den Bootstrap.
