# Project structure

[Overview](index.md) · [Generator](project-generator.md) · [Templates](project-templates.md)

New layout-2 projects separate global configuration from business apps:

```text
Shop/
  Cargo.toml                     # Rust package shop; directory/display name Shop
  fexu.toml                 # global configuration and app declarations
  .fexu/project.json        # generation manifest, no secrets
  .env.example
  src/main.rs                    # thin entry point
  src/lib.rs
  src/apps.rs                    # explicit modules, installation and schema selection
  src/project/mod.rs             # startup
  src/project/settings.rs
  src/project/urls.rs             # global named routes
  migrations/.gitkeep            # no business migration
  templates/base.html
  templates/index.html
  templates/errors/{404,500}.html
  static/{css,js}/
  media/.gitkeep
  tests/project.rs
```

`fexu new app produkte` adds `apps/produkte/` by default or `produkte/` with `project.apps_dir = "."`. Each is a module in the same Cargo package, connected by `#[path = "../apps/produkte/mod.rs"]` or `#[path = "../produkte/mod.rs"]` in `src/apps.rs`. Cargo does not discover arbitrary directories. The generator inserts registration lines only at designated markers and appends the matching TOML app declaration.

Apps contain `mod.rs`, `models/mod.rs`, `urls.rs`, `views.rs`, `admin.rs`, `migrations/.gitkeep`, `templates/produkte/index.html` and `static/produkte/produkte.css`. With `--templates shared` the initial HTML instead lives in project `templates/produkte/`. No app model or migration is created until requested.

`Cargo.lock` appears on the first Cargo build. Existing layout-1 projects retain their files and root-model generation. See [manual migration](project-generator.md#migration-from-old-clilayouts).
