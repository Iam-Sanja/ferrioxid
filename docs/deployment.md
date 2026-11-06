# Deployment und Betrieb

[Zur Übersicht](index.md) · **IST:** lokaler Prozess mit SQLite. Docker ist optional und startet nur PostgreSQL. **SOLL:** App-Container und Reverse Proxy.

## Lokale Entwicklung (IST)

SQLite ist die Standarddatenbank. Rust und Cargo reichen aus. Docker und PostgreSQL müssen nicht installiert sein.

```powershell
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

Der Server startet ohne automatische Migration. Zuerst `ferrum migrate up` ausführen. `myshop --bootstrap` ändert die Datei nicht. Die Datei `db.sqlite3` liegt im Arbeitsverzeichnis. Eine ältere Bootstrap-Datei ohne `_ferrum_migrations` wird nicht übernommen und nicht gelöscht.

Eine abweichende `BIND_ADDRESS`, insbesondere `0.0.0.0:3000`, kann den Dienst auf weiteren Netzwerken erreichbar machen. Mit dem anonymen Demo-Modell ist das kein Internetdeployment.

## Optionales PostgreSQL (IST)

`compose.yaml` startet `postgres:16-alpine` mit Datenbank, User, Volume und `pg_isready`. Der Port ist auf `127.0.0.1:5432:5432` beschränkt. Das Repository enthält kein Dockerfile für den API-Server.

```powershell
docker compose up -d db
$env:DATABASE_URL="postgres://ferrum:ferrum_dev_only@127.0.0.1:5432/ferrum"
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

Dieselbe registrierte Anwendung spricht danach PostgreSQL. Die Handler bleiben unverändert. Docker ist ein optionales Werkzeug für diesen Server, nicht die Voraussetzung für Ferrum.

## Produktionsarchitektur (SOLL, nicht implementiert)

```text
TLS-Client → Reverse Proxy → Ferrum-App → PostgreSQL
                                  └→ Log-/Metrics-Ziel
```

1. Multi-Stage-Image mit Non-Root-User.
2. Ein App-Container und ein getrennt verwaltetes PostgreSQL.
3. Readiness inklusive Datenbank. `/healthz` prüft derzeit nur, dass der Prozess antwortet.
4. Graceful Shutdown, Timeouts und einstellbare Connection-Limits.
5. Forwarded-Header nur von vertrauenswürdigen Proxies.
6. HTTPS am Proxy. Sessions und CSRF erst zusammen mit Authentifizierung.
7. Secrets über die Einsatzumgebung, nicht im Image oder Repository.
8. Schemaänderungen bleiben ein eigener Schritt über `ferrum migrate up`, nicht beim Prozessstart. `down`, Rename und Tabellen-Neubau sind nicht Teil dieses Schritts.
9. Kein produktives `allow_anonymous=true` ohne Sicherheitsreview.

SQLite kann lokale Entwicklung und Tests tragen. Für mehrere App-Prozesse ist es nicht die vorgesehene Produktionsdatenbank: der Pool hat eine Verbindung, und Schreibzugriffe serialisieren sich über die Datei. PostgreSQL bleibt dafür das unterstützte Server-Backend, sobald es betrieben wird.

## Docker, Portainer und Nginx Proxy Manager (SOLL)

Ein internes Compose-Netz und Nginx Proxy Manager sind nicht implementiert. Eine spätere Anleitung muss TLS, Proxy-Header und die beabsichtigte Erreichbarkeit einzeln beschreiben.

## Abnahme

Der lokale SQLite-Start braucht keinen Datenbankserver. Ein optionales PostgreSQL-Setup bleibt auf localhost beschränkt. Secrets erscheinen nicht im Log. Es gibt keinen App-Container und keine automatische Migration.
