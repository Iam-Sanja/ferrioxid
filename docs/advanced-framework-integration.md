# Integration der Pull Requests #97 bis #100

Stand: 12. November 2026. Branch `work/integration` in `D:\04_Projekte_Arbeit\Projekte\ferrioxid-wt-integration`. Basis `origin/main` bei `53a95aa`. Die vier PR-Branches wurden nicht überschrieben und nicht nach `main` gemerged.

## Merge-Reihenfolge

1. [#97](https://github.com/Iam-Sanja/ferrioxid/pull/97) `fix/runtime-security-docker` @ `c3f078a`
2. [#98](https://github.com/Iam-Sanja/ferrioxid/pull/98) `feat/orm-migrations-lifecycle` @ `cc5f676`
3. [#99](https://github.com/Iam-Sanja/ferrioxid/pull/99) `feat/native-admin` @ `9cdc200`
4. [#100](https://github.com/Iam-Sanja/ferrioxid/pull/100) `feat/developer-apps` @ `a5a571e`

## Konfliktlösungen

| Datei | Branches | Lösung |
|---|---|---|
| `crates/ferrioxid-cli/src/main.rs` | #97 und #98 | `inspect_framework` und `plan_database` bleiben beide importiert. `migrate status` nutzt eingebettete Auth- und Job-Dateien. `migrate plan` bleibt read-only. |
| `docs/advanced-framework-progress.md` | alle | Die vier Blockberichte stehen untereinander. Kein Bericht hat einen anderen ersetzt. |
| `docs/index.md` | #97 und #98 | Die doppelte Deployment-Zeile aus dem Auto-Merge wurde zu einer Zeile mit `deployment.md` und `deploy-docker.md`. |
| `crates/ferrioxid-auth/src/config.rs` | #97 und #100 | `trusted_proxies` liegt in `development()` und in `try_from_env`. Die Liste kommt über `aliased_env("FERRIOXID_TRUSTED_PROXIES")`, also auch über `FERRUM_TRUSTED_PROXIES`. Widersprüchliche Werte sind ein Fehler und erscheinen nicht in der Meldung. Ein `*`-Eintrag bleibt ignoriert. |
| `crates/ferrioxid-core/src/lib.rs` | #98 und #100 | `canonical_json_text` und `aliased_env` bleiben. Die zusammengeschobenen Doc-Kommentare wurden getrennt. Beide Testmodule bleiben. |

Automatisch zusammengeführt und danach gebaut:

- `crates/ferrioxid-migrations/src/lib.rs` und `runner.rs` zwischen #97 und #98
- `crates/ferrioxid-migrations/src/project.rs`, `crates/ferrioxid-cli/src/diagnose.rs` und `crates/ferrioxid-cli/Cargo.toml` zwischen #97 und #100
- `crates/ferrioxid-admin/src/html.rs` zwischen #98 und #99: JSON-Kanonisierung und Audit-Timeline
- `Cargo.lock` nur aus #100. `cargo metadata --locked` war erfolgreich.

## Breaking Changes und Migrationsschritte

- #97: Ohne `FERRIOXID_TRUSTED_PROXIES` oder `FERRUM_TRUSTED_PROXIES` akzeptiert der Proxy-Modus von keinem Peer ein `X-Forwarded-For`. `FERRIOXID_TRUST_PROXY=true` allein reicht nicht. Ein leeres oder beschädigtes Auth- oder Job-Migrationspaket ist ein Fehler. Anwendungen, die bisher jedes Peer vertrauten, müssen die Peer-Liste setzen.
- #98: `#[field(json)]` ist eine neue Spaltenart. Bestehende Textspalten werden nicht umgestellt. Eine neue JSON-Spalte braucht eine eigene Migration. Gruppierte Berichte lehnen Boolean, Decimal, JSON als Gruppe, Nicht-Bigint-Summen und eine Sortierung nach dem Mittelwert ab. `migrate plan` schreibt nicht.
- #99: Der Historienlink verwendet den Modellschlüssel aus den Metadaten. PostgreSQL bindet Audit-UUIDs als `uuid`. SQLite bleibt bei Text. Die Audit-Tabelle selbst bleibt.
- #100: `AuthConfig::from_env` und `MailService::open` brechen ab, wenn `FERRIOXID_*` und `FERRUM_*` verschieden gesetzt sind. Zwei Projektdateien im selben Verzeichnis sind ein Fehler. `DATABASE_URL` und `BIND_ADDRESS` haben keinen zweiten Namen. Vorher wurden `FERRUM_*` an diesen Stellen ignoriert.
- Tabellen `_ferrum_*` und `.ferrum/project.json` bleiben unverändert.

## Regressionstests

Neu auf diesem Branch:

- `inspect_framework_and_plan_stay_read_only_until_apply` und das ignorierte `postgres_inspect_framework_and_plan_stay_read_only`
- `config::tests::trusted_proxies_use_try_from_env_and_reject_conflicting_aliases`
- CLI `migrate plan` nach `migrate status` auf temporärem SQLite, inklusive JSON-Spalte
- Formularbindung: `csrf_token` neben kanonischem JSON, zusätzliches Feld bleibt `validation.unknown_field`
- `openapi_describes_canonical_json_documents`

Vorhandene Tests, die das Zusammenspiel mitabdecken:

- `canonical_json_keeps_documents_distinct_from_empty_text`
- `conflicting_aliases_fail_without_returning_either_value`
- `sqlite_grouped_report_scope_json_and_one_to_one`
- `matching_uses_the_record_id_not_a_similar_name`
- `admin_v018_drafts_inlines_bulk_and_audit`
- `direct_admin_login_survives_require_browser_writes_from_a_fresh_jar`
- `discovery_reads_one_project_file_and_rejects_both`
- `conflicting_secret_aliases_fail_closed`
- `conflicting_smtp_passwords_are_rejected`

## Qualität

Lokal auf `work/integration`, nicht GitHub-CI:

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all -- --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok. Die bestehende `linker_messages`-Warnung von `ferrioxid-macros` bleibt und wird von `-D warnings` nicht abgelehnt. |
| `cargo test --workspace` | ok |
| `cargo build --workspace` | ok |
| `cargo metadata --locked` | ok |
| `cargo tree ... -i rsa` und `-i sqlx-mysql` | nothing to print |
| `cargo audit` | nur `RUSTSEC-2023-0071` (`rsa` 0.9.10, lockfile-only) |
| `cargo audit --ignore RUSTSEC-2023-0071` | ok |

## PostgreSQL

Admin-URL für `DisposablePostgres`: `postgresql://postgres@127.0.0.1:5432/postgres`. Die Kataloge `ferrioxid` und `ferrioxid_migrations_down_test` wurden nicht als Testdatenbanken verwendet.

| Test | Ergebnis |
|---|---|
| `postgres_embedded_auth_and_jobs` | ok |
| `postgres_grouped_report_scope_json_and_one_to_one` | ok |
| `postgres_plan_detects_live_drift_on_a_disposable_database` | ok |
| `postgres_matching_uses_the_record_id_not_a_similar_name` | ok |
| `postgres_inspect_framework_and_plan_stay_read_only` | ok |

Vorher und nachher: `ferrioxid` 15 öffentliche Tabellen, `ferrioxid_migrations_down_test` 1 öffentliche Tabelle, keine übrige `ferrioxid_test_*`-Datenbank.

`postgres_keyset_annotations_and_many_to_many` wurde lokal nicht ausgeführt. Der Test schreibt in die durch `DATABASE_URL` benannte Datenbank und löscht dort eigene Tabellen. Er darf nicht gegen die befüllten Kataloge `ferrioxid` oder `ferrioxid_migrations_down_test` laufen.

## Commits auf `work/integration`

1. `7aae7cb` merge(pr-97): integrate runtime security and embedded migrations
2. `2a77858` merge(pr-98): integrate ORM JSON documents and live migration plans
3. `9a13229` merge(pr-99): integrate native admin audit timeline
4. `ad6bb8b` merge(pr-100): integrate developer apps aliases and project files
5. `63a648f` test(integration): cover the combined pull requests 97-100

## Risiken

- GitHub-CI auf diesem Integrationsbranch ist noch nicht gelaufen.
- Docker ist auf dieser Maschine nicht gestartet. `examples/deploy/compose.yaml` wurde nicht gebootet.
- Browserabnahme der Admin-Timeline und der CSRF-Login-Ausnahme fehlt.
- `load_one_to_one` lädt nicht automatisch in Admin-Listen. Die vorhandene Inline-Liste bleibt bei höchstens 25 Zeilen.
- Issue #8 bleibt offen: `rsa` liegt weiter nur im Lockfile über optionales SQLx-MySQL.
- Ein Rollback von #98 nach einer bereits angelegten JSON-Spalte braucht eine eigene Schemaentscheidung.
- Alias-Konflikte, die vor #100 still auf `FERRIOXID_*` fielen, sind jetzt Startfehler.

## Rollback

Kein Force-Push. Kein Merge nach `main` in diesem Durchlauf. Jeder Merge-Commit auf `work/integration` kann mit `git revert` zurückgenommen werden, solange niemand danach darauf aufbaut.

Vor einem Rollback die öffentlichen Tabellenzahlen von `ferrioxid` und `ferrioxid_migrations_down_test` prüfen. Disposable-Datenbanken `ferrioxid_test_*` löschen, wenn ein Test sie zurückgelassen hat.
