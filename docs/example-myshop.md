# myshop – Verbindliche Beispielanwendung

[Zur Übersicht](index.md) · **IST-Dateien:** `examples/myshop/src/main.rs`, `examples/myshop/src/models/product.rs`.

## Zweck

`myshop` prüft, ob die öffentlichen Ferrum-Komponenten gemeinsam funktionieren. Das Beispiel nutzt dieselbe Fassade wie andere Anwendungen und enthält keine privaten Datenbankzugriffe.

## Modell (IST)

`Product` hat `id: Uuid`, `name: String`, `price_cents: i64`, `active: bool`. Die Attribute setzen eine automatische UUID, einen Pflichtnamen mit 3–150 Zeichen, einen nichtnegativen Preis und den Bool-Default `true`. Die Demo-API `/api/v1/products` ist ausdrücklich anonym. Das gilt nur für den lokalen Prozess.

## Start mit SQLite (IST)

```powershell
cargo run -p ferrum-cli -- migrate up
cargo run -p myshop
```

Die Befehle gehören ins Workspace-Root, weil dort `ferrum.toml` liegt. Ohne `DATABASE_URL` verwenden CLI und myshop `sqlite://db.sqlite3?mode=rwc` im Arbeitsverzeichnis. `examples/myshop/migrations/0001_initial.json` legt `products` an. Der Server startet auf `127.0.0.1:3000`, sofern `BIND_ADDRESS` nicht gesetzt ist, und migriert nicht selbst.

`cargo run -p myshop -- --bootstrap` bricht ab. Er löscht keine Datei, legt keine Tabelle an und schreibt keine Historie. `cargo run -p myshop -- --print-schema` gibt den Snapshot von `Product` aus. Das ist die Registrierung, die `schema_command` aufruft.

PostgreSQL ist optional:

```powershell
$env:DATABASE_URL="postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum"
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

`.env.example` wird nicht geladen. Docker ist für den SQLite-Ablauf nicht erforderlich.

### API-Probe

```powershell
curl.exe -i -X POST http://127.0.0.1:3000/api/v1/products -H "Content-Type: application/json" -d "{\"name\":\"Laptop\",\"price_cents\":1250}"
curl.exe -i http://127.0.0.1:3000/api/v1/products
```

POST antwortet 201 mit UUID und `active: true`. GET antwortet 200 mit `data`, `limit: 100` und `truncated: false`, solange höchstens 100 Zeilen existieren. `PATCH /api/v1/products/{id}` mit `{"name":"Notebook"}` antwortet 200 und behält Preis und `active`. `DELETE` antwortet 204 ohne Körper. Dieselben Antworten prüft `sqlite_http_roundtrip` in `cargo test --workspace`. Der PostgreSQL-Test bleibt getrennt und ignoriert.

## Grenzen

- Keine produktionsreife Shop-Anwendung.
- Kein Admin und kein Login. Die fünf Routen kommen aus `register::<Product>()`.
- Schemaänderungen kommen aus `ferrum migrate`, nicht aus `--bootstrap`. Eine ältere `products`-Tabelle ohne Historie wird nicht nachträglich als migriert behandelt.
- `allow_anonymous=true` nicht öffentlich betreiben.

## Tests

`examples/myshop/tests/postgres_api.rs` enthält den SQLite-HTTP-Test und den ignorierten PostgreSQL-Test. Nur der PostgreSQL-Lauf löscht feste Testtabellen in der durch `DATABASE_URL` gewählten Datenbank. `--bootstrap` nicht gegen eine ungesicherte Produktionsdatenbank ausführen. Details stehen in [testing.md](testing.md).
