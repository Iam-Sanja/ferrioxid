# Translation review queue

The following German source lines contain technical identifiers that the offline translator could not protect reliably. They were left unchanged to avoid corrupting code, commands, or Markdown links. Translate and review them manually before merging PR #21.

- Protected token missing: Felder mit `sensitive`, verborgene Felder, Namen `password`, `password_hash`, `token`, `token_hash`,
- Protected token missing: [Zur Übersicht](admin.md) · Eigene Aktionen: [admin-custom-actions.md](admin-custom-actions.md)
- Protected token missing: Eine Seite heißt `/admin/pages/{key}`. `key` passt auf `[a-z][a-z0-9_]{0,39}`. Reserviert sind `admi
- Protected token missing: Farben sind Rollen, nicht Einzelflächen. `--ink`, `--muted`, `--paper`, `--card`, `--focus`, `--dang
- Protected token missing: Jede Seite erhält `locale`, `theme`, `title`, Branding, den Katalog `i18n`, `signed_in`, `superuser`
- Protected token missing: Eine Spalte aus Seitenleiste und Inhalt, Umbruch unter 800 Pixel, Tabellen mit horizontalem Scroll,
- Protected token missing: Physische Crates: `crates/ferrioxid/`, `crates/ferrioxid-core/`, `crates/ferrioxid-macros/`, `crates
- Protected token missing: v0.24 hängt benannte Anwendungsrouten über `App::urls` an denselben Router. `UrlSet` lehnt `/admin`,
- Protected token missing: Die Rust-Struct bleibt die Quelle. `#[field(...)]` und `#[ferrioxid(...)]` schreiben Metadaten in `C
- Protected token missing: `DatabaseKind` ist `Sqlite` oder `Postgres`. `ColumnSpec` beschreibt Name, `ColumnKind`, Nullbarkeit
- Protected token missing: `cargo test --workspace` führt `sqlite_http_roundtrip` gegen `sqlite::memory:` aus. Der Test verände
- Protected token missing: `FerrioxidForm::from_form` nimmt `FormValues` entgegen. `ContactForm` ist die mitgelieferte Referenz
- Protected token missing: `validation.required`, `validation.min_length`, `validation.max_length`, `validation.min_value`, `va
- Protected token missing: Lies in dieser Reihenfolge `Cargo.toml`, [architecture.md](architecture.md), [core.md](core.md), [ma
- Protected token missing: SQLite ist die lokale Standarddatenbank. PostgreSQL bleibt über `DATABASE_URL` unterstützt. Neue Spa
- Protected token missing: Umgebungsvariablen überschreiben die Datei: `FERRUM_MAIL_BACKEND`, `FERRUM_MAIL_FROM`, `FERRUM_MAIL_
- Protected token missing: Die Operationsvarianten sind extern getaggt mit `type` in `snake_case`: `create_table`, `drop_table`
- Protected token missing: Erlaubte Kettenglieder sind `create_table`, `add_column`, `add_unique_constraint`, `add_index`, `ren
- Protected token missing: Verglichen werden Tabellen, Spalten, SQL-Typen, Nullbarkeit, Primärschlüssel, Defaults, Unique-Index
- Protected token missing: `ferrioxid generate model Customer` schreibt `src/models/customer.rs` und ergänzt Zeilen vor den Mar
- Protected token missing: Die vorhandenen Bereiche bleiben: `project`, `server`, `database`, `admin`, `auth`, `templates`, `br
- Protected token missing: Entwicklung ist der Normalfall. Generierte Projekte setzen `debug = true` und binden `127.0.0.1`. Pr
- Protected token missing: `StaticOptions` und `mount`. Der Pfad ist `{url_prefix}/{*path}`. `serve` setzt `Content-Type`, `Con
- Protected token missing: **Jobs:** `ferrioxid-jobs` prüft Erfolg, Retry, dauerhaften Fehler, Verzögerung, ein Intervall, zwei
- Protected token missing: Eine Schreiboperation, die mehrere Zeilen ändern soll, benutzt eine `DatabaseTransaction`. `ferrioxi
- Protected token missing: Diese Pfade und jeder Pfad unter `/admin` oder `/auth` sind `url.reserved`: `/healthz`, `/auth/login
- Protected token missing: Enthält `Accept` den Text `text/html` und nicht `application/json`, ist die Antwort HTML. Sonst blei
- Protected token missing: `Accept-Language`, das mit `en` beginnt, wählt die englische Überschrift. Sonst Deutsch. Ein Status
