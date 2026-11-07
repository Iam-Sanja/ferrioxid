# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primär sind Entwicklerinnen und Entwickler, die mit Ferrum eine Anwendung bauen und den nativen Admin mitliefern. Die zweite, konkrete Nutzung ist Staff, das im Admin Datensätze pflegt. Menschen, die nur Ferrum selbst weiterentwickeln, sind nicht die Hauptgruppe.

## Product Purpose

Ferrum ist ein Rust-Framework, mit dem eine Anwendung Modelle, HTTP, Migrationen, Rechte und einen mitgelieferten Admin erhält. Erfolg heißt: ein generiertes oder von Hand registriertes Projekt lässt Staff die relevanten Datensätze ohne ein separates Frontend-Build pflegen, und Entwickler behalten Schema, Rechte und Routen im Rust-Code.

## Positioning

Ein Nachbarprodukt kann diese Position nicht übernehmen, ohne Ferrum zu kopieren: Rust-Framework plus serverseitiger Admin (Axum, Tera), funktionsgleich zu den relevanten Aufgaben des Django-Admins, Kernaktionen ohne JavaScript. React, Vite und Node.js gehören nicht zum Standard-Admin. Die interne API muss nicht djangokompatibel sein.

## Operating Context

Entwicklung läuft über die `ferrum`-CLI, `ferrum.toml` und die Migrations-Engine. SQLite ist die lokale Datenbank; PostgreSQL ist das optionale Backend. Der Admin liegt unter `/admin` als HTML. Die JSON-API unter `/admin/api` bleibt. Das Integrationsbeispiel ist `examples/myshop`. Dokumentation steht auf Deutsch in `docs/`.

## Capabilities and Constraints

Bestätigt für spätere Arbeit:

- Kernaktionen des Admins, also Listen, Formulare, Löschen, Anmeldung und Abmeldung, funktionieren ohne JavaScript.
- React, Vite und Node.js bleiben aus dem Standard-Admin ausgeschlossen.
- SQLite ist der lokale Standard. PostgreSQL bleibt optional.
- `is_staff` öffnet den Admin. `is_superuser` ersetzt `is_staff` nicht. `is_active` bleibt erforderlich.
- Dokumentation und der Admin-Katalog bleiben Deutsch und Englisch. Öffentliche APIs, Feldnamen, CLI-Befehle und technische Fehlercodes bleiben Englisch.

Funktionsstand im Baum ist v0.18. Die Cargo-Version bleibt vorerst `0.1.0`. Formularentwürfe, Has-Many-Inlines, Sammelaktionen und die Änderungshistorie gehören dazu. Ein Datumstyp fehlt weiter.

Nicht vorhanden und nicht zu erfinden: OAuth, MFA, JWT, S3, Many-to-Many, `migrate down`, OpenAPI als ausgeliefertes Feature. Benutzer anlegen, E-Mail, Aktiv, Staff, Rollen und Passwort sind im nativen Admin umgesetzt. Superuser-Vergabe bleibt die CLI. Physisches Löschen von Benutzern gibt es nicht.

## Brand Commitments

Der Produktname ist Ferrum. Die Dokumentation spricht Deutsch und trennt vorhandenen Funktionsumfang von Plänen. Es gibt keine bestätigte Wortmarke, kein verbindliches Logo und keine bestätigte visuelle Welt.

## Evidence on Hand

Vorhanden: `docs/admin-native.md`, `docs/roadmap.md`, `docs/auth.md`, `examples/myshop`, Workspace-Tests in `examples/myshop/tests/admin_html.rs`. Es gibt keine Kundenstimmen, Fallstudien oder Pressestimmen. Solche Belege dürfen nicht erfunden werden.

## Product Principles

- Der Admin ist Teil der Anwendung, die der Entwickler ausliefert, nicht ein nachträgliches Frontend.
- Funktionsgleichheit mit dem Django-Admin zählt. Code- und Template-Kompatibilität zählen nicht.
- Eine Kernaktion, die nur mit JavaScript geht, ist keine Kernaktion.
- Staff, Superuser und Aktiv bleiben getrennte Türen.
- Sichtbarer Text folgt der Sprache der Oberfläche. Identifikatoren in Code und API bleiben Englisch.
