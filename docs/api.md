# ferrum-api – HTTP-Routing und Antwortverträge

[Zur Übersicht](index.md) · **IST-Datei:** `crates/ferrum-api/src/lib.rs`.

## Verantwortung (IST)

Die Crate stellt `App` als Builder für Registrierung, Entwicklungs-Bootstrap und Axum-Router bereit. Sie darf nicht versuchen, Datenbank-Models automatisch im Workspace zu entdecken.

```rust
let app = ferrum::App::new(db.clone()).register::<Product>();
app.bootstrap_schema().await?;  // separat, nur Demo/Entwicklung
let router = app.into_router();
```

### Registrierung

- `register<M: ModelRepository>(self)` liest `M::descriptor()`.
- `allow_anonymous == true`: GET `list::<M>`, POST `create::<M>`, sowie auf `/{id}` GET `get_one::<M>`, PATCH `update::<M>` und DELETE `delete_one::<M>`.
- `permissions = "authenticated"` oder ein Ressourcenname: Sitzung und Recht. GET prüft `read`, POST `create`, PATCH `update`, DELETE `delete`.
- `object_policy = true` ohne `register_policy`: 403. Mit `register_policy` prüft der Einzelabruf die Regel und antwortet bei Ablehnung mit 404. Die Liste bleibt für normale Benutzer 403.
- Sonst: dieselben Methoden über `denied`, jeweils 403.
- `with_auth` hängt `AuthService` an. `into_router` mountet `/auth/login`, `/auth/logout`, `/auth/me` und `/auth/change-password`.
- Bootstrap merkt sich Tabelle und `columns()`, unabhängig von der Freigabe, und rendert das DDL erst aus dem erkannten Datenbank-Backend.
- `into_router()` hängt `GET /healthz` an. Der Axum-State ist `AppState` mit Datenbank und optionalem Auth-Service.
- Cookie-Schreibzugriffe verlangen `x-csrf-token`. Das Token ist je Sitzung stabil, siehe [csrf.md](csrf.md). Anonyme Demo-Routen prüfen dieses Token nicht, weil sie das Session-Cookie nicht als Berechtigung verwenden.
- Ferrum-Fehler sind `application/problem+json` mit `type`, `title`, `status` und `detail`. 422 bleibt die Antwort für Validierung und unbekannte Sortier- oder Filterfelder. 409 ist reserviert und wird von den aktuellen Handlern nicht erzeugt. Axum-Rejections, zum Beispiel ungültiges JSON, behalten das Axum-Format und sind kein Ferrum-Problem-Detail. Datenbanktexte, Hashes, Tokens und Stacktraces gehen nicht an den Client.

### HTTP-API (IST)

| Route | Code | Response |
|---|---|---|
| `GET /healthz` | 200 | `{"status":"ok"}`; keine DB-Readiness-Prüfung |
| `GET /api/v1/products` | 200 | `{"data":[...],"limit":100,"truncated":false}` |
| `GET /api/v1/products/{id}` | 200 / 404 | typisiertes Model / Ferrum-Problem-JSON bei unbekannter UUID |
| `POST /api/v1/products` | 201 | erstelltes JSON-Model mit serverseitiger UUID |
| `PATCH /api/v1/products/{id}` | 200 / 404 / 422 | aktualisiertes Model, fehlende UUID oder Validierung |
| `DELETE /api/v1/products/{id}` | 204 / 404 / 422 | leerer Körper nur bei 204; fehlende UUID als Problem-JSON; Foreign-Key-`restrict` als 422 |
| nicht freigegebenes Model, alle diese Methoden | 403 | Ferrum-Problem-JSON |
| Validierungsfehler, Unique- oder Foreign-Key-Verletzung | 422 | Ferrum-Problem-JSON ohne SQL-Text |
| Datenbankfehler | 500 | generische Nachricht; Detail im `tracing`-Log |

Es existiert kein Auth-System, kein API-Versionierungsmodul, kein Filter und kein Offset/Cursor. Der `/api/v1`-Prefix stammt aus dem im Model hinterlegten Pfad. Eine ungültige UUID im Pfad beantwortet Axum selbst, nicht `ApiError`.

### Fehlervertrag (IST)

```json
{"type":"about:blank","title":"Validation failed","status":422,"detail":"name must not be blank"}
```

`ApiError` ist ein **lokaler Newtype** über `FerrumError`. Dies ist nötig wegen Rusts Orphan Rule: `IntoResponse` ist fremd und `FerrumError` gehört einer anderen Crate. Den Newtype nicht entfernen. `denied` nutzt dieselbe Fehlerform für GET, POST, PATCH und DELETE. HTTP 204 wird als `StatusCode::NO_CONTENT` ohne JSON-Körper zurückgegeben. **Ausnahme:** Fehler beim Dekodieren ungültigen JSONs entstehen in Axums `Json`-Extractor und behalten Axums eigene Rejection-Antwort.

### Sicherheitsgrenze (IST)

Wenn `allow_anonymous = true`, ist die Route ohne Authentifizierung benutzbar. Nur für lokal gebundene Demo-Prozesse. Für echte Anwendungen keine öffentlichen Standard-CRUD-Endpunkte ohne Auth- und Policy-Schicht.

## SOLL: API-Generator

1. Getrennte Response-DTOs, wenn Lese- und Schreibsicht nicht mehr dasselbe Model sein sollen.
2. Objekt-/Feld-Policies vor Datenzugriff und Antwortserialisierung.
3. Konfliktantworten wie HTTP 409 für Versionskonflikte. Der jetzige PATCH ist Last-Write-Wins in einer Anweisung.
4. Cursorbasierte Pagination mit stabiler Reihenfolge; Limits hart begrenzen.
5. Allowlists für sortier- und filterbare Felder.
6. Versionierung explizit deklarieren und nicht aus Pfadnamen erraten.
7. OpenAPI-Metadaten ohne doppelte manuelle Schemadefinitionen.
8. JSON-Fehlervertrag mit strukturierter Validierungsdetails und RFC-9457-Abgleich.

## API-Abnahmetests

- POST erfolgreich: 201, UUID, Datenwerte und Boolean-Default.
- GET `/{id}`: 200 bei existierender UUID, 404 bei nicht vorhandener UUID.
- PATCH nur `name`: Preis und `active` bleiben. `{}` verändert nichts. Leerer Name und negativer Preis ergeben 422. Unbekannte UUID ergibt 404.
- DELETE: 204 ohne Körper, danach GET und erneutes DELETE ergeben 404.
- Private Modelle: Liste, GET, PATCH und DELETE liefern 403 mit `title: Forbidden`.
- POST `name: ""` oder Blank: 422.
- GET 101+ Elemente: 100 Daten + `truncated: true`.
- Nicht freigegebenes Modell: 403 mit Problem-JSON.
- Fehler der DB: keine Geheimnisse in der Antwort.
- GET `/healthz` nicht als DB-Readiness-Test missverstehen.

Weiterlesen: [database.md](database.md), [validation.md](validation.md), [security.md](security.md).

## Admin

`App::with_admin` hängt `/admin/api` und `/admin` an denselben Router. Die öffentliche CRUD-API und ihr Limit von 100 bleiben unverändert. `allow_anonymous` gilt dort weiter und gilt nicht für den Admin. Details, Fehler und Tests: [admin-backend.md](admin-backend.md).
