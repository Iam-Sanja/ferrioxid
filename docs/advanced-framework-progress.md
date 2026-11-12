# Fortschritt des erweiterten Frameworks

Stand dieses Branches: `feat/developer-apps` bei `938c267a5fb057b3d5c5780ae798ec37e1dbce17`.

Basis ist `main` bei `53a95aa`. Dieser Branch enthält Block 3 nicht.

Die anderen lokalen Pakete bleiben unangetastet:

- Block 1, `fix/runtime-security-docker`: `ad487023d79d9faebd0a711258cdc848e1bb9fa2`
- Block 2, `feat/orm-migrations-lifecycle`: `3bb51b951f904a7bb092c1910c7af8c7dc3332ed`
- Block 3, `feat/native-admin`: `f87fd9f6c34cf115dd83d9d8e4611d0185112d98`
- Vorbereitung, `prep/native-admin`: `09172fdaf828e05a6988aff07c378ec97e98245d`

Kein Push, kein Merge. Die Issues bleiben offen.

## Block 4

### Umgesetzt

- `discover_project` lädt `ferrioxid.toml`. Liegt in demselben Verzeichnis nur `ferrum.toml`, wird diese Datei geladen. Liegen beide Dateien dort, bricht das Lesen ab und nennt beide Pfade. Es wird keine der beiden Datenbank-URLs gewählt.
- `ferrioxid_core::aliased_env` liest `FERRIOXID_<NAME>` und `FERRUM_<NAME>`. Ein gesetzter Name gilt. Gleiche Werte gelten. Unterschiedliche Werte sind ein Fehler, und die Meldung enthält die Werte nicht.
- `DATABASE_URL` und `BIND_ADDRESS` haben keinen Legacy-Alias.
- `AuthConfig::try_from_env` und `MailService::try_open` geben den Konflikt zurück. `from_env` und `open` brechen in diesem Fall ab, statt ein Geheimnis still zu wählen.
- `.ferrum/project.json` und die Tabellennamen `_ferrum_*` bleiben unverändert.

### Issue #19

Erfüllt auf diesem Branch: neue Projekte bleiben bei `ferrioxid.toml`. Ein altes Projekt mit nur `ferrum.toml` wird geladen. Zwei Projektdateien und widersprüchliche Geheimnis-Aliase werden getestet. Die Cargo-Version bleibt `0.1.0`.

Offen: kein eigener PostgreSQL-Lauf für diese Änderung, weil sie keine Datenbank schreibt. Die Generator- und Admin-Regression des gesamten Workspace ist für diesen Commit noch nicht erneut gelaufen. Linux-CI ist hier nicht ausgeführt.

### Issue #86

Nicht in diesem Commit erweitert. `App::install`, `FerrioxidApplication` und `docs/apps.md` beschreiben bereits die statische Registrierung. Ein versioniertes App-Paket, das zwei generierte Projekte ohne Quelländerung installieren, fehlt. Eine Deinstallation, die Daten stehen lässt, ist nicht neu spezifiziert.

### Issue #89

Nicht in diesem Commit erweitert. Listen, Formulare und CSRF bleiben in den bestehenden Admin- und Form-APIs. Eine consume-once Flash-Nachricht in der Sitzung und generische CRUD-Helfer außerhalb des Admin wurden nicht ergänzt.

### Issue #91

Nicht in diesem Commit erweitert. `ferrioxid check` meldet einen Alias-Konflikt für das CSRF-Geheimnis und das SMTP-Passwort mit dem Variablennamen und ohne den Wert. `ferrioxid dev` startet bereits mit Watch und ohne automatische Migration. Ein sauberer Projektstart von der Installation bis zum laufenden Formular wurde in diesem Commit nicht erneut auf Windows und Linux ausgeführt.

### Tests

- SQLite: nicht betroffen
- PostgreSQL: nicht ausgeführt, keine Schemaänderung
- `conflicting_aliases_fail_without_returning_either_value`
- `discovery_reads_one_project_file_and_rejects_both`
- `custom_settings_env_and_production_stay_explicit`
- `conflicting_secret_aliases_fail_closed`
- `conflicting_smtp_passwords_are_rejected`
- `cargo clippy` für `ferrioxid-core`, `ferrioxid-migrations`, `ferrioxid-auth`, `ferrioxid-mail` und `ferrioxid-cli` mit `-D warnings` war grün
- `cargo test --workspace` und `cargo build --workspace` sind für diesen Commit noch nicht erneut gelaufen

### Neue APIs

- `ferrioxid_core::aliased_env`
- `AuthConfig::try_from_env`
- `MailService::try_open`

### Verhalten

`AuthConfig::from_env` und `MailService::open` bleiben die bisherigen Aufrufe. Sie brechen ab, wenn ein Alias-Paar widersprüchlich ist. Vorher wurden `FERRUM_*` an diesen Stellen ignoriert.
