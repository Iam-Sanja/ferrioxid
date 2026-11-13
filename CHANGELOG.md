# Changelog

## Unreleased — Integration of pull requests #97 to #100

- Embedded auth and job migrations, trusted proxy peers, `inspect_framework`, `migrate plan`, JSON documents, one-to-one loads, grouped reports, the admin audit timeline, `aliased_env` and a single legacy project file are on one branch. See [the integration record](docs/advanced-framework-integration.md).
- `FERRIOXID_TRUST_PROXY` no longer trusts every peer. Set `FERRIOXID_TRUSTED_PROXIES` or the `FERRUM_` alias. Conflicting alias values are a startup error and are not printed.
- `#[field(json)]` is a new column kind. Existing text columns are not converted. `migrate plan` is read-only.
- The Cargo package version remains `0.1.0`.

## Unreleased — API contract, checks and release notes (v0.1.0)

- Generated list routes use keyset pages (`limit`, `cursor`, `sort`, repeated `filter`) instead of a silent 100-row window with `truncated`. The read JSON omits `write_only` fields and the secret-named columns. Invalid JSON and a non-UUID id use the problem document. OpenAPI 3.1 describes the same page, parameters and read/write schemas. `info.version` is the HTTP contract `1`. The crate version stays `0.1.0`.
- `/readyz` runs `SELECT 1` and does not migrate. `ferrioxid::serve` stops on Ctrl+C and SIGTERM. `FERRIOXID_LOG_FORMAT=json` selects JSON logs. `ferrioxid check --deploy` is read-only. An enabled proxy trusts only peers named by `FERRIOXID_TRUSTED_PROXIES`. A wildcard is ignored, and conflicting alias values are an error that is not printed.
- `deploy/` is a disposable Compose example. `tests/browser` opens the admin login with a fresh Playwright profile. The PostgreSQL job also runs the API contract test and a `pg_dump`/`pg_restore` roundtrip on `ferrioxid_test_*` databases.
- Nothing was published to crates.io. See [release readiness](docs/release-readiness.md) and [the framework comparison](docs/framework-comparison.md).

## Unreleased — Ferrioxid names only (breaking)

- Configuration discovery and generation use `ferrioxid.toml`; metadata and runtime CSRF keys use `.ferrioxid/`. Environment settings use `FERRIOXID_*`, and the default session cookie is `ferrioxid_session`. No old-name aliases are read or generated.
- Auth, jobs and all migration histories use `_ferrioxid_*` tables. The five built-in migration definitions were renamed and resealed together; migration IDs remain unchanged. Existing database histories are incompatible and require fresh disposable databases, not automatic conversion.
- Generator, CLI help, examples, tests and current documentation use the new names. Secret exclusions and the model markers introduced in PR #24 remain intact.
- See [the breaking-change record](docs/ferrum-legacy-removal.md). Milestones below are historical records: names and behavior describe those earlier implementations, not current setup instructions.

## Unreleased — Admin 2.0 (v0.28)

- Die Seitenleiste gruppiert Modelle und eigene Admin-Seiten unter der App, die sie registriert hat. Modelle ohne App stehen unter dem Projektnamen. Übersicht, Benutzer, Rollen und Historie sind keine Modelle.
- `/admin/models` zeigt die erlaubten Modelle und leitet nicht mehr auf die Übersicht um. Ein Benutzer sieht nur Einträge, die er lesen darf.
- Audit-Zeitpunkte werden als UTC-Datum mit `<time>` angezeigt. Gespeicherte Unix-Sekunden bleiben unverändert. Ein Eintrag kann über Modell und Datensatz gefiltert werden, wenn die vorhandene Historie diese Zuordnung speichert.
- Formulare bestehender Datensätze und die Sprachauswahl starten sauber. Das Script sperrt Speichern und Übernehmen, bis sich ein Feld ändert, und warnt beim Verlassen. Ohne Script bleiben die Aktionen benutzbar.
- Zeilenaktionen sitzen in einem aufklappbaren Menü. Rollen werden auf der Rollenseite angelegt. Physisches Löschen von Benutzern gibt es weiterhin nicht.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Anwendungs-Worker (v0.27.1)

- `WorkerHost` startet den vorhandenen Worker mit den Projekt-Einstellungen, den registrierten App-Handlern und auf Wunsch dem Mail-Handler. Der Prozess startet keinen Webserver und wendet keine Migration an.
- `ferrioxid generate worker` schreibt `src/bin/worker.rs` einmal. Eine vorhandene Datei bleibt stehen. `ferrioxid dev` startet diesen Prozess nicht.
- `FerrioxidApplication::register_jobs` hat eine leere Standardmethode. `App::install` ruft sie nicht auf. `examples/notes` registriert `notes_pulse` und `notes_retry` in `src/bin/worker.rs`.
- `ferrioxid jobs inspect` zeigt Status, Handler, Version, Versuche, Zeiten und Fehlercode. Die Payload bleibt ohne `--show-payload` verborgen. Das Lease-Token erscheint nicht. Eine ausdrücklich angeforderte Payload kann weiterhin personenbezogene Daten enthalten.
- Fehlercodes bleiben auf sichere Bezeichner beschränkt. Eine Meldung mit Geheimnis-Markern wird zurückgehalten. In der Job-Zeile steht nur der Code.
- SQLite bleibt bei einer Verbindung und Parallelität 1. Die Zustellung bleibt at least once. Eine Exactly-once-Garantie gibt es nicht.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Hintergrundjobs (v0.27)

