# Benutzer- und Rollenverzeichnis

[Zur Übersicht](index.md) · Sicherheit: [admin-security.md](admin-security.md)

**Stand:** v0.11. Nur ein angemeldeter Staff-Superuser erreicht die Endpunkte. Die Prüfung bleibt auf dem Server.

## API

| Methode | Pfad | Wirkung |
|---|---|---|
| `GET` | `/admin/api/directory/users` | Benutzer ohne `password_hash`, mit sortierten Rollennamen |
| `POST` | `/admin/api/directory/users/{id}/active` | aktivieren oder deaktivieren |
| `POST` | `/admin/api/directory/users/{id}/staff` | Staff-Flag setzen |
| `GET`/`POST` | `/admin/api/directory/roles` | Rollen lesen oder anlegen |
| `GET` | `/admin/api/directory/roles/{role}/permissions` | Codes der Rolle |
| `POST`/`DELETE` | `/admin/api/directory/roles/{role}/permissions/{code}` | Recht geben oder entziehen |
| `POST`/`DELETE` | `/admin/api/directory/assignments` | Rolle an E-Mail binden oder lösen |

`GET /auth/me` bleibt das kleine `PublicUser` ohne Rollen. Die Rollen stehen nur in der Verzeichnisantwort. Sie entstehen aus `_ferrum_roles` und `_ferrum_user_roles`, ohne das Passwort-Hash zu lesen.

Eigenes Deaktivieren und der Entzug des eigenen Staff-Zugangs antworten 422. Änderungen an Superusern und Berechtigungscodes mit `superuser` antworten 403. Es gibt keinen Endpunkt, der einen Benutzer zum Superuser macht.

## Oberfläche

`/admin/directory/users` sucht die E-Mail serverseitig, sortiert sie und zeigt 25 Zeilen je Seite. Status und Rollen stehen in der Tabelle. Deaktivieren, Staff ändern, Rechte vergeben und Rollen zuweisen sind POST-Formulare mit CSRF. Abbrechen ist der Link zurück, kein Dialog.

Die HTML-Sperre für die eigene Deaktivierung und für Superuser antwortet 403. Passwort ändern, Benutzer anlegen und Superuser vergeben bleiben bei der Kommandozeile. Dafür gibt es keine Admin-Schaltfläche.

## Tests

`examples/myshop/tests/admin_html.rs` sucht einen Benutzer, legt eine Rolle an, vergibt und entzieht eine Berechtigung und weist die Rolle zu. `examples/myshop/tests/admin_http.rs` prüft die JSON-Sperren.
