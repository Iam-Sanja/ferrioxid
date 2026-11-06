# Konfiguration und Laufzeitumgebung

[Zur Übersicht](index.md) · **IST:** `examples/myshop/src/main.rs`, `.env.example`, `compose.yaml`.

## Tatsächliche Variablen (IST)

| Name | Einleser | Default | Verwendung |
|---|---|---|---|
| `DATABASE_URL` | `myshop/src/main.rs` | `postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum` | DB-Verbindung |
| `BIND_ADDRESS` | `myshop/src/main.rs` | `127.0.0.1:3000` | TCP-Listener |
| `POSTGRES_PASSWORD` | Docker Compose | `ferrum_dev_only` | Postgres-Container |

`.env.example` wird nicht automatisch eingelesen. Umgebungswerte müssen tatsächlich exportiert werden. Das Beispielpasswort ist **nur** für lokale Entwicklung bestimmt. `DATABASE_URL` samt Passwort niemals vollständig protokollieren.

## SOLL: Konfigurationsschema

- Zentraler, typisierter `FerrumConfig` (server, database, logs, security, admin).
- Reihenfolge der Quellen ausdrücklich definieren (z. B. Defaults → Datei → Env → CLI).
- Produktion: fehlende Secrets als Startfehler statt Demo-Defaults.
- Fehlkonfiguration mit Feldname, nicht mit Geheimnisinhalt melden.
- Profile `development`/`production` nicht implizit anhand bloßer Bind-Adresse erraten.
- Pro Wert dokumentieren: Typ, Default, Pflicht ab welcher Phase, Sicherheitswirkung, Beispiel.

## Abnahme

Test für Localhost-Default; Test für ungültige Bind-Adresse; keine API-Bind-Adresse mit `0.0.0.0` als produktionssicher ausgeben. Vergleiche [deployment.md](deployment.md) und [security.md](security.md).
