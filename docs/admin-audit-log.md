# Änderungshistorie

[Zur Übersicht](admin.md) · Sicherheit: [admin-security.md](admin-security.md)

## Tabelle

`_ferrum_admin_audit` aus `crates/ferrum-auth/migrations/0002_admin_journal.json`. SQLite speichert die UUID-Spalten als Text. Dieselbe Migration läuft über die vorhandene Engine. PostgreSQL-Kompatibilität folgt aus dem Dialekt der Engine. Die ignorierten PostgreSQL-Tests wurden dafür nicht ausgeführt.

| Spalte | Inhalt |
|---|---|
| `id` | Ereignis-UUID |
| `created_at` | Unix-Sekunden |
| `actor_id` | Benutzer-UUID |
| `model_key` | Modellschlüssel |
| `record_id` | Datensatz-ID, bei einer verweigerten Sammelaktion leer |
| `action` | siehe unten |
| `outcome` | `ok` oder `denied` |
| `summary` | Gekürzte Textzusammenfassung |

Aktionen: `create`, `update`, `delete`, `bulk_update`, `bulk_delete`, `user_create`, `user_update`, `password_reset`, `role_assign`, `role_revoke`.

## Was nicht in der Zusammenfassung steht

Felder mit `sensitive`, verborgene Felder, Namen `password`, `password_hash`, `token`, `token_hash`, `secret`, `csrf_token`, `csrf_token_hash` und Endungen `_hash` oder `_secret`. Höchstens 12 Zeilen, insgesamt 1500 Zeichen, ein Wert höchstens 80 Zeichen, Zeilenumbrüche ersetzt. Die Vorlage escaped die Ausgabe.

Eine normale Änderung nutzt das Feldlabel: `Name: "Apfel" → "Grüner Apfel"`. Ein Passwort-Reset hat eine leere Zusammenfassung. Benutzer anlegen speichert nur die E-Mail. Rollen speichern den Rollennamen.

## Transaktion

`audited_create`, `audited_update` und `audited_delete` schreiben Modellzeile und Audit in einer Transaktion. Die öffentliche `ModelRepository::create` tut das nicht. Die JSON-Admin-API geht über `ManagedModel` und schreibt mit.

Verzeichnisereignisse werden nach dem Auth-Schreibvorgang eingefügt. Schlägt das Audit fehl, bleibt die Auth-Änderung bestehen. Das ist ausdrücklich nicht atomar.

## Seite

`GET /admin/audit` verlangt Staff. Superuser sehen alle Zeilen, absteigend nach Zeit und ID, 25 je Seite. Anderes Staff sieht nur `actor_id` gleich der eigenen ID. Dadurch werden unlesbare Datensätze und fremde Feldwerte nicht über die Historie sichtbar. Das ist enger als „jeder lesbare Datensatz“.

Code: `crates/ferrum-admin/src/audit.rs`. Test: `summary_skips_sensitive_values` und `admin_v018_drafts_inlines_bulk_and_audit`.
