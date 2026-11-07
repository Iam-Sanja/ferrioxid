# App erzeugen

[Zur Übersicht](index.md) · CLI: [cli.md](cli.md) · **IST-Datei:** `crates/ferrum-cli/src/apps.rs`.

```powershell
ferrum generate app shop
```

Der Befehl legt diese Dateien an und überschreibt nichts:

```text
src/apps/mod.rs
src/apps/shop/mod.rs
src/apps/shop/models.rs
src/apps/shop/urls.rs
src/apps/shop/views.rs
src/apps/shop/admin.rs
src/apps/shop/migrations/
src/apps/shop/templates/shop/index.html
src/apps/shop/static/shop/shop.css
```

`src/apps/mod.rs` erhält nur die Zeile `pub mod shop;`, wenn sie fehlt. `main.rs` wird nur geändert, wenn sie die Markierung `// ferrum:apps:end` enthält. Sonst nennt die Ausgabe die Zeile `.install(shop::ShopApp)?`.

Der erzeugte Typ implementiert `FerrumApplication`. Die Route `shop.index` liegt auf `/` und wird unter dem Präfix `/shop` eingehängt. Die Antwort ist JSON `{"app":"shop"}`. Vorlagen und Staticfiles werden im isolierten Modus angemeldet. `admin::attach` ist absichtlich leer.

Der Name folgt denselben Regeln wie `AppConfig`: Kleinbuchstaben, Ziffern, `_`, höchstens 32 Zeichen, kein Pfad und keiner der reservierten Namen.

`ferrum new` bleibt unverändert. Die Template-Version bleibt `0.24`. Eine separate Crate erzeugt der Befehl nicht. Dafür steht [reusable-apps.md](reusable-apps.md).

Die Migrationen trägt das Projekt selbst ein:

```toml
[app.shop]
migrations = "src/apps/shop/migrations"
```

Dieser Abschnitt lädt die Rust-App nicht. `install` bleibt erforderlich. `ferrum migrate up` wendet die Dateien an, `install` nicht.
