# Integration der Pull Requests #97 bis #100

Stand: die vier Branches sind unabhängig von `main` bei `53a95aa` gezogen. Jeder lässt sich einzeln auf `main` anwenden. Konflikte entstehen erst, wenn sie nacheinander in denselben Branch gelangen. Dieser Plan merged nichts.

## Merge-Reihenfolge

1. [#97](https://github.com/Iam-Sanja/ferrioxid/pull/97) `fix/runtime-security-docker`
2. [#98](https://github.com/Iam-Sanja/ferrioxid/pull/98) `feat/orm-migrations-lifecycle`
3. [#99](https://github.com/Iam-Sanja/ferrioxid/pull/99) `feat/native-admin`
4. [#100](https://github.com/Iam-Sanja/ferrioxid/pull/100) `feat/developer-apps`

#97 zuerst, weil es die Proxy-Vertrauensgrenze und die eingebetteten Migrationen festlegt. #98 danach, weil `migrate plan` und `inspect_framework` in derselben CLI-Funktion zusammengebaut werden müssen. #99 danach, weil die Admin-Timeline die Audit-Tabelle nur liest und den ORM-Branch nicht zum Übersetzen braucht. #100 zuletzt, weil `from_env` die Proxy-Felder aus #97 und `aliased_env` die Kern-API aus #98 aufnehmen muss. `Cargo.lock` ändert sich nur in #100 und wird so als letzter Lockfile-Stand geprüft.

## Erwartete Konflikte

| Datei | Branches | Art |
|---|---|---|
| `docs/advanced-framework-progress.md` | alle Paare | add/add. Jeder Branch hat einen eigenen Bericht. Beim Merge die Abschnitte untereinander stehen lassen, nicht einen Bericht durch den anderen ersetzen. |
| `crates/ferrioxid-cli/src/main.rs` | #97 und #98 | Inhalt. #97 ersetzt den Status-Aufruf durch `inspect_framework` mit eingebetteten Auth- und Job-Dateien. #98 fügt davor `MigrateCommand::Plan` und den Import `plan_database` ein. Beide Importblöcke und der `run_migrate`-Zweig müssen erhalten bleiben. |
| `crates/ferrioxid-auth/src/config.rs` | #97 und #100 | Inhalt. #97 ergänzt `trusted_proxies` und liest `FERRIOXID_TRUSTED_PROXIES` in `from_env`. #100 zerlegt `from_env` in `try_from_env` und liest Aliase über `aliased_env`. Das Proxy-Feld gehört in `development()` und in `try_from_env`, mit Alias `FERRUM_TRUSTED_PROXIES`, ohne einen der beiden Werte still zu wählen. |
| `crates/ferrioxid-core/src/lib.rs` | #98 und #100 | Inhalt. #98 fügt `canonical_json_text` ein. #100 fügt `aliased_env` ein. Beide Funktionen bleiben. |

Automatisch zusammenführbar, trotzdem danach bauen:

- `crates/ferrioxid-migrations/src/lib.rs` und `runner.rs` zwischen #97 und #98
- `crates/ferrioxid-migrations/src/project.rs`, `crates/ferrioxid-cli/src/diagnose.rs` und `crates/ferrioxid-cli/Cargo.toml` zwischen #97 und #100
- `crates/ferrioxid-admin/src/html.rs` zwischen #98 und #99
- `docs/index.md` zwischen #97 und #98

`Cargo.lock` ändert sich nur in #100, weil Mail und CLI dort direkt von `ferrioxid-core` abhängen. #97 fügt nur das Binary `embedded-migrate` hinzu und kein neues Crate. Nach dem letzten Merge `cargo metadata --locked` ausführen. Schlägt das fehl, den Lockfile neu schreiben und den Diff prüfen. Keine vorhandene Datenbank dafür öffnen.

## Breaking Changes

- #97: Ohne `FERRIOXID_TRUSTED_PROXIES` akzeptiert der Proxy-Modus von keinem Peer ein `X-Forwarded-For`. Ein leeres Auth- oder Job-Migrationspaket ist ein Fehler.
- #98: `#[field(json)]` ist eine neue Spaltenart. Bestehende Textspalten werden nicht umgestellt. Gruppierte Berichte lehnen Boolean, Decimal, JSON als Gruppe, Nicht-Bigint-Summen und eine Sortierung nach dem Mittelwert ab. `migrate plan` schreibt nicht. `migrate up` und `migrate down` bleiben in ihrer bisherigen Wirkung.
- #99: Der Historienlink verwendet den Modellschlüssel aus den Metadaten. In den vorhandenen Routen ist das derselbe Schlüssel wie im Pfad.
- #100: `AuthConfig::from_env` und `MailService::open` brechen ab, wenn `FERRIOXID_*` und `FERRUM_*` verschieden gesetzt sind. Zwei Projektdateien im selben Verzeichnis sind ein Fehler. `DATABASE_URL` und `BIND_ADDRESS` haben keinen zweiten Namen.

## Abhängigkeiten

Kein Branch braucht den Code eines anderen zum Übersetzen. Die fachlichen Abhängigkeiten sind:

- #99 nennt `load_one_to_one` als fehlend. Die Funktion liegt in #98 und wird hier nicht nachgebaut.
- #100 muss nach #97 das Feld `trusted_proxies` in den neuen `try_from_env`-Pfad übernehmen.
- #98 und #97 teilen sich die Migrations-CLI, aber keine Laufzeitdaten.
- Tabellen `_ferrum_*` und `.ferrum/project.json` bleiben in allen Branches unverändert.

## Regressionstests nach jedem Merge

Nach #97:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` ohne `DATABASE_URL`
- `postgres_embedded_auth_and_jobs` nur mit einer Admin-URL auf `postgres` und einer Datenbank `ferrioxid_test_*`
- `cargo tree` für `rsa` und `sqlx-mysql` im aktivierten Graph

Nach #98 zusätzlich:

- `sqlite_grouped_report_scope_json_and_one_to_one`
- `postgres_grouped_report_scope_json_and_one_to_one`
- `postgres_plan_detects_live_drift_on_a_disposable_database`
- `postgres_keyset_annotations_and_many_to_many` nicht gegen `ferrioxid` oder `ferrioxid_migrations_down_test` ausführen

Nach #99 zusätzlich:

- `matching_uses_the_record_id_not_a_similar_name`
- `postgres_matching_uses_the_record_id_not_a_similar_name`
- `admin_v018_drafts_inlines_bulk_and_audit`

Nach #100 zusätzlich:

- `conflicting_aliases_fail_without_returning_either_value`
- `discovery_reads_one_project_file_and_rejects_both`
- `conflicting_secret_aliases_fail_closed`
- `conflicting_smtp_passwords_are_rejected`
- `generated_api_and_fullstack_projects_compile`
- `cargo metadata --locked`

Nach dem vollständigen Stapel dieselben vier Workspace-Befehle einmal auf dem Integrationsergebnis. GitHub-CI auf dem jeweiligen PR ersetzt den lokalen Lauf nicht.

## Rollback

Jeder Merge bleibt ein eigener Commit auf `main` und kann mit `git revert` zurückgenommen werden, solange niemand danach weiter darauf aufbaut. Kein Force-Push.

- #97 zurücknehmen stellt die weitere Proxy-Vertrauensgrenze zurück und liest Migrationen wieder aus dem Quellverzeichnis. Bereits geschriebene Migrationshistorie bleibt liegen.
- #98 zurücknehmen entfernt JSON-Felder und `migrate plan` aus dem Code. Eine Datenbank, in der bereits eine JSON-Spalte angelegt wurde, braucht danach eine eigene Schemaentscheidung. Der Plan-Befehl selbst schreibt nicht.
- #99 zurücknehmen entfernt die Timeline. Die Audit-Tabelle bleibt. Der PostgreSQL-UUID-Bind für Audit-Schreiben ginge mit zurück und die bisherigen Inserts auf PostgreSQL würden wieder am Texttyp scheitern.
- #100 zurücknehmen ignoriert `FERRUM_*` wieder und lädt `ferrum.toml` nicht. Widersprüchliche Alias-Paare würden dann still auf den `FERRIOXID_*`-Wert fallen.

Vor jedem Rollback die öffentlichen Tabellenzahlen von `ferrioxid` und `ferrioxid_migrations_down_test` prüfen. Disposable-Datenbanken `ferrioxid_test_*` löschen, wenn ein Test sie zurückgelassen hat.
