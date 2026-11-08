# Wiederverwendbare Apps

[Zur Übersicht](index.md) · Apps: [apps.md](apps.md) · **IST:** `examples/notes`.

Eine App kann in einer eigenen Crate liegen. Sie hängt an `ferrioxid`, nicht an `examples/myshop` und nicht an privaten Framework-Modulen.

```toml
[dependencies]
ferrioxid = { path = "../ferrioxid/crates/ferrioxid" }
notes = { path = "../notes" }
```

```rust
let app = ferrioxid::App::new(database)
    .install(notes::NotesApp)?
    .seal()?;
```

`examples/notes` registriert `notes.list` unter `/notes/items`. Der Test `examples/notes/tests/reuse.rs` installiert zusätzlich eine zweite App und zeigt, dass `install` und `seal` keine Tabelle `_ferrum_migrations` anlegen. Die Antwort von `/notes/items` ist JSON.

## Grenzen

- Die Crate muss im Binary vorkommen. Ferrioxid lädt keine Dynamic Library und liest den Crate-Namen nicht aus `ferrum.toml`.
- Modelle nutzen `FerrioxidModel` und `registry.model`. Ein Tabellenname bleibt der Name aus dem Modell. Zwei Modelle mit derselben Tabelle werden abgelehnt.
- Beziehungen zwischen Apps bleiben normale Fremdschlüssel. Die Zieltabelle muss existieren. Ein qualifizierter Modellname ändert den SQL-Namen nicht.
- App-Einstellungen sind `[custom.<app>]` und `ProjectConfig::custom_settings`. `Registration::project` liefert die Konfiguration nur, wenn `with_settings` vor `install` aufgerufen wurde. Framework-Schlüssel werden dadurch nicht ersetzt.
- Migrationen einer externen App liegen in ihrem Verzeichnis und werden über `registry.migrations` oder `[app.<name>]` angemeldet. Die Version steht in der Datei-ID und der Prüfsumme, nicht in einem zweiten Versionsfeld.
- `install` ändert die Datenbank nicht.
