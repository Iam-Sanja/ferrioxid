# Sicherheit – Bedrohungsmodell und Sicherheitsgrenzen

[Zur Übersicht](index.md) · **IST + SOLL.** **Ferrum v0.1 ist nicht produktionsreif.**

## IST-Schutzmechanismen

- Ohne `allow_anonymous = true` werden GET, POST, PATCH und DELETE als 403 registriert. Der JSON-Körper ist derselbe `ApiError`-Vertrag.
- Create- und Update-Validierung prüft bekannte Felder gemäß Makroregeln. `deny_unknown_fields` verhindert, dass `id` oder `readonly`-Felder im JSON ankommen.
- INSERT, UPDATE und DELETE binden Werte über SeaORM. Die DELETE- und UPDATE-Anweisungen enthalten immer `WHERE "id" = $1`. Tabellen- und Spaltennamen stammen nur aus geprüften Makroattributen.
- `ApiError` loggt DB-Details intern und liefert bei 500 nur generische Informationen nach außen.
- Axums JSON-Extractor verwendet standardmäßig Serde-DTOs mit `deny_unknown_fields` auf dem Create-Typ.
- Demo-Bind ist `127.0.0.1:3000`; Docker-Postgres-Port ist auf `127.0.0.1` publiziert.

## Explizite Lücken (IST)

**Keine** Benutzeridentitäten, Passwort-Hashes, Sessions, JWTs, CSRF, RBAC, Objekt-/Feldrechte, Ratelimits, sichere Produktionsdefaults oder Admin-UI. `allow_anonymous = true` erlaubt jedem erreichenden Client, Daten zu lesen, anzulegen, zu ändern und zu löschen. Das ist keine Auth-Lösung. Kein sicherer Internetbetrieb. Parallele PATCH-Anweisungen haben kein Versionsfeld.

## SOLL-Bedrohungsmodell

| Bedrohung | Zielmaßnahme | Nachweis |
|---|---|---|
| Unberechtigte Reads/Writes | Deny by default + serverseitige Policies | 401/403-Tests |
| IDOR | Objekt-Policy **nach** Identitätsprüfung für jede einzelne ID | Querzugriffstests |
| Mass Assignment | geteilte DB-Models nie blind als Writable DTO verwenden | Felder-Ausblendtests |
| CSRF | Schutz bei Cookie-basierten Zustandsänderungen | Cross-Origin-Tests |
| SQL Injection | gebundene Werte, Identifier-Allowlist | fuzzer-/negativ-Tests |
| Brute Force | Ratelimit, Lockout nach Risiko, Auditing | Login-Stresstests |
| Session-Diebstahl | HttpOnly/Secure/SameSite, Rotation, Ablauf | Session-Tests |
| Secrets in Logs | Redaction, generische API-Fehler | Log-Audit |
| Öffentliches Demo | Bind- und Konfigurationschecks, kein Prod-Default | Deployment-Test |

## Umsetzungsvorgaben (SOLL)

- `auth` und Policies sind getrennte Konzepte: Authentifizierung stellt Identität fest, Autorisierung entscheidet Zugriff.
- RBAC allein genügt nicht für Object-Level/Field-Level-Policies.
- Standardmäßig kein anonymes CRUD und keine automatisch exportierten sensiblen Felder.
- React-UI darf Berechtigungen nur visualisieren; die Serverprüfung bleibt maßgeblich.
- Auth-Kontext darf nicht über Clientfelder oder Header mit ungeprüfter Vertrauenskette entstehen.
- Vor jedem produktiven Release Security-Review, Dependency-Audit und dokumentierte Threat-Model-Annahmen.

**Meldeweg:** `SECURITY.md` im Repository enthält derzeit nur eine knappe Projektinformation, keinen etablierten Vulnerability-Disclosure-Prozess. Für öffentliche Releases muss der gepflegt werden.
