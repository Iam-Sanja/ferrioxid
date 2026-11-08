# Project templates

[Overview](index.md) · **Implemented file:** `crates/ferrioxid-cli/src/scaffold.rs`.

## Purpose

The API template provides a minimal API application; the fullstack template adds authentication and admin registration. Both include static files and an empty media directory. Only fullstack projects register `/contact` and the admin. Uploads are not made public automatically.

## Responsible files

- `crates/ferrioxid-cli/src/scaffold.rs`
- `crates/ferrioxid-cli/src/models.rs`
- `crates/ferrioxid/Cargo.toml` (`admin-ui`)
- `crates/ferrioxid-admin/Cargo.toml` (`embedded-admin`)

## Public interface

| Command | Contents |
|---|---|
| `--template api` | Registry, CRUD, SQLite and migrations; no admin feature |
| `--template fullstack` |Additional Auth, Admin Registry, Feature `admin-ui`|

The standard is `fullstack`.

## Structure

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

Only `--template fullstack` generates `src/views/` and `templates/home.html`. The API template registers `api.ping` and retains the HTML homepage. See [project-structure.md](project-structure.md) for the exact file list. The template version is `0.24`.

`Cargo.lock` is created in the first cargo run.

## Configuration

Both templates enable Tera and a homepage. In the API template, `admin.enabled` and `embedded_assets` are `false`, and there is no admin link. In fullstack projects, admin and auth are enabled. Both include an anonymous product API; fullstack projects also expose the model in the admin. The generated binary rejects bind addresses starting with `0.0.0.0`.

## Data flow

`ferrioxid generate model Customer` writes `src/models/customer.rs` and inserts lines before the `ferrum:models`, `ferrioxid:uses`, `ferrioxid:schema`, `ferrioxid:registry` and, when present, `ferrioxid:admin` markers. The table is named `customers`. Repeating the command for the same name returns an error without replacing the existing model.

## Error cases

An invalid model name, missing marker or duplicate registration aborts generation. Files remain unchanged if writing has not started. The generator does not modify Rust code outside the designated marker locations.

## Security rules

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

Owner fields are not assigned from the session automatically. The example product API is anonymous and has no object policy. Configure users and roles for a fullstack project, starting with `ferrioxid admin create-user`.
