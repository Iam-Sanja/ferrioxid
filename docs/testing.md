# Tests, Quality and Continuous Integration

[Overview](index.md) · **Implemented files:** `.github/workflows/ci.yml`, `examples/myshop/tests/postgres_api.rs`, `examples/myshop/tests/events.rs`, `examples/myshop/tests/orders.rs`, `crates/ferrioxid-db/src/lib.rs`. `events.rs` checks the SQLite round trip, sorting, range, year group, and blocked text-to-date change. `orders.rs` checks commit, rollback, foreign child IDs, maximum number, audit errors, `409` and the form without JavaScript. `admin_dates_use_native_inputs_and_utc_hierarchy` checks the HTML fields and the hierarchy. The ignored PostgreSQL tests are not considered to have passed. Destructive tests use temporary SQLite files, not `target/e2e-admin.sqlite`, `target/v019-admin.sqlite` or the databases under `D:\04_Projekte_Arbeit\Projekte\testprojekt`. `examples/myshop/tests/admin_extensions.rs` checks custom pages, CSRF, 403, reserved names, filters, widgets, XSS and bulk actions against a temporary SQLite file. `crates/ferrioxid-auth/src/reset_tests.rs` checks known, unknown and deactivated addresses, same HTTP response, rate limit, expiration, one-time use, parallel completion, delivery error without external recipients, session revocation and an audit without tokens. `ferrioxid-mail` reviews memory, header injection and redacted SMTP debug output. `crates/ferrioxid-cli/tests/diagnostics.rs` checks auth and app status, `--strict`, upgrade, missing and unreadable SQLite file. The library tests in `inspect` and `upgrade` check for checksum, unknown migration, reading lock, path escape, junction, backup and abort. `generator.rs` sets `CARGO_BUILD_JOBS` only on the child process and never higher than 2. rustc 1.98.1 crashed in an unlimited release build in this test with `0xc0000005` and `0xc0000409`. This is not a fixed compiler error. A run with `CARGO_BUILD_JOBS=2` and `--test-threads=2` was green. The subsequent unrestricted `cargo test --workspace --offline` was also green, because the nested release builds continue to use at most two jobs. `--test-threads` only limits the threads of a test binary.

## Test Levels (Implemented)

1. **Build and Lint:** `cargo fmt`, Clippy with `-D warnings` and `cargo build --workspace`.
2. **Workspace tests:** `cargo test --workspace` performs unit tests and `sqlite_http_roundtrip`.
3. **SQLite-HTTP:** `sqlite_http_roundtrip` uses `sqlite::memory:`, a pool connection and an Axum listener on a random localhost port. The `db.sqlite3` file is not used.
4. **DDL without server:** `boolean_default_is_emitted_into_postgres_schema` checks the rendered PostgreSQL and SQLite definition of `active`.
5. **Migrations:** Snapshot, Diff, SQLite chain and `ferrioxid migrate` against a temporary file run in `cargo test --workspace`.
6. **Forms, static files, storage and middleware:** `ferrioxid-forms`, `ferrioxid-static`, `ferrioxid-storage` and `ferrioxid-middleware` run with SQLite or without a database. PostgreSQL is not started for this.
7. **Auth and Admin:** Password, Session, CSRF, Scope and Asset tests in `ferrioxid-auth`, `examples/myshop/tests/auth.rs`, `examples/myshop/tests/admin_http.rs` and `examples/myshop/tests/admin_html.rs` use SQLite. They are not ignored. `html_admin_works_without_javascript` checks forms without JavaScript, logging out a non-staff including missing and foreign CSRF, superuser without staff, an inactive staff superuser, default columns, explicit `list_display`, list line, standard sorting, page size, field sets that create three save destinations, received form values, the selection page over 100 destinations, rejected foreign keys, the role selection as `<select>`, user, password and session revocation, staff revocation, `confirm=1`, readable foreign keys and bool/number filters. `admin_v018_drafts_inlines_bulk_and_audit` checks form designs, the enforced inline foreign key, bulk actions including rollback, foreign IDs, CSRF and history. `summary_skips_sensitive_values` checks that hashes are not in the summary. `ferrioxid-admin` checks the contrast gate of the project color. `ferrioxid-forms` checks that only `confirm=1` applies. `ferrioxid-core` checks the error codes. Playwright is not part of the workspace test. A browser run is additional and does not replace these tests.
8. **PostgreSQL, optional:** `postgres_http_roundtrip` and `postgres_migration_roundtrip` are `#[ignore]`. They only run with `--ignored` and set `DATABASE_URL`.
9. **Jobs:** `ferrioxid-jobs` checks success, retry, permanent failure, delay, intervals, two workers, expired leases, idempotency, missing migrations, secret handling, mail and an untouched reset outbox. It also checks `WorkerHost`, a missing SQLite file, cancellation of a timed-out handler, retry through a new connection and hidden payloads. Queue tests use `ManualClock`. `examples/notes/tests/jobs.rs` exercises the same path. `examples/notes/tests/worker.rs` starts `src/bin/worker.rs` and checks success, restart, an unknown handler and the mail job. `ferrioxid-cli` checks `--yes` and that `generate worker` never overwrites a file.
10. **Model fields and app launch:** `examples/notes/tests/components.rs` checks two models, flat timestamps, rejected client values, slug validation, admin-readonly, schema diff and REST. `apps.rs` checks `try_into_router`, missing `seal`, cycles, missing TOML binding and deviating directories. `packages.rs` checks the contract comparison without a database.
11. **Apps:** `ferrioxid-api` checks duplicate apps, reserved names, path collisions, cycles, sealed registries, admin template names, isolated template conflicts, and `/static/{app}`. `pages.rs` checks `url` and a missing route without listing the map. `views.rs` checks `require_mutation` without authentication. `packages.rs` checks two apps with `0001_initial`, the order, cycles, the legacy line and a checksum. `project.rs` checks `[app.shop]` and a nameless `[app]`. `examples/notes/tests/reuse.rs` installs an external crate without creating a history table. `ferrioxid-cli` checks `new app` without overwriting.
12. **Settings, URLs and views:** `project.rs` checks legacy files, unknown keys, `[custom.shop]`, production debug and redacted secrets. `urls.rs` checks names, nesting, parameters, reserved paths and duplicate paths. `views.rs` checks JSON problem responses, HTML escaping, local redirects, staff without session, and the request ID. **Generator:** `crates/ferrioxid-cli/tests/generator.rs` creates projects outside the workspace, checks `doctor`, `check`, `dev` and `build` and compiles projects generated from both templates. `check` must not change the migration file. Setting and template tests are in `project.rs` and `pages.rs`. Each child process of `ferrioxid` and `cargo` gets its own file `ferrioxid-isolated-<pid>-<n>.sqlite` under the temporary directory. `DATABASE_URL` is only set to this process. `FERRIOXID_E2E_DATABASE` and `FERRIOXID_PASSWORD` are removed for the child process. The process environment of the test remains unchanged. After the test, `cleanup_isolated_databases` deletes the files of this process ID. Parallel tests use different file numbers.

