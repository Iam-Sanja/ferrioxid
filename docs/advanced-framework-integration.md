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

## Release-Absicherung am 12. November 2026

GitHub Actions auf dem ersten Push von `work/integration` (`f479808`):

| Check | Ergebnis |
|---|---|
| Rust (ubuntu-latest) | bestanden |
| Rust (windows-latest) | bestanden, 25m 41s |
| PostgreSQL | fehlgeschlagen |
| Cargo audit | bestanden, mit der bestehenden RSA-Ausnahme |
| CodeQL Rust, Actions, JavaScript, Python | bestanden |

Der PostgreSQL-Job scheiterte nur an `postgres_matching_uses_the_record_id_not_a_similar_name`. Der Test öffnete fest `postgresql://postgres@127.0.0.1:5432/postgres`. In GitHub Actions gibt es die Rolle `postgres` nicht. Die Admin-URL ist jetzt `FERRIOXID_POSTGRES_ADMIN`, sonst `DATABASE_URL`. `DisposablePostgres` legt weiterhin nur `ferrioxid_test_*` an und löscht nur diese Datenbank. Lokal gegen `postgresql://postgres@127.0.0.1:5432/postgres` war der Test danach grün. `ferrioxid` blieb bei 15 öffentlichen Tabellen, `ferrioxid_migrations_down_test` bei 1.

`postgres_keyset_annotations_and_many_to_many` lief lokal gegen eine neu angelegte Datenbank `ferrioxid_test_013b7cba8b9478b7b66a12455f3a8ea9` und wurde danach gelöscht. Der Test löscht nur `ferrioxid_query_membership`, `ferrioxid_query_tags` und `ferrioxid_query_items`. Dieselben Zähler der beiden bestehenden Kataloge blieben unverändert. Der GitHub-PostgreSQL-Job führt denselben Test jetzt in seinem frischen Container aus. Lokal darf `DATABASE_URL` dafür nicht auf `ferrioxid` oder `ferrioxid_migrations_down_test` zeigen.

HTTP- und CLI-Abnahme, jeweils SQLite im Prozess, ohne Browserfenster:

- `direct_admin_login_survives_require_browser_writes_from_a_fresh_jar`
- `sqlite_auth_http_permissions_and_csrf`
- `admin_v018_drafts_inlines_bulk_and_audit`
- `migrate_make_up_and_status_use_a_temporary_sqlite_database`

Docker ist auf dieser Maschine nicht installiert. `examples/deploy/compose.yaml` wurde gelesen und nicht gestartet. PostgreSQL darin hat keinen veröffentlichten Host-Port. Das Passwort kommt aus der Umgebung. `embedded-migrate` beendet sich nach Auth, Login und einem Job. Ein dauerhafter App-Container und eine Browserabnahme über Caddy sind Folgearbeit, kein Teil dieses Integrationscommits.

## Kompatibilität

Neue Generatorprojekte schreiben `ferrioxid.toml` und setzen `BIND_ADDRESS=127.0.0.1:3000`. Sie setzen `FERRIOXID_TRUST_PROXY` nicht. Für sie ändert die Peer-Liste nichts.

Bestehende Deployments, die `FERRIOXID_TRUST_PROXY=true` ohne `FERRIOXID_TRUSTED_PROXIES` nutzen, akzeptieren `X-Forwarded-For` danach von keinem Peer. Die Peer-Adresse des Reverse-Proxy muss gesetzt werden. `FERRUM_TRUSTED_PROXIES` gilt nur, wenn der neue Name fehlt oder denselben Wert hat.

Ein Verzeichnis mit nur `ferrum.toml` wird weiter geladen. Liegen `ferrioxid.toml` und `ferrum.toml` zusammen, bricht das Lesen ab. `DATABASE_URL` und `BIND_ADDRESS` haben keinen Legacy-Namen. Generierte Projekte, die bereits `ferrioxid.toml` verwenden und keine widersprüchlichen `FERRUM_*`-Variablen setzen, starten unverändert.

`#[field(json)]` verändert keine vorhandene Textspalte. Eine JSON-Spalte braucht eine neue Migration.

## Issue #8