- `ferrioxid-jobs` speichert Jobs in `_ferrum_jobs`. Die Historie heißt `_ferrum_job_migrations`. Auth-Migrationen bleiben unverändert. Der Worker migriert nicht selbst.
- Handler werden typisiert in einer `JobRegistry` registriert. Es gibt keine globale Registry. `ferrioxid worker` startet keinen Webserver und führt nur den Mail-Handler aus. `ferrioxid dev` startet keinen Worker.
- Claims nutzen Lease-Token. Ein abgelaufener Worker kann einen neu beanspruchten Job nicht abschließen. Wiederholungen sind auf höchstens 20 Versuche und exponentiellen Backoff begrenzt.
- Intervalle planen nach Erfolg genau eine nächste Fälligkeit in UTC. Cron gibt es nicht. Die Zustellung ist at least once.
- Passwort-Reset-Tokens bleiben außerhalb der Queue. Gewöhnliche Hinweise können als Job versendet werden. Eine SMTP-Annahme ist keine Zustellbestätigung.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Modellbausteine und App-Start (v0.26)

- `try_into_router` versiegelt installierte Apps und deklarierte `[app.<name>]`-Einträge vor dem Start. `into_router` bleibt ohne `Result` und startet bei einer ungültigen Registry nicht. Ohne Apps bleibt der bisherige Startweg.
- `registry.migrations` und `[app.<name>]` müssen beim Start dieselben Namen, Abhängigkeiten und Verzeichnisse nennen. Die CLI prüft nur die TOML-Seite und sagt das ausdrücklich. Diagnose bleibt lesend.
- `model_fields!` und `with_timestamps!` kopieren Felder in konkrete Models. Die Feldgruppe hat keine Tabelle. JSON und SQL bleiben flach.
- `#[field(managed = "created"|"updated")]` setzt `DateTime<Utc>` im Repository. Create- und Update-DTOs nehmen die Werte nicht an. `#[field(format = "slug"|"email")]` ergänzt die vorhandene Validierung.
- Soft Delete ist nicht umgesetzt. `ferrioxid generate model Name --timestamps` ist optional. Die Cargo-Version und die Template-Version bleiben `0.1.0` und `0.24`.

## Unreleased — Apps und App-Registry (v0.25)

- `FerrioxidApplication` und `App::install` registrieren Apps typisiert auf derselben `App`. Es gibt kein globales Singleton und keinen Abschnitt, der unbekannte Crates lädt. `[app.<name>]` nennt nur ein Migrationsverzeichnis und `depends_on`.
- Apps melden Modelle, `UrlSet`, Admin-Erweiterungen, Vorlagen und Staticfiles an die vorhandenen Register. Ein belegter Pfad oder eine belegte Tabelle wird abgelehnt. `merge` eines rohen Routers sieht Namen weiterhin nicht.
- `seal` prüft den Migrationsgraphen und die Admin-Registry und wendet keine Migration an. `into_router` bleibt ohne `Result`.
- App-Migrationen schreiben `shop:0001_initial` in `_ferrum_migrations`. Die Datei-ID und die Prüfsumme bleiben unqualifiziert. Alte Zeilen ohne `:` bleiben gültig und werden nicht neu zugeordnet.
- Tera erhält `url(name=..., ...)`. Das Ergebnis wird von Tera kodiert. Eine fehlende Route nennt nur den angeforderten Namen. `views::require_mutation` prüft POST über `AuthService::authenticate_csrf`.
- `ferrioxid generate app` legt `src/apps/<name>/` an und überschreibt nichts. Die Template-Version bleibt `0.24`. `examples/notes` ist eine externe App-Crate. myshop und seine Migrationen bleiben unverändert.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Settings, URLs und Views (v0.24)

- `Settings` liest `ferrum.toml`, prüft Defaults, Datei und Umgebung und bleibt ohne globales Singleton. `[custom.<name>]` wird über `custom_settings` in eine `Deserialize`-Struktur gelesen. Unbekannte andere Schlüssel bleiben Fehler.
- `FERRUM_ENV=production` zusammen mit aktivem Debug ist ein Fehler. `FERRUM_DEBUG` akzeptiert nur `true` und `false`. `BIND_ADDRESS` darf nicht mit `0.0.0.0` beginnen. `Debug` von `ProjectConfig` schwärzt Zugangsdaten in der Datenbank-URL.
- `UrlSet` registriert Axum-Routen unter `bereich.name`. `App::urls` hängt sie an denselben `AppState`. `/admin`, `/auth` und die bekannten Auth-Pfade bleiben reserviert. `RouteMap::path` löst Namen auf und lehnt unsichere Parameter ab.
- `ferrioxid::views` rendert Tera, JSON und lokale Redirects und bildet 400, 401, 403, 404, 409, 422 und 500 auf HTML oder das bestehende `problem_json` ab. `require_user`, `require_staff`, `require_permission` und `require_object_read` nutzen den vorhandenen Auth-Dienst.
- Neue Projekte erhalten `src/settings.rs` und `src/urls.rs`. Fullstack zusätzlich `src/views/home.rs` auf `/home`. Die Template-Version ist `0.24`. `ferrioxid upgrade` ersetzt keine Projektdatei. Eine ältere Version wird `project.template.outdated`.
- Bestehende `main.rs` mit `App::new`, `register` und `merge` bleiben gültig. `AppState` hat keine neuen Felder. Die Cargo-Version bleibt `0.1.0`. API-Vorlagen behalten die HTML-Startseiten, weil `templates.enabled` wahr bleibt.

## Unreleased — E-Mail und Passwort-Reset (v0.23)

