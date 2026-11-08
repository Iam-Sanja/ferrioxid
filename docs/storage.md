# Speicher

**Stand:** v0.12, IST für lokalen Speicher. S3 ist geplant und nicht implementiert. Die Cargo-Version bleibt `0.1.0`.

## Verantwortlichkeit

`ferrioxid-storage` speichert Bytes unter einem Root, den die Anwendung festlegt. Der Client-Dateiname wird nie zum Pfad.

## Crates und Dateien

- Trait und Typen: `crates/ferrioxid-storage/src/lib.rs`
- Lokal: `crates/ferrioxid-storage/src/local.rs`
- HTTP, nur nach ausdrücklichem Einhängen: `crates/ferrioxid-storage/src/http.rs`

## Öffentliche API

`Storage` ist asynchron: `save`, `read`, `delete`, `exists`, `metadata`. `LocalStorage::open(root, max_bytes, allow)` legt den Root an. `detect_type` erkennt PNG, JPEG, GIF, WEBP, PDF und einfachen Text anhand der Bytes. `routes` liefert `POST /uploads`, `GET /uploads/{id}` und `DELETE /uploads/{id}`.

Die lokale Implementierung blockiert kurz in `std::fs`. Das ist eine bekannte Grenze, kein zweites Speicher-Laufzeitsystem.

## Datenfluss

`save` prüft Größe und Inhalt, erzeugt eine UUID und schreibt `{root}/{owner}/{id}.partial`. Erst danach wird die Datei exklusiv nach `{id}` verschoben. Metadaten liegen in `{id}.json` daneben. Ein Fehler löscht die unvollständige Datei. Es gibt keine neue SQL-Migration. Die bestehenden Checksummen bleiben gültig.

## Konfiguration

```toml
[storage]
backend = "local"
directory = "media"
max_upload_size_mb = 10
```

`backend` akzeptiert nur `local`. `directory` ist relativ ohne `..`. Die Größe liegt zwischen 1 und 100 MiB. `ferrioxid::web::prepare` setzt das Body-Limit mindestens auf diese Größe. myshop öffnet `examples/myshop/media` mit 1 MiB und der Allowlist `text/plain`, `image/png`, `image/jpeg`, `application/pdf`.

## Sicherheit

Pfade werden kanonisiert und müssen im Owner-Verzeichnis bleiben. `create_new` verhindert stilles Überschreiben. Der Originalname wird auf ASCII-Buchstaben, Ziffern, Punkt, Bindestrich und Unterstrich gekürzt. HTML, SVG, Skripte, `MZ` und `ELF` werden nicht als Text akzeptiert. Ein anderer Owner erhält 404, nicht 403. Downloads setzen `Content-Disposition: attachment`, `application/octet-stream` und `nosniff`.

## Fehlerfälle

`upload.too_large` ist 413. `upload.type` ist 415. `resource.not_found` ist 404. Speicherfehler sind `request.rejected` mit 500, ohne Pfad und ohne Systemtext.

## Beispiel

```rust
let stored = storage.save(user_id, "notes.txt", b"hello").await?;
let (_meta, bytes) = storage.read(user_id, stored.id).await?;
```

## Tests

`cargo test -p ferrioxid-storage` speichert, liest, löscht, isoliert Owner, lehnt Größe, HTML und Pfadnamen ab und prüft anonyme sowie fremde HTTP-Zugriffe.

## Einschränkungen

S3 ist nicht implementiert. Malware-Scanning fehlt. Die Inhaltserkennung ersetzt keinen Virenscanner. Metadaten sind Sidecar-JSON, keine Datenbanktabelle.

## Erweiterung

Ein weiteres `Storage`-Backend kann dieselben Methoden implementieren. Eine optionale Tabelle kann später über eine neue Migration entstehen. Bestehende `0001`-Dateien werden dafür nicht verändert.
