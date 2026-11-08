# myshop – Verbindliche Beispielanwendung

[Zur Übersicht](index.md) · **IST-Dateien:** `examples/myshop/src/lib.rs`, `examples/myshop/src/main.rs`, `examples/myshop/src/models/category.rs`, `examples/myshop/src/models/product.rs`.

## Zweck

`myshop` prüft, ob die öffentlichen Ferrioxid-Komponenten gemeinsam funktionieren. Das Beispiel nutzt dieselbe Fassade wie andere Anwendungen und enthält keine privaten Datenbankzugriffe. Es aktiviert das Feature `admin-ui`. Ein neues eigenständiges Projekt erzeugt `ferrioxid new`, beschrieben in [project-generator.md](project-generator.md). myshop selbst bindet keine Objektregel an Produkt oder Kategorie. Die Create-Policy wird in den Tests `auth.rs` und `admin_http.rs` geprüft. Seit v0.12 liefert `/` eine Seite mit `/static/css/app.css`. `/contact` prüft `ContactForm` und versendet keine E-Mail. `POST /uploads` ist nur mit Sitzung erreichbar. Request-ID und Sicherheitsheader kommen aus `ferrioxid-middleware`.

## Modell (IST)

`Category` hat `id` und einen eindeutigen `name`. `Product` hat `id`, einen indizierten Pflichtnamen mit 3–150 Zeichen, `price_cents`, `active` mit Default `true` und ein optionales `category_id`. Das Feld verweist mit `on_delete = "restrict"` auf `categories.id`. `product.category(&db)` lädt die Kategorie, `category.products(&db)` lädt die Produkte. Beide Demo-APIs sind ausdrücklich anonym. Das gilt nur für den lokalen Prozess. `InternalNote` liegt unter `/api/v1/internal_notes`, verlangt `notes.read|create|update|delete` und ist die geschützte Ressource. `0003_internal_notes.json` legt ihre Tabelle an.

## Start mit SQLite (IST)

```powershell
cargo run -p ferrioxid-cli -- migrate up
cargo run -p myshop
```

Die Befehle gehören ins Workspace-Root, weil dort `ferrum.toml` liegt. Ohne `DATABASE_URL` verwenden CLI und myshop `sqlite://db.sqlite3?mode=rwc` im Arbeitsverzeichnis. `0001_initial.json` legt `products` an und bleibt unverändert. `0002_categories_and_product_relations.json` legt `categories` an, fügt das nullable `category_id` mit Foreign Key hinzu und erzeugt `ix_products_name`. Der Server startet auf `127.0.0.1:3000`, sofern `BIND_ADDRESS` nicht gesetzt ist, und migriert nicht selbst. Benutzer legt `ferrioxid admin create-user` an. Login, Cookie, `/auth/me`, geschützter Zugriff, Rollen und Logout zeigt `examples/myshop/tests/auth.rs`.

`cargo run -p myshop -- --bootstrap` bricht ab. Er löscht keine Datei, legt keine Tabelle an und schreibt keine Historie. `cargo run -p myshop -- --print-schema` gibt den Snapshot von `Category` und `Product` aus. Das ist die Registrierung, die `schema_command` aufruft.

PostgreSQL ist optional:

```powershell
$env:DATABASE_URL="postgres://ferrioxid:ferrioxid_dev_only@127.0.0.1:5432/ferrioxid"
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
- Kein Admin und kein Login. Die CRUD-Routen kommen aus `register::<Category>()` und `register::<Product>()`. Es gibt keine verschachtelte Beziehungsroute.
- Schemaänderungen kommen aus `ferrioxid migrate`, nicht aus `--bootstrap`. Eine ältere `products`-Tabelle ohne Historie wird nicht beim Start markiert. Passt ihr Schema zur Migrationskette und enthält die Kette keine Löschung oder Typänderung, kann `ferrioxid migrate baseline --confirm` nur die Historie schreiben. Ohne `--confirm` bleibt die Datei unverändert.
- `allow_anonymous=true` nicht öffentlich betreiben.

## Tests

`examples/myshop/tests/relations.rs` legt Kategorie und Produkt an, lädt beide Richtungen, lehnt einen doppelten Namen und einen ungültigen Foreign Key ab und wendet `0001` und `0002` auf eine temporäre Datei an. `examples/myshop/tests/postgres_api.rs` enthält den SQLite-HTTP-Test und den ignorierten PostgreSQL-Test. `examples/myshop/tests/schema_migration.rs` hält `0001` am ursprünglichen Produktmodell fest. Nur der PostgreSQL-Lauf löscht feste Testtabellen in der durch `DATABASE_URL` gewählten Datenbank. `--bootstrap` nicht gegen eine ungesicherte Produktionsdatenbank ausführen. Details stehen in [testing.md](testing.md).

## Admin

`Product`, `Category` und `InternalNote` tragen `admin = true`. `main.rs` registriert sie in `AdminRegistry`. Öffentliche Produkt- und Kategorierouten bleiben anonym. Interne Notizen bleiben über `notes` geschützt. Die Admin-Oberfläche verlangt Staff und das jeweilige Modellrecht. Sie liegt unter `http://127.0.0.1:3000/admin` und braucht kein Node.js. Die Migrationen ändern sich dadurch nicht.
