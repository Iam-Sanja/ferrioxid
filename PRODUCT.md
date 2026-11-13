# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primär sind Entwicklerinnen und Entwickler, die mit Fexu eine Anwendung bauen und den nativen Admin mitliefern. Die zweite, konkrete Nutzung ist Staff, das im Admin Datensätze pflegt. Menschen, die nur Fexu selbst weiterentwickeln, sind nicht die Hauptgruppe.

## Product Purpose

Fexu ist ein Rust-Framework, mit dem eine Anwendung Modelle, HTTP, Migrationen, Rechte und einen mitgelieferten Admin erhält. Erfolg heißt: ein generiertes oder von Hand registriertes Projekt lässt Staff die relevanten Datensätze ohne ein separates Frontend-Build pflegen, und Entwickler behalten Schema, Rechte und Routen im Rust-Code.

## Positioning

Ein Nachbarprodukt kann diese Position nicht übernehmen, ohne Fexu zu kopieren: Rust-Framework plus serverseitiger Admin (Axum, Tera), funktionsgleich zu den relevanten Aufgaben des Django-Admins, Kernaktionen ohne JavaScript. React, Vite und Node.js gehören nicht zum Standard-Admin. Die interne API muss nicht djangokompatibel sein.

## Operating Context

Entwicklung läuft über die `fexu`-CLI, `fexu.toml` und die Migrations-Engine. SQLite ist die lokale Datenbank; PostgreSQL ist das optionale Backend. Der Admin liegt unter `/admin` als HTML. Die JSON-API unter `/admin/api` bleibt. Das Integrationsbeispiel ist `examples/myshop`. Die Entwicklerdokumentation steht auf Englisch in `docs/`.

## Capabilities and Constraints

Bestätigt für spätere Arbeit:

- Kernaktionen des Admins, also Listen, Formulare, Löschen, Anmeldung und Abmeldung, funktionieren ohne JavaScript.
- React, Vite und Node.js bleiben aus dem Standard-Admin ausgeschlossen.
- SQLite ist der lokale Standard. PostgreSQL bleibt optional.
- `is_staff` öffnet den Admin. `is_superuser` ersetzt `is_staff` nicht. `is_active` bleibt erforderlich.
- Dokumentation und der Admin-Katalog bleiben Deutsch und Englisch. Öffentliche APIs, Feldnamen, CLI-Befehle und technische Fehlercodes bleiben Englisch.

Funktionsstand v0.27.1 lässt ein normales Projekt seine Handler in einem eigenen Worker-Binary ausführen. `fexu jobs inspect` zeigt die Payload nur mit `--show-payload`. Funktionsstand v0.27 stellt eine SQLite-Queue, einen eigenen Worker und begrenzte Wiederholungen bereit. Reset-Tokens bleiben außerhalb dieser Queue. Funktionsstand v0.26 lässt konkrete Models dieselben Felder wiederverwenden und verhindert einen ungeprüften Start installierter Apps. Soft Delete ist nicht enthalten. Funktionsstand v0.25 registriert eigenständige Apps über `App::install` auf derselben Engine. `fexu.toml` lädt dafür keinen Rust-Code. Ein Beispiel liegt in `examples/notes`. Funktionsstand v0.24 ist die Settings-, URL- und View-Schicht auf dem bestehenden Axum-Router und auf `fexu.toml`. Es gibt kein zweites Routing und kein Settings-Singleton. Funktionsstand v0.23 bleibt gültig. Anwendungen versenden E-Mail über `fexu::mail` und bieten einen Self-Service-Passwort-Reset unter `/auth/password-reset`. Der Standard bleibt `disabled` und öffnet keine SMTP-Verbindung. v0.22 bleibt gültig. Entwickler hängen eigene Admin-Seiten, Aktionen, Filter, Widgets und Vorlagen über `fexu::admin` an die vorhandene Registry. v0.21 bleibt gültig. Bestehende Projekte können auf ausstehende Auth- und App-Migrationen geprüft werden. `fexu upgrade` schreibt nur ein fehlendes `.fexu/project.json` und lässt Modelle, Vorlagen, Migrationen und Datenbanken unverändert. v0.20 bleibt gültig. Die Cargo-Version bleibt `0.1.0`. Formularentwürfe, Has-Many-Inlines, Sammelaktionen, die Änderungshistorie und echte Datums- und Zeittypen gehören dazu. Verzeichnis-Audit und opt-in Eltern-Kind-Formulare laufen in einer Transaktion.

Nicht vorhanden und nicht zu erfinden: OAuth, MFA, JWT, S3, Many-to-Many, `migrate down`, OpenAPI als ausgeliefertes Feature, ein Admin-Benachrichtigungscenter und eine garantierte E-Mail-Zustellung. Benutzer anlegen, E-Mail, Aktiv, Staff, Rollen und Passwort sind im nativen Admin umgesetzt. Superuser-Vergabe bleibt die CLI. Physisches Löschen von Benutzern gibt es nicht.

## Brand Commitments

Der Produktname ist Fexu. Die Entwicklerdokumentation spricht Englisch und trennt vorhandenen Funktionsumfang von Plänen. Es gibt keine bestätigte Wortmarke, kein verbindliches Logo und keine bestätigte visuelle Welt.

## Evidence on Hand

Vorhanden: `docs/admin-native.md`, `docs/roadmap.md`, `docs/auth.md`, `examples/myshop`, Workspace-Tests in `examples/myshop/tests/admin_html.rs`. Es gibt keine Kundenstimmen, Fallstudien oder Pressestimmen. Solche Belege dürfen nicht erfunden werden.

## Product Principles

- Der Admin ist Teil der Anwendung, die der Entwickler ausliefert, nicht ein nachträgliches Frontend.
- Funktionsgleichheit mit dem Django-Admin zählt. Code- und Template-Kompatibilität zählen nicht.
- Eine Kernaktion, die nur mit JavaScript geht, ist keine Kernaktion.
- Staff, Superuser und Aktiv bleiben getrennte Türen.
- Sichtbarer Text folgt der Sprache der Oberfläche. Identifikatoren in Code und API bleiben Englisch.
