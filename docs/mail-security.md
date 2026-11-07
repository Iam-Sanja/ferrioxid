# Mail- und Reset-Sicherheit

[Zur Übersicht](index.md) · Passwort: [password-reset.md](password-reset.md) · SMTP: [mail-smtp.md](mail-smtp.md)

**Stand:** v0.23. Die Cargo-Version bleibt `0.1.0`.

## Was gespeichert wird

| Ort | Inhalt |
|---|---|
| `_ferrum_password_resets.token_hash` | SHA-256-Hex, eindeutig |
| `_ferrum_password_reset_exchanges.secret_hash` | SHA-256-Hex, eindeutig |
| `_ferrum_mail_outbox` | Art, Empfängeradresse, optional `reset_id`, Zeitstempel, Versuchszahl, Fehlercode |
| `_ferrum_admin_audit` | `password reset completed`, ohne Token und ohne Passwort |
| Tracing | Codes wie `reset.limited`, `mail.timeout`, `mail.rejected` |

Die Outbox enthält den Roh-Token nicht. Konsole und Memory dürfen ihn enthalten, weil sie die Zustellung selbst sind. Beide sind in Produktion abgelehnt.

## Versand und Transaktion

SMTP lässt sich nicht mit SQLite zurückrollen. Die Reihenfolge ist deshalb fest: Hash und Outbox-Zeile werden committet, danach wird die Nachricht aus dem Speicher versendet. Ein eindeutiger SMTP-Fehler verbraucht den Token und setzt `failed_at` plus den Code. Ein Timeout setzt nur `mail.timeout` und lässt `sent_at` und `failed_at` leer. Die nächste Reset-Anfrage markiert solche Zeilen nach 120 Sekunden als `mail.expired` und verbraucht den zugehörigen Token. Es gibt keinen zweiten automatischen Versuch.

Das ist keine garantierte Zustellung. Ein angenommener SMTP-Versand kann den Posteingang verfehlen. Ein Prozessabbruch nach dem Commit und vor der Markierung kann einen bereits angenommenen Token beim nächsten Aufräumen entwerten. Die Bestätigungsmail nach einem erfolgreichen Passwortwechsel liegt außerhalb der Passworttransaktion. Schlägt sie fehl, bleibt das neue Passwort gültig.

## Weitere Grenzen

- Das Rate-Limit ist prozesslokal und teilt sich nicht zwischen Instanzen.
- `ferrum doctor` prüft die Form und das Vorhandensein von `FERRUM_SMTP_PASSWORD`, nicht die Erreichbarkeit des Servers.
- Zertifikatsprüfung ist aktiv. Ein unsicherer TLS-Fallback existiert nicht.
- Die Reset-Seiten laden keine Drittanbieterdatei und setzen keinen Analytics-Aufruf.
- Ein manipuliertes, abgelaufenes oder fremdes Token sieht von außen wie ein ungültiger Link aus.
- Parallele Abschlüsse serialisieren sich auf der einen SQLite-Verbindung. Der bedingte Update lässt nur einen Erfolg zu.
