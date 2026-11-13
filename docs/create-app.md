# Create an app

[Overview](index.md) · [Projects](project-generator.md) · [Registry](app-registry.md)

From a layout-2 project:

```powershell
fexu new app produkte
fexu new app berichte --templates shared
```

`project.apps_dir = "apps"` creates `apps/produkte`; `"."` creates `produkte/`. The generator checks names, destinations and all registration markers before writing. Existing app/template/model files are refused. It updates the module/install/schema markers in `src/apps.rs` and appends `[app.produkte]` with the matching migration directory to `fexu.toml`, preserving other code.

Each app has `mod.rs`, `models/mod.rs`, `urls.rs`, `views.rs`, `admin.rs`, `migrations/.gitkeep`, `templates/produkte/index.html` and `static/produkte/produkte.css`. Its `FexuApplication` type registers a named `produkte.index` HTML route at `/produkte`. Model/schema/admin functions initially contain no business models:

```powershell
fexu generate model Customer --app produkte
fexu migrate make initial --app produkte
fexu migrate up
```

`--app` is required for models in layout 2. Migration history IDs are namespaced, such as `produkte:0001_initial`. Empty apps need no initial migration. Registration itself does not apply migrations.

## Templates

`--templates local` (default) writes `<app>/templates/<appname>/index.html`. `--templates shared` writes project `templates/<appname>/index.html`. New apps explicitly use `TemplateMode::ProjectOverride`: project files win matching names, local files are fallback, and shared-only files are supported even with no local directory. Global `base.html` can be inherited. Namespace, reserved admin/login/base names and path containment checks remain enforced.

Existing `TemplateMode::Isolated` apps still reject project collisions. Placement changes do not move existing files. Custom `templates.templates_dir` remains respected.

## Names and removal

Names start with an ASCII lowercase letter and contain only lowercase letters, numbers and underscores, up to 32 characters. Rust keywords, Windows device names, framework routes and project directories are reserved. `order_items` produces `OrderItemsApp`.

To unregister an app, remove its module, install call and schema arm from `src/apps.rs` and its `[app.<name>]` declaration together. Remove references/dependencies in other apps deliberately and archive the migration files. Existing database tables, history and data are retained. Never reset a database to unregister an app. Removing only the TOML or runtime migration declaration fails the existing consistency check.

`generate app` reports the replacement command. Layout-1 projects are not automatically converted; see [manual migration](project-generator.md#migration-from-old-clilayouts). For reusable Cargo crates see [reusable-apps.md](reusable-apps.md).
