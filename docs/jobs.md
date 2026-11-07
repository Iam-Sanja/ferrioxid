# Hintergrundjobs

[Zur Übersicht](index.md) · Worker: [workers.md](workers.md) · Retries: [job-retries.md](job-retries.md) · Mail: [job-mail-integration.md](job-mail-integration.md)

**IST.** Funktionsstand v0.27.1. Cargo-Version `0.1.0`. Die Queue liegt in `crates/ferrum-jobs`. Es gibt kein Redis und keine zweite Datenbank-Engine.

## Zustellung

Ferrum garantiert **at least once**. Ein Handler kann denselben Job mehr als einmal sehen. Exactly once gibt es nicht. SMTP und andere externe Effekte lassen sich nicht in derselben Transaktion wie der Jobabschluss festschreiben. Eine angenommene SMTP-Antwort ist keine Zustellbestätigung.

## Öffentliche API

```rust
use ferrum::jobs::{JobContext, JobError, JobHandler, JobRegistry, Queue, When};
use ferrum::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pulse { label: String }

struct PulseHandler;

#[ferrum::async_trait::async_trait]
impl JobHandler for PulseHandler {
    const NAME: &'static str = "notes_pulse";
    const VERSION: u32 = 1;
    type Payload = Pulse;

    async fn run(&self, payload: Self::Payload, _ctx: &JobContext) -> Result<(), JobError> {
        if payload.label.is_empty() {
            return Err(JobError::permanent("notes.empty"));
        }
        Ok(())
    }
}

let mut registry = JobRegistry::new();
registry.register(PulseHandler)?;
queue.enqueue::<PulseHandler>(&Pulse { label: "sofort".into() }, When::now()).await?;
queue.enqueue::<PulseHandler>(&Pulse { label: "spaeter".into() }, When::at(1_800_000_030)).await?;
queue.enqueue::<PulseHandler>(&Pulse { label: "wieder".into() }, When::every("notes-pulse", 60)).await?;
```

`JobRegistry` gehört dem Prozess, der den Worker baut. Es gibt keine globale Registry und kein Nachladen von Programmen. `NAME` besteht aus Kleinbuchstaben, Ziffern und Unterstrichen. `VERSION` beginnt bei 1. Der Payload ist ein JSON-Objekt, höchstens 16 KiB. Schlüssel wie `password`, `token`, `secret` und `reset_url` werden abgelehnt.

`When::now` ist sofort fällig. `When::at` setzt `available_at` als Unix-Sekunde in UTC. `When::every` trägt ein Intervall von 1 bis 2_592_000 Sekunden und einen `schedule_key`. Cron-Syntax gibt es nicht.

## Zustand

| Zustand | Bedeutung |
|---|---|
| `pending` | wartet auf `available_at` |
| `running` | ein Worker hält die Lease |
| `retry` | temporärer Fehler, `next_attempt_at` liegt in der Zukunft |
| `succeeded` | abgeschlossen |
| `failed` | endgültig oder Versuche aufgebraucht |
| `cancelled` | vom Operator abgebrochen |

`pending` und `retry` werden beansprucht. `running` mit abgelaufener Lease ebenfalls. Erfolg, dauerhafter Fehler und Abbruch sind für den automatischen Worker zu Ende. `retry` und `cancel` über die CLI setzen die erlaubten Übergänge zurück. Details stehen in [job-retries.md](job-retries.md) und [workers.md](workers.md).

## Schema

Die Migration `crates/ferrum-jobs/migrations/0001_jobs.json` legt `_ferrum_jobs` an. Die Historie heißt `_ferrum_job_migrations`. Auth-Migrationen `0001` bis `0004` bleiben unverändert. `ferrum migrate up` wendet die Job-Migration an. Der Worker und `ferrum dev` tun das nicht.

Zeiten sind UTC-Unix-Sekunden. Ein eindeutiger `idempotency_key` liefert beim zweiten Eintrag denselben Job. Ein `schedule_key` darf gleichzeitig nur an einer Zeile hängen. Nach Erfolg wird er auf die nächste Fälligkeit weitergereicht. Verpasste Intervalle werden übersprungen, damit ein stiller Worker keine Lawine erzeugt.

Abgeschlossene Jobs, die älter als `jobs.retention_hours` sind, löscht der Worker in Blöcken von 100 Zeilen. Laufende Jobs bleiben.

Das Beispiel mit Sofort-, Verzögerungs-, Intervall- und Mail-Job liegt in `examples/notes`. `examples/notes/src/bin/worker.rs` führt dieselben Handler in einem eigenen Prozess aus. myshop bleibt unverändert.

## Ablauf in einem Projekt

1. Job definieren. Ein Typ implementiert `JobHandler` mit `NAME`, `VERSION` und einem JSON-Objekt als Payload.
2. Handler registrieren. Die App implementiert `FerrumApplication::register_jobs`. `App::install` ruft diese Methode nicht auf.
3. Job einreihen. Der Webprozess nutzt `queue.enqueue::<Handler>(...)`. Dafür muss `_ferrum_jobs` bereits durch `ferrum migrate up` existieren.
4. Worker starten. `ferrum generate worker` legt `src/bin/worker.rs` an, wenn die Datei fehlt. Der Prozess lädt `ferrum.toml`, verbindet die konfigurierte Datenbank und ruft `WorkerHost::run_until_ctrl_c` auf. `ferrum dev` startet ihn nicht.
5. Jobstatus prüfen. `ferrum jobs list` und `ferrum jobs inspect <id>` zeigen Status, Handler, Version, Versuche, Zeiten und Fehlercode. Die Payload bleibt verborgen, bis `--show-payload` gesetzt ist.
6. Fehler behandeln. `JobError::retry` plant einen neuen Versuch. `JobError::permanent` beendet den Job. `ferrum jobs retry <id> --yes` und `ferrum jobs cancel <id> --yes` gelten für die erlaubten Zustände.

Ein unbekannter Handler bleibt `pending` und erhöht `attempts` nicht. Fehlt die Migration, endet der Worker mit `job.migration.missing` und Exitcode 2.