- `ferrioxid-mail` stellt `disabled`, `console`, `memory` und `smtp` bereit. SMTP nutzt STARTTLS oder implizites TLS, prüft Zertifikate und liest das Passwort nur aus `FERRUM_SMTP_PASSWORD`. Ein Request versucht den Versand einmal.
- `0004_password_reset.json` speichert Reset-Hashes, Austausch-Hashes und eine Outbox ohne Roh-Token. `0001` bis `0003` bleiben unverändert.
- `/auth/password-reset` antwortet für bekannte, unbekannte und deaktivierte Adressen gleich. Der Abschluss hasht vor der Transaktion, verbraucht den Token bedingt, ändert das Passwort, widerruft die Sitzungen und schreibt das Audit gemeinsam.
- Hinweise sind `Notice` mit deutschen und englischen Text- und HTML-Vorlagen. `AccountDeactivated` ist aufrufbar und nicht an die Benutzerpflege gehängt.
- Neue Projekte setzen `[mail] backend = "disabled"`. `ferrioxid check` verbindet sich nicht mit SMTP. `ferrioxid upgrade` überschreibt keine `ferrum.toml`. Die Template-Version dieses Stands war `0.21` und ist seit v0.24 `0.24`.
- Die Cargo-Version bleibt `0.1.0`. Eine angenommene SMTP-Antwort ist keine Zustellgarantie. Der Workspace-Lauf mit Formatprüfung, Clippy, Tests und Build ist grün. Die ignorierten PostgreSQL-Laufzeittests wurden nicht ausgeführt. Der rustc-Absturz aus v0.21.1 bleibt ungeklärt.

## Unreleased — Erweiterbarer Admin (v0.22)

- `AdminRegistry` registriert eigene Seiten unter `/admin/pages/{key}`, eigene Sammelaktionen, Listenfilter, Formular-Widgets und Tera-Vorlagen. `seal` prüft die Kombination vor dem Start.
- Eine Seite verlangt Staff und einen Code `resource.action`. POST ohne CSRF bleibt 403. Reservierte Namen und Vorlagen mit Pfad oder `|safe` werden abgelehnt.
- Eine eigene Aktion schreibt nur deklarierte Spalten. Der Handler plant die Änderung. Ferrioxid führt sie in der bestehenden Sammeltransaktion über `update_in` aus und schreibt das Audit.
- Filter und Widgets ändern keine Rechte. Versteckte Felder bleiben versteckt. Fremdschlüssel behalten das eingebaute Auswahlfeld.
- myshop zeigt die Bestellübersicht, die Aktion `mark_processed`, den Statusfilter, ein Textfeld für den Kunden und `templates/admin/orders_report.html`.
- Die Cargo-Version bleibt `0.1.0`. Bestehende `AdminAction`-Literale für `Delete` und `SetBool` bleiben gültig. Ein externes `match` auf `ActionOperation` braucht den Zweig `Custom`.

## Unreleased — Windows-Teststabilität (v0.21.1)

- Der Generator-Test begrenzt verschachtelte Cargo-Builds auf höchstens zwei Jobs. Das gilt nur für die Kindprozesse dieses Tests und ändert die globale Umgebung nicht.
- rustc 1.98.1 ist bei unbegrenzten Release-Builds dieses Tests mit `0xc0000005` und `0xc0000409` abgestürzt. Die Ursache ist nicht aufgeklärt. Danach waren ein Workspace-Lauf mit `CARGO_BUILD_JOBS=2` und `--test-threads=2` sowie ein uneingeschränkter `cargo test --workspace --offline` erfolgreich. Der uneingeschränkte Lauf nutzt dieselbe Kindprozess-Grenze von höchstens zwei Jobs. Das erklärt den Compilerabsturz nicht.
- Diagnoseprüfungen für eine unlesbare SQLite-Datei, eine unbekannte Migration und ein nicht geladenes Schema behaupten keinen Erfolg. Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Projekt-Upgrade und Migrationsdiagnose (v0.21)

- Ein gemeinsamer Inspector liest Auth-Historie `_ferrum_auth_migrations` und App-Historie `_ferrum_migrations`. `check`, `doctor`, `migrate status`, `dev` und `upgrade --check` sowie `--dry-run` legen keine Tabelle und keine SQLite-Datei an.
- `ferrioxid migrate status` trennt Auth und App, nennt ausstehende Migrationen und bleibt dabei erfolgreich. Checksum-, Reihenfolge- und unbekannte IDs beenden ihn mit Exitcode 1.
- `ferrioxid check --strict` beendet Warnungen mit Exitcode 2. Ohne `--strict` bleiben Warnungen bei Exitcode 0. Fehler bleiben Exitcode 1.
- `ferrioxid dev` warnt vor dem Start und migriert nicht.
- `ferrioxid upgrade --yes` legt nur ein fehlendes Legacy-Manifest `.ferrum/project.json` an. Unbekannte, geänderte und fehlende Dateien werden nicht ersetzt. Neue Projekte erhalten ein Manifest mit SHA-256 und Template-Version `0.21`.
- `admin-ui` bleibt gültig. Die Cargo-Version bleibt `0.1.0`.
- Die ignorierten PostgreSQL-Laufzeittests wurden nicht ausgeführt.

## Unreleased — Transaktionen (v0.20)

