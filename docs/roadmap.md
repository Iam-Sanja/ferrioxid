# Ferrum Roadmap – Status und nächste Phasen

[Zur Übersicht](index.md) · Admin-Vergleich: [admin-django-parity.md](admin-django-parity.md) · UX-Backlog: [admin-ux-backlog.md](admin-ux-backlog.md)

**Stand:** Funktionsstand v0.27.1 ergänzt Anwendungs-Worker. Funktionsstand v0.27 ergänzt persistente Jobs. Funktionsstand v0.26 ergänzt Modellfelder und den geprüften App-Start. Funktionsstand v0.25 bleibt gültig. Funktionsstand v0.24 bleibt gültig. Funktionsstand v0.23 bleibt gültig. Die Cargo-Version bleibt `0.1.0`. Dieser Text trennt umgesetzte Meilensteine von geplanten Phasen. Eine geplante Phase ist keine Freigabe und kein fertiges Feature.

## Produktentscheidung

Der Standard-Admin bleibt serverseitiges HTML mit Rust, Axum und Tera. React, Vite und Node.js gehören nicht zum Standard-Admin. Kernaktionen, also Listen, Formulare, Löschen, Anmeldung, Abmeldung und Verzeichnis, müssen ohne JavaScript funktionieren. Das optionale `admin.js` darf nur Komfort bieten, zum Beispiel Fokus.

Langfristig soll der Admin die relevanten Funktionen des Django-Admins funktionsgleichwertig abdecken. Die interne API muss nicht djangokompatibel sein. Die Oberfläche soll eine dichte Arbeitsumgebung werden, keine dekorative Neugestaltung.

## Umgesetzte Meilensteine

