# Wechsel vom React-Admin

[Zur Übersicht](admin-native.md) · Routen: [admin-routing.md](admin-routing.md)

## Was sich ändert

Bis v0.12 lag die Oberfläche in `apps/admin-ui` und wurde mit Vite gebaut. v0.13 rendert dieselben Aufgaben in Rust. `apps/admin-ui` ist entfernt. `npm run build` und `FERRUM_ADMIN_DIST` gehören nicht mehr zum Build.

Der Browser spricht jetzt direkt mit HTML-Routen. Die JSON-API unter `/admin/api` bleibt für Programme und die bisherigen Tests. Sie ist keine Voraussetzung für die Oberfläche.

## Was gleich bleibt

- `AdminRegistry`, `admin = true` und die Feldmetadaten
- Sitzungen, Staff, RBAC, Objektregeln und Query-Scopes
- HMAC-CSRF der Sitzung für schreibende Aktionen
- `POST /auth/login` als JSON
- Datenbank, Migrationen und die REST-API der Modelle
- Branding aus `ferrum.toml`
- Deutsch und Englisch
- Das Cargo-Feature `admin-ui` als leeres Flag, damit bestehende `Cargo.toml`-Dateien weiter bauen

## Anmeldung

Die HTML-Anmeldung kann noch kein Sitzungs-CSRF senden, weil die Sitzung erst im Erfolgsfall entsteht. Dafür gibt es das kurzlebige Cookie `ferrioxid_login_csrf`. Es ersetzt das HMAC der Sitzung nicht. Nach dem Login gelten wieder `ferrioxid_session` und das Feld `csrf_token`.

## Build

Ein Fullstack-Projekt braucht nur Cargo:

```powershell
ferrioxid new myproject --template fullstack --ferrioxid-path D:\04_Projekte_Arbeit\Projekte\ferrioxid
cd myproject
ferrioxid migrate up
ferrioxid admin create-user --staff --superuser
ferrioxid dev
```

Die Startseite liegt unter `/`, der Admin unter `/admin/login`. Die API-Vorlage bindet den Admin nicht ein.

## Nicht übernommen

Die React-Komponenten, Vitest-Tests und Tailwind-Klassen gibt es nicht mehr. Die Prüfung der Oberfläche liegt in `examples/myshop/tests/admin_html.rs`. Sie spricht HTTP ohne JavaScript. Ein Browserlauf ist davon unabhängig und wird nur gezählt, wenn er tatsächlich stattgefunden hat.
