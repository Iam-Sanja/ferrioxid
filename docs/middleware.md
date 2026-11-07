# Middleware

**Stand:** v0.12, IST. Die Cargo-Version bleibt `0.1.0`.

## Verantwortlichkeit

`ferrum-middleware` hängt Tower-Schichten an einen Axum-Router. Es gibt keine zweite Middleware-Laufzeit. Authentifizierung, Sitzungen und CSRF bleiben in den Handlern.

## Crates und Dateien

- `crates/ferrum-middleware/src/lib.rs`
- Anwendung: `ferrum::web::prepare` und `examples/myshop/src/main.rs`

## Öffentliche API

`Standard` trägt `body_limit`, Standard 2 MiB. `apply(router, standard)` hängt die Schichten an. `RequestId` liegt in den Request-Extensions. `logged_path` gibt nur den Pfad zurück. Die eingehende Kopfzeile `x-request-id` wird ignoriert. Die Antwort setzt eine neue ID.

## Datenfluss

Tower führt die zuletzt hinzugefügte Schicht zuerst aus. `apply` hängt deshalb in dieser Reihenfolge an: Sicherheitsheader, Access-Log, Request-ID, Body-Limit. Von außen nach innen gilt:

1. Body-Limit
2. Request-ID
3. Access-Log
4. Sicherheitsheader
5. Route, danach Auth, Sitzung und CSRF im Handler

## Konfiguration

`Standard { body_limit }` ist die einzige Option. `prepare` verwendet mindestens die konfigurierte Upload-Größe. Es gibt keinen TOML-Schalter, der die Sicherheitsheader abschaltet.

## Sicherheit

Das Log enthält Methode, Pfad, Status, Dauer und Request-ID. Query, Cookies, `Authorization`, CSRF, Passwörter und Multipart-Inhalte werden nicht geschrieben. Antwortheader sind `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY` und `Referrer-Policy: no-referrer`. Eine Content-Security-Policy wird nicht gesetzt, damit der Admin nicht bricht. Die Middleware ersetzt keine Berechtigungsprüfung.

Der Header `x-ferrum-trace-order` ist nur eine Testsonde. Er verändert die Route nicht.

## Fehlerfälle

Ein Body über dem Limit ergibt 413, bevor die Route läuft. Eine Request-ID, die nicht als Headerwert darstellbar wäre, wird nicht gesetzt. UUIDs sind darstellbar.

## Beispiel

```rust
let router = ferrum::middleware::apply(router, ferrum::middleware::Standard::default());
```

## Tests

`cargo test -p ferrum-middleware` prüft die Request-ID, die Sicherheitsheader, die Reihenfolge `request_id,access_log,security_headers`, das Body-Limit und dass Query und Cookie nicht im Antwortstrom des Log-Pfads stehen.

## Einschränkungen

Eigene Middleware wird mit den normalen Axum- und Tower-Schichten ergänzt. Eine deklarative Liste in `ferrum.toml` gibt es nicht. Logging schreibt über `tracing` und prüft den Text nicht ein zweites Mal.

## Erweiterung

Zusätzliche Schichten können vor oder nach `apply` gesetzt werden. Wer sie außen anlegt, sieht die Anfrage vor dem Body-Limit.
