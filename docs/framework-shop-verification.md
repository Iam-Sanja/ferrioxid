# Shop-Abnahme der Framework-Änderungen

Diese Liste beschreibt, was im Ferrioxid-Repository geändert wurde und was anschließend im bestehenden TestShop geprüft werden kann. Der TestShop wurde dabei nicht verändert.

PostgreSQL-Laufzeittests waren in dieser Runde nicht gestartet. Ein `#[ignore]`-Test gilt nicht als bestanden.

## F01 – UUID-Bindung

1. Befund: Ein als Text gebundener UUID-Parameter findet einen binär gespeicherten UUID-Wert nicht.
2. Art: Anwendungsfehler. Die Speicherung wurde nicht geändert. `render_schema` bleibt SQLite `TEXT` und PostgreSQL `UUID`.
3. Geänderte Dateien: `crates/ferrioxid-db/src/query.rs`, `docs/database.md`.
4. Neue API: keine. Weiterhin `sea_orm::Value::Uuid` und `Statement::from_sql_and_values`.
5. Shop-Änderung nötig: nur dort, wo Raw-SQL eine UUID als Text gegen eine BLOB-Spalte bindet. Den Query Builder bevorzugen.
6. Prüfung: Eine Suche über den Query Builder findet die Produkt-ID. Dieselbe ID als Textparameter gegen eine BLOB-Spalte findet nichts.
7. Erwartet: typisierter Parameter trifft, Textparameter trifft die BLOB-Spalte nicht.
8. Test: `sqlite_uuid_text_does_not_match_a_blob_parameter` im Crate `ferrioxid-db`.

## F02 – Textsuche und Relationsfilter

