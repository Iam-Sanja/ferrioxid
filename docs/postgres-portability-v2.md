# PostgreSQL-Portabilität V2

Stand: 11.11.2026. Branch `fix/postgres-portability-v2`, ausgehend von `main` `26d28611625b57d2965f09b92a5ba8850a384110` (Squash von PR #72).

Dieser Bericht prüft die Framework-Feststellungen aus `FERRIOXID_PROBLEME_V2.md` gegen den Quellstand nach dem Merge und beschreibt die Korrekturen. Er behauptet keine vollständige PostgreSQL-Freigabe des TestShops.

## Bestandsaufnahme

| Punkt | Vor der Korrektur | Ort | Bereits vorhandene API | Kleine Lösung |
|---|---|---|---|---|
| V2-06 | Vorhanden | `constrain` in `crates/ferrioxid-db/src/sql.rs` | `contains_ignore_case` baut `LOWER(spalte) LIKE` mit `LikeExpr` und backendabhängigem Platzhalter | Dieselbe Ausdrucksform für die Admin-Suche |
| V2-07 | Vorhanden | `bind_for_column`, `seek_expr`, `cursor_value` in `crates/ferrioxid-db/src/query.rs` | `ferrioxid_core::parse_date`, `parse_time`, `parse_datetime_utc`; Modell-Inserts binden Chrono-Werte | Gemeinsamer Binder für Filter, Relationen und Keyset |
| V2-08 | Vorhanden | `bind_value`, `read_cell` in `crates/ferrioxid-migrations/src/fixtures.rs` | Dieselben Parser; `json_temporal` liest native Typen bereits | JSON bleibt ISO, die Datenbankgrenze bindet und liest typisiert |
| V2-09 | Öffentliche Schreib-API in `crates/ferrioxid-db/src/write.rs` | `Select` und `ModelQuery` | Einzelzeile über `ModelRepository`, `rows_affected` bei `delete_record` | Gefiltertes Update und Delete, atomare Ganzzahl, Sperre, `on_conflict_do_nothing` |
| V2-11 | Vorhanden | `TestDatabase::postgres` in `crates/ferrioxid-api/src/testing.rs` | `down.rs` legt bereits `ferrioxid_test_<uuid>` an | Expliziter Modus `postgres_isolated` |

V2-01 bis V2-05, V2-10 und V2-12 liegen im TestShop. Dieses Repository ändert sie nicht.

## Reproduzierte Fehler

### V2-06

`constrain` baute die Admin-Suche mit `Expr::cust_with_values` und einem fest eingebauten `?`. SeaORM lässt dieses Fragezeichen unter PostgreSQL stehen und übernimmt die Werte der Vorlage dabei nicht als gebundene Parameter. Eine Suche nach `Hemd` wurde dadurch zu `LOWER("name") LIKE LOWER(?) ESCAPE '\'` ohne Suchwert. Die Zählabfrage der Admin-Seite ist davon genauso betroffen wie die Seite selbst.

### V2-07

`bind_for_column` hat `DATE`, `TIME` und `DATETIME` wie Text behandelt und `Value::String` erzeugt. PostgreSQL vergleicht eine native `date`-, `time`- oder `timestamptz`-Spalte nicht mit einem Textparameter. `seek_expr` nutzt denselben Binder, daher betrifft der Fehler auch die zweite Keyset-Seite. `where_related` und `where_has` laufen über `predicate_expr` und damit über denselben Pfad.

### V2-08

Fixture-Import band Zeitwerte als `Value::String`, auch `NULL`. Fixture-Export las sie mit `try_get::<Option<String>>`. Unter PostgreSQL erwartet SQLx für `date`, `time` und `timestamptz` die nativen Chrono-Typen. Der vorhandene Test `postgres_fixture_roundtrip` deckt Decimal, Choice und UUID ab, aber keine Zeitspalten.

## Behobene Fehler

### V2-06

Die Admin-Suche verwendet `LOWER(spalte) LIKE <Muster> ESCAPE '\'`. Das Muster ist ein SeaQuery-`LikeExpr`, kein SQL-Fragment. SQLite erhält `?`, PostgreSQL `$1`, `$2` und so weiter. `%`, `_` und `\` im Suchbegriff werden vor dem Binden als Literale escaped. Spalten kommen nur aus der übergebenen Allowlist und müssen sichere Bezeichner des Modells sein. Ein leerer Suchbegriff fügt kein Prädikat hinzu. Mehrere Spalten werden mit `OR` verbunden, die Zeile bleibt eine Zeile. Scope, Zählung und Seitenfenster sind unverändert.

Die Groß-/Kleinschreibung wird in Rust gefaltet, bevor das Muster gebunden wird, genau wie bei `contains_ignore_case`. SQLite-`LOWER` faltet nur ASCII, daher wird das Muster mit `to_ascii_lowercase` vorbereitet: `München` entspricht dort nicht `MÜNCHEN`. PostgreSQL-`LOWER` folgt der Datenbank-Locale, daher wird das Muster mit `to_lowercase` vorbereitet. Die beiden Backends liefern für nicht-ASCII-Buchstaben keine identischen Treffer. Das ist dokumentiert und getestet, nicht still vereinheitlicht.

### V2-07

Der gemeinsame Binder prüft den logischen `ColumnKind` und lehnt ungültige Werte ab, bevor ein Statement entsteht.

- `DATE`: nur `YYYY-MM-DD`. `2026-02-31` ist `validation.date`.
- `TIME`: `HH:MM` oder `HH:MM:SS`. Bruchteile und `24:00:00` werden abgelehnt, nicht abgeschnitten. `14:30` und `14:30:00` sind dieselbe Uhrzeit.
- `DATETIME`: UTC. `Z` und `+00:00` sind derselbe Zeitpunkt. Jeder andere Offset ist `validation.datetime_offset` und wird nicht umgerechnet. Ein Sekundenbruchteil bleibt im Parameter erhalten. `2026-11-11T13:30:00Z` ist nicht gleich `2026-11-11T13:30:00.123456Z`.

PostgreSQL bindet `Value::ChronoDate`, `Value::ChronoTime` und `Value::ChronoDateTimeUtc`. SQLite-Spalten bleiben Text: Datum `YYYY-MM-DD`, Uhrzeit `HH:MM:SS`, Zeitstempel als RFC-3339-Text mit `+00:00`, weil der SQLite-Treiber `DateTime<Utc>` so schreibt. Filter, Relationsfilter und Keyset-Cursor verwenden diesen Binder. `NULL`-Vergleiche bleiben `IS NULL` und binden keinen Wert.

Die JSON-Form eines ganzen Zeitpunkts bleibt `2026-11-07T13:30:00Z`. Ein vorhandener Bruchteil wird mitgeschrieben, zum Beispiel `2026-11-07T13:30:00.123456Z`.

### V2-08

Fixture-JSON bleibt zeichenbasiert. Vor dem Schreiben werden Datum, Uhrzeit und Zeitpunkt mit denselben Parsern geprüft. PostgreSQL erhält native Parameter einschließlich typisiertem `NULL`. SQLite erhält die kanonische Textdarstellung. Der Export liest zuerst den nativen Chrono-Typ und danach, für SQLite-Text, die Zeichenkette. Der Dump normalisiert `14:30` zu `14:30:00` und `+00:00` zu `Z`. UUID, Decimal, Choice und Fremdschlüssel bleiben auf ihrem bisherigen Pfad.

### V2-11

`TestDatabase::postgres` verbindet sich weiterhin mit der übergebenen URL, wenn der Datenbankname `ferrioxid_test` enthält. Das ist keine Isolation. `cleanup` löscht dort nur zuvor mit `record_table` genannte Tabellen und führt kein `DROP DATABASE` aus.

`TestDatabase::postgres_isolated` und `ferrioxid_db::DisposablePostgres` legen eine Datenbank `ferrioxid_test_` plus 32 Hexzeichen an. `cleanup` bzw. `drop_database` löscht nur diesen Namen, nach `pg_terminate_backend` für genau diese Datenbank. Die Admin-URL muss `ferrioxid`, `ferrioxid_migrations_down_test`, `postgres`, `template1` oder einen Namen mit `ferrioxid_test` bezeichnen. Andere Namen werden abgelehnt, bevor eine Verbindung geöffnet wird. Fehlertexte schwärzen Zugangsdaten in URLs. Das Anlegen erfordert das Recht `CREATEDB` oder ein gleichwertiges Rollenrecht. PostgreSQL 13 und damit 16 und 18 akzeptieren `DROP DATABASE ... WITH (FORCE)`.

Ein fehlgeschlagenes Anlegen ist kein stiller Rückfall auf die Admin-Datenbank.

## Geänderte öffentliche APIs

- `TestDatabase::postgres_isolated(admin_url)` legt eine eigene Testdatenbank an.
- `ferrioxid_db::DisposablePostgres::create` und `drop_database` sind dieselbe Grundlage für Crate-Tests.
- `format_datetime` behält einen vorhandenen Sekundenbruchteil. Ganze Sekunden bleiben ohne Dezimalpunkt.
- Die Admin-Suche und die temporalen Filter ändern ihre Aufrufe nicht. PostgreSQL bindet andere Parametertypen als zuvor. Ungültige Datums- und Zeittexte, die früher als Text gebunden wurden, sind jetzt `Validation`.
- `Select::update_matching`, `Select::update_all`, `Select::delete_matching`, `Select::delete_all` und `Select::lock_existing`.
- `ModelQuery::insert`, `RowInsert::on_conflict_do_nothing`.
- `ColumnUpdate::set`, `set_null`, `increment`, `decrement`.
- `WriteResult.rows_affected` und `LockResult.rows`.

## Betroffene Rust-Dateien

- `crates/ferrioxid-core/src/temporal.rs`
- `crates/ferrioxid-db/src/sql.rs`
- `crates/ferrioxid-db/src/query.rs`
- `crates/ferrioxid-db/src/lib.rs`
- `crates/ferrioxid-db/src/testing.rs`
- `crates/ferrioxid-db/src/write.rs`
- `examples/desk`
- `crates/ferrioxid-migrations/src/fixtures.rs`
- `crates/ferrioxid-migrations/Cargo.toml`
- `crates/ferrioxid-api/src/testing.rs`
- `.github/workflows/ci.yml`
- `docs/database.md`, `docs/admin-backend.md`, `docs/date-time-types.md`, `docs/fixtures.md`, `docs/testing.md`, `docs/index.md`

## SQLite- und PostgreSQL-Unterschiede

| Thema | SQLite | PostgreSQL |
|---|---|---|
| Admin-Suche | `?`, ASCII-`LOWER` | `$n`, Locale-`LOWER` |
| Datum / Uhrzeit / Zeitpunkt | Text | `date`, `time`, `timestamptz` |
| Zeitpunkt-Parameter | Treibertext mit `+00:00` | `timestamptz` |
| Fixture-NULL für Zeitspalten | Text-NULL | typisiertes Chrono-NULL |
| Testisolation | eigene temporäre Datei | nur mit `postgres_isolated` eine eigene Datenbank |
| Zeilensperre | kein `FOR UPDATE`; Schreibtransaktion und eine Pool-Verbindung | `SELECT … FOR UPDATE` sperrt eine vorhandene Zeile |

Beide Backends lehnen Offsets außer UTC ab. Beide behandeln `%`, `_` und `\` in der Admin-Suche als Literale.

## V2-09: öffentliche Schreib-API

`ModelRepository` bleibt der Weg für eine Zeile: lesen, anlegen, ersetzen, löschen, auch in einer Transaktion und mit `QueryScope`. Zusätzlich stehen gefilterte Sammeländerungen in `crates/ferrioxid-db/src/write.rs`.

```rust
Product::query()
    .filter(col("id").eq(product))
    .filter(col("active").eq(true))
    .filter(col("stock").gte(quantity))
    .filter(col("price_cents").eq(price_cents))
    .update_matching()
    .decrement("stock", quantity)
    .execute(db)
    .await?;
```

`rows_affected == 0` heißt, die Bedingung hat keine Zeile geändert. Das ist kein zweites Lesen. `increment` und `decrement` rechnen in derselben Anweisung. Der Betrag muss größer als null sein, die Spalte eine Ganzzahl. Passt das Ergebnis nicht in `i64` oder in die am Modell deklarierte Grenze, bleibt die Zeile unverändert. Es gibt keine still überlaufende Arithmetik, keine Multiplikation und keine Division.

`update_matching` und `delete_matching` lehnen eine Abfrage ohne Filter, Relationsprädikat oder einschränkenden Scope ab. `update_all` und `delete_all` sind die ausdrückliche Form. `QueryScope::None` trifft weiterhin nichts. Das Seitenlimit von 100 begrenzt diese Schreibanweisungen nicht. Spaltennamen kommen aus dem Modell. Werte sind gebundene Parameter.

Diese Sammelwege prüfen Spalte und Typ. Sie führen die Feldregeln von `create`, `update` und `delete` nicht aus. Eine E-Mail- oder Längenregel eines Einzelupdates gilt hier nicht automatisch.

`lock_existing` verlangt eine offene `DatabaseTransaction`. PostgreSQL erzeugt `SELECT … FOR UPDATE` für die getroffenen Zeilen. SQLite kennt diese Anweisung nicht. Dort schreibt dieselbe Bedingung den Primärschlüssel auf sich selbst und nimmt damit die Schreibsperre, die SQLite anbietet. Null Zeilen legen nichts an und reservieren keine noch nicht vorhandene Zeile. Zwei gleichzeitige Erstanlagen brauchen einen echten Unique-Constraint. `on_conflict_do_nothing` ignoriert genau diesen Konflikt und überschreibt die vorhandene Zeile nicht. Ein Überschreiben bei Konflikt gibt es nicht.

PostgreSQL meldet `ON DELETE RESTRICT` als SQLSTATE 23001. Das Framework ordnet 23001 derselben Validierung zu wie eine Fremdschlüsselverletzung. SQLite verwendet für denselben Fall den festen Text `FOREIGN KEY constraint failed`.

`examples/desk` führt dieselben Aufrufe auf einer frischen SQLite-Datei und auf einer isolierten PostgreSQL-Datenbank aus. Die Geschäftslogik enthält keine selbst geschriebenen SQL-Anweisungen. Die Migration entsteht aus den Modelldefinitionen.

## Offene Shop-Aufgaben

Diese Punkte sind im TestShop zu ändern, nicht im Framework-Branch:

- V2-01: sieben direkte SQL-Stellen auf `Select`, `update_matching`, `delete_matching` und `insert` umstellen. Keine eigenen `?`-Texte.
- V2-02: Katalog von `active=1` und `instr` auf `col("active").eq(true)`, `any_of` / `contains_ignore_case`, `where_has` und `keyset` umstellen.
- V2-03: Erstanlage über einen Unique-Constraint oder `on_conflict_do_nothing`. Das Adresslimit sperrt die vorhandene Besitzerzeile mit `lock_existing` und zählt danach.
- V2-04: Checkout-Preis als Filter von `decrement` verwenden. `rows_affected == 0` ist der veraltete Preis.
- V2-05: Warenkorbanzahl über `Aggregate::sum` lesen.
- V2-10: Shoptests gegen isolierte PostgreSQL-Fixtures ausführen, nicht nur gegen `TestDatabase::sqlite`.
- V2-12: PostgreSQL-Backup und Datenübernahme getrennt von der SQLite-Sicherung dokumentieren. V2-08 ist die Fixture-Grundlage, kein Ersatz für `pg_dump`.

`migrate baseline` bleibt auf PostgreSQL nicht implementiert. Eine leere Datenbank nutzt `migrate up`.

## Tests

Die neuen PostgreSQL-Tests sind `#[ignore]`. Ein ignorierter Lauf zählt nicht als bestanden. Die CI-Jobdefinition für PostgreSQL 16 ruft sie zusätzlich auf:

- `postgres_admin_search_runtime`
- `postgres_temporal_filters_and_keyset`
- `fixtures::tests::postgres_temporal_fixture_roundtrip`
- `testing::tests::postgres_isolated_databases_do_not_share_rows`
- `write::tests::postgres_shop_cases`
- `desk` `postgres_same_operations`

Lokal ausgeführte SQLite- und Typprüfungen:

- Admin-Suche: normaler Begriff, Groß-/Kleinschreibung, `%`, `_`, `\`, leerer Begriff, mehrere Spalten, Scope, Count, zweite Seite, abgelehnte Spalte, SQLite-Laufzeit.
- Gerendertes PostgreSQL-SQL der Admin-Suche enthält `$1`/`$2`, kein `?`, und der Suchtext steht nicht im SQL.
- Datum, Uhrzeit, Zeitpunkt: `eq`, `gt`, `gte`, `lt`, `lte`, `IN`, `NULL`, ungültige Werte, Offset, Sekundenbruchteil, Relationsfilter, Keyset-Seite 1 bis 3, SQLite-Laufzeit.
- PostgreSQL-Parameter sind `ChronoDate` und `ChronoDateTimeUtc`. SQLite-Datum bleibt `YYYY-MM-DD`.
- Fixture: ungültiges Datum vor dem Schreiben, SQLite-Dump, erneutes Laden, kanonische Werte einschließlich Decimal, UUID und Fremdschlüssel.

Am 2026-11-11 ohne gesetzte `DATABASE_URL`:

- `cargo fmt --all -- --check` bestanden.
- `cargo clippy --workspace --all-targets -- -D warnings` bestanden. Die bestehende Meldung `linker_messages` in `ferrioxid-macros` bleibt eine Warnung und ist kein Fehler.
- `cargo test --workspace -- --test-threads=8` bestanden, 0 fehlgeschlagen, 16 ignoriert. Die neuen SQLite- und Typprüfungen in diesem Lauf sind bestanden.
- `cargo build --workspace` bestanden.

Nach der Schreib-API, wieder ohne gesetzte `DATABASE_URL`:

- `cargo fmt --all -- --check` bestanden.
- `cargo clippy --workspace --all-targets -- -D warnings` bestanden. `linker_messages` in `ferrioxid-macros` bleibt eine Warnung und ist kein Fehler.
- `cargo test --workspace -- --test-threads=8` bestanden, 0 fehlgeschlagen. `write::tests::sqlite_shop_cases` und `examples/desk` `sqlite_same_operations` sind in diesem Lauf bestanden. Die ignorierten PostgreSQL-Tests liefen nicht.
- `cargo build --workspace` bestanden.

Danach lief PostgreSQL 18.6 lokal über `pg_ctl`, nur auf `127.0.0.1:5432`. Es ist kein Windows-Dienst. Die Rolle `ferrioxid` hatte kein `CREATEDB`; das Attribut wurde für diese lokale Testrolle gesetzt, nicht `SUPERUSER`. Die bestehenden Datenbanken `ferrioxid` und `ferrioxid_migrations_down_test` wurden nicht als Schreibziel verwendet. Neue Laufzeittests legen `ferrioxid_test_<32 hex>` an und löschen nur diesen Namen. Die ältere CI-Reihe lief gegen drei leere Datenbanken derselben Form. Diese drei Datenbanken wurden nach dem Lauf gelöscht. Die Tabellenlisten der beiden bestehenden Datenbanken waren danach unverändert, und keine `ferrioxid_test_*`-Datenbank blieb übrig.

Auf PostgreSQL 18.6 bestanden:

- `postgres_admin_search_runtime`
- `postgres_temporal_filters_and_keyset`
- `fixtures::tests::postgres_temporal_fixture_roundtrip`
- `testing::tests::postgres_isolated_databases_do_not_share_rows`
- `write::tests::postgres_shop_cases`
- `desk` `postgres_same_operations`
- `postgres_http_roundtrip`
- `postgres_migration_roundtrip`
- `postgres_enforces_cross_app_foreign_keys`
- `postgres_anonymous_session_roundtrip`
- `postgres_decimal_and_choice_roundtrip`
- `fixtures::tests::postgres_fixture_roundtrip`
- `down::tests::postgres_reverts_backfill_and_foreign_key`
- `postgres_filters_and_pages_runtime`
- `postgres_transaction_query_and_rollback_runtime`
- `postgres_keyset_annotations_and_many_to_many`
- `postgres_text_search_relations_count_and_pages`
- `fresh_postgres_initializes_auth_jobs_and_application`

PostgreSQL 16 in GitHub-CI ist in der Workflow-Datei ergänzt, aber nicht gelaufen, weil dieser Branch nicht gepusht wird. Ein grüner Workspace-Lauf ohne `--ignored` führt diese Tests weiterhin nicht aus.

## Verbleibende Einschränkungen

- Die Admin-Suche faltet Großschreibung je Backend unterschiedlich. Auf diesem PostgreSQL-18.6-Cluster hat `MÜNCHEN` die Zeile `München` getroffen. Das bleibt von der Datenbank-Locale abhängig und ist keine Zusage für jeden PostgreSQL-16-CI-Lauf.
- `DisposablePostgres` löscht nur `ferrioxid_test_` plus 32 Hexzeichen. Ein Panic vor `drop_database` lässt diese Datenbank stehen. Der Pool wird vor dem Löschen geschlossen; `DROP DATABASE … WITH (FORCE)` hat auf PostgreSQL 18.6 für den Besitzer mit `CREATEDB` funktioniert.
- SQLite speichert Zeitpunkte als Text mit `+00:00`. Vergleiche laufen über den Treibertext, nicht über eine native Uhr.
- `TestDatabase::postgres` isoliert Tests weiterhin nicht.
- `postgres_isolated` braucht `CREATEDB`. Ohne dieses Recht legt der Test keine Ersatzdatenbank in einer fremden Datenbank an.
- Die Schreib-API überschreibt bei einem Unique-Konflikt nicht und bietet keine Multiplikation oder Division. Der TestShop selbst ist in diesem Branch nicht umgestellt.
- `postgres_same_operations` erwartet auf PostgreSQL, dass `LOWER` den Buchstaben `ü` faltet. Eine `C`-Locale tut das nicht. Das lokale PostgreSQL 18.6 und das CI-Image `postgres:16` sind UTF-8-Locales. SQLite faltet in diesem Test nur ASCII, deshalb findet `MÜNCHEN` dort `München` nicht.
- Ein grüner `cargo test --workspace` ohne `--ignored` führt die neuen PostgreSQL-Tests nicht aus.
