# Sicherheit – Bedrohungsmodell und Sicherheitsgrenzen

[Zur Übersicht](index.md) · **IST + SOLL.** **Ferrum v0.1 ist nicht produktionsreif.**

## IST-Schutzmechanismen

- Ohne `allow_anonymous = true` werden GET, POST, PATCH und DELETE als 403 registriert. Der JSON-Körper ist derselbe `ApiError`-Vertrag.
- Create- und Update-Validierung prüft bekannte Felder gemäß Makroregeln. `deny_unknown_fields` verhindert, dass `id` oder `readonly`-Felder im JSON ankommen.
- INSERT, UPDATE und DELETE binden Werte über SeaORM. Die DELETE- und UPDATE-Anweisungen enthalten immer `WHERE "id" = $1`. Tabellen- und Spaltennamen stammen nur aus geprüften Makroattributen.
- `ApiError` loggt DB-Details intern und liefert bei 500 nur generische Informationen nach außen.
- Axums JSON-Extractor verwendet standardmäßig Serde-DTOs mit `deny_unknown_fields` auf dem Create-Typ.
- Demo-Bind ist `127.0.0.1:3000`; Docker-Postgres-Port ist auf `127.0.0.1` publiziert.

## Authentifizierung (IST, v0.7)

Benutzer, Argon2id, serverseitige Sitzungen, HttpOnly-Cookies, CSRF für Cookie-Schreibzugriffe, Rollen und Modellrechte sind implementiert. Details stehen in [auth.md](auth.md), [sessions.md](sessions.md), [permissions.md](permissions.md) und [policies.md](policies.md).

`allow_anonymous = true` bleibt eine ausdrückliche öffentliche Freigabe. Produkt und Kategorie in myshop sind solche Demo-Endpunkte. Geschützte Routen verweigern ohne Sitzung und Recht.

## Verbleibende Lücken (IST)

Kein OAuth, kein MFA, kein Passwort-Reset, kein JWT für externe APIs und kein Admin-UI. Das Login-Rate-Limit gilt pro Prozess. Objektregeln haben keinen SQL-Filter; ihre Listen sind blockiert. `allow_anonymous` prüft kein CSRF, weil der Zugriff nicht am Cookie hängt. Parallele PATCH-Anweisungen haben kein Versionsfeld. Ferrum ist damit nicht als öffentlicher Internetdienst ohne zusätzlichen Proxy und Betriebsgrenzen beschrieben.

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

## Admin

Der Admin prüft Sitzung, aktiven Benutzer, Staff, Modellrecht, Objektregel und bei Schreibzugriff CSRF. Pfadparameter werden nicht ungeprüft zu SQL-Bezeichnern. Sortierung, Suche und Filter sind Whitelists. Details und die bekannten Grenzen stehen in [admin-security.md](admin-security.md).
