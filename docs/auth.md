# Authentifizierung & Autorisierung – Entwurf (NICHT implementiert)

[Zur Übersicht](index.md) · **Status:** ausschließlich SOLL; in v0.1 gibt es keine `ferrum-auth`-Crate.

## Ziel

Identitäten, sichere Sessions und durchgängige Autorisierungsentscheidungen für API **und** Admin. `allow_anonymous` aus v0.1 ist **kein Ersatz** für Auth und soll später durch explizite Zugriffspolicies ergänzt/abgelöst werden.

## Begriffe und Grenzen

- **Identity:** wer stellt die Anfrage?
- **Authentication:** wie wurde diese Identität geprüft?
- **Authorization:** darf diese Identität diese Ressource/Aktion/Feld nutzen?
- **Policy:** überprüfbare Serverregel für Zugriff (Modell, Objekt, Feld, Aktion).

## Soll-Komponenten

1. `User`/`Session`-Persistenz mit DB-Schema und Migrationen.
2. Passwort-Hashing mit Argon2id und gepflegter Parameterkonfiguration; keine Klartextspeicherung.
3. Login/Logout/Session-Rotation; serverseitige Sessions für Browser-Admin.
4. Sichere Cookies (`HttpOnly`, `Secure` über HTTPS, `SameSite` passend zur Topologie).
5. Rollen/Permission-Sets als Verwaltungsschicht; Policies als letztgültige Prüfung.
6. Objekt-/Feldberechtigungen vor Read, Write und Serialisierung.
7. Optional OAuth/OIDC nur mit expliziter Threat-Model-Erweiterung.

## Request-Pipeline (SOLL)

```text
Request → Session/Token prüfen → User/IdentityContext
 → ResourceAction bestimmen → Model-/Object-/Field-Policy
 → erlaubte DB-Abfrage/Mutation → Response-Filter → HTTP
```

**Deny by default** auf allen Stufen. 401 für nicht authentifizierte Anfragen in authentifizierungspflichtigen Kontexten, 403 bei fehlenden Rechten; Semantik und unnötige Informationslecks gesondert prüfen.

## Datenmodell und Verträge (SOLL)

- `Actor`/`Principal`-Kontext, der nicht aus untrusted Request-JSON stammt.
- `PolicyDecision { allow, reason? }` intern, ohne sensible Gründe an Clients auszugeben.
- `Action` (`list`, `read`, `create`, `update`, `delete`, ggf. custom).
- Feldprojektion pro Rolle und Aktion; readonly ist keine Berechtigung.
- Query-Scoping für List-Endpunkte, damit unautorisierte Datensätze nicht erst nachträglich gefiltert werden.

## Sicherheitsprüfungen

Cookie-/CSRF-Tests; Session-Fixation; Logout und abgelaufene Sessions; IDOR über Fremd-IDs; verbotene Schreibfelder; Rollenwechsel während einer Session; Rate Limiting und Login-Auditing. Keine Production-Freigabe ohne komplette Abnahme.
