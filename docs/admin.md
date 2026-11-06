# Admin-System – Backend und React-UI (NICHT implementiert)

[Zur Übersicht](index.md) · **Status:** ausschließlich SOLL. Es existieren weder `ferrum-admin` noch `apps/admin-ui` im aktuellen Workspace.

## Leitprinzip

Eine Admin-Anwendung für beliebig viele Modelle, statt generierter React-Projektkopien pro Modell. Das Backend gibt ausschließlich **für den angemeldeten Actor berechtigte** Metadaten und Daten zurück. Frontend-Validierung unterstützt die Benutzerführung, ist aber nie Sicherheitsgrenze.

## Zielarchitektur

```text
Rust Models → Field-/UI-Metadaten + Policies
     → Admin Schema API (nur freigegebene Felder/Aktionen)
     → React Admin Renderer
           ├─ Navigation
           ├─ List mit Filter/Sort/Pagination
           ├─ Create/Edit Form
           └─ Detailansicht/Custom Widgets
```

### 1. Admin-Backend (SOLL)

- Ressourcenregistrierung referenziert dieselbe Model Registry wie API/Migrations.
- Auth verpflichtend; ohne Session kein Modellkatalog.
- Beispielkontrakt (nur Entwurf): `GET /admin/schema` (verfügbare Ressourcen), `GET /admin/schema/products` (erlaubte Felder und Actions).
- Datenrouten serverseitig authorisieren, Payloads strikt validieren.
- Keine technischen DB-Spalten oder verborgenen Felder allein wegen Model-Sichtbarkeit ausgeben.
- Schema-Version im Response für UI-Kompatibilität.

### 2. Admin-Frontend (SOLL)

- React + TypeScript + Vite; modulare UI-Layer: API-Client, Auth-State, Resource Registry, Tabellenrenderer, Formularrenderer, Custom Components.
- Routing über zulässige Ressourcen, nicht via frei eingebbare Tabellenamen.
- Typbasierte Widgets für `String`, `i64`, `bool`, `Uuid`; weitere Typen nach Model-Erweiterung.
- Such- und Filter-UI nur für serverseitig zugelassene Felder.
- Fehler-, Leer-, Lade- und Forbidden-Zustände; Tastaturbedienung und responsive Layouts.

### 3. Erweiterungspunkte (SOLL)

- `AdminResourceConfig` (Anzeige-Label, Listenfelder, Suchfelder, Filters, Edit-Rechte).
- Custom Form Field Renderer mit expliziter Registrierung.
- Custom Action mit eigener Auth-Policy und Audit-Hinweisen.
- Detailansichten/Widgets; React-Komponenten werden **nicht** aus dem Makro kompiliert.

## Sicherheitsanforderungen

Keine UI-only-Policies; vor jedem List-, Create-, Update-, Delete-Aufruf dieselbe Server-Policy wie REST-API. CSRF bei Session-Cookies; XSS-Schutz durch korrektes Escaping und sicheren Content-Flow; kein blindes HTML aus DB-Feldern; Audit Trails später separat versionieren.

## Umsetzungsphasen und Abnahme (SOLL)

1. Reiche Field-Metadaten und Auth/Policies stabilisieren.
2. Geschützte Schema-API mit klar versioniertem JSON definieren und testen.
3. Statisches React-Dashboard, API-Client und Sessions.
4. Tabellen, Formulare und CRUD mit Feld-Policy-Tests.
5. Custom Widgets als optionale Erweiterung.

Abnahme: Zwei registrierte Models erscheinen ausschließlich für berechtigte Nutzer; Create/Edit funktioniert ohne handgeschriebene Formulare; ein nicht berechtigtes Feld ist auch über direkte HTTP-Anfrage nicht änderbar.
