# CSRF für mehrere Admin-Tabs

[Zur Übersicht](index.md) · [sessions.md](sessions.md) · [admin-security.md](admin-security.md)

## Zweck

Schreibende Cookie-Anfragen brauchen ein CSRF-Token, das an die gültige Sitzung gebunden ist. Ein Abruf in einem zweiten Browser-Tab darf die Schreibberechtigung des ersten Tabs nicht entziehen.

## Architektur

Das Token ist ein HMAC-SHA256 über den festen Präfix `ferrum-csrf-v1`, die Sitzungs-ID und den SHA-256-Hex des Session-Tokens. Der Schlüssel ist 32 Byte. Dieselbe Sitzung ergibt deshalb immer dasselbe Token. `GET /auth/csrf` schreibt die Datenbank nicht.

Logout, Passwortwechsel und Ablauf widerrufen oder verwerfen die Sitzung. Danach schlägt die Prüfung fehl, bevor das Token überhaupt verglichen wird. Ein Token einer anderen Sitzung passt nicht, weil ID und Token-Hash in den HMAC eingehen.

## Verantwortliche Dateien

- `crates/ferrum-auth/src/service.rs`
- `crates/ferrum-auth/src/config.rs`
- `crates/ferrum-auth/src/routes.rs`
- `crates/ferrum-auth/src/cookie.rs`
- `apps/admin-ui/src/api/client.ts`

## Öffentliche APIs

- `AuthConfig.csrf_secret`
- `AuthService::refresh_csrf`
- `GET /auth/csrf` liefert `{ "csrf_token": "..." }`
- Header `x-csrf-token` bei unsicheren Methoden

## Datenfluss

1. Login legt die Sitzung an und berechnet das Token.
2. Die Datenbank speichert nur `digest(token)` in `csrf_token_hash`, nicht das rohe Token.
3. Jeder weitere Abruf berechnet dasselbe Token neu.
4. `authenticate` öffnet die Sitzung und vergleicht den Header in konstanter Zeit.

## Sicherheitsregeln

- Tokens stehen nicht in URLs und werden nicht geloggt.
- Das Session-Cookie bleibt `HttpOnly`.
- Der Vergleich nutzt `constant_eq`.
- Entwicklung verwendet den festen Schlüssel `ferrum-dev-csrf-secret-32-bytes!`.
- `FERRUM_CSRF_SECRET` ist 64 Hex-Zeichen. Sonst liest Ferrum `.ferrum/csrf.key` und legt die Datei einmal an, wenn sie fehlt. Produktion soll die Umgebungsvariable setzen, damit ein Neustart nicht alle Sitzungen ungültig macht.
- Anonyme Demo-Schreibrouten prüfen kein CSRF. Sie hängen nicht am Cookie.

## Fehlerfälle

| Fall | Antwort |
|---|---|
| Header fehlt oder Token ist falsch | 403, `csrf token is missing or invalid` |
| Token einer anderen Sitzung | 403 |
| Sitzung abgelaufen, widerrufen oder Benutzer inaktiv | 401 |
| Logout ohne Token | 403, die Sitzung bleibt |

## Beispiel

```rust
let token = auth.refresh_csrf(&session_token).await?;
let again = auth.refresh_csrf(&session_token).await?;
assert_eq!(token, again);
```

```http
GET /auth/csrf
Cookie: ferrum_session=...

POST /admin/api/models/admin_products/records
Cookie: ferrum_session=...
x-csrf-token: ...
```

## Tests

`examples/myshop/tests/admin_http.rs` prüft zwei Abrufe, einen erfolgreichen Schreibzugriff mit dem Login-Token, ein gefälschtes Token, einen sitzungsfremden Token und den Admin-Schreibweg. `examples/myshop/tests/auth.rs` prüft Logout, fehlenden Header und eine abgelaufene Sitzung.

## Einschränkungen

Alle Tabs einer Sitzung teilen sich ein Token. Logout beendet deshalb alle Tabs dieser Sitzung. Es gibt keine Token-Liste mit einzelnem Widerruf. Der Schlüssel in `.ferrum/csrf.key` liegt im Arbeitsverzeichnis und ist nur die Entwicklungsquelle, wenn keine Umgebungsvariable gesetzt ist.
