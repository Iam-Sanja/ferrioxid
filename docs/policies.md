# Object policies

[Overview](index.md) · [permissions.md](permissions.md)

## Purpose

An object policy decides access using the authenticated user and record. It supplements role-based permissions.

## Architecture

```rust
pub trait ObjectPolicy<T> {
    fn can_read(user: &AuthUser, object: &T) -> bool;
    fn can_update(user: &AuthUser, object: &T) -> bool;
    fn can_delete(user: &AuthUser, object: &T) -> bool;
    fn read_scope(user: &AuthUser) -> QueryScope { QueryScope::None }
    fn write_scope(user: &AuthUser) -> QueryScope { Self::read_scope(user) }
}
```

The model sets `object_policy = true` and a `permissions` attribute. The application registers the rule with `register_policy::<Model, Policy>()` instead of with `register`. `register` alone answers with 403 on this model and the indication that the rule is not bound.

The rule runs after the session and authorization check. An active superuser skips `can_read`, `can_update` and `can_delete`.

## Responsible files

- `crates/fexu-auth/src/policy.rs`
- `crates/fexu-api/src/lib.rs`
- `crates/fexu-macros/src/lib.rs`

## Public API

`fexu::auth::ObjectPolicy` and `App::register_policy`.

For GET, PATCH and DELETE, the handler loads the record. If the rule fails, the answer is 404, even if the line exists. Thus, the interface does not confirm the identifier.

POST checks the Create right first and then `can_create`. The method gets the Create type. The standard is `false`. A superuser skips them. The test is before the INSERT. A rejection is 403 with the detail `create policy rejected the record` and does not create a line.

## Data model

Policies have no separate database table; they inspect the loaded model.

## Security mechanisms

Lists, counts and writes use `QueryScope`. The default is `None`, allowing no rows. A policy must return `All` explicitly to allow every row; active superusers receive `All` automatically. See [query-scopes.md](query-scopes.md) for SQL behavior.

`register` and `register_policy` for the same path may not both be called. Axum keeps both routes.

## Error cases

| Case | Response |
|---|---|
|Rule not bound| 403, `object policy is not bound` |
|Scope is `None` or the line does not belong to it|empty list or 404|
|Dataset missing or `can_*` refuses| 404 |
| Create permission is missing | 403 before writing |
| `can_create` rejects the input | 403; no row is written |
| Required field is missing | 422 |

## Example

```rust
struct OwnerPolicy;

impl fexu::auth::ObjectPolicy<SecretNote> for OwnerPolicy {
    fn can_read(user: &fexu::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_update(user: &fexu::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_delete(user: &fexu::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_create(user: &fexu::auth::AuthUser, input: &SecretNoteCreate) -> bool {
        input.owner == user.id.to_string()
    }
}

let app = App::new(db).with_auth(auth).register_policy::<SecretNote, OwnerPolicy>();
```

myshop itself does not bind any object rule. `InternalNote` only uses roles so that the list and CRUD work demonstratively.

## Tests

`examples/myshop/tests/auth.rs` and `admin_http.rs` check their own notes, foreign owners, missing fields and the superuser. Anonymous demo models remain without an object rule.

## Limitations

The scope knows `All`, `None` and an equality comparison. Joins and OR links are not implemented. `can_create` sees the input, but does not set the owner ID itself. This server-side setting is planned. Field-by-field masking is missing.

## Planned extensions

Scopes with multiple columns or joins are planned and not implemented. A rejected individual record remains 404. Superusers get `All` and skip `can_*`. Test: `auth.rs` and `admin_http.rs`.
