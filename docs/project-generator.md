# Projektgenerator

[For overview](index.md) · **IST file:** `crates/ferrioxid-cli/src/scaffold.rs`.]

## Aufgabe

`ferrioxid new` creates an independent ferrioxide project. It compiles outside the ferrioxide workspace, uses SQLite and registers an example model. Fullstack projects transfer branding and currency to `AdminRegistry::with_branding` and do not need Node.js for the admin. The homepage contains fixed language links and a localized 500 text. Already created projects do not automatically take over. Since v0.12, the generator is `static/css/app.css`, `static/js/app.js` and `media/.gitkeep`. `media/*` is ignored. `ferrum.toml` contains `[static]` and `[storage]`. Fullstack projects have `/contact`. The API template does not mount the form.

## Responsible files

- `crates/ferrioxid-cli/src/scaffold.rs`
- `crates/ferrioxid-cli/src/main.rs`
- `crates/ferrioxid-cli/tests/generator.rs`

## Public interface

```powershell
ferrioxid new myproject
ferrioxid new myapi --template api
ferrioxid new myshop --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
```

The standard template is `fullstack`. `--ferrioxid-path` points to the local ferrioxide repository as long as the crates are not published. Without the argument, the CLI uses the path known when translating `ferrioxid-cli`.

## Konfiguration

The generated `ferrum.toml` contains project, database, server, admin, authentic, templates and branding. There is also `templates/` with `base.html`, `index.html` and `errors/`. The database URL is `sqlite://db.sqlite3?mode=rwc`. The server is `127.0.0.1:3000`. Secrets are not in the file. `.env.example` names `DATABASE_URL`, `BIND_ADDRESS` and commented references to `FERRUM_CSRF_SECRET` and `FERRUM_PASSWORD`.

## Datenfluss

1. The project name is checked.
2. The target list must not yet exist.
3. The ferrioxide path is canonized and must contain `crates/ferrioxid/Cargo.toml`.
4. On Windows, the CLI removes the prefix `\\?\` in order for Cargo to accept the path as a dependency.
5. Files are rewritten. An existing file is not replaced.
6. `0001_initial.json` is created via the same seal path as other migrations.

## Error cases

| Fall | Verhalten |
|---|---|
|Name does not begin with a letter, contains path or shell signs, or is reserved|Abort, nothing is put on|
|Target list exists|Abortion, content remains|
|`--ferrioxid-path` does not contain ferrioxide crate| Abbruch |

## Sicherheitsregeln

No shell interpolation of the name. No `0.0.0.0`. No built-in password. No overwriting. The dependency path points to `crates/ferrioxid`, not individual internal files.

## Example

```powershell
ferrioxid new demo --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd demo
ferrioxid migrate up
ferrioxid admin create-user --email admin@example.com --staff --superuser
ferrioxid dev
```

`create-user` asks for the password concealed. For automations, the CLI `FERRUM_PASSWORD` reads at least 12 characters. There is no standard value.

## Tests

`crates/ferrioxid-cli/tests/generator.rs` generates API and fullstack projects in a temporary directory, rejects `../escape` and an existing target, and verifies `cargo check`.

## Limitations

Versioned crates.io dependencies are planned and not the standard. `Cargo.lock` writes Cargo at the first translation, not the generator. A generated model according to `ferrioxid generate model` does not receive its own migration. It is created by `ferrioxid migrate make`.
