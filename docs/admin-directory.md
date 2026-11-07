# Benutzer- und Rollenverzeichnis

[Zur Übersicht](index.md) · Sicherheit: [admin-security.md](admin-security.md)

**Stand:** v0.16. Nur ein angemeldeter Staff-Superuser erreicht die Endpunkte. Die Prüfung bleibt auf dem Server. Die HTML-Bestätigung `confirm=1` gilt nicht für die JSON-API.

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

Die HTML-Sperre für die eigene Deaktivierung und für Superuser antwortet 403, auch ohne Bestätigung. Superuser-Vergabe bleibt die CLI. Der Admin hat dafür kein Feld und lehnt `is_superuser` ab.

Aktiv, Staff und Superuser stehen in getrennten Spalten. Die Liste verlinkt Anzeigen, Bearbeiten und Rollen, wenn die Zeile kein Superuser ist. Aktivieren und Staff sind nur für andere, nicht geschützte Benutzer Formulare. Eine neue Rolle entsteht nur über das Rollenformular.

| Methode | Pfad | Wirkung |
|---|---|---|
| `GET`/`POST` | `/admin/directory/users/new` | E-Mail, Passwort, Wiederholung, Aktiv, Staff, optionale vorhandene Rollen |
| `GET` | `/admin/directory/users/{id}` | E-Mail, Flags, Rollen, Berechtigungscodes |
| `GET`/`POST` | `/admin/directory/users/{id}/edit` | E-Mail, Aktiv, Staff. Rollen über die bestehende Zuweisung |
| `POST` | `/admin/directory/users/{id}/password` | Neues Passwort, danach Sitzungswiderruf |

Diese Wege sind keine Modell-CRUD-Routen. Passwort-Hashes und Klartextpasswörter stehen nicht im HTML, in Logs oder in der Weiterleitung. Ein Superuser ohne Staff betritt den Admin nicht. Benutzer werden deaktiviert, nicht gelöscht.

v0.28 trennt die Navigation: Benutzerliste und Rollenliste sind eigene Einträge unter „Benutzer und Zugang“. „Rolle anlegen“ steht nur auf `/admin/directory/roles`. Die Benutzerliste weist vorhandene Rollen zu. Ein physisches Löschen ist nicht vorhanden. Es würde Sitzungen, Rollenbindungen und die Historie betreffen und bleibt einem eigenen Funktionsstand vorbehalten. Die Oberfläche bietet es nicht an.

Schreibende HTML-Aktionen des Verzeichnisses prüfen in dieser Reihenfolge: CSRF, Staff-Superuser, betroffene Zeile, Superuser- und Selbstsperre, dann `confirm`. Nur der Wert `1` gilt. Fehlt er, ist die Antwort 422 und der Datensatz bleibt. `true`, `on` oder ein leerer Wert gelten nicht. Die Bestätigung ersetzt keine Berechtigung.

Passwortänderung und Deaktivierung widerrufen alle Sitzungen des Ziels. Ändert ein Superuser sein eigenes Passwort, endet auch seine Sitzung und die Antwort führt zur Anmeldung. Staff-Entzug löscht die Sitzung nicht. Die nächste Anfrage liest `is_staff` neu und erhält 403.

## Tests

`examples/myshop/tests/admin_html.rs` sucht einen Benutzer, legt eine Rolle an, vergibt und entzieht eine Berechtigung, weist die Rolle zu, legt einen Benutzer an, setzt das Passwort, prüft die ungültige alte Sitzung, den Staff-Entzug, die Bestätigung, CSRF, die Superuser-Sperre und einen Nicht-Superuser. `examples/myshop/tests/admin_http.rs` prüft die JSON-Sperren. Die JSON-API verlangt das Häkchen nicht.
