# Fortschritt des erweiterten Frameworks

Stand dieses Branches: `feat/native-admin`. Implementierung `e53f976d85ae0258b88554b0d42ac04dffcd5138`. Der Workspace-Nachweis steht in `b23b0df4e2e09014c6c6777f0ed78ceaef27ad78`.

Die anderen Pakete bleiben eigene Branches. Ihre Implementierungen sind:

- Block 1, `fix/runtime-security-docker`: `d556bcc3bbfa5b624c5d27ef13d2682ee06b5652`
- Block 2, `feat/orm-migrations-lifecycle`: `f3a663243b030eb3f391032c17a69314ff78abc3`
- Vorbereitung, `prep/native-admin`: `09172fdaf828e05a6988aff07c378ec97e98245d`

Basis ist `main` bei `53a95aa`. Dieser Branch enthält die Änderungen aus Block 2 nicht. `load_one_to_one`, JSON-Spalten, gruppierte Berichte und `migrate plan` bleiben dort.

Der Branch bleibt von den anderen drei Entwicklungsbranches getrennt. GitHub-CI ist noch nicht gelaufen. Die Issues bleiben offen, bis Anforderungen, Tests, GitHub-CI und Merge nach `main` zusammen erfüllt sind.

## Block 3

### Umgesetzt

- Datensatzseiten für Modelle, Benutzer und Rollen zeigen eine Audit-Timeline. Sie nutzt `audit::matching` mit Modellschlüssel und der gespeicherten Datensatz-ID. Es gibt keine zweite Audit-Tabelle.
- Die Timeline zeigt Aktion, Akteur, UTC-Zeitstempel und die bereits gespeicherte gekürzte Zusammenfassung. Höchstens acht Einträge, neueste zuerst, plus Link zur gefilterten Historie.
- Personen ohne Superuser-Recht sehen nur Ereignisse mit der eigenen `actor_id`.
- PostgreSQL speichert Audit-UUIDs als `uuid`. Schreiben und der Akteur-Filter binden diese Spalten jetzt als UUID. SQLite bleibt bei Text.

### Issue #32

Erfüllt auf diesem Branch: Zuordnung über die Datensatz-ID, getrennte Ereignisse bei ähnlichen Namen, Sichtbarkeit nach Akteur, gekürzte Zusammenfassung ohne neue Geheimnisausgabe, responsive Stapelung unter 800 px, SQLite- und PostgreSQL-Test der Abfrage, HTTP-Test der HTML-Seite.

Offen: keine manuelle Browserabnahme. Die Darstellung wurde über HTTP geprüft, nicht in einem Browser. Verzeichnisereignisse für Benutzer bleiben an der Benutzer-UUID, Rollenereignisse am Rollennamen. Direkte Repository-Schreibzugriffe und die Auth-CLI schreiben weiterhin kein Audit.

### Issue #79

Die typisierte Konfiguration ist das bestehende `AdminModelMeta` aus `#[ferrioxid(admin = true)]`. Eine zweite Admin-Konfiguration wurde nicht angelegt. `docs/admin.md` beschreibt diese Zuordnung.

Offen: keine zusätzliche Berechtigung je Aktion über `{permission}.read|create|update|delete` hinaus. Keine neue Startup-Prüfung für app-übergreifende Relationen. Die bestehenden Compile-Time-Prüfungen für Fieldsets und geheime Felder bleiben.

### Issue #80

Auswahllisten, Suche, Seitenzählung bis 100 je Seite, Query-Scope und Sammelaktionen sind auf `main` vorhanden und wurden nicht dupliziert. Die bestehenden HTTP-Tests decken Suche, Seite 5 und fremde Geheimnisse ab.

Offen: `load_one_to_one` liegt nur auf Block 2. Dieser Branch lädt One-to-One-Beziehungen nicht gesammelt nach. Ein eigener Test mit 0, 1, 100 und 1000 Inline-Zeilen wurde nicht ergänzt. Die vorhandene Inline-Liste bleibt auf höchstens 25 Zeilen je Seite begrenzt.

### Issue #84

Modellformulare, Fieldsets, Formsets, Upload und der Schutz vor mitgesendeten Readonly-Feldern sind auf `main` vorhanden. Der HTML-Test lehnt eine mitgesendete `id` weiterhin ab.

Offen: keine neue Prüfung, die HTML- und JSON-Validierungstexte Feld für Feld gegeneinander stellt. Kein zusätzlicher Upload-Lifecycle über `store_upload` hinaus.

### Tests auf diesem Branch

- SQLite: `matching_uses_the_record_id_not_a_similar_name`, `admin_v018_drafts_inlines_bulk_and_audit`
- PostgreSQL: `postgres_matching_uses_the_record_id_not_a_similar_name` auf einer Datenbank `ferrioxid_test_<32 hex>`, danach gelöscht
- Danach: `ferrioxid` 15 öffentliche Tabellen, `ferrioxid_migrations_down_test` 1 öffentliche Tabelle, keine übrige `ferrioxid_test_*`-Datenbank
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` und `cargo build --workspace` waren auf diesem Branch grün. Die bestehende `linker_messages`-Warnung von `ferrioxid-macros` bleibt.

### Browser

Keine Browserabnahme. CRUD bleibt serverseitig. Der neue HTTP-Test prüft die Timeline, die Datensatz-ID und die Akteur-Isolation.
