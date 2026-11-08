# Admin-Backend

[Zur Übersicht](admin.md) · Sicherheit: [admin-security.md](admin-security.md)

## Zweck

`ferrioxid-admin` veröffentlicht registrierte Modelle unter `/admin/api`, prüft Rechte und liefert die gebaute Oberfläche aus. Es gibt keine zweite ORM- und keine zweite Anmeldeimplementierung.

## Dateien

| Datei | Aufgabe |
|---|---|
| `crates/ferrioxid-admin/src/lib.rs` | `mount` |
| `crates/ferrioxid-admin/src/registry.rs` | `AdminRegistry`, Adapter auf `ModelRepository` |
| `crates/ferrioxid-admin/src/http.rs` | Routen, Schema, Verzeichnis |
| `crates/ferrioxid-admin/src/assets.rs` | Dateiauslieferung, Pfadprüfung |
| `crates/ferrioxid-db/src/sql.rs` | parameterisierte Listenabfrage |

Die Crate hängt von `ferrioxid-core`, `ferrioxid-db` und `ferrioxid-auth` ab, nicht von `ferrioxid-api`. `ferrioxid-api` ruft `ferrioxid_admin::mount` auf.

## Schnittstelle

```http
GET    /admin/api/me
GET    /admin/api/models
GET    /admin/api/models/{model}/schema
GET    /admin/api/models/{model}/records
GET    /admin/api/models/{model}/records/{id}
POST   /admin/api/models/{model}/records
PATCH  /admin/api/models/{model}/records/{id}
DELETE /admin/api/models/{model}/records/{id}
GET    /admin/api/models/{model}/fields/{field}/options
GET    /auth/csrf
```

`{model}` ist der Tabellenname aus `AdminModelMeta.key`. Unbekannte Schlüssel antworten 404 und werden nicht als SQL-Bezeichner verwendet.

Schemafelder enthalten `name`, `label`, `type` (`string`, `integer`, `boolean`, `uuid`), `required`, `readonly`, `nullable`, `searchable`, `sortable`, `filterable`, `list_display` und `foreign_key`. Versteckte und sensible Felder fehlen.

Listenparameter: `page` ab 1, `page_size` 1 bis 100, `sort`, `dir=desc`, `q`, `filter.{feld}`. Die Sortierung hängt den Primärschlüssel als zweites Kriterium an. Die Suche nutzt `LOWER(spalte) LIKE LOWER(?) ESCAPE '\'` mit gebundenem Muster.

## Verzeichnis

Nur Staff-Superuser:

```http
GET/POST /admin/api/directory/users
POST     /admin/api/directory/users/{id}/active
POST     /admin/api/directory/users/{id}/staff
GET/POST /admin/api/directory/roles
GET      /admin/api/directory/roles/{role}/permissions
POST/DELETE /admin/api/directory/roles/{role}/permissions/{code}
POST/DELETE /admin/api/directory/assignments
```

Passwort-Hashes stehen nicht in `PublicUser`. Änderungen an `is_superuser` gibt es nicht. Ein Superuser kann sich nicht selbst deaktivieren oder das eigene Staff-Flag entfernen. Berechtigungscodes mit dem Bestandteil `superuser` werden nicht vergeben.

## Konfiguration

Die HTML-Oberfläche kommt aus dem Binary. Siehe [admin-assets.md](admin-assets.md) und [admin-routing.md](admin-routing.md). Die JSON-Routen dieser Datei bleiben.

## Fehler

Ungültige Filter und Sortierungen sind 422. Schreibversuche auf Primärschlüssel, `readonly` oder unbekannte Schlüssel sind `field:{name}: field is not writable`. Fremdschlüssel-Optionen sind 403, wenn das Ziel nicht registriert ist oder eine Objektregel trägt. Es werden höchstens 25 Zeilen geliefert. Das Anzeigefeld ist die erste durchsuchbare Textspalte.

## Tests

`sqlite_admin_security_crud_and_csrf_bootstrap` in `examples/myshop/tests/admin_http.rs`. Die HTML-Wege prüft `examples/myshop/tests/admin_html.rs`. Die HTML-Fremdschlüsselauswahl lädt höchstens 100 Zeilen.

## Grenzen

Die Listenabfrage ergänzt das vorhandene SeaQuery, erzeugt aber keine zweite Schemaquelle. Prozentkodierung in Filtern ist ASCII-orientiert. `json_of` setzt bei einem Serialisierungsfehler ein leeres Objekt; die generierten Modelle sind serialisierbar.

## Später

Eingebettete Assets und ein SQL-Scope, damit Objektregeln Listen nicht mehr pauschal sperren.
