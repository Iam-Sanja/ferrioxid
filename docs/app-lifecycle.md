# App-Lebenszyklus

[Zur Übersicht](index.md) · Registry: [app-registry.md](app-registry.md) · Migrationen: [app-migrations.md](app-migrations.md)

`install` prüft Namen, Duplikate und die Beiträge der App. `seal` prüft danach den Migrationsgraphen, die Übereinstimmung mit `[app.<name>]` und die Admin-Registry. `seal` schreibt keine Migration.

`try_into_router` ruft `seal` auf, wenn Apps installiert sind oder `with_settings` mindestens eine `[app.<name>]`-Deklaration sieht und noch nicht versiegelt wurde. Ein zweiter Aufruf nach einem erfolgreichen `seal` prüft nicht noch einmal. `into_router` bleibt für bestehende Projekte ohne `Result`. Sind Apps oder Deklarationen vorhanden und die Prüfung scheitert, startet der Server nicht.

Ohne installierte Apps und ohne `[app.<name>]` bleibt `into_router` der bisherige Weg. myshop nutzt ihn unverändert.

Die CLI liest nur `ferrum.toml`. Sie führt den Rust-Code der App nicht aus. `check`, `doctor` und `migrate status` sagen das ausdrücklich und melden die TOML-Migrationen nicht als bestätigt gegen `registry.migrations`. Der Prozessstart ist die Stelle, die beide Seiten vergleicht. Diagnosebefehle legen keine Datenbankdatei und keine Historientabelle an.