- `ferrioxid_db::conclude` committet nur bei Erfolg. Auth-Methoden `*_on` und `ManagedModel::*_in` schreiben auf einer übergebenen Transaktion und öffnen keine zweite.
- Verzeichnisänderungen in HTML und JSON schreiben Auth-Zustand, Sitzungswiderruf und Audit in einer Transaktion. Der Argon2id-Hash entsteht vor der Schreibsperre. Passwörter, Hashes und Tokens stehen nicht im Audit. CLI-Auth bleibt ohne dieses Audit.
- `0003_audit_operation.json` ergänzt `operation_id`. `0001` und `0002` bleiben unverändert. Ältere Audit-Zeilen bleiben lesbar.
- `admin_inline` akzeptiert `|form` und weitere Optionen. Ein solches Formular speichert Elternobjekt und eine Has-Many-Ebene gemeinsam. `save=rows` ändert nur den Entwurf. Ein veralteter Fingerabdruck antwortet mit `409`.
- myshop erhält `orders` und `order_lines` in `0005_orders.json`. `0001` bis `0004` bleiben unverändert.
- Die Cargo-Version bleibt `0.1.0`. Der Workspace-Lauf mit Formatprüfung, Clippy, Tests und Build ist grün. Die ignorierten PostgreSQL-Laufzeittests wurden nicht ausgeführt.

## Unreleased — Datum und Uhrzeit (v0.19)

- `ColumnKind` kennt `Date`, `Time` und `DateTime`. Die Rust-Typen sind `chrono::NaiveDate`, `chrono::NaiveTime` und `chrono::DateTime<Utc>`.
- SQLite speichert kanonischen Text: `YYYY-MM-DD`, `HH:MM:SS` und RFC 3339 mit `Z`. PostgreSQL nutzt `DATE`, `TIME` und `TIMESTAMPTZ`. Die PostgreSQL-Laufzeittests bleiben ungetestet, wenn kein Server erreichbar ist.
- Eine Textspalte wird nicht stillschweigend zu einem Datum. `plan_change` blockiert diese Änderung.
- `0004_events.json` hängt die Tabelle `events` an. `0001` bis `0003` und `0002_admin_journal` bleiben unverändert.
- JSON verwendet dieselben kanonischen Zeichenketten. Andere Offsets als UTC sind `validation.datetime_offset`.
- HTML nutzt `date`, `time` und `datetime-local`. Das lokale Feld ist als UTC beschriftet und wird nicht in die Rechnerzeitzone umgerechnet.
- `admin_date_hierarchy` filtert Listen nach UTC-Jahr, Monat und Tag. Die Zählung respektiert Suche, Filter und Query-Scope und bleibt bei 100 Gruppen.
- Die Cargo-Version bleibt `0.1.0`.
- v0.19 ist abgeschlossen. Der Workspace-Lauf mit Formatprüfung, Clippy, Tests und Build ist grün. Die ignorierten PostgreSQL-Laufzeittests wurden nicht ausgeführt.

## Unreleased — Django-Admin-Parität II (v0.18)

- Formularwerte überleben die Fremdschlüssel-Auswahl in einem sitzungsgebundenen Entwurf. Die URL trägt nur die zufällige Draft-ID und den gewählten Schlüssel. Passwörter, Hashes, CSRF und unbekannte Felder werden nicht gespeichert. Lebensdauer 30 Minuten, höchstens 8 KiB, höchstens 20 Entwürfe je Sitzung.
- `admin_inline = "name=tabelle.spalte"` zeigt eine Has-Many-Liste am Elterndatensatz. Ein neuer Kinddatensatz entsteht in einem eigenen Formular. Der Parent-Fremdschlüssel kommt aus der URL und überschreibt den POST. Die Create-Policy bleibt.
- Listen können registrierte Sammelaktionen ausführen: Löschen, und bei einem sichtbaren Bool-Feld `active` Aktivieren und Deaktivieren. Die erste POST-Anfrage schreibt nur einen Entwurf. Die Bestätigung verlangt `confirm=1` und CSRF. Eine verweigerte Zeile oder ein Fremdschlüsselfehler rollt die ganze Auswahl zurück.
- Eigene Aktionen registriert `AdminRegistry::action` als `AdminAction` mit `ActionOperation::Delete` oder `SetBool`. Der HTTP-Name führt keinen Code und kein SQL aus.
- `_ferrum_admin_audit` zeichnet erfolgreiche Modelländerungen in derselben Transaktion auf. Verzeichnisereignisse folgen dem Auth-Schreibvorgang und sind nicht dieselbe Transaktion. Superuser sehen die Historie. Anderes Staff sieht nur eigene Ereignisse. Passwörter, Hashes, Tokens und als sensibel markierte Felder fehlen in der Zusammenfassung.
- Die Cargo-Version bleibt `0.1.0`. Datum und Uhrzeit folgen in v0.19.

## Unreleased — Django-Admin-Parität I (v0.17)

- Listen nutzen `list_display`, `list_link`, `admin_ordering`, `admin_per_page`, `filterable` und `searchable` aus den bestehenden Metadaten. Die HTML-Seite liegt zwischen 1 und 100. Es gibt kein unbegrenztes „alle zeigen“. Ein unbekanntes `sort` bleibt 422.
- `admin_fieldset` und `#[admin(fieldset)]` teilen Formulare in benannte Abschnitte. Optionale Abschnitte sind `details`. Ohne Deklaration bleibt eine unbenannte Gruppe.
- Modellformulare senden `save=detail`, `save=edit` oder `save=new`. Nur ein erfolgreiches Schreiben antwortet mit 303 auf eine serverseitig gebaute Admin-URL. Ein anderer Wert schreibt nicht.
- Nach einem Validierungsfehler bleiben E-Mail, Text, Auswahl und Häkchen. Passwortfelder haben keinen `value`. CSRF-Tokens werden nicht protokolliert.
- Fremdschlüssel über 100 Zeilen haben eine GET-Auswahlseite mit Suche, Seitengröße und Query-Scope. Eine UUID, die der Benutzer am registrierten Ziel nicht lesen darf, wird abgelehnt.
- Datum und Uhrzeit sind nicht ergänzt. `ColumnKind` bleibt `Uuid`, `Text`, `BigInt` und `Boolean`.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Benutzerverwaltung und Relationen (v0.16)

