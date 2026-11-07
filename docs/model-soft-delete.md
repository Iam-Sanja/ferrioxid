# Soft Delete

[Zur Übersicht](index.md) · Modelle: [models.md](models.md)

**Offen.** v0.26 setzt kein Soft Delete um.

Ein optionales `deleted_at` allein würde gelöschte Zeilen in Listen, REST, Admin, Fremdschlüsseln und Sammelaktionen weiter anzeigen. Gleichzeitig müssten Löschen, Wiederherstellen, Audit, Query-Scopes und Unique-Constraints dieselbe Bedeutung haben. Das ist mit dem heutigen `delete_record` nicht abgeschlossen, ohne die vorhandene Löschsemantik still zu ändern.

`#[field(managed = "deleted")]` ist deshalb ein Compile-Fehler. Erlaubte Werte sind nur `"created"` und `"updated"`.

Ein späterer Baustein braucht:

- Standardabfragen ohne gelöschte Zeilen
- Delete setzt den Zeitpunkt statt `DELETE`
- Restore mit derselben Berechtigungsprüfung wie die übrigen Schreibwege
- Audit-Einträge, die Delete und Restore unterscheiden
- unveränderte Unique-Constraints, ausdrücklich dokumentiert
- Fremdschlüssel, die gelöschte Ziele nicht als vorhanden behandeln