| Meilenstein | Status | Ziel | Grenze |
|---|---|---|---|
| v0.1 | Implementiert, SQLite im Workspace getestet | Typed Model, Liste, GET, POST, PATCH, DELETE, sichere Migrationen | PostgreSQL-HTTP-Test bleibt ignoriert, solange kein Server läuft |
| v0.6 | Implementiert | Unique, Indizes, Foreign Keys, Belongs-To, Has-Many, Snapshot und Diff | Many-to-Many, Eager Loading, `down` und PostgreSQL-Baseline fehlen |
| v0.7 | Implementiert | Argon2id, Sitzungen, Cookies, CSRF, RBAC, Modellrechte | OAuth, MFA und JWT fehlen. Passwort-Reset folgt in v0.23 |
| v0.8 bis v0.12 | Implementiert | Admin-Metadaten, JSON-API, Query-Scopes, Generator, Settings, Formulare, Staticfiles, Uploads, Middleware | Historische React-Oberfläche ist seit v0.13 entfernt |
| v0.13 | Implementiert | Nativer Admin unter `/admin`, Tera, CSS im Binary, JSON-API bleibt | Cargo-Version bleibt `0.1.0` |
| v0.13.1 | Implementiert | Login-Origins aus der Bind-Adresse, `Origin: null` nur mit `Sec-Fetch-Site: same-origin` | Host-Header wird nicht als Allowlist gelesen |
| v0.14 | Implementiert | Abmelden auf authentifizierten 403-Seiten, Flag-Hinweise, Standardspalten | Benutzer anlegen bleibt v0.16 |
| v0.15 | Implementiert | Design-Tokens, Feldgruppen, Tabellenrhythmus, Rollenauswahl, Kontrasttor | Cargo-Version bleibt `0.1.0` |
| v0.16 | Implementiert | Benutzerpflege, serverseitige Bestätigung, lesbare Fremdschlüssel, Filterauswahl, isolierte Generator-Datenbanken | Superuser bleibt CLI. Kein Benutzer-Hard-Delete. JSON-API ohne `confirm` |
| v0.17 | Implementiert | Listenkonfiguration, Fieldsets, drei Speicherziele, Formularzustand, Fremdschlüssel-Auswahlseite | Kein Datumstyp. Cargo-Version bleibt `0.1.0` |
| v0.18 | Implementiert | Formularentwürfe, Has-Many-Inlines, Sammelaktionen, Audit, registrierte Aktionen | Kein gemeinsames Parent-Kind-Speichern. Verzeichnis-Audit ist nicht dieselbe Transaktion |
| v0.19 | Abgeschlossen | `Date`, `Time`, `DateTime<Utc>`, Migration `0004_events`, Admin-Felder, UTC-Datumshierarchie | PostgreSQL-Laufzeit ungetestet. Keine stillschweigende Text-zu-Datum-Umwandlung |
| v0.20 | Abgeschlossen | Gemeinsame Transaktion für Modell, Auth und Audit. Verzeichnis-Audit. Opt-in Eltern-Kind-Formular | Kein Many-to-Many. Kein frameworkweites Optimistic Locking. PostgreSQL-Laufzeit ungetestet |
| v0.21 | Abgeschlossen | Lesende Auth- und App-Diagnose. `check --strict`. Upgrade legt nur ein fehlendes Legacy-Manifest an | Keine automatische Vorlagenersetzung. Keine stillen Migrationen. PostgreSQL-Laufzeit ungetestet |
| v0.21.1 | Abgeschlossen | Generator-Test begrenzt verschachtelte Release-Builds auf zwei Jobs. Uneingeschränkter Workspace-Test danach grün | rustc-Absturz `0xc0000005` und `0xc0000409` ist nicht ursächlich geklärt |
| v0.22 | Abgeschlossen | Öffentliche Admin-Erweiterung: Seiten, Aktionen, Filter, Widgets, Vorlagen | Keine zweite Admin-Engine. PostgreSQL-Laufzeit ungetestet. Der rustc-Absturz bleibt ungeklärt |
| v0.23 | Abgeschlossen | E-Mail-Dienst, Self-Service-Passwort-Reset, typisierte Hinweise | Keine Zustellgarantie. Kein Admin-Benachrichtigungscenter. PostgreSQL-Laufzeit ungetestet |
| v0.24 | Abgeschlossen | Typisierte Settings, `[custom.*]`, `UrlSet`, View-Helfer, Generatorstruktur | `require_user` prüft kein CSRF. API-Vorlagen behalten HTML-Startseiten. PostgreSQL-Laufzeit ungetestet |
| v0.25 | Abgeschlossen | `App::install`, App-Migrationen, Tera-`url`, App-Vorlagen und Staticfiles, `ferrum generate app` | `merge` eines rohen Routers sieht keine Namen. PostgreSQL-Laufzeit ungetestet |
| v0.27 | Abgeschlossen | Persistente Queue, Lease, begrenzte Retries, Worker, gewöhnliche Mail-Jobs | Cron fehlt. Reset-Tokens bleiben außerhalb der Queue. PostgreSQL-Laufzeit ungetestet |
| v0.27.1 | Abgeschlossen | `WorkerHost`, `ferrum generate worker`, App-Handler, verdeckte Job-Payload | CLI-Worker lädt keinen Anwendungscode. Exactly once fehlt. PostgreSQL-Laufzeit ungetestet |
| v0.26 | Abgeschlossen | Geprüfter App-Start, Abgleich von Rust- und TOML-Migrationen, flache Feldgruppen, verwaltete Zeitstempel | Soft Delete fehlt. PostgreSQL-Laufzeit ungetestet |

## Offen nach v0.27.1

- Cron-Syntax.
- Soft Delete.
- PostgreSQL-Laufzeit gegen einen laufenden Server.
- Der CLI-Worker führt nur den Mail-Handler aus. Anwendungs-Handler bleiben im Projekt-Binary.

## Offen nach v0.26

- Soft Delete, einschließlich Abfragen, Restore, Audit und Fremdschlüsseln.
- `merge` eines rohen Routers sieht keine Routennamen.
- Die CLI führt App-Code nicht aus. Den Abgleich macht der Prozessstart.

## Offen nach v0.25

