# Admin-Routen

[Zur Übersicht](admin-native.md) · Sicherheit: [admin-security.md](admin-security.md) · CSRF: [csrf.md](csrf.md)

## HTML

| Methode | Pfad | Wirkung |
|---|---|---|
| GET | `/admin`, `/admin/` | Übersicht. Ohne Sitzung Weiterleitung zur Anmeldung. |
| GET | `/admin/login` | Formular. Setzt oder übernimmt `ferrum_login_csrf`. |
| POST | `/admin/login` | Prüft Login-CSRF, Origin und Zugangsdaten. Danach 303. |
| POST | `/admin/logout` | Prüft das Sitzungs-CSRF und beendet die Sitzung. |
| POST | `/admin/preferences` | Sprache und Theme. Verlangt Staff und Sitzungs-CSRF. |
| GET | `/admin/models` | Weiterleitung auf `/admin`. |
| GET | `/admin/models/{model}` | Liste. Query: `q`, `sort`, `dir`, `page`, `filter.{feld}`. |
| GET | `/admin/models/{model}/new` | Leeres Formular. |
| POST | `/admin/models/{model}/new` | Anlegen, danach 303 auf das Detail. |
| GET | `/admin/models/{model}/{id}` | Detail. |
| GET, POST | `/admin/models/{model}/{id}/edit` | Formular und Speichern. |
| GET | `/admin/models/{model}/{id}/delete` | Bestätigung. Löscht nichts. |
| POST | `/admin/models/{model}/{id}/delete` | Löschen, danach 303 auf die Liste. |
| GET | `/admin/directory/users` | Benutzer. Nur Staff-Superuser. |
| GET, POST | `/admin/directory/users/new` | Benutzer anlegen. Kein Superuser-Feld. |
| GET | `/admin/directory/users/{id}` | Benutzerdetail ohne Hash. |
| GET, POST | `/admin/directory/users/{id}/edit` | E-Mail, Aktiv und Staff. |
| POST | `/admin/directory/users/{id}/password` | Passwort setzen und Sitzungen des Ziels widerrufen. |
| POST | `/admin/directory/users/{id}/active` | Aktivieren oder deaktivieren. |
| POST | `/admin/directory/users/{id}/staff` | Staff geben oder entziehen. |
| GET, POST | `/admin/directory/roles` | Rollen anzeigen und anlegen. |
| GET | `/admin/directory/permissions` | Berechtigungen, optional `?role=`. |
| POST | `/admin/directory/roles/{role}/permissions` | `action=grant` oder `revoke`. |
| POST | `/admin/directory/assignments` | Rolle zuweisen oder entziehen. |
| GET | `/admin/assets/admin.css` | Stylesheet. |
| GET | `/admin/assets/admin.js` | Optionales Script. |

GET verändert keine Daten. Schreibende Aktionen sind POST und antworten bei Erfolg mit 303. Query-Parameter legen, ändern oder löschen nichts.

## JSON

`/admin/api` bleibt die bestehende JSON-API: Konfiguration, Sitzung, Modelle, Datensätze und Verzeichnis. Sie liest CSRF aus dem Header `x-csrf-token`. HTML-Formulare senden dasselbe Token als Feld `csrf_token`. Die beiden Wege teilen sich `authenticate_csrf`.

## Anmeldung ohne Sitzung

Das Sitzungs-CSRF existiert erst nach dem Login. Das Anmeldeformular nutzt deshalb ein eigenes Cookie `ferrum_login_csrf`. Es ist 64 Hex-Zeichen, HttpOnly, Pfad `/`, SameSite aus der Auth-Konfiguration, `Secure` nur wenn das Session-Cookie `Secure` ist, und höchstens zehn Minuten gültig. Die HTML-Antwort setzt `Cache-Control: no-store`, damit Formular und Cookie zusammenbleiben. Mehrere Tabs übernehmen dasselbe gültige Cookie. Nach erfolgreicher Anmeldung wird es gelöscht. Die JSON-Route `POST /auth/login` bleibt ohne dieses Cookie, wie bisher.

`Origin` oder, falls kein Origin gesendet wird, `Referer` muss zur Allowlist passen. Fehlen beide, bleibt die Anfrage aus Sicht der Origin-Prüfung erlaubt; das Login-Cookie ist davon unabhängig Pflicht. Die Allowlist liest `AuthConfig::apply_bound_address` aus `BIND_ADDRESS` oder aus `server.host` und `server.port` in `ferrum.toml`. Ohne gesetztes `FERRUM_ALLOWED_ORIGINS` sind das `http://127.0.0.1:{port}` und `http://localhost:{port}`. Ein früher fest eingetragener Port 3000 gilt dann nicht mehr, wenn der Prozess an einem anderen Port lauscht. Öffentliche Hosts und der Host-Header der Anfrage werden nicht ergänzt. `Origin: null` ist ungültig, außer der Browser sendet gleichzeitig `Sec-Fetch-Site: same-origin`. Das passiert bei `Referrer-Policy: no-referrer` für ein Formular derselben Origin. Die sichtbare Fehlermeldung bleibt „Die Sicherheitsprüfung ist fehlgeschlagen.“ Das Log nennt den Grundcode und bei einer Origin-Ablehnung nur Quelle, Klasse und Port, nie das Cookie oder das Token.

`next` wird nur akzeptiert, wenn der Wert mit `/admin` beginnt, höchstens 200 Zeichen hat und weder `//`, `\`, `://` noch Steuerzeichen enthält. Alles andere wird zu `/admin`.

## Fehler

| Lage | HTML |
|---|---|
| Auth ist nicht konfiguriert | 403, Klartext |
| Keine Sitzung | 303 auf `/admin/login?next=` |
| Kein Staff | 403, Klartext `staff access is required` |
| Fehlendes Modellrecht | 403, Fehlerseite |
| Datensatz nicht sichtbar | 404, derselbe Text wie bei einem fehlenden Datensatz |
| Ungültige Sortierung oder ungültiger Filter | 422 |
| Fehlendes oder falsches CSRF | 403 |
| Falsche Zugangsdaten, unbekannte E-Mail oder inaktiver Benutzer | 401, derselbe Text |

Die JSON-API behält ihre bisherigen Statuscodes. Eine anonyme JSON-Anfrage bleibt 401.
