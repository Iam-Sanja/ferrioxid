# Admin backend

[Overview](admin.md) · Security: [admin-security.md](admin-security.md)

## Purpose

`fexu-admin` exposes registered models under `/admin/api`, checks permissions and serves the native HTML interface. It reuses the existing ORM and authentication implementation.

## Files

| File | Responsibility |
|---|---|
| `crates/fexu-admin/src/lib.rs` | `mount` |
| `crates/fexu-admin/src/registry.rs` | `AdminRegistry`, adapters for `ModelRepository` |
| `crates/fexu-admin/src/http.rs` | Routes, schema and directory API |
| `crates/fexu-admin/src/assets.rs` |File delivery, path verification|
| `crates/fexu-db/src/sql.rs` | Parameterized list queries |

The crate depends on `fexu-core`, `fexu-db` and `fexu-auth`, not `fexu-api`. `fexu-api` calls `fexu_admin::mount`.

## Interface

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

Schema fields include `name`, `label`, `type` (`string`, `integer`, `boolean`, `uuid`), `required`, `readonly`, `nullable`, `searchable`, `sortable`, `filterable`, `list_display` and `foreign_key`. Hidden and sensitive fields are omitted.

List parameters are `page` starting at 1, `page_size` from 1 to 100, `sort`, `dir=desc`, `q` and `filter.{feld}`. Sorting adds the primary key as a tie-breaker. Search uses `LOWER(column) LIKE <bound pattern> ESCAPE '\'`. The placeholder is `?` on SQLite and `$1` on PostgreSQL. `%`, `_` and `\` in the term stay literals. Case folding follows the backend `LOWER` rules documented in [database.md](database.md): SQLite folds ASCII only, so `München` does not match `MÜNCHEN` there.

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

`PublicUser` never contains password hashes. These endpoints cannot change `is_superuser`. Superusers cannot deactivate themselves or remove their own staff flag. Permission codes containing the component `superuser` cannot be assigned.

## Configuration

The HTML interface comes from the binary. See [admin-assets.md](admin-assets.md) and [admin-routing.md](admin-routing.md). The JSON routes of this file remain.

## Errors

Invalid filters and sort fields return 422. Attempts to write a primary key, `readonly` field or unknown key return `field:{name}: field is not writable`. Foreign-key options return 403 if the target model is not registered or has an object policy. The JSON endpoint returns at most 25 options, labeled by the first searchable text field.

## Tests

`sqlite_admin_security_crud_and_csrf_bootstrap` in `examples/myshop/tests/admin_http.rs`. The HTML paths are checked by `examples/myshop/tests/admin_html.rs`. The HTML foreign key selection loads a maximum of 100 lines.

## Limitations

List queries extend the existing SeaQuery layer without introducing another schema source. Percent decoding in filters is ASCII-oriented. `json_of` returns an empty object on serialization failure; generated model values are serializable.

## Later

Any further changes must reuse the embedded assets and SQL query-scope implementation. See [query-scopes.md](query-scopes.md) for the current object-policy behavior.
