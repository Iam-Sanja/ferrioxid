# Change from React-Admin

[For overview](admin-native.md) · Routes: [admin-routing.md](admin-routing.md)]

## What is changing

Until v0.12 the surface was in `apps/admin-ui` and was built with Vite. v0.13 renders the same tasks in Rust. `apps/admin-ui` is removed. `npm run build` and `FERRUM_ADMIN_DIST` are no longer part of the build.

The browser now speaks directly to HTML routes. The JSON API under `/admin/api` remains for programs and previous tests. It is not a requirement for the surface.

## Was gleich bleibt

- `AdminRegistry`, `admin = true` and the field metadata
- Meetings, Staff, RBAC, Object Rules and Query Scopes
- HMAC-CSRF of the Writing Actions Session
- `POST /auth/login` als JSON
- Database, migrations and the REST API of the models
- Branding aus `ferrum.toml`
- German and English
- The cargo feature `admin-ui` as an empty flag, so that existing `Cargo.toml` files continue to build

## Anmeldung

The HTML login cannot yet send a session CSF, because the session only arises when successful. For this, there is the short-lived cookie `ferrioxid_login_csrf`. It does not replace the HMAC of the session. After login, `ferrioxid_session` and the field `csrf_token` apply again.

## Build

A fullstack project only needs Cargo:

```powershell
ferrioxid new myproject --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd myproject
ferrioxid migrate up
ferrioxid admin create-user --staff --superuser
ferrioxid dev
```

The homepage is under `/`, the admin under `/admin/login`. The API template does not integrate the admin.

## Not accepted

The React components, Vitest tests and Tailwind classes no longer exist. The test of the surface is in `examples/myshop/tests/admin_html.rs`. It speaks HTTP without JavaScript. A browser run is independent of this and is only counted if it has actually taken place.
