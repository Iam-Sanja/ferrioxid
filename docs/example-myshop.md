# myshop – Verbindliche Beispielanwendung

[Zur Übersicht](index.md) · **IST-Dateien:** `examples/myshop/src/main.rs`, `examples/myshop/src/models/product.rs`.

## Zweck

`myshop` prüft, ob die **öffentlichen** Ferrum-Komponenten gemeinsam funktionieren. Das Beispiel darf keine privaten Sonderwege erhalten, die gewöhnlichen Anwendern fehlen.

## Modell (IST)

`Product` hat `id: Uuid`, `name: String`, `price_cents: i64`, `active: bool`. Die Attribute setzen UUID-Auto-ID, Pflichtname mit 3–150 Zeichen, nichtnegativen Preis und `DEFAULT TRUE` für `active`. Die API ist für die lokal begrenzte Demo ausdrücklich anonym unter `/api/v1/products` freigegeben.

## Startablauf (IST)

```bash
docker compose up -d db
cargo check --workspace
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

`DATABASE_URL` ist optional, weil `main.rs` ein **lokales Demo-Default** verwendet. Für andere Umgebungen explizit setzen. Bind-Default `127.0.0.1:3000`, Überschreibung mit `BIND_ADDRESS` möglich und potentiell gefährlich. Bootstrap öffnet die DB, registriert das Modell, führt dessen `schema_sql()` aus, liest danach die typisierte Liste, loggt Zeilen nur auf Debug-Niveau und beendet sich.

### API-Probe

```bash
curl -i -X POST http://127.0.0.1:3000/api/v1/products -H 'Content-Type: application/json' -d '{"name":"Laptop","price_cents":1250}'
curl -i http://127.0.0.1:3000/api/v1/products
```

Erwartung: POST 201 mit UUID und `active:true`, GET 200 mit `{data, limit:100, truncated:false}` (sofern insgesamt ≤ 100 Einträge). `PATCH /api/v1/products/{id}` mit `{"name":"Notebook"}` liefert 200 und behält `price_cents` sowie `active`. `DELETE /api/v1/products/{id}` liefert 204 ohne Körper. Das sind **erwartete Ergebnisse**. Der PostgreSQL-Integrationstest, der sie prüft, wurde auf diesem Rechner nicht erfolgreich ausgeführt.

## Anti-Ziele

- Keine produktionsreife Demo, keine echte Shop-Anwendung.
- Kein Admin, Login oder Migration Engine. PATCH und DELETE kommen aus `register::<Product>()`, nicht aus eigenen Handlern.
- Kein öffentliches Deployment mit `allow_anonymous=true`.

## Tests

Optionaler DB/HTTP-Test: `examples/myshop/tests/postgres_api.rs`. Er verwendet eigenständige Testtabellen und löscht sie. [testing.md](testing.md) enthält den ausführbaren Befehl. `--bootstrap` niemals gegen ungesicherte Produktionsdatenbanken einsetzen.
