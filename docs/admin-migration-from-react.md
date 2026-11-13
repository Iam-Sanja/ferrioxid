# Migration from the React admin

[Overview](admin-native.md) · Routes: [admin-routing.md](admin-routing.md)

## What is changing

Until v0.12 the surface was in `apps/admin-ui` and was built with Vite. v0.13 renders the same tasks in Rust. `apps/admin-ui` is removed. `npm run build` and `FEXU_ADMIN_DIST` are no longer part of the build.

The browser now speaks directly to HTML routes. The JSON API under `/admin/api` remains for programs and previous tests. It is not a requirement for the surface.

## What stays unchanged

- `AdminRegistry`, `admin = true` and the field metadata
- Sessions, staff access, RBAC, object policies and query scopes
- Session-bound HMAC CSRF tokens for state-changing requests
- JSON `POST /auth/login`
- Database, migrations and the REST API of the models
- Branding from `fexu.toml`
- German and English
- The cargo feature `admin-ui` as an empty flag, so that existing `Cargo.toml` files continue to build

## Login

The HTML login cannot send a session CSRF token before a session exists, so it uses the short-lived `fexu_login_csrf` cookie. After login, the default `fexu_session` cookie and `csrf_token` form field provide session authentication and CSRF protection.

## Build

A fullstack project only needs Cargo:

```powershell
fexu new project myproject --template fullstack --fexu-path D:\04_Projekte_Arbeit\Projekte\fexu
cd myproject
fexu migrate up
fexu admin create-user --staff --superuser
fexu dev
```

The homepage is under `/`, the admin under `/admin/login`. The API template does not integrate the admin.

## Not accepted

The React components, Vitest tests and Tailwind classes no longer exist. The test of the surface is in `examples/myshop/tests/admin_html.rs`. It speaks HTTP without JavaScript. A browser run is independent of this and is only counted if it has actually taken place.