### Local commands

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Optional PostgreSQL run, only against a dispensable database:

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid"
cargo test -p myshop --test postgres_api -- --ignored --nocapture
```

The ignored test requires `DATABASE_URL` and breaks off without it. It deletes the tables `ferrioxid_v01_integration_products`, `ferrioxid_v01_required_only`, `ferrioxid_v01_private_products` and `ferrioxid_v01_notes` before and after the run. This is not proof as long as no PostgreSQL server can be reached. An earlier run without a server ended with `PoolTimedOut`.

## SQLite cases covered (Implemented)

The same HTTP checks run in the SQLite test:

- GET list, GET according to UUID, POST, PATCH and DELETE.
- POST 201 with UUID, fields and bool-default `true`.
- Empty or only from spaces existing name and negative price result in 422.
- `#[field(required)]` without `min_length`.
- PATCH only with `name`, empty `{}`, empty name, negative price and unknown UUID.
- DELETE 204 without body, following GET 404 and second DELETE 404.
- Unreleased model: List, detail, PATCH and DELETE with 403 and Ferrioxid JSON.
- Nullable `note`: missing key, value and JSON-`null`.
- After more than 100 lines: `limit` 100, `truncated: true`, exactly 100 objects.
- `PRAGMA foreign_keys` is on the test compound `1`.
- `PatchField` distinguishes missing, `null` and value. The DTO rejects `id` and `readonly`.

The JSON structure is the same for both backends because the handlers are dialect-independent. The PostgreSQL test runs the same sequence, but was not executed against a server in this operation.

## CI (Implemented)

`.github/workflows/ci.yml` starts rustfmt, Clippy, `cargo test --workspace` and `cargo build --workspace` on Linux and Windows. The SQLite test is running. A separate job starts PostgreSQL 16 and the two ignored tests. A local workspace run without a server is not a PostgreSQL proof.

The migration tests test the same snapshots, deterministic column order, duplicate names, unknown versions, new table, new column, deleted column, supported type and default change, blocked type change, empty diff, risky change, renaming refusal, explicit column and table renaming, successful and failed SQLite conversion, blocked `NOT NULL`, matching and deviating baseline, destructive chains without baseline, `reseal`, first application, second migration with received rows, repeated `up`, manipulated file and existing tables without history. `tests/constraints.rs` checks Unique including `NULL` and composite key, index attachment, repeated `up`, index removal, valid and invalid foreign keys, `restrict`, `cascade`, `set_null`, rejected `set_null` on `NOT NULL`, baseline with indices, rolled-back Unique migration and a reversible ON DELETE change. A renaming of an indexed column renames the index to SQLite. The `migrate_make_up_and_status_use_a_temporary_sqlite_database` CLI test uses a temporary SQLite file. `examples/myshop/tests/relations.rs` checks Belongs-To, Has-Many, empty children, optional `None`, duplicate category name, an invalid foreign key and the checked-in chain `0001` plus `0002`.