- Staff-Superuser legen Benutzer im Admin an und ändern E-Mail, Aktiv, Staff und Rollen. Das Passwort wird mit derselben Argon2id-Prüfung wie in der CLI gesetzt. Superuser bleibt der CLI vorbehalten. Es gibt kein physisches Löschen.
- Deaktivieren, Staff ändern, Rollen, Rechte, Anlegen, Profil und Passwort verlangen serverseitig `confirm=1`. Fehlende oder andere Werte ändern nichts. Die Prüfung ersetzt weder CSRF noch die Staff-Superuser-Prüfung. Geschützte Superuser und die eigene Sperre bleiben 403.
- Passwortänderung und Deaktivierung widerrufen die Sitzungen des Ziels. Staff-Entzug gilt bei der nächsten Anfrage, weil die Sitzung den Benutzer neu lädt.
- Fremdschlüssel in Liste und Detail zeigen ein konfiguriertes Textfeld. Die UUID bleibt als technische Referenz. Auflösung ist eine begrenzte Abfrage je Zieltabelle und Query-Scope. Filter für Bool, Fremdschlüssel und Zahl sind Auswahl- oder Zahlenfelder.
- Fehlerseiten lesen das Superuser-Flag aus der Sitzung. Generator-Kindprozesse bekommen eine eigene SQLite-Datei und übernehmen keine geerbte `DATABASE_URL`.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Admin-Gestaltung (v0.15)

- Die Admin-Fläche nutzt Abstands-, Typo- und Farbtokens. Formulare sind Feldgruppen mit einheitlicher Kontrollhöhe, höchstens 40rem breit. Tabellen trennen lesbare Felder von gekürzten UUIDs, Suche und Filter von der Hauptaktion.
- Die aktuelle Navigation ist unterstrichen und per `aria-current` benannt. Unter 800 Pixel bleibt sie eine offene Linkliste.
- Rollen werden aus vorhandenen Namen gewählt. Eine unbekannte Rolle legt der Zuweisungs-POST nicht an. Sicherheitskritische Verzeichnisaktionen verlangen im Browser eine Bestätigung. Die Prüfung von Sitzung, Staff-Superuser und CSRF ist unverändert.
- Helle Projektfarben, auf denen Weiß unter 4,5:1 liegt, färben die Tür nicht. Im Dunkeln bleibt die Tür bei der Nachtfläche. Gefahrentext im Dunkeln ist dunkel auf der hellen Gefahrenfläche.
- Ein boolesches Feld ohne gesendeten Wert übernimmt den Spaltenstandard. `default = true` ist im Neuanlageformular angehakt. Ein gespeicherter Wert hat Vorrang.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Admin P0 (v0.14)

- Angemeldete Benutzer ohne Staff erhalten eine HTML-403-Seite mit Abmeldung per POST. Die Abmeldung prüft weiter Sitzung und CSRF und verlangt kein `is_staff`. Ein fehlendes oder fremdes Token widerruft die Sitzung nicht.
- `is_staff`, `is_superuser` und `is_active` bleiben getrennte Flags. Die CLI erklärt die gesetzten Flags. `--superuser` ohne `--staff` setzt Staff nicht.
- Listen ohne weiteres `list_display` zeigen das erste sichtbare Textfeld, sonst ein anderes sichtbares Feld, und danach die ID. Explizite `list_display`-Felder haben Vorrang. Verborgene und sensible Felder bleiben unsichtbar.
- Werkzeugleiste, Formularabstand und der Rollenabschnitt haben mehr Abstand. Die Rollenauswahl bleibt ein Textfeld.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Admin-Login und lokale Origins (v0.13.1)

- `POST /admin/login` scheitert nicht mehr, nur weil die erlaubten Origins fest auf Port 3000 stehen. Die Loopback-Origins kommen aus der Bind-Adresse des Prozesses (`ferrum.toml` oder `BIND_ADDRESS`).
- `FERRUM_ALLOWED_ORIGINS` bleibt die explizite Liste und wird nicht erweitert. Der Host-Header der Anfrage fließt nicht in die Liste ein. `Origin: null` bleibt abgelehnt, außer `Sec-Fetch-Site` ist genau `same-origin`.
- Eine abgelehnte HTML-Anmeldung protokolliert nur einen Grundcode (`origin_rejected`, `form_token_missing`, `login_csrf_missing`, `login_csrf_mismatch`), nie Cookie oder Token.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Nativer Admin (v0.13)

- Die Oberfläche unter `/admin` ist serverseitiges HTML. Axum, Tera, CSS und ein optionales Script ersetzen React, Vite und Node.js. Die JSON-API unter `/admin/api` bleibt.
- Login nutzt das kurzlebige Cookie `ferrioxid_login_csrf`, solange noch keine Sitzung existiert. Schreibende Formulare verwenden das bestehende Sitzungs-HMAC im Feld `csrf_token`. `next` bleibt auf interne `/admin`-Pfade beschränkt.
- Listen, Formulare, Details, Löschen und das Benutzerverzeichnis laufen über GET und POST. Erfolg antwortet mit 303. Deutsch und Englisch kommen aus einem festen Katalog. Theme und Sprache liegen in HttpOnly-Cookies.
- `apps/admin-ui` ist entfernt. Das Feature `admin-ui` bleibt leer, damit bestehende Projektdateien weiter kompilieren. Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Formulare, Staticfiles, Uploads und Middleware (v0.12)

