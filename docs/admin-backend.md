# Admin-Backend

[For overview](admin.md) · Security: [admin-security.md](admin-security.md)]

## Zweck

`ferrioxid-admin` publishes registered models under `/admin/api`, checks rights and delivers the built surface. There is no second ORM and no second login implementation.

## Dateien

|File| Aufgabe |
|---|---|
| `crates/ferrioxid-admin/src/lib.rs` | `mount` |
| `crates/ferrioxid-admin/src/registry.rs` | `AdminRegistry`, Adapter auf `ModelRepository` |
| `crates/ferrioxid-admin/src/http.rs` |Routes, scheme, directory|
| `crates/ferrioxid-admin/src/assets.rs` |File delivery, path verification|
| `crates/ferrioxid-db/src/sql.rs` | parameterisierte Listenabfrage |

The crate depends on `ferrioxid-core`, `ferrioxid-db` and `ferrioxid-auth`, not `ferrioxid-api`. `ferrioxid-api` calls `ferrioxid_admin::mount`.

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

`{model}` is the table name from `AdminModelMeta.key`. Unknown keys respond 404 and are not used as SQL identifiers.

Schema fields include `name`, `label`, `type` (`string`, `integer`, `boolean`, `uuid`), `required`, `readonly`, `nullable`, ZXQQ, `searchable`, ZXQQ, `sortable`, ZXQQ, `filterable`, `list_display` and `foreign_key`. Hidden and sensitive fields are missing.

List parameters: `page` from 1, `page_size` 1 to 100, `sort`, `dir=desc`, `q`, `filter.{feld}`. Sorting attaches the primary key as a second criterion. The search uses `LOWER(spalte) LIKE LOWER(?) ESCAPE '\'` with a bound pattern.

## List

Staff Superusers only:

```http
GET/POST /admin/api/directory/users
POST     /admin/api/directory/users/{id}/active
POST     /admin/api/directory/users/{id}/staff
GET/POST /admin/api/directory/roles
GET      /admin/api/directory/roles/{role}/permissions
POST/DELETE /admin/api/directory/roles/{role}/permissions/{code}
POST/DELETE /admin/api/directory/assignments
```

Password hashes are not in `PublicUser`. There are no changes to `is_superuser`. A superuser cannot deactivate itself or remove its own staff flag. Eligibility codes with the component `superuser` are not assigned.

## Konfiguration

The HTML interface comes from the binary. See [admin-assets.md](admin-assets.md) and [admin-routing.md](admin-routing.md)]. The JSON routes of this file remain.

## Fehler

Invalid filters and sorts are 422. Write attempts to primary key, `readonly` or unknown keys are `field:{name}: field is not writable`. Foreign key options are 403 if the destination is not registered or carries an object rule. A maximum of 25 lines shall be provided. The display field is the first searchable text column.

## Tests

`sqlite_admin_security_crud_and_csrf_bootstrap` in `examples/myshop/tests/admin_http.rs`. The HTML paths are checked by `examples/myshop/tests/admin_html.rs`. The HTML foreign key selection loads a maximum of 100 lines.

## Grenzen

The list query complements the existing SeaQuery, but does not generate a second schema source. Percentage coding in filters is ASCII-oriented. `json_of` sets an empty object in the event of a serialization error; The generated models can be serialized.

## Later

Embedded assets and an SQL scope so that object rules no longer block lists.