PostgreSQL migration and HTTP tests remain ignored as long as no server is running. An ignored test does not count as passed.

The quality commands for this implementation are contained in the final report of the operation. `postgres_http_roundtrip` and `postgres_migration_roundtrip` remain ignored as long as no PostgreSQL server is running. An ignored test is not a success.

## In-process client (implemented)

`ferrioxid::testing::TestClient` sends `GET`, `POST`, `PUT`, `PATCH` and `DELETE` through the Axum router. It stores `Set-Cookie` name/value pairs and sends them back. `post_json` and `post_form` set the matching content type. `post_form_with_csrf` copies the `csrf_token` field from HTML. `assert_status` and `assert_json` compare the response.

`TestClient::request` returns `TestRequest`. `header` adds one request header such as `Origin`, `Accept` or `Authorization`. A `Cookie` header replaces the jar for that request only and does not rewrite the stored jar. `send` executes the request. `TestResponse::headers` exposes the response header map. `header` and `header_values` read `Location`, `Content-Type`, `Cache-Control` and repeated `Set-Cookie` values. `set_cookies` parses `HttpOnly`, `Secure`, `SameSite`, `Path`, `Domain` and `Max-Age`. The jar still keeps only the cookie pair and still drops a cookie whose `Max-Age` is `0`.

`TestDatabase::sqlite` creates a temporary directory named `ferrioxid-test-*` and deletes only that directory. `TestDatabase::postgres` accepts a URL only when the database name contains `ferrioxid_test`. `cleanup` drops recorded PostgreSQL tables and never runs `DROP DATABASE`. A name such as `ferrioxid` is refused before a connection is opened.

`ferrioxid::db::QueryCounter::install` counts model statements on the current thread. `QueryCounter::record` counts one statement the test executed itself. Direct `ConnectionTrait` calls are not observed, and the counter is not a pool-wide total. `assert_query_budget` fails when the count is above the limit. The executable example is `crates/ferrioxid/tests/openapi_client.rs`.

`TestClient::request` sends `Origin`, `Accept`, `Authorization`, `Content-Type` and any other valid header. `TestResponse::header`, `header_values` and `set_cookies` expose `Location`, `Content-Type`, `Cache-Control` and every `Set-Cookie` attribute (`HttpOnly`, `Secure`, `SameSite`, `Path`, `Max-Age`). The cookie jar still stores only the name and value. `browser_login_flow_exposes_cookies_and_rejects_bad_sessions` is the executable session example: anonymous GET, listed origin, foreign origin, missing and invalid CSRF, a user cookie that is not downgraded to the anonymous session, then a real user session and its `Location`.

PostgreSQL was not running on this machine while this client was added. The ignored PostgreSQL tests were not executed.

## Planned extension (planned)

| Area | Tests | Acceptance criteria |
|---|---|---|
| Macros | Compile-Fail |Invalid attributes create clear diagnostics|
|Migrations| Diff, Drift, Locks |No silent data loss|
| DB | Transactions, Constraint-Mapping |Same error codes on both backends|
| Auth | 401/403, sessions and CSRF | No permission bypass |
|Admin and Deployment|UI and containers|Policy on the server side, no anonymous production|

## Reporting requirements

Name executed commands, exit codes and relevant errors. Mark unexecuted tests as unexecuted. No blanket `allow(dead_code)` if the value can be used safely.

## Admin

SQLite: `cargo test -p myshop --test admin_http` and `cargo test -p myshop --test admin_html`. These tests use in-memory databases and never seed a database selected by `FERRIOXID_E2E_DATABASE` or the historical `FERRUM_E2E_DATABASE`. `tests_ignore_external_seed_database` verifies this in a child test process. For manual browser testing, create an account explicitly with `ferrioxid admin create-user --email admin@example.com --staff --superuser` and enter a unique password at the prompt; see [Authentication](auth.md#example). There are no PostgreSQL admin tests. A browser run is not a substitute for these HTTP tests.

## Fresh framework initialization

`examples/myshop/tests/framework_names.rs` initializes all auth, job and myshop migrations in an empty database, checks the 21 expected tables, and reapplies the migrations to verify idempotency. SQLite runs in memory during the workspace suite. The PostgreSQL variant is ignored locally and runs first in the dedicated PostgreSQL CI job against its empty disposable database; on success it drops only the verified tables created by the test.

The generator integration checks both API and fullstack outputs for current framework names, the project manifest, absence of a pre-generated CSRF key, secret exclusions and the model markers introduced in PR #24. Both templates compile, and model insertion is compiled again. `doctor`, `check`, `dev` and release builds remain part of that integration run.

See [Unified admin record editor](admin-record-editor.md) for canonical routes, permissions, navigation and regression checks.
