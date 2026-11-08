# Job-Sicherheit

[Zur Übersicht](index.md) · Jobs: [jobs.md](jobs.md) · Mail: [job-mail-integration.md](job-mail-integration.md)

**IST.** Job-Namen und Payloads aus einer HTTP-Anfrage werden nicht ausgeführt. Die Registry enthält nur Typen, die der Worker-Prozess ausdrücklich registriert hat.

## Grenzen

- Ein unbekannter Handler bleibt `pending` und wird nicht ausgeführt.
- Eine andere Payload-Version endet als `failed` mit `job.version`.
- Kein JSON-Objekt, ein unbekanntes Feld bei `deny_unknown_fields` und mehr als 16 KiB enden vor oder bei der Ausführung als `job.payload.invalid` oder `job.payload.large`.
- Geheimnis-Schlüssel werden nicht gespeichert.
- Fehlertexte und Tracing nennen Codes, nicht den Payload und nicht `FERRUM_SMTP_PASSWORD`.
- Ein Handler-Code mit Leerzeichen, Großbuchstaben oder `password=` wird zu `job.error`. In `_ferrum_jobs.error_code` steht dieser Code, nicht der Detailtext.
- `JobError::message` hält eine Meldung zurück, wenn sie `password`, `secret`, `token`, `authorization`, `cookie` oder `reset_url` enthält.
- SQL-Bezeichner kommen aus festen Spaltennamen. Suchspalten sind nur `idempotency_key` und `schedule_key`. Werte sind gebunden.
- Es gibt keinen Weg, aus einer Job-Zeile ein Shell-Kommando zu machen.
- `max_attempts` über 20 wird nicht angenommen.
- Eine verlorene Lease darf den neuen Besitzer nicht überschreiben.
- Zwei Worker bekommen denselben fälligen Job nicht beide als gültigen Claim.
- Lesende CLI-Befehle legen keine SQLite-Datei und keine Historie an.

`ferrioxid jobs inspect <id>` zeigt Status, Handler, Version, Versuche, Zeiten und Fehlercode. Die Payload erscheint als `payload [redigiert, N Bytes]`. `--show-payload` gibt das gespeicherte JSON aus und warnt auf stderr mit `job.payload.visible`. Auch diese ausdrücklich angeforderte Payload kann E-Mail-Adressen und andere personenbezogene Daten enthalten, die der Handler als normale Felder gespeichert hat. Das Lease-Token wird in beiden Fällen nicht ausgegeben.

Gewöhnliche Logs und Tracing schreiben die Payload nicht. Der CLI-Worker und der Anwendungs-Worker geben sie nicht aus.

Die Tests liegen in `crates/ferrioxid-jobs/src/tests.rs` und `examples/notes/tests/jobs.rs`.