1. Befund: Der Query Builder hatte keine ergonomische Textsuche und keine typisierten Beziehungsfilter.
2. Art: neue Funktion.
3. Geänderte Dateien: `crates/ferrioxid-db/src/query.rs`, `crates/ferrioxid-db/src/sql.rs`, `docs/database.md`.
4. API: `contains`, `starts_with`, `ends_with`, jeweils mit `*_ignore_case`. `where_related`, `where_has`, `where_linked`. `Related::negated`.
5. Sonderzeichen `%` und `_` werden mit Escape `\` gebunden. `100%` trifft nicht beliebige Texte. SQLite `LOWER` faltet nur ASCII; `München` entspricht dort nicht `MÜNCHEN`.
6. Shop-Änderung nötig: ja, wenn die Produktsuche diese Methoden nutzen soll.
7. Manuell prüfen: normaler Suchbegriff, `%` und `_`, Kategorie, Tag, Suche plus Pagination, leeres Ergebnis.
8. Erwartet: gleiche Filter für Zähler, Seite und Cursor. Keine verdoppelten Zeilen durch die Beziehung.
9. Tests: `sqlite_text_search_relations_count_and_pages` bestanden. `postgres_text_search_relations_count_and_pages` ignoriert, nicht ausgeführt.

## F03 – Gemeinsame Templates

1. Befund: Makros einer anderen App ließen sich nicht ausdrücklich importieren.
2. Art: neue Funktion.
3. Geänderte Dateien: `crates/ferrioxid-api/src/apps.rs`, `docs/templates.md`, `docs/apps.md`.
4. API: `Registration::export_templates`, `Registration::import_templates`, `App::render_template`.
5. Import vor `templates()`. Quelle muss in `depends_on` stehen und exportiert haben. Namen bleiben unter `{quelle}/`. `admin/`, `base.html` und `login.html` werden abgelehnt.
6. Shop-Änderung nötig: ja, wenn gemeinsame Formulare über Apps geteilt werden sollen.
7. Prüfen: App A exportiert ein Makro, App B importiert es, App C ohne Abhängigkeit scheitert, zwei lokale `page.html` bleiben getrennt, ein nicht exportiertes Makro scheitert beim Katalogaufbau mit dem fehlenden Namen.
8. Test: `exported_macros_cross_app_boundaries_explicitly` bestanden.

## F04 – TestClient

1. Befund: Sicherheitsheader ließen sich nicht öffentlich genug prüfen.
2. Art: neue Funktion, bisherige Methoden bleiben.
3. Geänderte Dateien: `crates/ferrioxid-api/src/testing.rs`, `docs/testing.md`.
4. API: `TestClient::request`, `TestRequest::header`, `bytes`, `send`. `TestResponse::headers`, `header`, `header_values`, `set_cookies`.
5. Shop-Änderung nötig: nur in Shop-Tests, die Header prüfen wollen.
6. Prüfen: eigener Origin, fremder Origin, `Location`, mehrere `Set-Cookie`, `HttpOnly`, `Secure`, `SameSite`, `Path`, `Max-Age`, angemeldete Sitzung, anonymer CSRF, fehlendes und falsches Token, ungültiges Benutzer-Cookie wird nicht zur anonymen Sitzung.
7. Test: `request_builder_exposes_headers_and_set_cookie_attributes` und `browser_login_flow_exposes_cookies_and_rejects_bad_sessions` bestanden.

## F05 – QueryCounter

1. Befund: Direkte SeaORM-Aufrufe wurden nicht gezählt und konnten als vollständige Query-Zahl missverstanden werden.
2. Art: Diagnose, keine Scheinlösung.
3. Geänderte Dateien: `crates/ferrioxid-db/src/query.rs`, `docs/database.md`, `docs/testing.md`.
4. API: `QueryCounter::record`.
5. Shop-Änderung nötig: nein. Wer direkte SQL-Aufrufe mitzählen will, ruft `record` selbst auf.
6. Test: `query_counter_does_not_see_direct_statements_or_other_threads` bestanden. ORM-Query zählt, direktes SQL und eine fehlgeschlagene Direktabfrage zählen nicht, ein anderer Thread bleibt getrennt.

## F06 – Bildprüfung

1. Befund: Eine PNG- oder JPEG-Signatur sagt nicht, dass die Datei dekodierbar und in der Größe begrenzt ist.
2. Art: optionale Funktion. Keine Pflichtbibliothek für volle Dekodierung.
3. Geänderte Dateien: `crates/ferrioxid-storage/src/local.rs`, `docs/storage.md`.
4. API: `inspect_image`, `ImageLimits`, `ImageHeaderCheck`, `LocalStorage::with_check`, `BeforeSave`. Fehler `StorageError::InvalidImage`.
5. Die Prüfung liest PNG-Chunks oder JPEG-Marker und die deklarierte Breite und Höhe. Sie entpackt keine PNG-Pixel. Normale Uploads ohne Hook bleiben unverändert.
6. Shop-Änderung nötig: ja, wenn Produktbilder vor dem Speichern geprüft werden sollen. Den Storage mit `with_check` bauen.
7. Prüfen: gültiges PNG und JPEG, abgeschnittene Datei, zu große Pixelzahl, PDF oder anderer Typ, Datei wird bei Ablehnung nicht angelegt.
8. Tests: `image_header_check_rejects_truncated_and_oversized_images` und `before_save_hook_rejects_without_writing` bestanden.

## F07 – Admin-Multipart

1. Befund: Eigene Admin-Seiten sahen nur URL-kodierte Felder.
2. Art: neue Funktion.
3. Geänderte Dateien: `crates/ferrioxid-admin/src/pages.rs`, `crates/ferrioxid-admin/src/views.rs`, `crates/ferrioxid-admin/src/html.rs`, `crates/ferrioxid-storage/src/http.rs`, `docs/admin-custom-views.md`.
4. API: `ViewRequest.uploads`, `UploadedFile`, `read_multipart`, `multipart_text_field`. Grenze `MULTIPART_BODY_LIMIT` = 1 MiB.
5. CSRF, Staff und die Seitenberechtigung bleiben Pflicht. Doppelte Feldnamen und unsichere Dateinamen werden abgelehnt. Der Browser-MIME-Typ wird nur mitgeliefert, nicht als Beweis verwendet. Das Framework speichert die Datei nicht selbst.
6. Shop-Änderung nötig: ja, wenn der Produktbild-Upload den Hex-Transport verlassen soll. Die Admin-Seite liest `request.uploads` und speichert danach selbst, inklusive Bildprüfung.
7. Prüfen: PNG, JPEG, fehlendes CSRF, falsches CSRF, nicht angemeldet, Benutzer ohne Recht, zu große Datei, Dateiname mit `../`, mehrere Felder, bestehendes URL-kodiertes Formular.
8. Test: `custom_page_accepts_multipart_and_keeps_urlencoded_forms` bestanden. Eine fehlende Berechtigung ruft den Handler nicht auf.

## F08 – Abgelaufene Sitzung

1. Befund: Ein ungültiges Benutzer-Cookie darf bei einem geschützten POST nicht still zu einer anonymen Sitzung werden.
2. Art: bewusst unverändertes Sicherheitsverhalten, plus Beispiel.
3. Datei: `crates/ferrioxid-api/src/testing.rs`, `docs/sessions.md`, `docs/security.md`.
4. Neue API: keine zusätzliche Hilfsfunktion. Der vorhandene Ablauf ist `ensure_anonymous_session` nach dem Löschen des Benutzer-Cookies und danach `authenticate_browser_form`.
5. Shop-Änderung nötig: nein, außer der Login soll das Cookie sichtbar löschen, bevor das anonyme Formular erscheint.
6. Test: im Browser-Flow wird ein erfundenes `ferrioxid_session` zusammen mit gültigem anonymem CSRF mit 401 abgelehnt.

## F09 – Strikter Migrationsstart

1. Befund: Ein gebauter Router bedeutet nicht, dass die Migrationen angewendet sind.
2. Art: optionale Funktion. Der bisherige Start bleibt ohne das Flag kompatibel.
3. Geänderte Dateien: `crates/ferrioxid-api/src/lib.rs`, `docs/migrations.md`.
4. API: `App::strict_migrations`, `App::try_into_router_strict`.
5. `try_into_router` nach dem Flag liefert `migration.strict` und schreibt nichts. Die async-Prüfung nennt App und Migrationskennung. Kein automatisches Migrieren oder Rollback.
6. Shop-Änderung nötig: ja, wenn der Shop vor dem ersten Request abbrechen soll. `strict_migrations()` setzen und `try_into_router_strict().await` verwenden.
7. Prüfen: vollständig migrierte Datenbank startet. Fehlende Migration wird vor dem ersten Request abgelehnt. Die Historientabelle verändert sich durch die Prüfung nicht.
8. Test: `strict_startup_reads_migration_status_without_writing` bestanden.

## F10 – App-übergreifende Migrationen

1. Art: vorhandene Funktion, Dokumentation.
2. Dateien: `docs/apps.md`, `docs/app-migrations.md`.
3. Weiterhin `depends_on`, `external_tables`, `resolve_installed_schemas`, `SchemaSnapshot`. Dieselbe SQL-Tabelle nicht in zwei Apps registrieren.
4. Shop-Änderung nötig: nein.

## F11 – Migrationskollision

1. Befund: Dieselbe Nummer in zwei Dateien ist ein Koordinationsproblem und muss abgelehnt bleiben.
2. Art: bessere Diagnose. Kein Löschen und kein Zusammenführen.
3. Datei: `crates/ferrioxid-migrations/src/store.rs`.
4. Die Meldung nennt beide Pfade, beide Kennungen, die Operationen und den Hinweis, die noch nicht angewendete Datei umzubenennen.
5. Shop-Änderung nötig: nein. Kollidierende Dateien von Hand neu nummerieren.
6. Test: `duplicate_numbers_name_both_paths_and_keep_the_files` bestanden.

## F12 – Windows-Testdateien

1. Art: Umgebungsverhalten, kein Engine-Eingriff.
2. Datei: `docs/migrations.md`.
3. Eine laufende EXE oder ein offener Pool sperrt die SQLite-Datei und die WAL-Datei. Pool schließen, temporäres Verzeichnis je Test verwenden, erst danach kopieren oder löschen.
4. Shop-Änderung nötig: nein.

## F13 – Fehlerkette

1. Befund: Logs nur mit `Display` verlieren die Ursache. Die HTTP-Antwort bleibt absichtlich allgemein.
2. Art: Diagnose.
3. Datei: `crates/ferrioxid-core/src/lib.rs`, `docs/security.md`.
4. API: `ferrioxid_core::diagnostic_chain`.
5. Die öffentliche Meldung bleibt `database operation failed`. Die Kette ist für das Serverlog. `password`, `secret`, `token`, `csrf_token` und die beiden Session-Cookies werden ersetzt.
6. Shop-Änderung nötig: ja, wenn der Shop bisher nur `Display` protokolliert. `diagnostic_chain(&error)` ins Serverlog schreiben, nicht in die HTTP-Antwort.
7. Test: `database_display_hides_the_source_and_the_chain_redacts_secrets` bestanden.

## PostgreSQL

Nicht ausgeführt in dieser Runde: `postgres_decimal_and_choice_roundtrip`, `postgres_anonymous_session_roundtrip`, `postgres_filters_and_pages_runtime`, `postgres_keyset_annotations_and_many_to_many`, `postgres_transaction_query_and_rollback_runtime`, `postgres_reverts_backfill_and_foreign_key`, `postgres_fixture_roundtrip`, `postgres_enforces_cross_app_foreign_keys`, `postgres_migration_roundtrip`, `fresh_postgres_initializes_auth_jobs_and_application`, `postgres_http_roundtrip`, `postgres_text_search_relations_count_and_pages`.

Diese Tests bleiben ignoriert, bis eine isolierte Datenbank mit `ferrioxid_test` im Namen und `DATABASE_URL` verfügbar ist.