- Typisierte Formulare prüfen Pflicht, Länge, Zahl, Bool, UUID, E-Mail, URL und Kreuzfelder. HTML wird escaped. Unbekannte Felder werden abgelehnt. Ein Derive-Makro gibt es nicht.
- HTML-Formulare verwenden das bestehende HMAC-CSRF, sobald eine Sitzung vorhanden ist. Öffentliche Formulare ohne Cookie bleiben ohne zweiten Mechanismus. Das Kontaktbeispiel versendet keine E-Mail.
- Statische Dateien kommen nur aus dem konfigurierten Root, mit MIME, Länge, Cache und ETag. Admin-Assets bleiben unter `/admin`.
- Lokaler Speicher nutzt UUID-Dateinamen, Inhaltserkennung, Größenlimit und Owner-Isolation. Uploads sind nicht montiert, bis `routes` eingehängt wird. S3 und Virenscanner sind nicht enthalten. Metadaten liegen neben der Datei, ohne neue Migration.
- Request-ID, Access-Log, Sicherheitsheader und Body-Limit laufen über Axum und Tower. Eingehende Request-IDs werden ignoriert. Query, Cookies und Tokens werden nicht protokolliert.
- Neue Projekte erhalten `static/`, `media/` und die Abschnitte `[static]` und `[storage]`. Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Internationalisierung, Settings-Anbindung und Admin (v0.11)

- Der Admin übersetzt sichtbare Texte über Kataloge unter `apps/admin-ui/src/i18n/locales/{de,en}`. Ein Wechsel speichert `ferrioxid-locale` und setzt `lang` am Dokument, ohne die Seite neu zu laden.
- API-, Auth- und Admin-Fehler enthalten `code`, optional `field` und leere `params`. `detail` und die HTTP-Statuscodes bleiben. Falsche E-Mail und falsches Passwort teilen sich weiter `auth.invalid_credentials` und denselben Text. SQL und `password_hash` werden nicht ausgegeben.
- `GET /admin/api/config` ist öffentlich und enthält nur Titel, Kopfzeile, Anwendungsname, geprüfte Farbe, Sprachen und Währung. Die Farbe muss `#rgb` oder `#rrggbb` sein.
- Startseiten wählen die Sprache über `?lang=`, danach `Accept-Language`, dann `default_locale`, dann `fallback_locale`. Der Wert wird nicht als Dateipfad verwendet.
- `branding.currency` ist ein ISO-4217-Code. Die Oberfläche formatiert Zahlen, Datum und Währung mit `Intl`. Cent-Beträge an Feldern auf `_cents` bleiben ganze Zahlen.
- Das Benutzerverzeichnis zeigt Rollen, sortiert und blättert lokal und bestätigt Deaktivieren, Staff-Entzug, Rechteentzug und das Aufheben einer Rollenzuordnung.
- Die CLI übersetzt die zentralen Erfolgsmeldungen von `new`, `generate model` und `dev` über `FERRUM_LOCALE` oder `default_locale`. Befehlsnamen und die Ausgaben von `migrate` bleiben stabil.
- Die Cargo-Version bleibt `0.1.0`. Nicht enthalten bleiben OAuth, MFA, GraphQL, WebSockets, Mandanten, Plugins, Cloud-Deployments und `migrate down`.

## Unreleased — Settings, Vorlagen und Admin-Oberfläche (v0.10)

- `ferrum.toml` kennt Projekt, Server, Auth, Vorlagen und Branding. Unbekannte Schlüssel bleiben Fehler. `DATABASE_URL`, `FERRUM_SESSION_HOURS` und `FERRUM_IDLE_MINUTES` überschreiben die Datei.
- Neue Projekte rendern `/` mit Tera. Dazu gehören Layout, Startseite, 404 und 500. API-Projekte verlinken den Admin nicht.
- Die Admin-Oberfläche hat gemeinsame Buttons, Felder, Suche, Filter, Trefferzahl und ein durchsuchbares Benutzer- und Rollenverzeichnis. Deutsch und Englisch sind als Katalog vorbereitet. Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Entwicklerwerkzeuge und Create-Policies (v0.9)

- `ObjectPolicy::can_create` prüft Create-Eingaben vor dem INSERT. Der Standard ist Ablehnung. Superuser folgen der bisherigen Semantik und überspringen die Objektregel. REST und Admin verwenden dieselbe Regel. Die Antwort ist 403, `create policy rejected the record`. Fehlende Pflichtfelder bleiben 422.
- Das Cargo-Feature `admin-ui` bettet Admin-Assets ein. Ohne das Feature kompiliert eine API auch ohne Frontend-Build. Mit dem Feature fehlt `apps/admin-ui/dist/index.html` als Build-Fehler. `FERRUM_ADMIN_DIST` bleibt der Entwicklungs-Override.
- `ferrioxid new`, `dev`, `check`, `doctor`, `generate model` und `build` sind implementiert. Vorlagen sind `api` und `fullstack`. Solange Ferrioxid nicht veröffentlicht ist, verbindet `--ferrioxid-path` das neue Projekt mit diesem Repository.
- Die Cargo-Version bleibt `0.1.0`. v0.9 ist der Funktionsstand, keine Paketversion. Automatisches Setzen der Besitzer-ID aus der Sitzung ist geplant und nicht implementiert.
- Nicht enthalten: OAuth, MFA, GraphQL, WebSockets, Plugins, Mandanten, automatische Cloud-Deployments und `migrate down`.

## Unreleased — Stabilität, Sicherheit und Auslieferung (v0.8.1)

