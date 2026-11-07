# CSRF für mehrere Admin-Tabs

[Zur Übersicht](index.md) · [sessions.md](sessions.md) · [admin-security.md](admin-security.md)

## Zweck

Schreibende Cookie-Anfragen brauchen ein CSRF-Token, das an die gültige Sitzung gebunden ist. Ein Abruf in einem zweiten Browser-Tab darf die Schreibberechtigung des ersten Tabs nicht entziehen.

## Architektur

Das Token ist ein HMAC-SHA256 über den festen Präfix `ferrum-csrf-v1`, die Sitzungs-ID und den SHA-256-Hex des Session-Tokens. Der Schlüssel ist 32 Byte. Dieselbe Sitzung ergibt deshalb immer dasselbe Token. `GET /auth/csrf` schreibt die Datenbank nicht. Seit v0.12 akzeptiert `authenticate_csrf` dasselbe Token aus einem HTML-Feld. `authenticate` liest weiter nur `x-csrf-token`. Eine Anfrage ohne Sitzungscookie bleibt für öffentliche Formulare ohne diese Prüfung.

Logout, Passwortwechsel und Ablauf widerrufen oder verwerfen die Sitzung. Danach schlägt die Prüfung fehl, bevor das Token überhaupt verglichen wird. Ein Token einer anderen Sitzung passt nicht, weil ID und Token-Hash in den HMAC eingehen.

## Verantwortliche Dateien

- `crates/ferrum-auth/src/service.rs`
- `crates/ferrum-auth/src/config.rs`
- `crates/ferrum-auth/src/routes.rs`
- `crates/ferrum-auth/src/cookie.rs`
- `crates/ferrum-admin/src/html.rs`

## Öffentliche APIs

- `AuthConfig.csrf_secret`
- `AuthService::refresh_csrf`
- `GET /auth/csrf` liefert `{ "csrf_token": "..." }`
- Header `x-csrf-token` bei unsicheren JSON-Methoden
- HTML-Feld `csrf_token` mit demselben HMAC
- Cookie `ferrum_login_csrf` nur für `POST /admin/login`, bevor eine Sitzung existiert. Es ist 64 Hex-Zeichen, HttpOnly, `Path=/`, SameSite aus der Auth-Konfiguration, `Secure` nur bei `cookie_secure` und `Max-Age=600`. Ein gültiges Cookie wird für weitere Tabs wiederverwendet.

## Datenfluss

1. Login legt die Sitzung an und berechnet das Token.
2. Die Datenbank speichert nur `digest(token)` in `csrf_token_hash`, nicht das rohe Token.
3. Jeder weitere Abruf berechnet dasselbe Token neu.
4. `authenticate` öffnet die Sitzung und vergleicht den Header in konstanter Zeit.

## Sicherheitsregeln

- Tokens stehen nicht in URLs und werden nicht geloggt. Eine abgelehnte HTML-Anmeldung nennt intern nur `origin_rejected`, `form_token_missing`, `login_csrf_missing` oder `login_csrf_mismatch`.
- Sendet der Browser `Origin` oder `Referer`, muss der Wert exakt zu den erlaubten Origins passen. Die Liste kommt aus `FERRUM_ALLOWED_ORIGINS` oder aus der Loopback-Bind-Adresse, nicht aus dem Host-Header. `http://127.0.0.1:3000` gilt nur, wenn der Prozess wirklich an Port 3000 gebunden ist oder die Variable ihn nennt.
- `Referrer-Policy: no-referrer` lässt Browser bei einem HTML-Formular-POST `Origin: null` senden, auch wenn die Seite same-origin ist. Das gilt nur, wenn zusätzlich `Sec-Fetch-Site: same-origin` steht. Diesen Header kann eine Seite nicht selbst setzen. `null` mit `cross-site`, `same-site` oder ohne diesen Header bleibt abgelehnt. `null` steht nicht auf der Allowlist.
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

`examples/myshop/tests/admin_http.rs` prüft zwei Abrufe, einen erfolgreichen JSON-Schreibzugriff, ein gefälschtes Token und einen sitzungsfremden Token. `examples/myshop/tests/admin_html.rs` prüft das Login-Cookie, ein falsches Token, das Sitzungsfeld, Logout und eine abgelaufene Sitzung. `html_login_csrf_respects_the_bound_port` prüft `127.0.0.1` und `localhost` am gebundenen Port, den alten Port 3000 als fremde Origin, fehlendes und manipuliertes Cookie, fehlendes und falsches Formularfeld, eine fremde Origin, zwei Tabs, Login, Logout und erneuten Login. `examples/myshop/tests/auth.rs` prüft den JSON-Logout. JSON `POST /auth/login` verlangt weiter kein CSRF-Cookie.

## Einschränkungen

Alle Tabs einer Sitzung teilen sich ein Token. Logout beendet deshalb alle Tabs dieser Sitzung. Es gibt keine Token-Liste mit einzelnem Widerruf. Der Schlüssel in `.ferrum/csrf.key` liegt im Arbeitsverzeichnis und ist nur die Entwicklungsquelle, wenn keine Umgebungsvariable gesetzt ist.
