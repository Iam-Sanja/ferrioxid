# Framework quality review

[Overview](index.md) · [Capability matrix](django-capability-matrix.md) · [Roadmap](roadmap.md)

This note describes the local branch `feat/framework-completion`. It is a stable, integrated development snapshot of the work that has tests and documentation. It is not a numbered release, and it does not set a first public version.

The branch starts from `main` at `6d79283` (`fix(security): signed anonymous sessions and CSRF protection (#62)`). Nothing on this branch has been pushed.

## Verified on this branch

| Issue | Commit | What the tests show |
|---|---|---|
| [#63](https://github.com/Iam-Sanja/ferrioxid/issues/63) | `24fc96a` | `POST /admin/login` accepts `ferrioxid_login_csrf` while `ferrioxid_anonymous` and `ferrioxid_session` are present. A missing or anonymous token stays 403. Login revokes the anonymous session. Logout revokes the new session. Password-reset forms use `ferrioxid_reset_csrf` the same way. |
| [#68](https://github.com/Iam-Sanja/ferrioxid/issues/68) | `7ae3f60` | After `mount_pages` installs the project site, `views::fail` renders `templates/errors/{status}.html` for HTML clients. JSON stays a problem response. A 500 does not echo SQL. A missing template uses the built-in page. |
| [#69](https://github.com/Iam-Sanja/ferrioxid/issues/69) | `e48f862` | `ferrioxid::require_browser` rejects POST, PUT, PATCH and DELETE without a session cookie. `Authorization: Bearer` is not a session. A present anonymous cookie still needs its CSRF proof. Cookie-less `allow_anonymous` routes stay unchanged. |
| [#70](https://github.com/Iam-Sanja/ferrioxid/issues/70) | this documentation commit | The capability matrix and roadmap no longer say **In PR** for work merged in PR #59, #60, #61 and #62. Those rows are **Partial**. |

Targeted tests that passed locally:

- `cargo test -p myshop --test cookie_csrf`
- `cargo test -p ferrioxid-api --lib views::`
- `cargo test -p ferrioxid-api --lib pages::tests::error_pages`
- `cargo clippy -p ferrioxid-api -p ferrioxid -p myshop --all-targets -- -D warnings` (the known `ferrioxid-macros` linker message remains)

`cargo fmt --all` was applied before those tests. PostgreSQL was not started on this machine. The PostgreSQL CI job on `main` commit `6d79283` succeeded and includes `postgres_anonymous_session_roundtrip`, `postgres_decimal_and_choice_roundtrip` and `postgres_fixture_roundtrip`. It does not include `postgres_filters_and_pages_runtime`, `postgres_transaction_query_and_rollback_runtime` or `postgres_enforces_cross_app_foreign_keys`.

## Still open

These issues are unchanged on this branch. Their acceptance criteria are not met:

- [#64](https://github.com/Iam-Sanja/ferrioxid/issues/64): many-to-many relations, automatic eager loading, sum/group annotations and keyset pagination. The 100-row list cap remains. Offset pagination from PR #59 stays the supported page API.
- [#65](https://github.com/Iam-Sanja/ferrioxid/issues/65): `migrate down` and reversible foreign-key changes are still unsupported.
- [#66](https://github.com/Iam-Sanja/ferrioxid/issues/66): adding a required column without a default still needs an explicit backfill. There is no new reviewed backfill step in this branch.
- [#67](https://github.com/Iam-Sanja/ferrioxid/issues/67): there is still no `#[derive(FerrioxidForm)]`, formset, upload field, or numeric min/max taken from column metadata.

TestShop already depends on this checkout through `ferrioxid = { path = "D:/04_Projekte_Arbeit/Projekte/ferrioxid/crates/ferrioxid" }`. TestShop is not a Git repository. No TestShop file was edited, so its data and local changes stay as they were. Its `templates/errors/404.html` and `500.html` are used by `views::fail` only after `mount_pages` runs, which `src/project/mod.rs` already does.

## Limits that remain on purpose

- A request with no session cookie is still a stateless write on `allow_anonymous` routes. Opt in with `require_browser` where the browser must prove a session.
- JSON `POST /auth/login` does not use `ferrioxid_login_csrf`. With an anonymous cookie it still needs the anonymous CSRF proof.
- `record_id` in the admin audit table is a string column. Auth UUID columns are bound as UUID on PostgreSQL and as text on SQLite.
- Debug output redacts session cookies and CSRF secrets. Password hashes in `UserRow` are still visible to `Debug`.
