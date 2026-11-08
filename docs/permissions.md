# Roles and authorisations

[For overview](index.md) · [auth.md](auth.md)] · [policies.md](policies.md)]

## Zweck

Rights apply on the server side. A change in role or authorization takes effect at the next request. Ferrioxide does not cached these rights in session.

## Architektur

A user can have multiple roles. A role can have multiple permissions. The code has the form `ressource.aktion`. The promotion is `read`, `create`, `update` or `delete`.

`authorize` checks the tables `_ferrum_user_roles`, `_ferrum_role_permissions` and `_ferrum_permissions` on each call.

## Responsible files

- `crates/ferrioxid-auth/src/service.rs`
- `crates/ferrioxid-auth/src/store.rs`
- `crates/ferrioxid-macros/src/lib.rs` für `permissions`
- `crates/ferrioxid-api/src/lib.rs` for assigning HTTP method to action

## Public API

```rust
auth.ensure_role("editor").await?;
auth.grant_permission("editor", "notes.read").await?;
auth.assign_role("editor@example.com", "editor").await?;
auth.revoke_permission("editor", "notes.read").await?;
auth.revoke_role("editor@example.com", "editor").await?;
```

The CLI provides the same steps: `auth create-role`, `auth grant-permission`, `auth revoke-permission`, `auth assign-role`, `auth revoke-role`.

## Datenmodell

| Tabelle | Inhalt |
|---|---|
| `_ferrum_roles` | `id`, eindeutiger `name` |
| `_ferrum_permissions` | `id`, eindeutiger `code` |
| `_ferrum_user_roles` |own UUID, `user_id`, `role_id`, uniquely as a pair|
| `_ferrum_role_permissions` |own UUID, `role_id`, `permission_id`, uniquely as a pair|

Foreign Keys delete assignments via `CASCADE`.

## Modellattribut

```rust
#[ferrioxid(table = "internal_notes", api = "/api/v1/internal_notes", permissions = "notes")]
```

| Attribut | Wirkung |
|---|---|
| keines, `allow_anonymous = false` | jede Methode 403 |
| `allow_anonymous = true` |Public, without cookie check|
| `permissions = "authenticated"` | jede aktive Sitzung |
| `permissions = "notes"` | `notes.read`, `notes.create`, `notes.update`, `notes.delete` je nach Methode |

`allow_anonymous` together with `permissions` or `object_policy` is a compile error. The standard remains refusal.

GET list and GET detail check `read`. POST checks `create`. PATCH checks `update`. DELETE checks `delete`.

An active superuser passes the role test. `is_staff` does not.

## Security mechanisms

There is no authorization without entry. `grant_permission` creates role and code if they are missing. `revoke_permission` does not create anything and is without hits.

## Error cases

Missing rights are HTTP 403 `Access denied`. A missing session is HTTP 401 and is returned before the rights check.

## Example

```powershell
cargo run -p ferrioxid-cli -- auth create-role editor
cargo run -p ferrioxid-cli -- auth grant-permission notes.read --role editor
cargo run -p ferrioxid-cli -- auth assign-role --email editor@example.com --role editor
```

myshop protects `InternalNote` with `permissions = "notes"`. `Product` and `Category` remain `allow_anonymous = true` and are demo endpoints.

## Tests

`sqlite_tests` forgives and withdraws `notes.*` and checks the superuser. `examples/myshop/tests/auth.rs` checks the protected HTTP methods and the anonymous demo route.

## Limitations

There are no field rights. Permissions are global codes, not line filters. Object rules are responsible for this. Lists use the same `QueryScope` as the REST API. See [policies.md](policies.md) and [query-scopes.md](query-scopes.md)].

## Planned extensions

Field rights, groups beyond roles and a cache with targeted disability are not implemented.

## Admin

Staff alone does not grant model rights. A superuser skips the role test, but needs `is_staff` for the admin. An empty admin permission means superuser-only. `permissions = "authenticated"` allows any Staff user after the Staff exam. The directory checks superusers separately from the model rights.
