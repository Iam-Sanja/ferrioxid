# Query scopes for object rules

[For overview](index.md) · [policies.md](policies.md)] · [database.md](database.md)]

## Zweck

An object rule is designed to restrict lists in the database. Ferrioxide does not load all lines to filter them in the process.

## Architektur

`QueryScope` is located in `ferrioxid-core` so that database, API and admin use the same representation.

```rust
pub enum QueryScope {
    All,
    None,
    Equals { column: &'static str, value: ScopeValue },
}
```

`ScopeValue` is `Text`, `Uuid`, `BigInt` or `Bool`. The standard method `ObjectPolicy::read_scope` provides `None`. `write_scope` takes over `read_scope` as long as the rule says otherwise. An active superuser receives `All`.

## Responsible file

- `crates/ferrioxid-core/src/lib.rs`
- `crates/ferrioxid-auth/src/policy.rs`
- `crates/ferrioxid-db/src/sql.rs`
- `crates/ferrioxid-macros/src/lib.rs`
- `crates/ferrioxid-api/src/lib.rs`
- `crates/ferrioxid-admin/src/registry.rs`
- `crates/ferrioxid-admin/src/http.rs`

## Public APIs

- `ObjectPolicy::read_scope` und `write_scope`
- `ModelRepository::list_scoped`, `get_scoped`, `update_scoped`, `delete_scoped`
- `list`, `get`, `update` and `delete` remain and use `QueryScope::All`
- Admin lists respond additionally with `total`

## Datenfluss

REST and Admin call the same SeaQuery helpers. The scope is attached as a parameterized condition before search, sorting, offset and limit. `None` becomes `0 = 1`. `Equals` checks the column name against the model metadata and binds the value as a parameter. `total` counts with the same filters and the same scope, without limit.

PATCH and DELETE carry the scope in the `WHERE` clause. A previous SELECT remains as a second check on `can_update` and `can_delete`.

## Sicherheitsregeln

- Column names come only from trusted model metadata.
- Values from the HTTP request are never adopted as scope expression.
- A missing implementation of `read_scope` results in `None` and thus no row.
- `All` is created only by an explicit policy or by the superuser rule.
- An unbound object rule remains 403 with `object policy is not bound`.
- Sorting and filter fields remain a whitelist. An unknown name is 422.

## Error cases

| Fall | Antwort |
|---|---|
|Rule not bound| 403 |
|The line is outside the scope| 404 |
|Column of the scope does not belong to the model| 422 |
| Superuser |Full access, still staff duty in the admin|

## Example

```rust
fn read_scope(user: &AuthUser) -> QueryScope {
    QueryScope::Equals {
        column: "owner",
        value: ScopeValue::Text(user.id.to_string()),
    }
}
```

## Tests

`examples/myshop/tests/auth.rs` checks that a user sees only their own row in the REST list and the superuser sees both. `examples/myshop/tests/admin_http.rs` checks the same list in the admin, `total`, a foreign GET, PATCH and DELETE.

## Limitations

There is no `OR`, no joins and no expressions from multiple columns. A `can_*` method that is more stringent than `write_scope` remains a second test after the SELECT. `write_scope` must therefore be at least as close as `can_update` and `can_delete`. Create does not check the new content against the scope.
