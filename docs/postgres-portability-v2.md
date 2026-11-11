# PostgreSQL-Portabilität V2

Stand: 11.11.2026. Branch `fix/postgres-portability-v2`, ausgehend von `main` `26d28611625b57d2965f09b92a5ba8850a384110` (Squash von PR #72).

Dieser Bericht prüft die Framework-Feststellungen aus `FERRIOXID_PROBLEME_V2.md` gegen den Quellstand nach dem Merge und beschreibt die Korrekturen. Er behauptet keine vollständige PostgreSQL-Freigabe des TestShops.

## Bestandsaufnahme

| Punkt | Vor der Korrektur | Ort | Bereits vorhandene API | Kleine Lösung |
|---|---|---|---|---|
| V2-06 | Vorhanden | `constrain` in `crates/ferrioxid-db/src/sql.rs` | `contains_ignore_case` baut `LOWER(spalte) LIKE` mit `LikeExpr` und backendabhängigem Platzhalter | Dieselbe Ausdrucksform für die Admin-Suche |
| V2-07 | Vorhanden | `bind_for_column`, `seek_expr`, `cursor_value` in `crates/ferrioxid-db/src/query.rs` | `ferrioxid_core::parse_date`, `parse_time`, `parse_datetime_utc`; Modell-Inserts binden Chrono-Werte | Gemeinsamer Binder für Filter, Relationen und Keyset |
| V2-08 | Vorhanden | `bind_value`, `read_cell` in `crates/ferrioxid-migrations/src/fixtures.rs` | Dieselben Parser; `json_temporal` liest native Typen bereits | JSON bleibt ISO, die Datenbankgrenze bindet und liest typisiert |
| V2-09 | Grenze, nicht als kleiner Patch umgesetzt | `ModelRepository` in `crates/ferrioxid-db/src/lib.rs` | `update_tx` / `delete_tx` pro Primärschlüssel, `delete_record` liefert `rows_affected`, SeaORM kann Ausdrücke und `LockType` | Separat planen, siehe unten |
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

Es gibt keine neue Abfrage-API neben `Select`.

## Betroffene Rust-Dateien

- `crates/ferrioxid-core/src/temporal.rs`
- `crates/ferrioxid-db/src/sql.rs`
- `crates/ferrioxid-db/src/query.rs`
- `crates/ferrioxid-db/src/lib.rs`
- `crates/ferrioxid-db/src/testing.rs`
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

## V2-09: nicht in diesem Branch umgesetzt

`ModelRepository` kann eine Zeile anhand ihrer UUID lesen, ersetzen und löschen, auch innerhalb einer Transaktion und mit `QueryScope`. `delete_record` gibt die betroffene Zeilenzahl zurück. `update_record` setzt konkrete Spaltenwerte. Das ist kein atomares `stock = stock - menge` und kein gefiltertes Löschen vieler Zeilen.

SeaORM kann darunter `UPDATE` mit Ausdrücken, `DELETE` mit Bedingung und `rows_affected` ausführen. `QuerySelect::lock(LockType::Update)` erzeugt auf PostgreSQL eine Zeilensperre. SQLite versteht `FOR UPDATE` nicht. Eine Ferrioxid-SQLite-Verbindung ist auf eine Connection begrenzt. Das serialisiert Schreibvorgänge in diesem Prozess, nicht zuverlässig über mehrere Prozesse hinweg, und es sperrt keine Zeile, die noch nicht existiert.

Deshalb reicht ein überall gleiches `FOR UPDATE` nicht. Zwei gleichzeitige Erstanlagen eines Warenkorbs finden keine Zeile zum Sperren. Dieselbe Grenze gilt für das Adresslimit, wenn keine vorhandene Besitzerzeile serialisiert wird.

Eine kleine API wäre später ungefähr:

```rust
// Nicht implementiert. Nur der vorgeschlagene Zuschnitt.
pub struct WriteResult { pub rows_affected: u64 }

// Bestand senken, nur wenn Bestand und Preis noch passen.
// rows_affected == 0 ist die fachliche Ablehnung, kein zweites SELECT.
update_matching(filter, assign_bigint("stock", Add(-qty)))

// Positionen eines bestätigten Besitzers löschen oder umhängen.
delete_matching(filter)
update_matching(filter, assign("cart_id", new_id))

// Vorhandene Besitzerzeile sperren.
// PostgreSQL: SELECT … FOR UPDATE.
// SQLite: Schreibtransaktion und eine schreibende Berührung dieser Zeile.
// Kein Treffer bleibt ein eigenes Ergebnis und begründet keine Erstanlage.
lock_existing(id)
```

Hooks, Validierung und Scopes müssen dabei ausdrücklich gelten oder ausdrücklich ausgenommen sein. Massenänderungen dürfen Modell-Hooks nicht still überspringen, ohne das zu sagen. Das ist mehr als ein lokaler Binder-Fix und bleibt eine eigene Aufgabe.

Bis dahin kann der Shop für diese fünf Fälle SeaQuery-Ausdrücke mit dem Verbindungs-Backend bauen und `rows_affected` auswerten. Rohe `?`-Zeichenketten bleiben auch danach falsch.

Konkrete Shop-Fälle, die diese API tragen soll:

1. Bestand um eine Menge senken, nur wenn der Bestand reicht.
2. Dieselbe Senkung nur zulassen, wenn der zuvor geprüfte Preis noch gilt. Positionssumme und Gesamtsumme müssen denselben Snapshot verwenden.
3. Warenkorbpositionen nur mit Besitzerprüfung entfernen.
4. Warenkorbpositionen atomar einem bestätigten Besitzer zuordnen.
5. Eine vorhandene Besitzerzeile in der Transaktion sperren. Das Sperren einer fehlenden Warenkorbzeile löst die gleichzeitige Erstanlage nicht.

## Offene Shop-Aufgaben

Diese Punkte sind im TestShop zu ändern, nicht im Framework-Branch:

- V2-01: sieben direkte SQL-Stellen mit SQLite-`?` auf vorhandene Ferrioxid-Abfragen oder backendgerechte SeaQuery-Ausdrücke umstellen.
- V2-02: Katalog von `active=1` und `instr` auf `col("active").eq(true)`, `any_of` / `contains_ignore_case`, `where_has` und `keyset` umstellen.
- V2-03: Erstanlage und Adresslimit über eine vorhandene Besitzerzeile oder eine eindeutige aktive Zuordnung serialisieren.
- V2-04: Checkout-Preis und Positionen aus einem Snapshot schreiben.
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

Lokal ausgeführte SQLite- und Typprüfungen:

- Admin-Suche: normaler Begriff, Groß-/Kleinschreibung, `%`, `_`, `\`, leerer Begriff, mehrere Spalten, Scope, Count, zweite Seite, abgelehnte Spalte, SQLite-Laufzeit.
- Gerendertes PostgreSQL-SQL der Admin-Suche enthält `$1`/`$2`, kein `?`, und der Suchtext steht nicht im SQL.
- Datum, Uhrzeit, Zeitpunkt: `eq`, `gt`, `gte`, `lt`, `lte`, `IN`, `NULL`, ungültige Werte, Offset, Sekundenbruchteil, Relationsfilter, Keyset-Seite 1 bis 3, SQLite-Laufzeit.
- PostgreSQL-Parameter sind `ChronoDate` und `ChronoDateTimeUtc`. SQLite-Datum bleibt `YYYY-MM-DD`.
- Fixture: ungültiges Datum vor dem Schreiben, SQLite-Dump, erneutes Laden, kanonische Werte einschließlich Decimal, UUID und Fremdschlüssel.

Am 2026-11-11 auf diesem Arbeitsstand, ohne gesetzte `DATABASE_URL`:

- `cargo fmt --all -- --check` bestanden.
- `cargo clippy --workspace --all-targets -- -D warnings` bestanden. Die bestehende Meldung `linker_messages` in `ferrioxid-macros` bleibt eine Warnung und ist kein Fehler.
- `cargo test --workspace -- --test-threads=8` bestanden, 0 fehlgeschlagen, 16 ignoriert. Die neuen SQLite- und Typprüfungen in diesem Lauf sind bestanden: Admin-Suche, temporale Filter und Keyset, Fixture-Roundtrip, Ablehnung einer nicht freigegebenen Admin-Datenbank.
- `cargo build --workspace` bestanden.

PostgreSQL 18.6 wurde nicht ausgeführt. `127.0.0.1:5432` war geschlossen, `DATABASE_URL` war nicht gesetzt, und es war kein PostgreSQL-Dienst sowie kein `psql` oder `postgres` im `PATH`. Die ignorierten PostgreSQL-Tests gelten dadurch nicht als bestanden. PostgreSQL 16 in GitHub-CI ist in der Workflow-Datei ergänzt, aber nicht gelaufen, weil dieser Branch nicht gepusht wird.

## Verbleibende Einschränkungen

- Die Admin-Suche und die Modellsuche falten Großschreibung unterschiedlich je Backend. Deutsche Produktnamen mit `Ü` brauchen einen ausdrücklichen Treffertest auf beiden Servern.
- SQLite speichert Zeitpunkte als Text mit `+00:00`. Vergleiche laufen über den Treibertext, nicht über eine native Uhr.
- `TestDatabase::postgres` isoliert Tests weiterhin nicht.
- `postgres_isolated` braucht `CREATEDB`. Ohne dieses Recht legt der Test keine Ersatzdatenbank in einer fremden Datenbank an.
- V2-09 ist nicht implementiert. Gleichzeitige Erstanlage, Preiswechsel im Checkout und große besitzgeprüfte Schreibvorgänge bleiben Shop-Arbeit.
- Ein grüner `cargo test --workspace` ohne `--ignored` führt die neuen PostgreSQL-Tests nicht aus.