- `views::fail` nutzt eine eigene HTML-Seite und nicht `templates/errors/`.
- `require_user` authentifiziert weiterhin mit GET. Mutationen nutzen `require_mutation`.
- Seit v0.26 vergleicht der Prozessstart beide Listen. Die CLI bleibt bei `ferrum.toml`.
- `ferrum new` erzeugt noch kein `src/apps/`.

## Offen nach v0.24

- `views::fail` nutzt eine eigene HTML-Seite und nicht `templates/errors/`.
- `require_user` authentifiziert mit GET und prüft dabei kein CSRF.
- Die API-Vorlage schreibt weiterhin `templates/index.html`, weil die Startseite aktiv bleibt.

## Offen nach v0.23

- Eine angenommene SMTP-Antwort beweist den Posteingang nicht. Ein Prozessabbruch kann einen bereits angenommenen Token beim Aufräumen entwerten.
- `AccountDeactivated` und `SecurityNotice` hängen nicht an der Benutzerpflege.
- Das Reset-Rate-Limit bleibt prozesslokal.
- Die JSON-Verzeichnis-API verlangt `confirm` nicht.
- Ein Nicht-Superuser kann eine fehlende Fremdschlüsselzeile nicht von einer unsichtbaren unterscheiden.
- Filterauswahlen und das Formular-Select zeigen höchstens 100 berechtigte Ziele. Die Auswahlseite blättert darüber hinaus.
- Abbrechen löscht einen Formularentwurf nicht sofort. Er läuft nach 30 Minuten ab.
- Der Formular-Fingerabdruck ist kein versionsstabiler Datenbankwert.
- Weitere Kind-Fremdschlüssel werden in der Inline-Transaktion nicht aufgelöst.

## Geplante Phasen

Die Reihenfolge ist verbindlich für die nächste Arbeit. Eine spätere Phase beginnt nicht, bevor die vorherige P0-Abnahme erfüllt ist.

| Phase | Priorität | Inhalt | Hängt ab von |
|---|---|---|---|
| v0.15 | umgesetzt | Design-Tokens, Abstände, Formulargruppen, Auswahl statt Freitext für Rollen | v0.14 |
| v0.16 | umgesetzt | Benutzer anlegen, E-Mail und Passwort ändern, Sitzungen widerrufen, Bestätigung und lesbare Relationen | v0.15 |
| v0.17 | umgesetzt | Listenlinks, Standardsortierung, Seitengröße, Fieldsets, Speicherziele, Formularzustand, Auswahlseite | [admin-django-parity.md](admin-django-parity.md) |
| v0.18 | umgesetzt | Entwürfe, Has-Many-Inlines, atomare Sammelaktionen, Audit, `AdminAction` | [admin-bulk-actions.md](admin-bulk-actions.md) |
| v0.19 | abgeschlossen | Datum, Uhrzeit, UTC-Zeitpunkt, Datumshierarchie | [date-time-types.md](date-time-types.md) |
| v0.20 | abgeschlossen | Transaktionen, Verzeichnis-Audit, Eltern-Kind-Formulare | [transactions.md](transactions.md) |
| v0.21 | abgeschlossen | Migrationsdiagnose und sicheres Projektmanifest | [migration-diagnostics.md](migration-diagnostics.md) |
| v0.22 | abgeschlossen | Registrierte Admin-Seiten, Aktionen, Filter, Widgets und Vorlagen | [admin-extension-api.md](admin-extension-api.md) |
| v0.23 | abgeschlossen | E-Mail, Passwort-Reset und typisierte Hinweise | [password-reset.md](password-reset.md) |
| v0.24 | abgeschlossen | Settings, benannte URLs, Views und Projektstruktur | [settings.md](settings.md) |

Nicht in diesen Phasen: React-Rückkehr, OAuth, MFA, JWT, S3, Virenscanner, Many-to-Many, `migrate down`, PostgreSQL-Baseline, OpenAPI, automatische Superuser-Semantik.

## Grenzen, die bleiben

