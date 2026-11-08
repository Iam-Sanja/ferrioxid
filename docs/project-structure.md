# Project structure

[Overview](index.md) · Templates: [project-templates.md](project-templates.md) · Upgrade: [project-upgrade.md](project-upgrade.md) · **Implemented file:** `crates/ferrioxid-cli/src/scaffold.rs`.

## Purpose

`ferrioxid new` creates a project outside the Ferrioxid repository. The Cargo version of dependency remains that of the workspace. The template version in the manifest is `0.24`. `ferrioxid generate app` then supplements `src/apps/<name>/`, described in [create-app.md](create-app.md). The generator of new projects does not write this directory.

## Fullstack

```text
meinprojekt/
├── Cargo.toml
├── ferrioxid.toml
├── .gitignore
├── .env.example
├── README.md
├── .ferrioxid/project.json
├── src/main.rs
├── src/lib.rs
├── src/settings.rs
├── src/urls.rs
├── src/views/mod.rs
├── src/views/home.rs
├── src/models/mod.rs
├── src/models/product.rs
├── templates/base.html
├── templates/index.html
├── templates/home.html
├── templates/errors/404.html
├── templates/errors/500.html
├── static/css/app.css
├── static/js/app.js
├── media/.gitkeep
├── migrations/0001_initial.json
└── tests/api.rs
```

`main.rs` calls `settings::load`, obtains `bind_address`, and adds `.with_settings` and `.urls(urls::routes()?)?` to the existing admin, auth and contact setup. `/` remains the built-in homepage. The named route `site.home` uses `/home` and renders `templates/home.html`.

## API

The API template includes models, migrations, static files and homepage templates. It does not register admin or authentication routes and has no `src/views/`. `src/urls.rs` registers `api.ping` at `/api/v1/ping`, returning JSON `{"ok": true}`.

The HTML files remain because `templates.enabled` is true in both templates and the existing homepage expects them. An API template without any HTML file is not the generated state.

## Explicit exclusions

There is no second settings module besides `settings.rs`. This module only calls `Settings::discover`. There is no additional example table besides `products`. Contact remains a function in `main.rs`, not its own view file.

## Legacy

A project whose `main.rs` uses `discover_project`, `App::new`, `register` and `merge` does not need to rewrite this file. `ferrioxid.toml` without `[custom]` remains valid. Existing migrations are not changed. `admin-ui` remains an accepted feature.

`ferrioxid upgrade` does not replace a custom file. If the template version is not on `0.24`, the plan reports `project.template.outdated` and leaves the files. A modified file kept in the manifest remains `project.file.modified`. The adjustment to `src/settings.rs` and `src/urls.rs` is manual if an old project wants to adopt it.

The `D:\04_Projekte_Arbeit\Projekte\testprojekt` directory is not newly created and not changed by this template. Tests create their own temporary copies.