- `GET /auth/csrf` berechnet ein HMAC-Token aus Sitzungs-ID, Session-Hash und stabilem Schlüssel. Ein zweiter Tab ersetzt das Token nicht.
- Objektregeln liefern `QueryScope`. REST und Admin filtern Listen, Zähler, Details, PATCH und DELETE mit derselben SQL-Bedingung. Ohne ausdrücklichen Scope bleibt `None`.
- Das Admin-Frontend wird beim Rust-Build aus `apps/admin-ui/dist` eingebettet. `FERRUM_ADMIN_DIST` bleibt ein Entwicklungs-Override. Node.js läuft zur Laufzeit nicht.
- Admin-Listen enthalten `total`. Ungebundene Objektregeln bleiben 403. Fehlerantworten nutzen weiterhin Problem-Details und geben keine Datenbanktexte, Hashes oder Tokens aus.
- Nicht enthalten: OAuth, MFA, GraphQL, WebSockets, Plugins, Mandanten, JWT und ein neues Migrationsformat. `0001`, `0002` und `0003` bleiben unverändert. Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Automatischer Admin

- Neue Crate `ferrioxid-admin` und React-Anwendung `apps/admin-ui`. Modelle mit `admin = true` erscheinen nach expliziter `AdminRegistry`-Registrierung.
- Admin-Metadaten hängen am `FerrioxidModel`-Trait und ändern `ColumnSpec` nicht. Bestehende Migrationen bleiben gültig.
- HTTP unter `/admin/api`. Die Oberfläche wird aus `apps/admin-ui/dist` oder `FERRUM_ADMIN_DIST` ausgeliefert. Ohne Build antwortet `/admin` mit 503.
- `GET /auth/csrf` rotiert das sitzungsgebundene CSRF-Token, ohne die Sitzung zu ersetzen.
- Benutzer- und Rollenverzeichnis nur für Staff-Superuser. Superuser-Vergabe ist blockiert. Passwort-Hashes verlassen den Server nicht.
- Objektregeln blockieren Admin-Listen und Fremdschlüssel-Auswahlen, bis ein SQL-Scope existiert.
- myshop registriert Produkt, Kategorie und interne Notiz im Admin. `0001`, `0002` und `0003` sind unverändert.
- Nicht enthalten: OAuth, MFA, Plugin-System, Drag-and-Drop-Dashboard, GraphQL, WebSockets, Mandanten.

## Unreleased — Authentifizierung, Sitzungen und Berechtigungen

- Neue Crate `ferrioxid-auth` mit Argon2id, serverseitigen Sitzungen, HttpOnly-Cookie `ferrioxid_session`, CSRF-Token und prozesslokalem Login-Limit.
- Tabellen `_ferrum_users`, `_ferrum_sessions`, `_ferrum_roles`, `_ferrum_permissions`, `_ferrum_user_roles` und `_ferrum_role_permissions`. Historie `_ferrum_auth_migrations`, Datei `crates/ferrioxid-auth/migrations/0001_auth_schema.json`.
- HTTP: `POST /auth/login`, `POST /auth/logout`, `GET /auth/me`, `POST /auth/change-password`.
- Modelle: `permissions` und `object_policy`. Standard bleibt Verweigerung. `allow_anonymous = true` bleibt die öffentliche Demo-Freigabe.
- Objektregeln ohne SQL-Scope blockieren Listen. Ein abgelehnter Einzeldatensatz antwortet mit 404.
- CLI: `admin create-user`, `auth list-users` und Befehle für Rollen. `migrate up` wendet Auth-Migrationen zuerst an. Kein voreingestelltes Admin-Passwort.
- myshop: geschützte `InternalNote` und `0003_internal_notes.json`. Produkte und Kategorien bleiben anonyme Demo-Routen.
- `0001_initial.json` ist unverändert. Die Cargo-Version bleibt `0.1.0`.
- Nicht enthalten: OAuth, MFA, JWT für externe APIs, E-Mail-Reset. Der Admin folgt in v0.8.

## Unreleased — Constraints, Foreign Keys und typisierte Beziehungen

- `#[field(unique)]` und `#[ferrioxid(unique_together = "a,b")]` erzeugen einen benannten Unique Index. Mehrere `NULL`-Werte bleiben erlaubt.
- `#[field(index)]` und `#[ferrioxid(index_together = "a,b")]` erzeugen sekundäre Indizes. Ein Primärschlüssel oder Unique Constraint wird nicht doppelt indiziert.
- `#[field(foreign_key = "tabelle.spalte", on_delete = "restrict|cascade|set_null")]` erzeugt einen echten Foreign Key. `ON UPDATE` bleibt `NO ACTION`. `set_null` verlangt eine nullable Spalte.
- Belongs-To und Has-Many laden typisierte Modelle. Die Gegenrichtung wird explizit deklariert. Eager Loading fehlt.
- Snapshots der Formatversion 1 nehmen die neuen Objekte auf, lassen leere Listen weg und lesen alte Dateien weiter. `0001_initial.json` behält die Checksum `06393cd03c3d2afb2e4fbe111d0cf8430caf2ef0eaeda3c503d21edaf3f7a026`.
- Neue Operationen: `add_unique_constraint`, `drop_unique_constraint`, `add_index`, `drop_index`. `add_foreign_key` und `drop_foreign_key` werden erkannt und blockiert. Ein Foreign Key auf einer neuen Spalte hängt an `add_column`.
- Unique- und Foreign-Key-Verletzungen werden HTTP 422. SQLite-`RESTRICT` nutzt den erweiterten Code 1811 mit der festen Meldung `FOREIGN KEY constraint failed`.
- myshop erhält `Category`, ein optionales `Product.category_id` und `0002_categories_and_product_relations.json`.
- Die Cargo-Version bleibt `0.1.0`.

