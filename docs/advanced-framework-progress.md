# Advanced framework progress

**Historical engineering log (November 2026):** this file preserves the status of the original integration branches when written. Statements such as 'not merged', 'not on main', and 'CI not run' apply to those earlier snapshots; they do not describe the current `main` at `a39c952`. Consult the [roadmap](roadmap.md) and [capability matrix](django-capability-matrix.md) for current status.

The integration branch `work/integration` combines the four pull requests in order: #97, #98, #99, #100. The sections below keep the original per-branch reports.

## Block 1 branch

`fix/runtime-security-docker`, based on `main` at `53a95aa` (PR #73).

| Issue | Status | Already present | Added in this pass | Tests | Still open |
| --- | --- | --- | --- | --- | --- |
| #74 | implemented locally | Auth and jobs called `apply_directory` on `CARGO_MANIFEST_DIR`. | JSON is embedded with `include_str`. `apply_migrations` uses those bytes. Job primary keys are bound as PostgreSQL `uuid` and as SQLite text. `embedded-migrate` runs from another directory. | SQLite memory test. Release binary copied outside the source tree: SQLite apply, login and enqueue succeeded. Ignored PostgreSQL test applies auth and jobs, logs in, enqueues and reads the job back. CI steps exist and have not run on GitHub. | CI and merge. Docker is not installed on this machine, so the compose file was not booted. |
| #75 | implemented locally | A missing directory was an empty migration set. | Required auth and jobs packages fail when empty, incomplete or corrupt. `inspect_framework` reports auth, jobs and the application separately and does not migrate. An absent app directory stays optional. | Unit tests for empty, corrupt, absent and empty directories. Readiness test checks that history tables are not created. | CI and merge. |
| #76 | implemented locally | Security middleware always wrote `Referrer-Policy: no-referrer`. | The handler's `Referrer-Policy` is kept. The framework default fills a missing value and can be set to a listed policy. `nosniff` and `DENY` are always enforced. | Middleware tests over the full chain. | A browser walk of LAN HTTP and HTTPS is not automated in this pass. Header and origin tests cover the HTTP contract. |
| #77 | implemented locally | `require_browser` rejected `POST /admin/login` before the login CSRF cookie was checked. | A fresh jar can submit the native admin login with its own CSRF cookie. Other writes still need a session cookie. Foreign origins and unverified `Origin: null` stay rejected. | `direct_admin_login_survives_require_browser_writes_from_a_fresh_jar`. | Browser click-through is not a separate automated test. |
| #78 | implemented locally | `0.0.0.0` was rejected. `trust_proxy` accepted `X-Forwarded-For` from every peer. | Wildcard bind is opt-in and is not an origin. Forwarded client keys require both the flag and a listed peer. Compose example keeps PostgreSQL unpublished. | Project tests and the auth proxy test. | `docker` is not installed on this machine, so the compose file was reviewed and not booted. |
| #8 | removed with the database driver | The `sqlx` facade and optional `sqlx-mysql` left the lockfile, so `rsa` 0.9.10 left with them. | `docs/security-rsa.md`. | `cargo audit` has no ignore for `RUSTSEC-2023-0071`. | The old ignore must not be restored to make a check green. |

## Verification in this pass

Local, not GitHub CI, and not merged:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed. The pre-existing `linker_messages` warning on `fexu-macros` remains and is not denied by `-D warnings`.
- `cargo test --workspace` passed on SQLite. Ignored PostgreSQL tests in that command were not executed.
- `postgres_embedded_auth_and_jobs` passed against a disposable database created from the local `postgres` admin database. The populated `fexu` and `fexu_migrations_down_test` databases were unchanged (15 and 1 public tables).
- The release `embedded-migrate` binary was copied outside the source tree. It applied auth and jobs, logged in, and enqueued a job on a fresh SQLite file and on a disposable PostgreSQL database. That database was dropped.
- `cargo tree --workspace --target all --prefix none --locked -i rsa` and the same query for `sqlx-mysql` both printed nothing.
- Docker is not installed here, so `examples/deploy/compose.yaml` was not started.

## Local commits

Implementation: `d556bcc3bbfa5b624c5d27ef13d2682ee06b5652`.
The earlier progress note is `ad487023d79d9faebd0a711258cdc848e1bb9fa2`.
GitHub CI has not run. This branch is not merged.

## Block 2 — ORM and migrations

Branch: `feat/orm-migrations-lifecycle`, created from `main` at `53a95aa`. Implementation commit: `f3a663243b030eb3f391032c17a69314ff78abc3`. Progress note: `3bb51b951f904a7bb092c1910c7af8c7dc3332ed`.

| Issue | Local implementation | Local tests | GitHub CI | Merge |
|---|---|---|---|---|
| [#64](https://github.com/Iam-Sanja/ferrioxid/issues/64) | Many-to-many, keyset and one annotation were already on `main`. This branch adds `load_one_to_one`. | SQLite and PostgreSQL 18.6, same example code | Not run | Not merged |
| [#81](https://github.com/Iam-Sanja/ferrioxid/issues/81) | `Select::aggregate` with group, measures, having and named filters | Same SQLite and PostgreSQL run | Not run | Not merged |
| [#82](https://github.com/Iam-Sanja/ferrioxid/issues/82) | `#[field(json)]`, canonical documents, forms, admin, fixtures and OpenAPI | Same run, plus the form binding test | Not run | Not merged |
| [#83](https://github.com/Iam-Sanja/ferrioxid/issues/83) | Read-only `migrate plan`, live SQLite and PostgreSQL catalog, checksum conflicts | SQLite file bytes unchanged; PostgreSQL disposable database | Not run | Not merged |

The shared application code is `crates/fexu-db/examples/grouped_report`. `sqlite_grouped_report_scope_json_and_one_to_one` and the ignored `postgres_grouped_report_scope_json_and_one_to_one` both call it. The PostgreSQL test was executed locally on 12 November 2026 against PostgreSQL 18.6 through `DisposablePostgres`. The admin URL was the local `postgres` database. The populated databases `fexu` (15 public tables) and `fexu_migrations_down_test` (1 public table) had the same counts after the run. No `fexu_test_*` database remained.

`postgres_plan_detects_live_drift_on_a_disposable_database` also passed on that server. A clean applied schema produced no finding. An added column was reported as drift. The disposable database was dropped.

## SQLite and PostgreSQL differences

- UUID columns are text on SQLite and `uuid` on PostgreSQL. Bigint aggregates are cast to `integer` on SQLite and `bigint` on PostgreSQL because PostgreSQL `SUM` of bigint is numeric.
- Average is computed in Rust from the integer sum and count. It is truncated toward zero at six decimal places. Overflow of `sum * 1_000_000` is an error. SQL `AVG` is not used.
- Character length uses SQLite `length` and PostgreSQL `char_length`. Both count characters for the text stored by this API. The bound is 0..=200 and applies only to text and choice columns.
- JSON is SQLite `text` plus `json_valid`, and PostgreSQL `jsonb`. Path operators, containment and JSON equality filters are not generated. Object keys in the canonical text are sorted.
- A JSON `null` document is distinct from SQL `NULL`.
- Boolean, decimal and JSON columns cannot be grouped. Decimal and non-bigint columns cannot be summed.
- Reports return at most 100 groups and have no offset. `HAVING` accepts only integer measures. Ordering by the average is rejected.
- One-to-one is a unique UUID foreign key plus an explicit loader. It is not a separate macro relation and it does not load automatically.
- `migrate plan` reads `sqlite_master` and `PRAGMA` on SQLite, and `information_schema` plus `pg_catalog` on PostgreSQL. History tables named `_fexu_*` are ignored. Pending migrations are not compared as if they were already applied. A checksum or history-order error suppresses the live comparison.
- PostgreSQL baseline, automatic type changes to or from JSON, and MySQL remain unimplemented.
- Changing a column type is still limited to the existing supported pairs. Destructive changes still need `--allow-destructive` on `migrate up` and `migrate down`. The plan command does not perform them.

## Still open

- Issues stay open until the branch is pushed, reviewed, green in GitHub CI and merged.
- `postgres_keyset_annotations_and_many_to_many` was not executed in this pass. It targets whatever `DATABASE_URL` points at and drops its own tables there, so it was not pointed at the populated `fexu` database.
- There is no automatic eager loading, no JSON path query, no decimal average and no offset inside a grouped report.
- Multi-column foreign-key order on PostgreSQL is taken from the catalog join. The plan test covers a table without a foreign key. A schema with secondary indexes and foreign keys still uses the same name and column comparison as the SQLite baseline.

## Block 3 — Native admin

Branch: `feat/native-admin`. Implementation: `e53f976d85ae0258b88554b0d42ac04dffcd5138`. Workspace evidence: `b23b0df4e2e09014c6c6777f0ed78ceaef27ad78`.

### Umgesetzt

- Datensatzseiten für Modelle, Benutzer und Rollen zeigen eine Audit-Timeline. Sie nutzt `audit::matching` mit Modellschlüssel und der gespeicherten Datensatz-ID. Es gibt keine zweite Audit-Tabelle.
- Die Timeline zeigt Aktion, Akteur, UTC-Zeitstempel und die bereits gespeicherte gekürzte Zusammenfassung. Höchstens acht Einträge, neueste zuerst, plus Link zur gefilterten Historie.
- Personen ohne Superuser-Recht sehen nur Ereignisse mit der eigenen `actor_id`.
- PostgreSQL speichert Audit-UUIDs als `uuid`. Schreiben und der Akteur-Filter binden diese Spalten jetzt als UUID. SQLite bleibt bei Text.

### Issue #32

Erfüllt auf diesem Branch: Zuordnung über die Datensatz-ID, getrennte Ereignisse bei ähnlichen Namen, Sichtbarkeit nach Akteur, gekürzte Zusammenfassung ohne neue Geheimnisausgabe, responsive Stapelung unter 800 px, SQLite- und PostgreSQL-Test der Abfrage, HTTP-Test der HTML-Seite.

Offen: keine manuelle Browserabnahme. Die Darstellung wurde über HTTP geprüft, nicht in einem Browser. Verzeichnisereignisse für Benutzer bleiben an der Benutzer-UUID, Rollenereignisse am Rollennamen. Direkte Repository-Schreibzugriffe und die Auth-CLI schreiben weiterhin kein Audit.

### Issue #79

Die typisierte Konfiguration ist das bestehende `AdminModelMeta` aus `#[fexu(admin = true)]`. Eine zweite Admin-Konfiguration wurde nicht angelegt. `docs/admin.md` beschreibt diese Zuordnung.

Offen: keine zusätzliche Berechtigung je Aktion über `{permission}.read|create|update|delete` hinaus. Keine neue Startup-Prüfung für app-übergreifende Relationen. Die bestehenden Compile-Time-Prüfungen für Fieldsets und geheime Felder bleiben.

### Issue #80

Auswahllisten, Suche, Seitenzählung bis 100 je Seite, Query-Scope und Sammelaktionen sind auf `main` vorhanden und wurden nicht dupliziert. Die bestehenden HTTP-Tests decken Suche, Seite 5 und fremde Geheimnisse ab.

Offen: `load_one_to_one` liegt nur auf Block 2. Dieser Branch lädt One-to-One-Beziehungen nicht gesammelt nach. Ein eigener Test mit 0, 1, 100 und 1000 Inline-Zeilen wurde nicht ergänzt. Die vorhandene Inline-Liste bleibt auf höchstens 25 Zeilen je Seite begrenzt.

### Issue #84

Modellformulare, Fieldsets, Formsets, Upload und der Schutz vor mitgesendeten Readonly-Feldern sind auf `main` vorhanden. Der HTML-Test lehnt eine mitgesendete `id` weiterhin ab.

Offen: keine neue Prüfung, die HTML- und JSON-Validierungstexte Feld für Feld gegeneinander stellt. Kein zusätzlicher Upload-Lifecycle über `store_upload` hinaus.

### Tests auf diesem Branch

- SQLite: `matching_uses_the_record_id_not_a_similar_name`, `admin_v018_drafts_inlines_bulk_and_audit`
- PostgreSQL: `postgres_matching_uses_the_record_id_not_a_similar_name` auf einer Datenbank `fexu_test_<32 hex>`, danach gelöscht
- Danach: `fexu` 15 öffentliche Tabellen, `fexu_migrations_down_test` 1 öffentliche Tabelle, keine übrige `fexu_test_*`-Datenbank
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` und `cargo build --workspace` waren auf diesem Branch grün. Die bestehende `linker_messages`-Warnung von `fexu-macros` bleibt.

### Browser

Keine Browserabnahme. CRUD bleibt serverseitig. Der neue HTTP-Test prüft die Timeline, die Datensatz-ID und die Akteur-Isolation.

## Block 4 — Developer apps

Branch: `feat/developer-apps`. Implementation: `938c267a5fb057b3d5c5780ae798ec37e1dbce17`. Workspace evidence: `84141b097f5ea46d937fc7cff3c379fe37007d4c`.

### Umgesetzt

- `discover_project` lädt nur `fexu.toml`.
- `fexu_core::aliased_env` liest genau eine `FEXU_<NAME>`-Variable. `DATABASE_URL` und `BIND_ADDRESS` sind keine Framework-Einstellungen.
- `AuthConfig::try_from_env` und `MailService::try_open` lesen dieselben einzelnen Namen.
- `.fexu/project.json` und die Tabellennamen `_fexu_*` bleiben unverändert.

### Issue #19

Erfüllt auf diesem Branch: neue Projekte verwenden `fexu.toml`. Die Cargo-Version bleibt `0.1.0`.

Offen: kein eigener PostgreSQL-Lauf für diese Änderung, weil sie keine Datenbank schreibt. Linux-CI ist hier nicht ausgeführt.

### Issue #86

Nicht in diesem Commit erweitert. `App::install`, `FexuApplication` und `docs/apps.md` beschreiben bereits die statische Registrierung. Ein versioniertes App-Paket, das zwei generierte Projekte ohne Quelländerung installieren, fehlt. Eine Deinstallation, die Daten stehen lässt, ist nicht neu spezifiziert.

### Issue #89

Nicht in diesem Commit erweitert. Listen, Formulare und CSRF bleiben in den bestehenden Admin- und Form-APIs. Eine consume-once Flash-Nachricht in der Sitzung und generische CRUD-Helfer außerhalb des Admin wurden nicht ergänzt.

### Issue #91

Nicht in diesem Commit erweitert. `fexu check` meldet einen Alias-Konflikt für das CSRF-Geheimnis und das SMTP-Passwort mit dem Variablennamen und ohne den Wert. `fexu dev` startet bereits mit Watch und ohne automatische Migration. Ein sauberer Projektstart von der Installation bis zum laufenden Formular wurde in diesem Commit nicht erneut auf Windows und Linux ausgeführt.

### Tests

- SQLite: nicht betroffen
- PostgreSQL: nicht ausgeführt, keine Schemaänderung
- `conflicting_aliases_fail_without_returning_either_value`
- `discovery_reads_one_project_file_and_rejects_both`
- `custom_settings_env_and_production_stay_explicit`
- `conflicting_secret_aliases_fail_closed`
- `conflicting_smtp_passwords_are_rejected`
- `cargo clippy --workspace --all-targets -- -D warnings` war grün. Die bestehende `linker_messages`-Warnung von `fexu-macros` bleibt und bricht `-D warnings` nicht ab.
- `cargo fmt --all -- --check`, `cargo test --workspace` und `cargo build --workspace` waren auf diesem Branch grün. Der Generator-Test `generated_api_and_fullstack_projects_compile` lief dabei mit.

### Neue APIs

- `fexu_core::aliased_env`
- `AuthConfig::try_from_env`
- `MailService::try_open`

### Verhalten

`AuthConfig::from_env` und `MailService::open` lesen `FEXU_*`. `DATABASE_URL` und `BIND_ADDRESS` bleiben unverändert.

## Integration branch

`work/integration` contains #97, #98, #99 and #100 in that order. The execution record is [advanced-framework-integration.md](advanced-framework-integration.md).

Local evidence on 12 November 2026:

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo build --workspace` passed.
- Ignored PostgreSQL tests `postgres_embedded_auth_and_jobs`, `postgres_grouped_report_scope_json_and_one_to_one`, `postgres_plan_detects_live_drift_on_a_disposable_database`, `postgres_matching_uses_the_record_id_not_a_similar_name` and `postgres_inspect_framework_and_plan_stay_read_only` passed against disposable `fexu_test_*` databases created from the local `postgres` admin database.
- After those runs, `fexu` still had 15 public tables and `fexu_migrations_down_test` still had 1. No `fexu_test_*` database remained.
- `cargo audit --ignore RUSTSEC-2023-0071` passed on that date. The unfiltered audit then reported only that lockfile-only RSA advisory. The database driver later removed the `sqlx` facade, and the ignore was removed with it.
- GitHub CI has not run for this combined branch. The four source PR branches were not rewritten.
