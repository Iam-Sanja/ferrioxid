# Uploads

**Stand:** v0.12, IST für ausdrücklich registrierte Routen. Die Cargo-Version bleibt `0.1.0`.

## Verantwortlichkeit

Die Upload-Routen sind standardmäßig nicht montiert. Eine Anwendung ruft `ferrioxid::storage::routes` auf und hängt den Router mit `App::merge` ein. Ohne Auth-Service antworten die Routen mit 403.

## Crates und Dateien

- `crates/ferrioxid-storage/src/http.rs`
- Beispiel: `examples/myshop/src/main.rs`
- Speicherdetails: [storage.md](storage.md)

## Öffentliche API

`POST /uploads` nimmt entweder den rohen Body mit `x-original-name` und `x-csrf-token` oder `multipart/form-data` mit den Feldern `csrf_token` und `file`. Die JSON-Antwort enthält `id`, `original_name`, `content_type` und `size`. Sie enthält keinen Dateisystempfad. `GET /uploads/{id}` lädt nur die eigene Datei. `DELETE /uploads/{id}` verlangt Sitzung und CSRF-Header.

## Datenfluss

Der Body wird bis `max_bytes + 1` gelesen und bei Überschreitung verworfen. Der Inhalt bestimmt den Typ, nicht der Client-Header. Danach prüft `authenticate_csrf` die bestehende Sitzung. `save` schreibt die UUID-Datei. Download und Löschen filtern über die User-ID aus der Sitzung.

## Konfiguration

Die Größe kommt von `[storage]`. Die erlaubten Typen setzt der Aufrufer von `LocalStorage::open`. Sie sind keine zusätzlichen TOML-Schlüssel, damit eine Allowlist nicht versehentlich alles öffnet.

## Sicherheit

Deny-by-default. Authentifizierung ist erforderlich. Die Datei-ID ist eine UUID, kein Pfad. Fremde IDs sind 404. Löschen ohne CSRF ist 403. Unvollständige Dateien werden entfernt. Hochgeladene Dateien werden nicht ausgeführt und nicht als HTML oder SVG ausgeliefert. `media/` steht in der Projekt-`.gitignore`.

## Fehlerfälle

401 ohne Sitzung, 403 ohne CSRF oder ohne Auth-Service, 404 für fremde oder fehlende Dateien, 413 über dem Limit, 415 bei unerlaubtem Inhalt, 500 bei einem Speicherfehler ohne Systemdetail.

## Beispiel

```rust
let storage = ferrioxid::storage::LocalStorage::open("media", 1024 * 1024, &["text/plain"])?;
let app = app.merge(ferrioxid::storage::routes(std::sync::Arc::new(storage)));
```

## Tests

Der HTTP-Test legt zwei Benutzer in SQLite an, lädt eine Textdatei, verweigert den anonymen Zugriff, liefert einem zweiten Benutzer 404 und löscht nur mit dem CSRF-Token des Eigentümers.

## Einschränkungen

Es gibt keinen Virenscanner, keine Bildverarbeitung und keine öffentliche Upload-URL. Multipart erwartet einen quoted Boundary und höchstens wenige Teile.

## Erweiterung

Ein Virenscanner kann nach `detect_type` und vor `save` eingehängt werden. Er ist nicht enthalten.
