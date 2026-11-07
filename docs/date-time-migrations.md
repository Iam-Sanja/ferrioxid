# Migrationen für Datum und Uhrzeit

**Stand:** v0.19.

`LogicalType` hat `date`, `time` und `date_time`. Der Snapshot und die Checksum behandeln sie wie die bisherigen Typen.

| Backend | Date | Time | DateTime |
|---|---|---|---|
| SQLite | `TEXT` | `TEXT` | `TEXT` |
| PostgreSQL | `DATE` | `TIME` | `TIMESTAMPTZ` |

SQLite speichert die kanonische Zeichenkette. Sortierung und Präfixfilter folgen der ISO-Reihenfolge. PostgreSQL bindet die Chrono-Werte typisiert. Die Abbildung ist im Schema-Renderer vorhanden. Ein PostgreSQL-Laufzeittest dieser Version wurde nicht ausgeführt.

## Neue Spalten

Eine neue nullable oder Pflichtspalte entsteht über `ferrum migrate make`. myshop hängt `events` als `migrations/0004_events.json` an `0003_internal_notes`. Die Dateien `0001_initial.json`, `0002_categories_and_product_relations.json`, `0003_internal_notes.json` und `crates/ferrum-auth/migrations/0002_admin_journal.json` sind unverändert.

## Blockierte Umwandlung

`String` und `I64` bleiben die einzige unterstützte Typänderung. `String` nach `Date`, `Time` oder `DateTime` erzeugt eine Ablehnung in `plan_change`. Es gibt keine stille Reparatur ungültiger Bestandszeilen. Ein späterer, ausdrücklich geprüfter Datenumbau ist nicht Teil von v0.19.

Nullable und Defaults folgen den bisherigen Regeln. Ein Bool-Default gilt nicht für Datumsfelder.