`rsa 0.9.10` bleibt eine Lockfile-Abhängigkeit von `sqlx-mysql 0.8.6`. Der aktivierte Graph enthält sie nicht. Der ungefilterte Audit bleibt absichtlich rot. Ein Patch auf `rsa` 0.10 ist keine Lösung, weil `sqlx-mysql` 0.9 erwartet. Der konkrete Weg ist ein späteres SQLx-Release mit `rsa >= 0.10`, danach `cargo update -p sqlx-mysql` und erst dann das Entfernen der Ausnahme. Details stehen in [security-rsa.md](security-rsa.md).

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
| `cargo clippy --workspace --all-targets -- -D warnings` | ok. Die bestehende `linker_messages`-Warnung von `ferrioxid-macros` bleibt und wird von `-D warnings` nicht abgelehnt. |
| `cargo test --workspace` | ok |
| `cargo build --workspace` | ok. Dieselbe Folge lief nach der Audit-Korrektur erneut durch: Formatierung, Clippy, `cargo test --workspace` und Build, Exit-Code 0. Ignorierte PostgreSQL-Tests dieses Workspace-Laufs waren übersprungen, weil `DATABASE_URL` nicht gesetzt war. |
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

`postgres_keyset_annotations_and_many_to_many` lief gegen die isolierte Datenbank `ferrioxid_test_013b7cba8b9478b7b66a12455f3a8ea9` und wurde danach gelöscht. Der Test schreibt nur `ferrioxid_query_membership`, `ferrioxid_query_tags` und `ferrioxid_query_items`. Er darf nicht gegen die befüllten Kataloge `ferrioxid` oder `ferrioxid_migrations_down_test` laufen.

## Commits auf `work/integration`

1. `7aae7cb` merge(pr-97): integrate runtime security and embedded migrations
2. `2a77858` merge(pr-98): integrate ORM JSON documents and live migration plans
3. `9a13229` merge(pr-99): integrate native admin audit timeline
4. `ad6bb8b` merge(pr-100): integrate developer apps aliases and project files
5. `63a648f` test(integration): cover the combined pull requests 97-100
6. `f479808` docs: record the local integration of pull requests 97-100
7. Release-Absicherung nach `f479808`: der Audit-Test liest `FERRIOXID_POSTGRES_ADMIN` oder `DATABASE_URL`, der PostgreSQL-Job führt `postgres_keyset_annotations_and_many_to_many` aus, und [security-rsa.md](security-rsa.md) beschreibt den Abbau von `RUSTSEC-2023-0071`.

## Risiken

- Der erste GitHub-Lauf auf `f479808` war auf Ubuntu, Windows, Cargo audit und CodeQL grün und auf PostgreSQL rot, weil der Audit-Test die Rolle `postgres` verlangte. Der Folgecommit liest `DATABASE_URL`. Der neue Lauf ist erst nach dem Push belegt.
- Docker ist auf dieser Maschine nicht installiert. `examples/deploy/compose.yaml` wurde nicht gebootet.
- Eine Desktop-Browserabnahme fehlt. Die HTTP-Verträge liefen über den Axum-Router.
- `load_one_to_one` lädt nicht automatisch in Admin-Listen. Die vorhandene Inline-Liste bleibt bei höchstens 25 Zeilen.
- Issue #8 bleibt offen, bis SQLx `rsa >= 0.10` zieht. Die Ausnahme wird nicht erweitert.
- Ein Rollback von #98 nach einer bereits angelegten JSON-Spalte braucht eine eigene Schemaentscheidung.
- Alias-Konflikte, die vor #100 still auf `FERRIOXID_*` fielen, sind jetzt Startfehler.
- Der PostgreSQL-16-Log des ersten Laufs enthielt `unsupported jsonb version number 110` während `postgres_grouped_report_scope_json_and_one_to_one`. Der Test selbst war grün. Das ist keine stille Freigabe und bleibt eine Folgeprüfung, kein Schemaumbau in diesem Commit.

## Rollback

Kein Force-Push. Kein Merge nach `main` in diesem Durchlauf. Jeder Merge-Commit auf `work/integration` kann mit `git revert` zurückgenommen werden, solange niemand danach darauf aufbaut.

Vor einem Rollback die öffentlichen Tabellenzahlen von `ferrioxid` und `ferrioxid_migrations_down_test` prüfen. Disposable-Datenbanken `ferrioxid_test_*` löschen, wenn ein Test sie zurückgelassen hat.
