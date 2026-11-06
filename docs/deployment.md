# Deployment und Betrieb

[Zur Übersicht](index.md) · **IST:** nur Docker Compose für PostgreSQL; **SOLL:** App-Container/Reverse-Proxy-Konzept.

## Entwicklung (IST)

`compose.yaml` startet `postgres:16-alpine` mit Datenbank, User, Volume und `pg_isready`-Healthcheck. Portfreigabe ist auf `127.0.0.1:5432:5432` beschränkt. Das Projekt stellt **keinen** Dockerfile für den Ferrum-API-Server bereit.

```bash
docker compose up -d db
cargo run -p myshop -- --bootstrap
cargo run -p myshop
```

Die Beispielanwendung läuft direkt auf dem Host und bindet standardmäßig an `127.0.0.1:3000`. Jede abweichende `BIND_ADDRESS`, insbesondere `0.0.0.0:3000`, kann einen Dienst auf zusätzlichen Netzwerken exponieren. Mit dem anonymen Demo-Modell ist das **nicht** für ein Internetdeployment geeignet.

## SOLL: Produktionsarchitektur

```text
TLS-Client → Reverse Proxy → Ferrum-App → PostgreSQL
                                  └→ Log-/Metrics-Ziel
```

1. Multi-Stage-Dockerfile: Rust-Build und schlankes Runtime-Image mit Non-Root-User.
2. Ein `ferrum`-App-Container, PostgreSQL nach Möglichkeit separat verwaltet.
3. Readiness-Endpoint mit Datenbankcheck; `/healthz` ist derzeit nur Prozess-Liveness.
4. Graceful Shutdown und sinnvolle Timeouts/Connection-Limits.
5. Forwarded-Header nur von konfigurierten Trusted Proxies auswerten.
6. HTTPS am Proxy; sichere Session-Cookies und CSRF-Schutz erst mit implementiertem Auth-System.
7. Secrets über Orchestrator/Environment, nie im Image oder Repo.
8. Migrationen als gesonderten Review-/Deploy-Schritt, niemals blind beim Start.
9. Kein produktives `allow_anonymous=true` für CRUD ohne explizites Security-Review.

## Docker + Portainer / Nginx Proxy Manager (SOLL)

Dokumentiere später ein nachvollziehbares Compose-Setup, bei dem die App nur in einem internen Docker-Netz erreichbar ist und Nginx Proxy Manager auf den Containernamen leitet. NPM-TLS, Proxy-Header, Websocket-Konfiguration (falls benötigt) und vertrauenswürdige Netzwerke als eigene Optionen dokumentieren. Diese Anleitung ist **kein** Hinweis darauf, dass eine solche Konfiguration bereits im Repository implementiert wurde.

## Abnahmekriterien

Image startet mit gültiger Konfiguration, stoppt sauber, Healthcheck erkennt Ausfälle, API ohne Auth bleibt nicht öffentlich, Secrets sind nicht im Log, Migrationen werden separat freigegeben und ein Netzwerktest bestätigt die beabsichtigte Exposition.
