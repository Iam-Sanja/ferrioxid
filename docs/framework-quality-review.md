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

## Implemented on this branch, not merged

These issues are implemented locally. Their matrix status stays **In PR** until the branch is merged. They are not a numbered release.

| Issue | What landed | What remains limited |
|---|---|---|
| [#64](https://github.com/Iam-Sanja/ferrioxid/issues/64) | `Select::keyset`, `annotate` for count and bigint sum, and `load_many_to_many`. SQLite test `sqlite_keyset_annotations_and_many_to_many` passed. | Page size stays 1..=100. Nullable and boolean order columns are rejected. Eager loading is explicit. One annotation per query. `postgres_keyset_annotations_and_many_to_many` was not run locally. |
| [#65](https://github.com/Iam-Sanja/ferrioxid/issues/65) | `migrate down`, `migrate down --app`, dry-run, and reversible foreign-key changes when the previous constraint is stored. A later applied file id blocks the revert. SQLite covers the backfill/foreign-key roundtrip and a namespaced app revert. | Dropping a table, column, index or unique constraint without a stored snapshot is refused. Dropping a created table or column needs `--allow-destructive` and does not restore those rows. `postgres_reverts_backfill_and_foreign_key` was not run locally. |
| [#66](https://github.com/Iam-Sanja/ferrioxid/issues/66) | A new required column without a default needs an explicit backfill. After the write, remaining NULL rows abort the migration and the transaction rolls back. | The backfill is one reviewed literal. It does not invent a different value per row. SQLite rebuilds the table. PostgreSQL uses `ALTER TABLE`. The PostgreSQL runtime test was not run locally. |
| [#67](https://github.com/Iam-Sanja/ferrioxid/issues/67) | `#[derive(FerrioxidForm)]`, `bind_formset`, `store_upload`, and numeric min/max copied from `i64` and `Decimal` column metadata. Decimal and choice binding stay in `bind_model`. | The derive accepts `String`, `i64`, `bool` and `Option` of those. A formset allows at most 100 rows. Uploads reject traversal and empty names, then use `LocalStorage` size and type limits. There is no model `FileField`. |

`App::require_browser_writes` layers `require_browser` before `with_state`, so an installed project can opt into [#69](https://github.com/Iam-Sanja/ferrioxid/issues/69) without wrapping a finished router.

## TestShop

TestShop depends on this checkout through `ferrioxid = { path = "D:/04_Projekte_Arbeit/Projekte/ferrioxid/crates/ferrioxid" }`. It is not a Git repository. Changed files were copied to `_backup_framework_completion` before editing. Git was not initialized there.

`tests/shop_flow.rs` copies `db.sqlite3`, applies the new products migration only to that copy, and checks:

1. A product page sets `ferrioxid_anonymous`.
2. Adding to the cart with that cookie and CSRF returns 303.
3. Admin login then succeeds without deleting the anonymous cookie first.
4. Admin logout revokes the session.
5. An unknown path renders the project 404, including “Zum Katalog”.
6. A cookie-less POST and a Bearer POST without a session cookie are 403.
7. The catalog shows an eager category name and a summed order quantity.
8. The keyset cursor returns a later page without repeating those product ids.
9. An explicit backfill fills an existing row, dry-run writes nothing, down without `--allow-destructive` is refused, and down of `0003_tags` drops the new tables while products remain.
10. Product edit rejects a negative price, formset tags save two rows, a traversal upload is rejected, and a PNG is stored through the storage API.

The test hashes `db.sqlite3` before and after. The original file was unchanged. The new migration is not applied to that file. The staff user exists only on the copy.

PostgreSQL was not running on this machine. Ignored PostgreSQL tests, including the two new ones, were not executed and are not a pass.

## Workspace commands

These commands were run on `feat/framework-completion` after the TestShop integration:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed. `ferrioxid-macros` still prints `linker_messages`. That warning is not turned into an error by `-D warnings`.
- `cargo test --workspace -- --test-threads=8` passed. Eleven PostgreSQL tests stayed ignored because `DATABASE_URL` was unset and nothing was listening on `127.0.0.1:5432`. They are not a pass: `postgres_decimal_and_choice_roundtrip`, `postgres_anonymous_session_roundtrip`, `postgres_filters_and_pages_runtime`, `postgres_keyset_annotations_and_many_to_many`, `postgres_transaction_query_and_rollback_runtime`, `postgres_reverts_backfill_and_foreign_key`, `postgres_fixture_roundtrip`, `postgres_enforces_cross_app_foreign_keys`, `postgres_migration_roundtrip`, `fresh_postgres_initializes_auth_jobs_and_application`, and `postgres_http_roundtrip`.
- `cargo build --workspace` passed.

The SQLite constraint test now expects an `ON DELETE` change to be written as a reversible foreign-key replacement. That matches the down-migration behavior. TestShop `cargo test` passed, including `shop_flow`, with the demo seed left ignored.

## Limits that remain on purpose

- A request with no session cookie is still a stateless write on `allow_anonymous` routes. Opt in with `require_browser` or `App::require_browser_writes` where the browser must prove a session.
- JSON `POST /auth/login` does not use `ferrioxid_login_csrf`. With an anonymous cookie it still needs the anonymous CSRF proof.
- `record_id` in the admin audit table is a string column. Auth UUID columns are bound as UUID on PostgreSQL and as text on SQLite.
- Debug output redacts session cookies and CSRF secrets. Password hashes in `UserRow` are still visible to `Debug`.
