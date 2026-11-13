# Roles and permissions

[Overview](index.md) · [auth.md](auth.md) · [policies.md](policies.md)

## Purpose

Permissions are enforced on the server and reloaded on each request. Role and permission changes take effect immediately without being cached in the session.

## Architecture

A user can have several roles, and each role can grant several permissions. Codes use the form `ressource.aktion` (resource and action), with actions `read`, `create`, `update` or `delete`.

`authorize` checks the tables `_fexu_user_roles`, `_fexu_role_permissions` and `_fexu_permissions` on each call.

## Responsible files

- `crates/fexu-auth/src/service.rs`
- `crates/fexu-auth/src/store.rs`
- `crates/fexu-macros/src/lib.rs` for `permissions`
- `crates/fexu-api/src/lib.rs` for assigning HTTP method to action

## Public API

```rust
auth.ensure_role("editor").await?;
auth.grant_permission("editor", "notes.read").await?;
auth.assign_role("editor@example.com", "editor").await?;
auth.revoke_permission("editor", "notes.read").await?;
auth.revoke_role("editor@example.com", "editor").await?;
```

The CLI provides the same steps: `auth create-role`, `auth grant-permission`, `auth revoke-permission`, `auth assign-role`, `auth revoke-role`.

## Data model

| Table | Contents |
|---|---|
| `_fexu_roles` | `id`, unique `name` |
| `_fexu_permissions` | `id`, unique `code` |
| `_fexu_user_roles` |own UUID, `user_id`, `role_id`, uniquely as a pair|
| `_fexu_role_permissions` |own UUID, `role_id`, `permission_id`, uniquely as a pair|

Foreign Keys delete assignments via `CASCADE`.

## Model attribute

```rust
#[fexu(table = "internal_notes", api = "/api/v1/internal_notes", permissions = "notes")]
```

| Attribute | Effect |
|---|---|
| No permission and `allow_anonymous = false` | Every method returns 403. |
| `allow_anonymous = true` | Public without a session cookie. A session cookie still requires CSRF on writes. |
| `permissions = "authenticated"` | Any active authenticated session |
| `permissions = "notes"` | `notes.read`, `notes.create`, `notes.update` or `notes.delete`, according to the method |

`allow_anonymous` together with `permissions` or `object_policy` is a compile error. The standard remains refusal.

GET list and GET detail check `read`. POST checks `create`. PATCH checks `update`. DELETE checks `delete`.

An active superuser passes the role test. `is_staff` does not.

## Security mechanisms

Permissions must be explicitly granted. `grant_permission` creates a missing role or permission code; `revoke_permission` creates nothing and succeeds when no matching grant exists.

## Error cases

Missing permissions are HTTP 403 `Access denied`. A missing session is HTTP 401 and is returned before the permissions check.

## Example

```powershell
cargo run -p fexu-cli -- auth create-role editor
cargo run -p fexu-cli -- auth grant-permission notes.read --role editor
cargo run -p fexu-cli -- auth assign-role --email editor@example.com --role editor
```

myshop protects `InternalNote` with `permissions = "notes"`. `Product` and `Category` remain `allow_anonymous = true` and are demo endpoints.

## Tests

`sqlite_tests` grants and revokes `notes.*` permissions and checks superuser behavior. `examples/myshop/tests/auth.rs` covers protected HTTP methods and anonymous demo routes.

## Limitations

There are no field permissions. Permissions are global codes, not line filters. Object rules are responsible for this. Lists use the same `QueryScope` as the REST API. See [policies.md](policies.md) and [query-scopes.md](query-scopes.md).

## Planned extensions

Field permissions, groups beyond roles and permission caching with targeted invalidation are not implemented.

## Admin

Staff status alone grants no model permissions. A superuser bypasses role checks but still needs `is_staff` to enter the admin. An empty admin permission allows superusers only. `permissions = "authenticated"` allows any authenticated staff user. The user directory checks superuser status separately from model permissions.
