# Admin-Oberfläche

[Zur Übersicht](admin.md) · Nativ: [admin-native.md](admin-native.md) · Wechsel: [admin-migration-from-react.md](admin-migration-from-react.md)

Die Oberfläche ist HTML aus Tera. Sie liegt in `crates/ferrioxid-admin/templates` und `crates/ferrioxid-admin/static`. Ein Vite-Prozess und `apps/admin-ui` gibt es nicht mehr.

Der Browser lädt `/admin/assets/admin.css` und optional `/admin/assets/admin.js`. CRUD hängt nicht am Script. Sprache und Darstellung wechseln über `POST /admin/preferences`.

Die frühere React-Anwendung ist in [admin-migration-from-react.md](admin-migration-from-react.md) beschrieben. Die JSON-API bleibt unter `/admin/api` und ist in [admin-backend.md](admin-backend.md) dokumentiert.
