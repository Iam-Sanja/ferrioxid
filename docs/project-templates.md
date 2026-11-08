# Projektvorlagen

[For overview](index.md) · **IST file:** `crates/ferrioxid-cli/src/scaffold.rs`.]

## Aufgabe

Two templates distinguish a pure API from an application with authentication and admin. Both get static files and an empty media folder. Only Fullstack registers `/contact` and the admin. Uploads are not automatically made public.

## Responsible files

- `crates/ferrioxid-cli/src/scaffold.rs`
- `crates/ferrioxid-cli/src/models.rs`
- `crates/ferrioxid/Cargo.toml` (`admin-ui`)
- `crates/ferrioxid-admin/Cargo.toml` (`embedded-admin`)

## Public interface

| Befehl | Inhalt |
|---|---|
| `--template api` | Registry, CRUD, SQLite, Migration, kein Admin-Feature |
| `--template fullstack` |Additional Auth, Admin Registry, Feature `admin-ui`|

The standard is `fullstack`.

## Struktur

```text
meinprojekt/
├── Cargo.toml
├── ferrum.toml
├── .gitignore
├── .env.example
├── README.md
├── .ferrum/project.json
├── src/main.rs
├── src/lib.rs
├── src/settings.rs
├── src/urls.rs
├── src/views/mod.rs
├── src/views/home.rs
├── src/models/mod.rs
├── src/models/product.rs
├── templates/
├── static/css/
├── static/js/
├── migrations/0001_initial.json
└── tests/api.rs
```

`src/views/` and `templates/home.html` write only `--template fullstack`. The API template registers `api.ping` and keeps the HTML homepage. The exact list is in [project-structure.md](project-structure.md)]. The template version is `0.24`.

`Cargo.lock` is created in the first cargo run.

## Konfiguration

Both templates activate Tera and a homepage. API: `admin.enabled` and `embedded_assets` are incorrect, the admin link is missing. Fullstack: Admin and Auth are true. The product model of the API is anonymous. The Fullstack model is also visible in the admin. The generated binary rejects a bind address that begins with `0.0.0.0`.

## Datenfluss

`ferrioxid generate model Customer` schreibt `src/models/customer.rs` und ergänzt Zeilen vor den Markierungen `ferrum:models`, `ferrioxid:uses`, `ferrioxid:schema`, `ferrioxid:registry` und, falls vorhanden, `ferrioxid:admin`. Die Tabelle heißt `customers`. Ein zweiter Aufruf desselben Namens ändert nichts und endet mit einem Fehler.

## Error cases

An invalid model name, a missing marker or an already registered model cancels. The remaining files will then remain unchanged if the writing process has not yet begun. The generator does not replace any Rust files outside of these markers.

## Sicherheitsregeln

The API template does not require the admin feature and does not include admin routes. Fullstack projects activate the empty feature `admin-ui` and register the admin. Both templates compile without Node.js. The frontend directory `apps/admin-ui` no longer exists.

## Example

```powershell
ferrioxid new shop --template api --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd shop
ferrioxid generate model Customer
```

## Tests

`generator.rs` checks both templates with `cargo check`, the model registration by a second `cargo check` and the rejection of a duplicate name.

## Limitations

Owner fields are not automatically set out of the session. That is planned. The template contains an anonymous product example, not an object rule. Authentication in the Fullstack project must complete the developer with `ferrioxid admin create-user` and roles.
