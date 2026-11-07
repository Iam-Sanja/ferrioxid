# Eigene Settings

[Zur Übersicht](index.md) · Gemeinsame Datei: [settings.md](settings.md) · **IST-Datei:** `crates/ferrum-migrations/src/project.rs`.

## Aufgabe

Anwendungswerte stehen unter `[custom.<name>]`. Ferrum speichert sie am gelesenen `ProjectConfig` und wandelt einen Abschnitt in eine Struktur um, die `serde::Deserialize` implementiert. Es gibt keinen zweiten Parser und kein Singleton.

## Datei

```toml
[custom.shop]
currency = "EUR"
items_per_page = 25
open = true
```

`shop` und jeder Schlüssel sind Kleinbuchstaben, Ziffern und Unterstriche, höchstens 32 Zeichen, und beginnen mit einem Buchstaben. Schlüssel, die `password`, `secret`, `token` oder `credential` enthalten, werden abgelehnt. Die Fehlermeldung enthält den Wert nicht.

## Lesen

```rust
#[derive(serde::Deserialize)]
struct ShopSettings {
    currency: String,
    items_per_page: u64,
    open: bool,
}

let settings = ferrum::migrations::Settings::discover(std::path::Path::new("."))?;
let shop: ShopSettings = settings.project().custom_settings("shop")?;
```

`custom_settings` liefert `custom.shop ist nicht konfiguriert.`, wenn der Abschnitt fehlt. Passt ein Feld nicht, lautet die Meldung `custom.shop passt nicht zur erwarteten Struktur.` Der Rohwert steht darin nicht.

## Typen

Der Parser kennt in dieser Familie nur drei Formen:

- `true` und `false` werden boolesch.
- Eine nicht leere Ziffernfolge wird `u64`.
- Jeder andere Wert bleibt eine Zeichenkette.

Dezimalzahlen, negative Zahlen und Listen gibt es hier nicht. Dafür bleiben die bestehenden typisierten Abschnitte zuständig.

## Grenzen

Ein Abschnitt ersetzt keine Framework-Einstellung. `custom.server` ist ein eigener Name und ändert `server.port` nicht. Unbekannte Abschnitte außerhalb von `custom.*` bleiben Fehler. Bestehende Dateien ohne `custom` bleiben gültig.
