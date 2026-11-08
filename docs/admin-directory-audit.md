# Verzeichnis-Audit

[Zur Übersicht](admin.md) · Auth: [auth.md](auth.md) · Transaktionen: [transactions.md](transactions.md) · Historie: [admin-audit-log.md](admin-audit-log.md)

**Stand:** v0.20. Die Cargo-Version bleibt `0.1.0`.

HTML und JSON des Verzeichnisses rufen dieselben Funktionen in `ferrioxid-admin` auf. Auth-Schreibvorgang, Sitzungswiderruf und Audit-Zeile teilen sich eine Transaktion. Schlägt das Audit fehl, bleibt der vorherige Auth-Zustand.

## Ablauf

1. Eingabe prüfen. Argon2id-Hashes entstehen vor `begin`, damit die SQLite-Schreibsperre nicht während des Hashens gehalten wird.
2. Transaktion starten.
3. Den ausführenden Benutzer neu lesen. Er muss aktiv, Staff und Superuser sein. Ein zwischenzeitlich entzogenes Superuser-Flag beendet den Vorgang mit `Forbidden`.
4. Den Zielzustand prüfen: kein Superuser-Ziel für Profil, Passwort, Aktiv und Staff. Keine Selbst-Deaktivierung. Kein Entzug des eigenen Staff-Flags. Ein Berechtigungscode, der `superuser` enthält, wird abgelehnt. Superuser-Vergabe bleibt die CLI.
5. Schreiben. Passwortänderung und Deaktivierung widerrufen die Sitzungen des Ziels in derselben Transaktion.
6. Eine Audit-Zeile mit `operation_id` schreiben.
7. Commit. Jeder Fehler davor rollt Hash, Flags, Rollen, Rechte, Sitzungen und Audit zurück.

## Ereignisse

| Aktion | `action` | Zusammenfassung |
|---|---|---|
| Benutzer anlegen | `user_create` | E-Mail, darunter die gesetzten Rollennamen |
| E-Mail, Aktiv, Staff | `user_update` | neue E-Mail, `active`/`inactive` oder `staff`/`not-staff` |
| Passwort | `password_reset` | leer |
| Rolle | `role_assign` / `role_revoke` | Rollenname |
| Recht | `permission_grant` / `permission_revoke` | Berechtigungscode |

`record_id` ist die Benutzer-UUID oder, bei einem Recht, der Rollenname. `actor_id` ist der ausführende Benutzer. Passwörter, Hashes, Sitzungs- und CSRF-Tokens stehen nicht in der Zusammenfassung.

Die CLI-Methoden `create_user`, `assign_role`, `grant_permission` und `set_user_active` schreiben kein Verzeichnis-Audit. Dort fehlt ein Admin-Actor.

## Migration

`crates/ferrioxid-auth/migrations/0003_audit_operation.json` hängt die nullable Spalte `operation_id` an `_ferrum_admin_audit`. `0001_auth_schema` und `0002_admin_journal` bleiben unverändert. Ältere Zeilen lesen `operation_id` als leer. Die Historie zeigt die ersten acht Zeichen, wenn ein Wert vorhanden ist.

## JSON

Die JSON-Verzeichnis-API nutzt dieselbe Transaktion. Sie verlangt `confirm` weiterhin nicht. Das bleibt offen.
