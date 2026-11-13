# Project templates

[Overview](index.md) · [Generator](project-generator.md) · [Structure](project-structure.md)

| Template | Behavior |
|---|---|
| `fullstack` (default) | Empty fullstack project with authentication and native server-rendered admin |
| `api` | Empty project with authentication, homepage and ping API; no admin registration |

Both use SQLite by default, support PostgreSQL, serve static assets and require no Node.js. Neither creates business models, business migrations, seeded data or users. Fullstack adds admin branding from project settings; admin access remains protected. Mail is disabled by default. Bind defaults to `127.0.0.1:3000`; existing production/secret validation remains enforced.

```powershell
fexu new project Shop --template api --apps-dir apps
cd Shop
fexu new app customers
fexu generate model Customer --app customers
fexu migrate make initial --app customers
fexu migrate up
fexu dev
```

New API/fullstack models require their declared model permissions. They do not grant staff status or anonymous access. Apps do not automatically namespace SQL table/API names; review attributes before applying migrations.

Project templates live in `templates/`. App templates live in `<app>/templates/<appname>/` with explicit project overrides in `templates/<appname>/`. Overrides apply to that namespace only. Apps cannot replace admin/login/base templates. Project `templates/base.html` can be inherited by app pages.

Generated files use internal template version `0.25` and layout version `2`; the Cargo version remains `0.1.0`. See [compatibility](project-generator.md#migration-from-old-clilayouts).
