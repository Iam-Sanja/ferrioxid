# Sitzungen und Cookies

[Zur Übersicht](index.md) · [auth.md](auth.md)

## Zweck

Das Admin- und Browser-Login nutzt serverseitige Sitzungen. JSON Web Tokens sind nicht der Standard.

## Architektur

Der Login erzeugt 32 Byte aus `OsRng` für das Session-Token und weitere 32 Byte für das CSRF-Token. Beide werden Base64url-kodiert. Die Datenbank speichert SHA-256 als Hex. `last_seen_at` wird bei jeder erfolgreichen Prüfung gesetzt. `expires_at` wird dabei nicht verlängert.

`rotate_session` widerruft die aktuelle Zeile und legt eine neue mit demselben `expires_at` an. Eine bereits abgelaufene Sitzung wird nicht erneuert. Der Login legt immer eine neue Sitzung an. Eine automatische Rotation bei jedem Request ist ausgeschaltet, weil dabei das CSRF-Token ohne zuverlässigen Kanal wechseln würde.

## Zuständige Dateien

- `crates/ferrum-auth/src/service.rs`
- `crates/ferrum-auth/src/cookie.rs`
- `crates/ferrum-auth/src/config.rs`
- `crates/ferrum-auth/migrations/0001_auth_schema.json`

## Öffentliche API

`AuthService::login` liefert `IssuedSession` mit Cookie-Zeile, rohem Token, CSRF-Token und `PublicUser`. HTTP-Handler setzen nur das Cookie und geben Token plus Benutzer im Login-JSON als `csrf_token` und `user` zurück. Folgeantworten enthalten kein Session-Token.

`logout` widerruft die Sitzung, wenn das Token noch bekannt ist, und löscht das Cookie. Ohne Cookie bleibt der Aufruf HTTP 204. Mit gültigem Cookie verlangt Logout das CSRF-Token.

## Datenmodell `_ferrum_sessions`

| Spalte | Bedeutung |
|---|---|
| `id` | UUID |
| `user_id` | Foreign Key auf `_ferrum_users`, `ON DELETE CASCADE` |
| `token_hash` | SHA-256-Hex, eindeutig |
| `csrf_token_hash` | SHA-256-Hex des CSRF-Tokens |
| `created_at`, `expires_at`, `last_seen_at` | Unix-Sekunden |
| `revoked_at` | gesetzt beim Logout, bei Rotation und beim Widerruf aller Sitzungen |

Standardlebensdauer: 12 Stunden absolut, 30 Minuten ohne Nutzung.

## Sicherheitsmechanismen

Das Cookie heißt `ferrum_session`. Es ist `HttpOnly`, `Path=/`, `SameSite=Lax` und ohne `Domain`. `Secure` steht nur, wenn `cookie_secure` gesetzt ist. Das entscheidet `FERRUM_COOKIE_SECURE` oder, wenn die Variable fehlt, `FERRUM_ENV=production`. `BIND_ADDRESS` spielt keine Rolle. Lokales HTTP bleibt in der Entwicklung ohne `Secure` nutzbar. Produktion ohne `Secure` schreibt eine Warnung.

Logout verwendet dieselben Cookie-Attribute und `Max-Age=0`.

Tokens stehen nicht in URLs. Die Datenbankzeile enthält nicht das rohe Token.

## Fehlerfälle

Abgelaufene, widerrufene, unbekannte und zu lange ungenutzte Tokens ergeben `Unauthenticated`. Ein deaktivierter Benutzer ebenfalls, auch wenn die Zeile noch gültig wäre.

## Beispiel

```http
POST /auth/logout
Cookie: ferrum_session=...
x-csrf-token: ...
```

Antwort 204 und `Set-Cookie` mit leerem Wert.

## Tests

`sqlite_tests` prüft Anlage, Hash, Ablauf, Idle-Timeout, Widerruf, Rotation, inaktive Benutzer und das Secure-Cookie. `examples/myshop/tests/auth.rs` prüft fehlenden, manipulierten und abgelaufenen Cookie sowie Logout.

## Einschränkungen

Idle-Timeout und absolute Laufzeit werden beim nächsten Request geprüft, nicht durch einen Hintergrundjob. Eine erneuerte Sitzung übernimmt das ursprüngliche `expires_at`.

## Geplante Erweiterungen

Geräteverwaltung und eine Sitzungsliste sind nicht umgesetzt. `GET /auth/csrf` rotiert nur den CSRF-Hash und lässt Ablauf sowie Sitzungstoken stehen. Die automatische Rotation je Request bleibt aus, damit ein Token für die folgende Admin-Aktion stabil ist. Ein Reload holt ein neues Token über denselben Endpunkt.
