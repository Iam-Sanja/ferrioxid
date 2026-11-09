# Project generator

[Overview](index.md) · [App generator](create-app.md) · [Structure](project-structure.md)

## Create an empty project

```powershell
ferrioxid new project Shop --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd Shop
ferrioxid migrate up
ferrioxid dev
```

The default is `--template fullstack`; `--template api` omits admin registration. Both create a runnable homepage, a named `project.ping` route at `/api/v1/ping`, authentication, static files and an empty media directory. Neither creates a business app, a `Product` model, business tables, seed data or an initial domain migration. `migrate up` applies the framework authentication and jobs migrations, then explicitly declared project/app migrations. It never resets a database. No user or default password is created.

The directory/display name remains `Shop`. Its Rust package/import name is `shop`; `MyShop` becomes `my_shop` and `HTTPServer` becomes `http_server`. Names must start with an ASCII letter, contain only ASCII letters, digits or underscores, and fit within 48 characters. Rust keywords, Windows device names and reserved project directories are rejected.

Crates are not published yet. `--ferrioxid-path` points to the framework checkout; without it the generator uses its build-time checkout. Create projects outside the framework workspace. Existing target directories are refused; newly generated files use exclusive creation.

## Project and app are different

Global configuration lives in `ferrioxid.toml`, global startup/settings/URLs in `src/project/`, and explicit Rust app registration in `src/apps.rs`. Apps own models, routes, views, admin extensions, migrations, templates and static files. See [project-structure.md](project-structure.md).

```powershell
ferrioxid new app produkte
ferrioxid generate model Customer --app produkte --timestamps
ferrioxid migrate make initial --app produkte
ferrioxid migrate up
ferrioxid admin create-user --email admin@example.com --staff --superuser
ferrioxid dev
```

Model generation creates only the model requested by the developer. New models require authentication and their model permissions, including API-template models. Table/API names are explicit model attributes and are not automatically prefixed by app names. Choose distinct names across apps.

## Placement and templates

`--apps-dir apps` (default) creates `apps/<name>/`; `--apps-dir .` creates `<name>/` directly in the project root. The choice is stored as `project.apps_dir` and affects only subsequently generated apps. Each is a module in the same Cargo package, connected with an explicit `#[path]` declaration in `src/apps.rs`. Cargo does not discover arbitrary directories. Moving an app requires updating its Rust module path and its runtime/TOML migration and asset paths together.

`new app <name> --templates local` creates an app-local template; `--templates shared` creates `templates/<name>/index.html`. Shared templates override matching app-local names and may exist without a local counterpart. The namespace and reserved-name checks remain enforced. See [create-app.md](create-app.md).

## Migration from old CLI/layouts

| Old call | New call |
|---|---|
| `ferrioxid new Shop` | `ferrioxid new project Shop` |
| `ferrioxid generate app produkte` | `ferrioxid new app produkte` |
| `ferrioxid generate model Customer` in a new project | `ferrioxid generate model Customer --app produkte` |
| `ferrioxid migrate make initial` for an app | `ferrioxid migrate make initial --app produkte` |

The first two old calls fail with actionable guidance and do not create files. Existing manifests without `project.layout_version` remain layout 1. Their startup, model generation and migration IDs keep working. `new app` refuses layout 1 rather than silently relocating code.

For an existing project, first commit/back up code and back up its database. Generate a layout-2 reference project in a separate, unused directory. Port startup, settings and root URLs manually; add `src/apps.rs` and explicitly install each migrated app. Set `project.layout_version = 2` only after adapting the source and declarations. Preserve existing model table names, API paths, project migration files, checksums and history IDs. Applied unqualified migrations stay in the project history; do not copy them into an app package or baseline/reset the database. Use `migrate status` and `check` before applying any new, reviewed migration. Compare behavior against the old project before deployment.

The internal template version is `0.25`; the Cargo version remains `0.1.0`. `upgrade` does not convert layouts or replace project files. App/model generation intentionally changes registration files, so later manifest diagnostics can report their original hashes as modified; those warnings do not authorize replacement.

## Validation

`crates/ferrioxid-cli/tests/generator.rs` exercises both templates outside the workspace, Rust package normalization, both app placements, local/shared templates, names and duplicates, legacy-command rejection, SQLite system-only initialization, app migrations, protected model APIs, HTTP startup, development startup and release builds. Existing projects and databases are never automatically restructured.
