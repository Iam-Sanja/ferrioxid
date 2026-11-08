# Job-Retries

[Zur Übersicht](index.md) · Jobs: [jobs.md](jobs.md) · Worker: [workers.md](workers.md)

**IST.** Versuche sind begrenzt. Eine unbegrenzte Wiederholung wird abgelehnt.

## Fehlerklassen

`JobError::retry("code")` setzt den Job auf `retry`, solange `attempts < max_attempts`. Danach wird er `failed` mit `job.attempts`. `JobError::permanent("code")` setzt sofort `failed`. Ein Handler-Zeitlimit ist ein Retry mit `mail.timeout` oder `job.timeout`.

Der gespeicherte Code besteht aus Kleinbuchstaben, Ziffern, Punkt und Unterstrich und hat höchstens 64 Zeichen. Alles andere wird `job.error`. Der Code enthält keinen Payload.

## Backoff

Die Wartezeit in Sekunden ist `min(300, 2^(attempt-1))`. Der erste Fehlversuch wartet eine Sekunde, der zweite zwei, danach vier, bis 300 Sekunden. `jobs.jitter = true` addiert einen Anteil aus der UTC-Sekunde. Tests setzen `JobPolicy.jitter` auf `false` und schieben `ManualClock` vor. Es gibt keine echte Wartezeit im Test.

`next_attempt_at` und `available_at` müssen beide erreicht sein, bevor der Job wieder beansprucht wird.

## Operator

```powershell
ferrioxid jobs list
ferrioxid jobs inspect <job-id>
ferrioxid jobs retry <job-id> --yes
ferrioxid jobs cancel <job-id> --yes
```

`list` und `inspect` öffnen SQLite mit `mode=ro` und schreiben nicht. `retry` gilt für `failed` und `cancelled`. Es setzt `pending`, `attempts = 0` und löscht Lease und Fehlercode. `cancel` gilt für `pending` und `retry`. Ein `running` Job bleibt unangetastet und liefert `job.running`.

Ohne `--yes` ist der Exitcode 2 und die Datenbank bleibt unverändert. Die Berechtigung ist die des lokalen Operators, dieselbe Vertrauensgrenze wie `ferrioxid migrate up`. Es gibt keinen HTTP-Endpunkt dafür.

## Intervalle

Nur ein erfolgreicher Abschluss plant die nächste Fälligkeit. Ein dauerhaft fehlgeschlagenes Intervall erzeugt keine weiteren Zeilen. `ferrioxid jobs retry --yes` öffnet dieselbe Zeile wieder. Zwei Einträge mit demselben `schedule_key` liefert der zweite Aufruf als `EnqueueOutcome::Existing`.
