# App-Registry

[Zur Übersicht](index.md) · Apps: [apps.md](apps.md) · **IST-Datei:** `crates/ferrum-api/src/apps.rs`.

Die Registry liegt auf `App`. Es gibt kein globales veränderbares Singleton. Die Reihenfolge ist die Reihenfolge der `install`-Aufrufe.

## Typen

`AppConfig` trägt den technischen Namen, einen optionalen Anzeigenamen, ein optionales URL-Präfix und optionale Abhängigkeiten.

`FerrumApplication` hat zwei Methoden: `config` und `contribute`. `contribute` erhält eine `Registration` mit der Lebensdauer des aktuellen `install`.

`Registration` reicht nur an die vorhandenen Register weiter:

- Routen an `UrlSet` und die Pfadliste von `App`.
- Modelle an `register`.
- Admin an `AdminRegistry`.
- Migrationen an `PackageMigration`.
- Vorlagen an `Site`.
- Staticfiles an `ferrum_static::mount`.

`AppError.code` nennt die Ursache, zum Beispiel `app.duplicate`, `app.sealed`, `app.route.namespace`, `app.model.duplicate`, `app.template.conflict`, `app.static.path` und `app.migration`.

## Versiegeln

`seal` ist der Punkt, ab dem die Registry unveränderlich ist. Ein zweites `seal` ist erfolgreich und ändert nichts. `install` danach ist `app.sealed`.

Der Migrationsgraph wird erst hier geprüft, weil eine später installierte App eine Abhängigkeit erfüllen kann. Ein Zyklus, eine fehlende Abhängigkeit und ein doppelter App-Name im Graphen sind `app.migration` und schreiben nichts.

`try_into_router` und `into_router` holen `seal` nach, wenn Apps installiert sind oder `[app.<name>]` deklariert ist. Ein Fehler verhindert den Start. Ein explizites `seal` bleibt gültig und wird nicht doppelt ausgeführt. Ohne Apps und ohne Deklaration bleibt der bisherige Startweg.

## ferrum.toml

Es gibt keinen Abschnitt `[apps] enabled = ["shop"]`. Ein String würde eine unbekannte App nicht laden, und ein stilles Ignorieren wäre irreführend.

`[app.shop]` deklariert nur das Migrationsverzeichnis und `depends_on` für die CLI. Die CLI liest kein Rust. Das Verhalten der App kommt ausschließlich aus `install`.

```toml
[app.shop]
migrations = "src/apps/shop/migrations"
depends_on = "accounts"

[custom.shop]
currency = "EUR"
```

`[app]` ohne Namen ist ein Fehler. Der Pfad muss relativ sein und darf `..` nicht enthalten. Fehlt `migrations`, ist das ebenfalls ein Fehler.

`seal` verlangt, dass beide Listen dieselben App-Namen, `depends_on` und Verzeichnisse enthalten, sobald eine Seite nicht leer ist. Fehlt `with_settings`, ist das `app.migration.undeclared` und gilt nicht als geprüft. Ein Unterschied ist `app.migration.mismatch`. `ferrum check`, `ferrum doctor` und `ferrum migrate status` lesen nur `ferrum.toml` und schreiben die Zeile `app.migration.runtime`. Sie bestätigen den Rust-Abgleich nicht.
