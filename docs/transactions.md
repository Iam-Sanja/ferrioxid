# Transaktionen

[Zur Übersicht](index.md) · Datenbank: [database.md](database.md) · Verzeichnis: [admin-directory-audit.md](admin-directory-audit.md) · Eltern-Kind-Formulare: [admin-transactional-inlines.md](admin-transactional-inlines.md)

**Stand:** v0.20. Die Cargo-Version bleibt `0.1.0`.

Eine Schreiboperation, die mehrere Zeilen ändern soll, benutzt eine `DatabaseTransaction`. `ferrioxid_db::conclude` committet nur bei `Ok` und rollt bei `Err` zurück. Es gibt keine zweite Transaktions-Engine.

## Grenze

- Modell-CRUD über `ModelRepository::create`, `update` und `delete` öffnet die eigene Transaktion und schreibt das Modell-Audit darin. Diese Methoden dürfen nicht aufgerufen werden, solange bereits eine Transaktion offen ist.
- `ManagedModel::read_in`, `create_in`, `update_in` und `delete_in` laufen auf der übergebenen Transaktion. Sie schreiben kein Audit und öffnen keine weitere Transaktion.
- Auth-Methoden mit dem Suffix `_on` nehmen `&impl ConnectionTrait`. Sie prüfen den Actor und den Zielzustand auf dieser Verbindung und committen nicht.
- Das Verzeichnis in `ferrioxid-admin` schreibt Auth-Änderung und Audit-Zeile in derselben Transaktion. Siehe [admin-directory-audit.md](admin-directory-audit.md).
- Ein Formular mit `form` in `admin_inline` speichert Elternobjekt, Kinder und Audit in einer Transaktion. Siehe [admin-transactional-inlines.md](admin-transactional-inlines.md).

`query_admin_page` nimmt `&impl ConnectionTrait`. Eine Kindliste innerhalb der Schreibtransaktion benutzt dieselbe Verbindung.

## SQLite

`ferrioxid_db::connect` setzt für SQLite `max_connections(1)`. Eine zweite Abfrage, die eine Verbindung aus demselben Pool verlangt, wartet auf die offene Schreibtransaktion und hängt fest. Vor `begin` gehören daher Passwort-Hashes, die Formularprüfung und `ensure_relations` des Elternobjekts. Innerhalb der Transaktion nur Methoden, die die Transaktion selbst entgegennehmen.

## PostgreSQL

Die Schnittstellen sind `ConnectionTrait` und `DatabaseTransaction`. Sie enthalten kein SQLite-SQL. Die PostgreSQL-Laufzeittests `postgres_migration_roundtrip` und `postgres_http_roundtrip` bleiben ignoriert, solange kein Server erreichbar ist. Ein grüner Workspace-Lauf ist kein PostgreSQL-Nachweis.

## Fehler

Ein Datenbankfehler wird zu `FerrioxidError::Database`. SQL-Text geht nicht in die HTML-Antwort. `conclude` bildet den Commit-Fehler auf den übergebenen Mapper ab. Eine fehlgeschlagene Teiloperation erzeugt kein `303`.
