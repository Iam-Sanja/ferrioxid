# Admin surface

[Zur Übersicht](admin.md) · Nativ: [admin-native.md](admin-native.md) · Wechsel: [admin-migration-from-react.md](admin-migration-from-react.md)

The interface is HTML from Tera. It is located in `crates/ferrioxid-admin/templates` and `crates/ferrioxid-admin/static`. A Vite process and `apps/admin-ui` no longer exist.

The browser loads `/admin/assets/admin.css` and optionally `/admin/assets/admin.js`. CRUD is not attached to the script. Language and presentation change over `POST /admin/preferences`.

The earlier React application is described in [admin-migration-from-react.md](admin-migration-from-react.md)]. The JSON API remains under `/admin/api` and is documented in [admin-baking.md](admin-backend.md)].
