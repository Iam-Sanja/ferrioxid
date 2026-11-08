# Worker

[Zur Übersicht](index.md) · Jobs: [jobs.md](jobs.md) · Sicherheit: [job-security.md](job-security.md)

**IST.** Funktionsstand v0.27.1. `ferrioxid worker` startet keinen Webserver und führt nur den Mail-Handler aus. `ferrioxid dev` startet keinen Worker. Eine installierte App führt durch `App::install` keine Jobs aus. Anwendungs-Handler starten über `WorkerHost` im Projekt-Binary.

## Öffentliche Start-API

```rust
let project = ferrioxid::migrations::discover_project(&root)?;
let mut host = ferrioxid::jobs::WorkerHost::from_project(&project)
    .with_mail(ferrioxid::mail_for(&project));
NotesApp.register_jobs(host.registry_mut())?;
host.run_until_ctrl_c()?;
```

`WorkerHost::new` nimmt eine `WorkerConfig`. `from_project` liest `[jobs]` und die aufgelöste Datenbank-URL. `register` und `registry_mut` tragen Handler in dieselbe `JobRegistry` ein. Ein zweiter Handler mit demselben `NAME` liefert `job.handler`. `with_mail` ergänzt `ferrioxid_mail_notice`, wenn dieser Name noch fehlt. `prepare` prüft die SQLite-Datei und `_ferrum_jobs` und gibt den vorhandenen `Worker` zurück. `run_until_ctrl_c` legt die Tokio-Laufzeit an und beendet die Schleife nach Ctrl+C.

Die API migriert nicht und öffnet keinen Router. Fehlt die SQLite-Datei, entsteht `job.database` und es wird keine Datei angelegt. Fehlt die Tabelle, entsteht `job.migration.missing`. `jobs.enabled = false` ergibt `job.disabled`.

SQLite nutzt den vorhandenen Pool mit einer Verbindung. `effective_concurrency` ist dann 1, auch wenn `[jobs].concurrency` höher steht. Bei PostgreSQL bleibt die Obergrenze 4. Die Schleife beansprucht diese Anzahl nacheinander auf derselben Verbindung. Gleichzeitige Worker sind getrennte Prozesse.

`ferrioxid generate worker` schreibt `src/bin/worker.rs`. Die Datei enthält ein kompilierbares Beispiel `app_example`, das durch `register_jobs` der eigenen App ersetzt wird. Eine vorhandene Datei wird nicht überschrieben. Windows und Linux nutzen denselben Quelltext und `tokio::signal::ctrl_c`. Ein Worker kann auch von Hand auf `WorkerHost` aufsetzen, ohne den Generator.

## Start

```powershell
ferrioxid worker
ferrioxid worker --concurrency 2 --poll-ms 500
```

Der CLI-Worker registriert nur `ferrioxid_mail_notice`. Handler einer App starten `src/bin/worker.rs`. Die CLI lädt dafür keinen Rust-Code und keine unbekannte Crate.

Fehlt die SQLite-Datei, wird sie nicht angelegt. Fehlt `_ferrum_jobs`, endet der Prozess mit Exitcode 2 und `job.migration.missing`. `jobs.enabled = false` beendet den Worker ebenfalls mit Exitcode 2. Ein sauberes Stoppsignal endet mit 0. Ein Datenbankfehler endet mit 1.

Ctrl+C setzt auf Windows und Linux ein Stopp-Flag. Der Worker beansprucht danach keinen neuen Job und beendet die aktuelle Ausführung. Ein Handler wird dadurch nicht mitten in einem externen Effekt zurückgerollt.

## Lease

Der Claim ist ein bedingtes `UPDATE`. Die neue Zeile erhält `lease_owner`, ein neues `lease_token` und `lease_expires_at`. Nur das Update, das genau eine Zeile trifft, besitzt den Job. Ein zweiter Worker erhält dieselbe Zeile erst, wenn die Lease abgelaufen ist. Dann steigt `attempts`.

Der Abschluss prüft `lease_token`, `status = running` und `lease_expires_at >= now`. Trifft das nicht zu, bleibt `Finish::LostLease`. Der alte Worker setzt den Job nicht auf `succeeded`. Die Lease ist mindestens das Handler-Zeitlimit plus fünf Sekunden und höchstens 600 Sekunden. Das Handler-Limit liegt zwischen 1 und 120 Sekunden.

SQLite serialisiert Schreibzugriffe. Ein gesperrter Schreibversuch liefert `job.locked`. Der Worker versucht den nächsten Durchlauf, statt den Job als Erfolg zu speichern. Die Parallelität eines SQLite-Workers bleibt 1. PostgreSQL akzeptiert 1 bis 4 aufeinanderfolgende Claims je Durchlauf. Der Standard ist 1.

## Grenzen der Ausführung

Ein Handler-Timeout verwirft die Future am nächsten `.await`. Code hinter diesem Await läuft nicht. Ein Effekt, der vor dem Await schon passiert ist, bleibt bestehen. SMTP, das der Server bereits angenommen hat, wird nicht zurückgenommen. `JobHandler` erhält kein Abbruch-Signal. Eine vom Handler selbst gestartete Aufgabe läuft weiter.

Nach dem Timeout speichert der Worker `retry` mit `job.timeout`. Sind die Versuche aufgebraucht, wird der Job `failed` mit `job.attempts`.

Verliert der Worker die Lease, liefert `finish_success` `Finish::LostLease`. Der alte Token setzt den Job nicht auf `succeeded`. Der Handler kann den externen Effekt schon ausgeführt haben. Der neue Besitzer führt ihn erneut aus.

Ctrl+C setzt das Stopp-Flag zwischen zwei Claims. Der aktuelle `run_once` läuft bis zum Ergebnis oder bis zum Timeout. Der laufende Effekt wird nicht zurückgerollt.

Die Zustellung ist at least once. Eine Exactly-once-Garantie stellt Ferrioxid nicht bereit. Zwei Prozesse können denselben fälligen Job nicht beide als gültigen Claim besitzen. Nach einer abgelaufenen Lease kann ein zweiter Lauf denselben Effekt wiederholen.

## Einstellungen

`[jobs]` in `ferrum.toml` ist optional.

| Schlüssel | Standard | Grenze |
|---|---|---|
| `enabled` | `true` | schaltet nur den Worker, nicht den Webserver |
| `poll_ms` | `1000` | 50 bis 60000 |
| `concurrency` | `1` | 1 bis 4; bei SQLite bleibt der Worker bei 1 |
| `lease_secs` | `60` | 5 bis 600 |
| `max_attempts` | `5` | 1 bis 20 |
| `retention_hours` | `168` | 1 bis 2160 |
| `jitter` | `true` | streut den Backoff |

Ein Projekt ohne `[jobs]` und ohne Worker startet wie bisher.
