# Integration der Pull Requests #97 bis #100

**Historischer Integrationsbericht (12. November 2026):** Angaben zu offenen PRs, nicht gemergten Branches und damaligen Release-Blockern sind zeitgebundene Test-/Entwicklungsnachweise, keine Aussagen über den aktuellen Stand `a39c952` vom 14. November 2026. Aktueller Stand: [Roadmap](roadmap.md), [Release readiness](release-readiness.md).

Stand: 12. November 2026. Branch `work/integration` in `D:\04_Projekte_Arbeit\Projekte\fexu-wt-integration`. Basis `origin/main` bei `53a95aa`. Die vier PR-Branches wurden nicht überschrieben und nicht nach `main` gemerged.

## Merge-Reihenfolge

1. [#97](https://github.com/Iam-Sanja/ferrioxid/pull/97) `fix/runtime-security-docker` @ `c3f078a`
2. [#98](https://github.com/Iam-Sanja/ferrioxid/pull/98) `feat/orm-migrations-lifecycle` @ `cc5f676`
3. [#99](https://github.com/Iam-Sanja/ferrioxid/pull/99) `feat/native-admin` @ `9cdc200`
4. [#100](https://github.com/Iam-Sanja/ferrioxid/pull/100) `feat/developer-apps` @ `a5a571e`

## Konfliktlösungen

| Datei | Branches | Lösung |
|---|---|---|
| `crates/fexu-cli/src/main.rs` | #97 und #98 | `inspect_framework` und `plan_database` bleiben beide importiert. `migrate status` nutzt eingebettete Auth- und Job-Dateien. `migrate plan` bleibt read-only. |
| `docs/advanced-framework-progress.md` | alle | Die vier Blockberichte stehen untereinander. Kein Bericht hat einen anderen ersetzt. |
| `docs/index.md` | #97 und #98 | Die doppelte Deployment-Zeile aus dem Auto-Merge wurde zu einer Zeile mit `deployment.md` und `deploy-docker.md`. |
| `crates/fexu-auth/src/config.rs` | #97 und #100 | `trusted_proxies` liegt in `development()` und in `try_from_env`. Die Liste kommt aus `FEXU_TRUSTED_PROXIES`. Ein `*`-Eintrag bleibt ignoriert. |
| `crates/fexu-core/src/lib.rs` | #98 und #100 | `canonical_json_text` und `aliased_env` bleiben. Die zusammengeschobenen Doc-Kommentare wurden getrennt. Beide Testmodule bleiben. |

Automatisch zusammengeführt und danach gebaut:

- `crates/fexu-migrations/src/lib.rs` und `runner.rs` zwischen #97 und #98
- `crates/fexu-migrations/src/project.rs`, `crates/fexu-cli/src/diagnose.rs` und `crates/fexu-cli/Cargo.toml` zwischen #97 und #100
- `crates/fexu-admin/src/html.rs` zwischen #98 und #99: JSON-Kanonisierung und Audit-Timeline
- `Cargo.lock` nur aus #100. `cargo metadata --locked` war erfolgreich.

## Breaking Changes und Migrationsschritte

- #97: Ohne `FEXU_TRUSTED_PROXIES` akzeptiert der Proxy-Modus von keinem Peer ein `X-Forwarded-For`. `FEXU_TRUST_PROXY=true` allein reicht nicht. Ein leeres oder beschädigtes Auth- oder Job-Migrationspaket ist ein Fehler. Anwendungen, die bisher jedes Peer vertrauten, müssen die Peer-Liste setzen.
- #98: `#[field(json)]` ist eine neue Spaltenart. Bestehende Textspalten werden nicht umgestellt. Eine neue JSON-Spalte braucht eine eigene Migration. Gruppierte Berichte lehnen Boolean, Decimal, JSON als Gruppe, Nicht-Bigint-Summen und eine Sortierung nach dem Mittelwert ab. `migrate plan` schreibt nicht.
- #99: Der Historienlink verwendet den Modellschlüssel aus den Metadaten. PostgreSQL bindet Audit-UUIDs als `uuid`. SQLite bleibt bei Text. Die Audit-Tabelle selbst bleibt.
- #100: `AuthConfig::from_env` und `MailService::open` lesen genau eine `FEXU_*`-Variable. `DATABASE_URL` und `BIND_ADDRESS` haben keinen zweiten Namen.
- Tabellen `_fexu_*` und `.fexu/project.json` bleiben unverändert.

## Regressionstests

Neu auf diesem Branch:

- `inspect_framework_and_plan_stay_read_only_until_apply` und das ignorierte `postgres_inspect_framework_and_plan_stay_read_only`
- `config::tests::trusted_proxies_use_try_from_env`
- CLI `migrate plan` nach `migrate status` auf temporärem SQLite, inklusive JSON-Spalte
- Formularbindung: `csrf_token` neben kanonischem JSON, zusätzliches Feld bleibt `validation.unknown_field`
- `openapi_describes_canonical_json_documents`

Vorhandene Tests, die das Zusammenspiel mitabdecken:

- `canonical_json_keeps_documents_distinct_from_empty_text`
- `framework_env_reads_one_variable`
- `sqlite_grouped_report_scope_json_and_one_to_one`
- `matching_uses_the_record_id_not_a_similar_name`
- `admin_v018_drafts_inlines_bulk_and_audit`
- `direct_admin_login_survives_require_browser_writes_from_a_fresh_jar`
- `discovery_reads_fexu_toml`
- `csrf_secret_uses_only_fexu_csrf_secret`
- `smtp_password_uses_only_fexu_smtp_password`

## Release-Absicherung am 12. November 2026

GitHub Actions auf dem ersten Push von `work/integration` (`f479808`):

| Check | Ergebnis |
|---|---|
| Rust (ubuntu-latest) | bestanden |
| Rust (windows-latest) | bestanden, 25m 41s |
| PostgreSQL | fehlgeschlagen |
| Cargo audit | bestanden, mit der bestehenden RSA-Ausnahme |
| CodeQL Rust, Actions, JavaScript, Python | bestanden |

GitHub Actions auf `b953360` nach der Audit-Korrektur:

| Check | Ergebnis |
|---|---|
| Rust (ubuntu-latest) | bestanden, 8m 27s, Lauf `43321495810` |
| Rust (windows-latest) | bestanden, 18m 57s, Lauf `43321495810` |
| PostgreSQL | bestanden, 4m 26s, Lauf `43321495810` |
| Cargo audit | bestanden, 21s, Lauf `43321495884`, mit der bestehenden RSA-Ausnahme |
| CodeQL Rust, Actions, JavaScript, Python | bestanden, Lauf `43321486733` |

Im PostgreSQL-Job dieses Laufs waren `postgres_keyset_annotations_and_many_to_many` und `postgres_matching_uses_the_record_id_not_a_similar_name` grün. Der Server-Log enthielt erneut `unsupported jsonb version number 110`. Der Job blieb grün. Die Meldung bleibt eine Folgeprüfung.

Der PostgreSQL-Job des ersten Laufs scheiterte nur an `postgres_matching_uses_the_record_id_not_a_similar_name`. Der Test öffnete fest `postgresql://postgres@127.0.0.1:5432/postgres`. In GitHub Actions gibt es die Rolle `postgres` nicht. Die Admin-URL ist jetzt `FEXU_POSTGRES_ADMIN`, sonst `DATABASE_URL`. `DisposablePostgres` legt weiterhin nur `fexu_test_*` an und löscht nur diese Datenbank. Lokal gegen `postgresql://postgres@127.0.0.1:5432/postgres` war der Test danach grün. `fexu` blieb bei 15 öffentlichen Tabellen, `fexu_migrations_down_test` bei 1.

`postgres_keyset_annotations_and_many_to_many` lief lokal gegen eine neu angelegte Datenbank `fexu_test_013b7cba8b9478b7b66a12455f3a8ea9` und wurde danach gelöscht. Der Test löscht nur `fexu_query_membership`, `fexu_query_tags` und `fexu_query_items`. Dieselben Zähler der beiden bestehenden Kataloge blieben unverändert. Der GitHub-PostgreSQL-Job führt denselben Test jetzt in seinem frischen Container aus. Lokal darf `DATABASE_URL` dafür nicht auf `fexu` oder `fexu_migrations_down_test` zeigen.

HTTP- und CLI-Abnahme, jeweils SQLite im Prozess, ohne Browserfenster:

- `direct_admin_login_survives_require_browser_writes_from_a_fresh_jar`
- `sqlite_auth_http_permissions_and_csrf`
- `admin_v018_drafts_inlines_bulk_and_audit`
- `migrate_make_up_and_status_use_a_temporary_sqlite_database`

Docker ist auf dieser Maschine nicht installiert. `examples/deploy/compose.yaml` wurde gelesen und nicht gestartet. PostgreSQL darin hat keinen veröffentlichten Host-Port. Das Passwort kommt aus der Umgebung. `embedded-migrate` beendet sich nach Auth, Login und einem Job. Ein dauerhafter App-Container und eine Browserabnahme über Caddy sind Folgearbeit, kein Teil dieses Integrationscommits.

## Kompatibilität

Neue Generatorprojekte schreiben `fexu.toml` und setzen `BIND_ADDRESS=127.0.0.1:3000`. Sie setzen `FEXU_TRUST_PROXY` nicht. Für sie ändert die Peer-Liste nichts.

Bestehende Deployments, die `FEXU_TRUST_PROXY=true` ohne `FEXU_TRUSTED_PROXIES` nutzen, akzeptieren `X-Forwarded-For` danach von keinem Peer. Die Peer-Adresse des Reverse-Proxy muss gesetzt werden.

Ein Verzeichnis mit `fexu.toml` wird geladen. Eine zweite Projektdatei gibt es nicht. `DATABASE_URL` und `BIND_ADDRESS` haben keinen zweiten Namen. Generierte Projekte, die `fexu.toml` verwenden, starten mit den `FEXU_*`-Variablen.

`#[field(json)]` verändert keine vorhandene Textspalte. Eine JSON-Spalte braucht eine neue Migration.

## Issue #8

`rsa 0.9.10` bleibt eine Lockfile-Abhängigkeit der optionalen Kiste `sqlx-mysql 0.8.6`. Der aktivierte Graph enthält sie nicht. Am 2026-11-13 drucken `cargo tree -i rsa` und `cargo tree -i sqlx-mysql` nichts. SeaORM hat MySQL bereits aus. Ein weiteres Abschalten ändert das Lockfile nicht. SQLx 0.9 macht `rsa` nur optional und bleibt bei `rsa` 0.9; der Sprung wäre mit dem vendored SQLite-0.8.6-Patch nicht verträglich. Der ungefilterte Audit bleibt absichtlich rot. Details stehen in [security-rsa.md](security-rsa.md).

## Folgeaufgaben

- Docker-Compose auf einer Maschine mit Docker starten und den Exit-Code von `embedded-migrate` prüfen. Der Compose-App-Dienst ist kein dauerhafter Webserver.
- Browserabnahme von Admin-Login, Formular und Audit-Timeline.
- Issue #8 schließen, sobald SQLx die RSA-Abhängigkeit anhebt.
- Kein Merge nach `main` in diesem Durchlauf.

## Qualität

Lokal auf `work/integration`, nicht GitHub-CI:

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all -- --check` | ok |
| `cargo clippy --workspace --all-targets -- -D warnings` | ok. Die bestehende `linker_messages`-Warnung von `fexu-macros` bleibt und wird von `-D warnings` nicht abgelehnt. |
| `cargo test --workspace` | ok |
| `cargo build --workspace` | ok. Dieselbe Folge lief nach der Audit-Korrektur erneut durch: Formatierung, Clippy, `cargo test --workspace` und Build, Exit-Code 0. Ignorierte PostgreSQL-Tests dieses Workspace-Laufs waren übersprungen, weil `DATABASE_URL` nicht gesetzt war. |
| `cargo metadata --locked` | ok |
| `cargo tree ... -i rsa` und `-i sqlx-mysql` | nothing to print |
| `cargo audit` | nur `RUSTSEC-2023-0071` (`rsa` 0.9.10, lockfile-only) |
| `cargo audit --ignore RUSTSEC-2023-0071` | ok |

Die Tabelle beschreibt diesen Integrationsstand. Der spätere Datenbanktreiber entfernt die `sqlx`-Fassade, damit auch `rsa` aus der Lock-Datei. `cargo audit` hat danach kein Ignore mehr. Siehe [database-driver.md](database-driver.md).

## PostgreSQL

Admin-URL für `DisposablePostgres`: `postgresql://postgres@127.0.0.1:5432/postgres`. Die Kataloge `fexu` und `fexu_migrations_down_test` wurden nicht als Testdatenbanken verwendet.

| Test | Ergebnis |
|---|---|
| `postgres_embedded_auth_and_jobs` | ok |
| `postgres_grouped_report_scope_json_and_one_to_one` | ok |
| `postgres_plan_detects_live_drift_on_a_disposable_database` | ok |
| `postgres_matching_uses_the_record_id_not_a_similar_name` | ok |
| `postgres_inspect_framework_and_plan_stay_read_only` | ok |

Vorher und nachher: `fexu` 15 öffentliche Tabellen, `fexu_migrations_down_test` 1 öffentliche Tabelle, keine übrige `fexu_test_*`-Datenbank.

`postgres_keyset_annotations_and_many_to_many` lief gegen die isolierte Datenbank `fexu_test_013b7cba8b9478b7b66a12455f3a8ea9` und wurde danach gelöscht. Der Test schreibt nur `fexu_query_membership`, `fexu_query_tags` und `fexu_query_items`. Er darf nicht gegen die befüllten Kataloge `fexu` oder `fexu_migrations_down_test` laufen.

## Commits auf `work/integration`

1. `7aae7cb` merge(pr-97): integrate runtime security and embedded migrations
2. `2a77858` merge(pr-98): integrate ORM JSON documents and live migration plans
3. `9a13229` merge(pr-99): integrate native admin audit timeline
4. `ad6bb8b` merge(pr-100): integrate developer apps aliases and project files
5. `63a648f` test(integration): cover the combined pull requests 97-100
6. `f479808` docs: record the local integration of pull requests 97-100
7. `e451da9` fix(test): read the audit database from DATABASE_URL
8. `b953360` docs: record the audit-database commit on the integration branch
9. `b27b2b0` docs: record the green GitHub run after the audit-database fix
10. `3a59dc3` merge(pr-103): integrate admin configuration and shared forms
11. `3985172` merge(pr-102): integrate the API contract and deploy checks
12. `7371dba` merge(pr-104): integrate MFA, scheduled jobs, cache and storage

## Risiken

- Der erste GitHub-Lauf auf `f479808` war auf Ubuntu, Windows, Cargo audit und CodeQL grün und auf PostgreSQL rot, weil der Audit-Test die Rolle `postgres` verlangte. `e451da9` liest `DATABASE_URL`. Der Lauf auf `b953360` war danach auf Ubuntu, Windows, PostgreSQL, Cargo audit und CodeQL grün.
- Docker ist auf dieser Maschine nicht installiert. `examples/deploy/compose.yaml` wurde nicht gebootet.
- Eine Desktop-Browserabnahme fehlt. Die HTTP-Verträge liefen über den Axum-Router.
- `load_one_to_one` lädt nicht automatisch in Admin-Listen. Die vorhandene Inline-Liste bleibt bei höchstens 25 Zeilen.
- Issue #8 bleibt offen. `rsa 0.9.10` ist nicht im aktivierten Graphen. SQLx 0.9 macht die Kiste nur optional und bleibt bei `rsa` 0.9. Die Ausnahme wird nicht erweitert.
- Ein Rollback von #98 nach einer bereits angelegten JSON-Spalte braucht eine eigene Schemaentscheidung.
- Alias-Konflikte, die vor #100 still auf `FEXU_*` fielen, sind jetzt Startfehler.
- Der PostgreSQL-16-Log enthielt auf `f479808` und erneut auf `b953360` bei `INSERT INTO "fexu_report_notes" ("id", "body")` die Meldung `unsupported jsonb version number 110`. Parameter `$2` ist die JSONB-Spalte. Byte 110 ist `n` aus dem absichtlich abgelehnten Text `not-json`. `Value::Json` trägt die JSONB-Version 1 und wurde gespeichert. Seit `7371dba` wird Text für eine JSON-Spalte vor dem Binden geprüft und nur als `Value::Json` gesendet. Ungültiger Text ist `validation.json`.

## Integration von #103, #102 und #104

Reihenfolge nach Abhängigkeit: #103, dann #102, dann #104. #102 ersetzt die Proxy-Prüfung und #104 die eingebetteten Migrationen. Deshalb liegt #104 zuletzt.

| Merge | Commit | Konflikte und Entscheidung |
|---|---|---|
| #103 `work/admin-devex` @ `1db4671` | `3a59dc3` | `ModelAdmin` verengt `AdminModelMeta`. HTML- und JSON-Formulare teilen die Textprüfung. `ColumnKind::Json` bleibt kanonisch. Audit-Timeline und One-to-one bleiben. |
| #102 `work/api-quality` @ `9fd1fc2` | `3985172` | OpenAPI 3.1, Keyset, `problem+json`, `/readyz`, `check --deploy`, Dump/Restore und der Playwright-Job bleiben. `#[field(json)]` und `write_only` bleiben beide. Ein aktiver Proxy vertraut nur `FEXU_TRUSTED_PROXIES`. `*` zählt nicht. Widersprüchliche Aliase sind ein Fehler und werden nicht ausgegeben. |
| #104 `work/framework-services` @ `f2293b4` | `7371dba` | Sechs Auth-Migrationen `0001`–`0006` und zwei Job-Migrationen `0001`–`0002` sind eingebettet. `apply_migrations` liest die eingebetteten Dateien, nicht das Quellverzeichnis. In `queue.rs` bleibt ein `id_expr`: PostgreSQL bindet die Id als UUID, SQLite als Text. |

`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` und `cargo test --workspace` waren nach jedem dieser Merges grün. Ignorierte PostgreSQL-Tests dieses Workspace-Laufs waren übersprungen, weil `DATABASE_URL` nicht gesetzt war.

PostgreSQL 18.6 auf `127.0.0.1` danach, nur über Wegwerfdatenbanken `fexu_test_<32 hex>`:

| Test | Ergebnis |
|---|---|
| `postgres_grouped_report_scope_json_and_one_to_one` | ok. Ungültiges JSON endet als `validation.json`. |
| `postgres_embedded_auth_and_jobs` | ok. Sechs Auth- und zwei Job-Migrationen. |
| `postgres_dump_and_restore_roundtrip` | ok, `pg_dump` 18.6 |
| `postgres_matching_uses_the_record_id_not_a_similar_name` | ok |
| `postgres_cursor_filters_and_openapi_describe_the_same_list` | ok |

`fexu` blieb bei 15 öffentlichen Tabellen, `fexu_migrations_down_test` bei 1. Die vier älteren Datenbanken `fexu_test_services_anon_20261112233529`, `fexu_test_services_auth_20261112233529`, `fexu_test_services_jobs_20261112233529` und `fexu_test_services_names_20261112233529` wurden nicht gelöscht. Keine neue `fexu_test_*`-Datenbank blieb zurück.

## Release-Blocker

- Neue TOTP-Geheimnisse verwenden `FEXU_MFA_KEY` (64 Hex-Zeichen, 32 Byte, kein `FEXU_`-Alias). Legacy-Geheimtext wird nach dem Öffnen neu versiegelt. Legacy-Recovery-Hashes bleiben HMAC und werden nicht umgeschrieben; dafür muss das bisherige CSRF-Geheimnis bleiben, bis die Codes verbraucht sind. Details stehen in [auth-threat-model.md](auth-threat-model.md).
- Issue #8 bleibt offen. `rsa 0.9.10` liegt nur als optionale `sqlx-mysql`-Abhängigkeit im Lockfile. `cargo tree -i rsa` und `cargo tree -i sqlx-mysql` drucken nichts. Ein Feature-Schalter entfernt den Eintrag nicht. Die Ausnahme wird nicht erweitert. Der Stand steht in [security-rsa.md](security-rsa.md).
- Docker ist auf dieser Maschine nicht installiert. `deploy/compose.yaml` und `examples/deploy/compose.yaml` wurden nicht gestartet.
- Playwright lief lokal nicht. Auf `febc241` installierte Playwright Chromium auf Ubuntu 26.04 nicht. Der Browser-Job läuft deshalb auf `ubuntu-24.04`. Auf `57f0266` startete `cargo run -p myshop` nicht, weil das Paket zusätzlich `fexu-manage` enthält. `default-run` ist `myshop`, und der Job ruft `--bin myshop` auf. Der PostgreSQL-Job ist ebenfalls auf `ubuntu-24.04` festgelegt: der Client von `ubuntu-latest` schreibt `SET transaction_timeout`, und der Dienst `postgres:16` lehnt das ab. Auf `d844517` sind Browser, PostgreSQL, beide Rust-Jobs und Cargo Audit grün. Die PostgreSQL-Logs dieses Laufs enthalten `unsupported jsonb version number 110` nicht. CodeQL meldete drei kritische Treffer der bekannten Regel `rust/hard-coded-cryptographic-value`: die In-Memory-Passwörter in `security_tests` und den mit Nullen angelegten Nonce-Puffer in `seal`, den `OsRng` danach füllte. Die Passwörter werden wie die übrigen Fixtures aus Bytes gebaut. Der Nonce kommt von `Aes256Gcm::generate_nonce`. Das ist kein neues Produktionsgeheimnis.

## Rollback

Kein Force-Push. Kein Merge nach `main` in diesem Durchlauf. Jeder Merge-Commit auf `work/integration` kann mit `git revert` zurückgenommen werden, solange niemand danach darauf aufbaut.

Vor einem Rollback die öffentlichen Tabellenzahlen von `fexu` und `fexu_migrations_down_test` prüfen. Disposable-Datenbanken `fexu_test_*` löschen, wenn ein Test sie zurückgelassen hat.
