# Fehlercodes

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrioxid-core/src/problem.rs`.

## Vertrag

`problem_json` baut die Antwort für API, Auth und Admin:

```json
{
  "type": "about:blank",
  "code": "validation.required",
  "title": "Validation failed",
  "status": 422,
  "detail": "field:name: Field is required",
  "field": "name",
  "params": {}
}
```

`field` fehlt, wenn kein `field:<name>:`-Präfix vorliegt. `name` darf nur Buchstaben, Ziffern und `_` enthalten und höchstens 64 Zeichen lang sein. `detail` bleibt für bestehende Clients erhalten. Die Statuscodes sind unverändert.

## Codes

| Code | Bisheriger `detail` |
|---|---|
| `validation.required` | `Field is required`, `active is required`, `email and role are required` |
| `validation.password` | `Password must contain 12 to 128 characters` |
| `validation.field` | anderes `field:<name>:`-Präfix |
| `auth.invalid_credentials` | `Invalid email or password` |
| `auth.required` | `Authentication required` |
| `auth.forbidden` | `Access denied` |
| `auth.csrf` | `csrf token is missing or invalid` |
| `auth.origin` | `origin is not allowed` |
| `auth.not_configured` | `authentication is not configured` |
| `auth.rate_limited` | `Too many login attempts` |
| `auth.rejected` | `request rejected` |
| `resource.not_found` | `Resource not found`, `user was not found` |
| `database.failed` | `Database operation failed` |
| `server.error` | `Internal server error` |
| `policy.unbound` | ungebundene Objektregel |
| `policy.create_rejected` | `create policy rejected the record` |
| `list.sort` | `sort field is not allowed` |
| `list.filter` | `filter field is not allowed` |
| `list.search` | `search is not available` |
| `list.relation` | blockierte Fremdschlüsselauswahl |
| `directory.self_deactivate` | eigenes Konto deaktivieren |
| `directory.self_staff` | eigenen Staff-Zugang entziehen |
| `directory.superuser_blocked` | `superuser changes are blocked` |
| `request.rejected` | verdächtiger oder zu langer Text, ersetzt durch `Request rejected` |
| `request.failed` | sonstiger kurzer Text |
| `validation.min_length` | Formularwert zu kurz |
| `validation.max_length` | Formularwert zu lang |
| `validation.email` | Formular-E-Mail ungültig |
| `validation.url` | Formular-URL ungültig |
| `validation.invalid_type` | Formularzahl oder Bool ungültig |
| `validation.uuid` | Formular-UUID ungültig |
| `validation.unknown_field` | zusätzliches Formularfeld |
| `validation.mismatch` | Kreuzfeldregel |
| `upload.too_large` | Upload über dem Limit, Status 413 |
| `upload.type` | Inhalt nicht erlaubt, Status 415 |
| `resource.not_found` | eigene Datei fehlt oder gehört jemand anderem, Status 404 |

Unbekannte Benutzer und falsche Passwörter erzeugen denselben Code und denselben `detail`. Texte mit `SELECT`, `INSERT`, `UPDATE`, `DELETE FROM`, `password_hash` oder `stack backtrace` sowie Texte über 240 Zeichen werden zu `request.rejected`.

## Frontend

`translateError` sucht `errors.<code>`. Fehlt die Übersetzung, wird `detail` gezeigt, sofern er nicht nach SQL oder Hash aussieht. Sonst gilt `errors.unknown`. Feldfehler bleiben am Eingabefeld, auch wenn der alte `field:<name>:`-Präfix noch im `detail` steht.

## Tests

`crates/ferrioxid-core/src/problem.rs` prüft gleiche Anmeldungsfehler, den Feldpräfix und unterdrücktes SQL. Die bestehenden Auth- und Admin-Tests lesen weiterhin `detail`.