## Unreleased — Explizite Umbenennung, SQLite-Umbau und Baseline

- `RenameColumn` und `RenameTable` bleiben Daten erhalten und werden nicht aus einem Drop plus Add erraten.
- SQLite baut unterstützte Spaltenänderungen als transaktionale Kopie um. Geprüft werden `string` ↔ `i64`, Nullbarkeit und Bool-Defaults. `NULL` bei einer `NOT NULL`-Verschärfung blockiert ohne Ersatzwert.
- PostgreSQL führt dieselben unterstützten Änderungen per `ALTER COLUMN` aus. Der SQLite-Umbau läuft dort nicht.
- `ferrioxid migrate baseline` schreibt nach `--confirm` nur Historieneinträge, wenn das SQLite-Schema zum letzten Migrationsziel passt. Destruktive oder typändernde Ketten werden nicht markiert. PostgreSQL-Baseline bricht ab.
- `ferrioxid migrate reseal` erneuert die Checksum einer noch nicht angewendeten Datei.

## Unreleased — Versionierte Migrationen

- Neue Crate `ferrioxid-migrations` mit Snapshot, Diff, Checksum, Historie und Runner für SQLite und PostgreSQL.
- CLI: `ferrioxid migrate make`, `ferrioxid migrate up`, `ferrioxid migrate status`. Modelle kommen über `ferrum.toml` und `myshop --print-schema`, nicht über Quelltextanalyse.
- Sichere Operationen: neue Tabelle, nullable Spalte, boolesche NOT-NULL-Spalte mit Default. Löschungen nur mit `--allow-destructive`.
- Primärschlüsseländerungen und nicht geprüfte Typänderungen werden abgelehnt. Umbenennung und der begrenzte Umbau stehen im neueren Eintrag.
- `myshop --bootstrap` legt nichts mehr an und markiert vorhandene Tabellen nicht als migriert.
- Breaking Change: der bisherige Bootstrap-Start erzeugt kein Schema mehr. Neue Datenbanken brauchen `ferrioxid migrate up`.

## Unreleased — SQLite als lokale Standarddatenbank

- Ohne `DATABASE_URL` verwendet myshop `sqlite://db.sqlite3?mode=rwc` im Arbeitsverzeichnis.
- PostgreSQL bleibt über `DATABASE_URL` unterstützt. Docker ist optional.
- DDL und CRUD laufen über eine SeaQuery-Schicht in `crates/ferrioxid-db/src/sql.rs`. Handler bleiben dialektunabhängig.
- SQLite speichert UUID als Text und Bool als Integer `0`/`1`. Die JSON-API bleibt für beide Backends gleich.
- SQLite-Verbindungen nutzen eine Pool-Verbindung und `PRAGMA foreign_keys=ON`. `sqlite::memory:` bleibt dadurch eine Datenbank.
- `sqlite_http_roundtrip` läuft in `cargo test --workspace` und fasst die lokale Datei nicht an. `postgres_http_roundtrip` bleibt ignoriert.
- Breaking Change: `FerrioxidModel::schema_sql` erwartet `DatabaseKind` und liefert `String`. Das bisherige myshop-Default war eine PostgreSQL-URL.

## Unreleased — CRUD mit PATCH und DELETE

- Jedes registrierte Modell erhält `PATCH /{id}` und `DELETE /{id}` zusätzlich zu Liste, Einzelabruf und POST.
- Das Makro erzeugt `{Model}Update`. Jedes änderbare Feld ist ein `PatchField<T>` mit den Zuständen `Missing`, `Null` und `Value`.
- Fehlende PATCH-Schlüssel behalten den gespeicherten Wert. `{}` liefert den Datensatz unverändert mit HTTP 200.
- JSON-`null` auf einer `NOT NULL`-Spalte ergibt HTTP 422. JSON-`null` auf `Option<T>` schreibt SQL `NULL`.
- `#[field(readonly)]` und der Primärschlüssel sind nicht im Update-DTO. Unbekannte Felder lehnt Serde ab.
- `DELETE` ist an `WHERE "id" = $1` gebunden. HTTP 204 hat keinen Körper. Eine zweite Löschung derselben UUID ergibt HTTP 404.
- Nullable Spalten sind `Option<String>`, `Option<i64>`, `Option<bool>` oder `Option<Uuid>`. `id` bleibt Pflicht und nicht null.
- Breaking Change: `ModelRepository` verlangt jetzt die zugehörigen Typen `Update` sowie die Methoden `update` und `delete`.
- Der PostgreSQL-HTTP-Test deckt PATCH, DELETE, 403 und Nullable-Felder ab, bleibt `#[ignore]` und wurde hier nicht gegen eine Datenbank ausgeführt.

## Unreleased — API detail endpoint and documentation

- Added generated, typed `ModelRepository::get(UUID)` and `GET /{id}` routes.
- Missing records return HTTP 404 with the shared Ferrioxid error body.
- Detail routes remain denied (403) for models without `allow_anonymous = true`.
- Tightened the PostgreSQL integration-test precondition; no silent success without `DATABASE_URL`.
- Documented the actual required Serde derives on model structs.

## 0.1.0 — source prototype

- Cargo workspace, FerrioxidModel macro, SeaORM PostgreSQL repository, Axum GET/POST routes.
- Typed create DTOs and typed model reads; 100-row list cap with `truncated` indicator.
- Required and trimmed string-length validation and Boolean SQL `DEFAULT TRUE/FALSE`.
- Shared error response for Ferrioxid validation and permission errors.
- Optional PostgreSQL HTTP integration test, demo app, Docker Compose, and modular documentation.
- Compilation and runtime verification of subsequent changes remain outstanding in this environment.
