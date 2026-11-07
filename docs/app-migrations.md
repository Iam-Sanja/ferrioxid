# App-Migrationen

[Zur Übersicht](index.md) · Engine: [migrations.md](migrations.md) · **IST-Dateien:** `crates/ferrum-migrations/src/packages.rs`, `runner.rs`, `inspect.rs`.

Alle App-Migrationen bleiben in `_ferrum_migrations`. Auth bleibt in `_ferrum_auth_migrations`. Es gibt keine zweite Historientabelle.

## Kennung

Die Datei behält die ID `0001_initial`. Das Feld `name` bleibt `initial`. Die Prüfsumme wird über den Dateiinhalt mit dieser unqualifizierten ID gebildet.

Die Historienzeile einer neuen App lautet `shop:0001_initial`. Der Leser für die App entfernt das Präfix `shop:` und vergleicht den Rest mit der Datei-ID. Der bisherige Leser ignoriert Zeilen, die `:` enthalten. Eine alte Zeile `0001_initial` wird deshalb nicht einer App zugeordnet und nicht erneut ausgeführt.

Zwei Apps dürfen beide eine Datei `0001_initial.json` haben. Die Historien-IDs `shop:0001_initial` und `blog:0001_initial` sind verschieden.

## Reihenfolge

`depends_on` nennt andere App-Namen. `package_order` sortiert topologisch. Bereite Apps kommen nach Namen. Ein Zyklus, eine fehlende Abhängigkeit, ein Eigenverweis und ein doppelter Name scheitern, bevor eine Datei angewendet wird.

`ferrum migrate up` wendet zuerst Auth an, dann das bisherige Projektverzeichnis, dann die Pakete aus `[app.<name>]`. Ein zweiter Lauf wendet nichts erneut an. Eine geänderte Prüfsumme ist ein Konflikt und wird nicht angewendet. Der Server startet nur, wenn `registry.migrations` dieselbe Menge beschreibt. Die CLI kann die Rust-Seite nicht lesen, ohne die Anwendung auszuführen, und behauptet die Übereinstimmung deshalb nicht.

`migrate status`, `check` und `doctor` zählen ausstehende Paketmigrationen mit. Eine fehlende `ferrum.toml` bei einem reinen `--migrations-dir` bleibt der bisherige leere Paketfall. Eine vorhandene, ungültige `ferrum.toml` ist ein Fehler. Die Diagnose legt keine Historientabelle und keine SQLite-Datei an.

## Bestehende Projekte

Projekte ohne `[app.<name>]` behalten ihre unqualifizierten IDs. Angewendete Zeilen und ihre Prüfsummen werden nicht umgeschrieben. Tabellen werden nicht umbenannt. Das Installieren einer App erzeugt keine Migration.

myshop `0001` bis `0005` bleiben die Projekthistorie. Sie sind kein App-Paket.
