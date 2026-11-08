# Project generator

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-cli/src/scaffold.rs`.

## Purpose

`ferrioxid new` creates an independent Ferrioxid project. It compiles outside the Ferrioxid workspace, uses SQLite and registers an example model. Fullstack projects transfer branding and currency to `AdminRegistry::with_branding` and do not need Node.js for the admin. The homepage contains fixed language links and a localized 500 text. Already created projects do not automatically take over. Since v0.12, the generator is `static/css/app.css`, `static/js/app.js` and `media/.gitkeep`. `media/*` is ignored. `ferrum.toml` contains `[static]` and `[storage]`. Fullstack projects have `/contact`. The API template does not mount the form.

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

The default template is `fullstack`. Until the crates are published, `--ferrioxid-path` points to the local Ferrioxid repository. Without that argument, the CLI uses the repository path recorded when `ferrioxid-cli` was compiled.

## Configuration

Generated `ferrum.toml` files configure project, database, server, admin, auth, templates and branding. Templates include `base.html`, `index.html` and `errors/`. The database defaults to `sqlite://db.sqlite3?mode=rwc` and the server to `127.0.0.1:3000`. Secrets are not stored in this file. `.env.example` lists `DATABASE_URL`, `BIND_ADDRESS`, and commented guidance for `FERRUM_CSRF_SECRET` and `FERRUM_PASSWORD`.

## Data flow

1. The project name is checked.
2. The target directory must not already exist.
3. Canonicalize the Ferrioxid path and require `crates/ferrioxid/Cargo.toml`.
4. On Windows, the CLI removes the prefix `\\?\` in order for Cargo to accept the path as a dependency.
5. Write the generated files without replacing existing files.
6. Create `0001_initial.json` using the same sealing procedure as other migrations.

## Error cases

| Case | Behavior |
|---|---|
| Invalid or reserved project name, including path or shell characters | Reject without creating a project |
| Target directory already exists | Reject; existing contents remain unchanged |
| `--ferrioxid-path` does not contain the Ferrioxid crate | Abort generation |

## Security rules

No shell interpolation of the name. No `0.0.0.0`. No built-in password. No overwriting. The dependency path points to `crates/ferrioxid`, not individual internal files.

## Example

```powershell
ferrioxid new demo --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd demo
ferrioxid migrate up
ferrioxid admin create-user --email admin@example.com --staff --superuser
ferrioxid dev
```

`create-user` prompts for a password without echoing it. For automation, set `FERRUM_PASSWORD` to a password of at least 12 characters. There is no default password.

## Tests

`crates/ferrioxid-cli/tests/generator.rs` generates API and fullstack projects in a temporary directory, rejects `../escape` and an existing target, and verifies `cargo check`.

## Limitations

Versioned crates.io dependencies are planned; generated projects currently use a local path dependency. Cargo writes `Cargo.lock` during the first build, not during generation. After `ferrioxid generate model`, create the model's migration with `ferrioxid migrate make`.