- Cargo-Version `0.1.0`. Funktionsstände stehen nur im Changelog und in dieser Roadmap.
- SQLite ist der lokale Standard. PostgreSQL-Tests bleiben ignoriert, bis ein Server erreichbar ist.
- `0001_initial.json` und die Auth-Migration `0001_auth_schema` werden nicht umgeschrieben.
- GET ändert keine Daten. Schreibzugriffe sind POST mit CSRF, außer die bestehende JSON-API, die den Header `x-csrf-token` nutzt.
- Passwort-Hashes, Session-Tokens und CSRF-Tokens erscheinen nicht in HTML, URLs oder Logs.
- Skills werden vor der Nutzung gelesen. Impeccable ist für v0.15, v0.16 und die Formularabschnitte in v0.17 gelesen worden. Kein Skill, Hook oder Agent committet oder pusht von sich aus.

## Skills für spätere Phasen

Geprüft am 2026-11-07: Impeccable liegt unter `.cursor/skills/impeccable/` und wurde für v0.15 gelesen. Ausgeführt wurden Kontext, Critique (zwei getrennte Assessments), Audit, Layout und der Feinschliff am bestehenden System „Die Nachtschicht“. Es wurde nichts nachinstalliert und nichts committet.

| Skill | Einsatz, erst nach Prüfung | Nicht tun |
|---|---|---|
| Impeccable | Designkritik, Audit, Layout und Feinschliff des nativen Admins in v0.15 | Externe Installation ohne Zustimmung, dekorative Umbauten ohne Funktionsgewinn |
| Cloudflare Security Audit | Gezielte Prüfung von CSRF, XSS, Sessions und Dateizugriff vor v0.16 und v0.18 | Blindes Übernehmen von Befunden ohne Abgleich mit dem Ferrum-Code |
| ub-mode | Architekturgrenzen und Tests für Verzeichnis und Audit | Neue Frameworks oder eine zweite CRUD-Engine |
| Ponytail | Komplexität in `html.rs` und den Metadaten senken, wenn eine Phase sonst wächst | Verhalten streichen, das die Abnahme verlangt |

## Umgesetzt in v0.16

1. Benutzer anlegen, anzeigen, bearbeiten und Passwort setzen über eigene HTML-Routen und `AuthService`.
2. `confirm=1` wird serverseitig geprüft. Superuser und die eigene Sperre bleiben geschützt.
3. Fremdschlüssel zeigen einen Metadaten-Namen. Bool- und Fremdschlüssel-Filter sind Auswahlen.
4. Generator-Kindprozesse überschreiben `DATABASE_URL` lokal und räumen die Testdatei auf.

## Umgesetzt in v0.17

1. `list_link`, `admin_ordering` und `admin_per_page` steuern Link, Standardsortierung und HTML-Seitengröße.
2. `admin_fieldset` gruppiert Formularfelder. Drei Speicherknöpfe unterscheiden `save=detail`, `save=edit` und `save=new`.
3. Validierungsfehler behalten Text, Auswahl und Häkchen. Passwörter werden nicht zurückgeschrieben.
4. Die Fremdschlüssel-Auswahlseite sucht und blättert im Leserecht des Ziels. Eine unlesbare UUID wird nicht gespeichert.

## Umgesetzt in v0.18

1. Ein Sitzungsentwurf hält Text, Zahl, Häkchen und Auswahl, während die Fremdschlüssel-Seite offen ist.
2. `admin_inline` zeigt verknüpfte Zeilen und legt einen Kinddatensatz mit festem Parent-Schlüssel an.
3. Sammelaktionen sind POST, Bestätigung und eine Transaktion. Eine verweigerte Zeile ändert keine Zeile.
4. Modelländerungen und ihr Audit-Eintrag teilen sich eine Transaktion. Die Historie ist für Staff auf eigene Ereignisse begrenzt.

Abnahme und Grenzen stehen in [admin-ux-backlog.md](admin-ux-backlog.md).
