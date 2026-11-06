# OpenAPI und API-Verträge – Entwurf (NICHT implementiert)

[Zur Übersicht](index.md) · **Status:** ausschließlich SOLL. v0.1 nutzt `utoipa` nicht und liefert keine OpenAPI-Spezifikation.

## Ziel

Aus derselben Modelldefinition und den **tatsächlich** generierten HTTP-Routen eine korrekte OpenAPI-Beschreibung erstellen. Nicht bloß DB-Felder kopieren: Request, Response und Rechte können unterschiedliche Sichten besitzen.

## Geplanter Generator

1. Makro erzeugt DTO-Metadaten (`Create`, `Update`, `Response`) und Validierungsregeln.
2. Route Registry verknüpft Methode, Pfad, Request-/Response-Typ, Statuscodes und Policy.
3. OpenAPI-Schicht setzt diese Daten zu deterministischen Schemas zusammen.
4. Ausgabe wird bei Build-/Integrationstests mit tatsächlich registrierten Routen verglichen.

### Vertrag pro Endpoint

- Methode, Pfad, tags, summary (wenn vorhanden).
- Request-Body und Content Types.
- Erfolg: 200/201 und JSON-Schema.
- Fehler: 401/403/404/409/422/500 nach implementierter Semantik, keine erfundenen Statuscodes.
- Pagination-Schema nur dokumentieren, wenn implementiert.
- Security Schemes erst nach implementierter Auth definieren.

## Designregeln

- `required`/Min-/Max-Länge nachvollziehbar aus Feldmetadaten übernehmen.
- Hidden/Readonly-Felder im richtigen DTO ausblenden.
- Kein API-Consumer darf aus einem DB-Schema eine Schreibberechtigung ableiten.
- Versioniertes API-Schema, stabile Operation-IDs und deterministische Sortierung.

## Abnahmetests

OpenAPI-JSON lässt sich parsen/validieren; Beispielpayloads entsprechen Rust-DTOs; dokumentierte und registrierte Routen stimmen überein; verbotene Response-Felder erscheinen nicht. Quellen: [api.md](api.md), [validation.md](validation.md), [auth.md](auth.md).
