# Objektregeln

[For overview](index.md) · [permissions.md](permissions.md)]

## Zweck

An object rule decides based on the user and data set. It complements the roller test and does not replace it.

## Architektur

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

- `crates/ferrioxid-auth/src/policy.rs`
- `crates/ferrioxid-api/src/lib.rs`
- `crates/ferrioxid-macros/src/lib.rs`

## Public API

`ferrioxid::auth::ObjectPolicy` and `App::register_policy`.

For GET, PATCH and DELETE, the trader loads the record. If the rule fails, the answer is 404, even if the line exists. Thus, the interface does not confirm the identifier.

POST checks the Create right first and then `can_create`. The method gets the Create type. The standard is `false`. A superuser skips them. The test is before the INSERT. A rejection is 403 with the detail `create policy rejected the record` and does not create a line.

## Datenmodell

The rule does not have its own table. She sees the already loaded model.

## Security mechanisms

Lists, counters and write accesses use `QueryScope`. The standard is `None`, so not a line. `All` must explicitly provide the rule. An active superuser receives `All`. The SQL form is in [query-scopes.md](query-scopes.md)].

`register` and `register_policy` for the same path may not both be called. Axum keeps both routes.

## Error cases

| Fall | Antwort |
|---|---|
|Rule not bound| 403, `object policy is not bound` |
|Scope is `None` or the line does not belong to it|empty list or 404|
|Dataset missing or `can_*` refuses| 404 |
| Create-Recht fehlt |403 before writing|
|`can_create` refuses to enter| 403, keine Zeile |
| Pflichtfeld fehlt | 422 |

## Example

```rust
struct OwnerPolicy;

impl ferrioxid::auth::ObjectPolicy<SecretNote> for OwnerPolicy {
    fn can_read(user: &ferrioxid::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_update(user: &ferrioxid::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_delete(user: &ferrioxid::auth::AuthUser, object: &SecretNote) -> bool {
        object.owner == user.id.to_string()
    }
    fn can_create(user: &ferrioxid::auth::AuthUser, input: &SecretNoteCreate) -> bool {
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

Scopes with multiple columns or joins are planned and not implemented. A rejected individual data set remains 404. Superusers get `All` and skip `can_*`. Test: `auth.rs` and `admin_http.rs`.
