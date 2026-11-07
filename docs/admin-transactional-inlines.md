# Transaktionale Inlines

[Zur Übersicht](admin-inlines.md) · Transaktionen: [transactions.md](transactions.md) · Entwürfe: [admin-form-drafts.md](admin-form-drafts.md)

**Stand:** v0.20. Die Cargo-Version bleibt `0.1.0`.

`admin_inline` ohne weitere Optionen bleibt die Liste auf der Detailseite plus das eigene Anlegeformular. Das gemeinsame Formular ist opt-in.

## Syntax

```rust
admin_inline = "lines=order_lines.order_id|form|add|change|delete|label=Positionen|max=20|extra=1|fields=product,quantity,unit_price"
```

Der Teil vor `|` bleibt `name=tabelle.spalte`. Optionen:

| Option | Bedeutung |
|---|---|
| `form` | Kinder stehen im Elternformular |
| `add`, `change`, `delete` | Anlegen, Ändern, Entfernen. `form` allein setzt alle drei. Steht eine der drei dabei, starten die anderen bei falsch |
| `label` | 1–40 Zeichen, ohne Steuerzeichen und ohne `\|` |
| `fields` | Bearbeitbare Kindfelder. Der Eltern-Fremdschlüssel ist verboten |
| `readonly` | Nur Felder aus `fields`, wenn `fields` gesetzt ist |
| `min`, `max`, `extra` | Mindestzahl, Höchstzahl, leere Zusatzzeilen. `max` ist höchstens 50. `form` ohne `max` setzt 20. `extra` wird 1, wenn `add` gilt und `extra` fehlt |
| `order` | Sortierspalte, sonst `id` |

Unbekannte Optionen sind Compile-Fehler. `form` ohne `add`, `change` oder `delete` ebenfalls. `examples/myshop` nutzt das an `Order` und `OrderLine`. Die Kategorie bleibt beim alten Attribut.

## Speichern

`save=rows` erhöht nur die Zusatzzeilen und schreibt einen Formularentwurf. Es entsteht keine Modellzeile. `save=detail`, `save=edit` und `save=new` speichern, wenn mindestens ein Inline `form` trägt:

1. Struktur prüfen: bekannte Felder, keine doppelte Kind-ID, Index unter `max`, Kind gehört zum Elternobjekt, Mindest- und Höchstzahl.
2. Feldwerte, Pflicht, Unique und Policies prüfen.
3. Transaktion starten und Actor, Datensätze und Fingerabdruck erneut lesen.
4. Elternobjekt schreiben. Den Eltern-Fremdschlüssel jedes Kindes auf diese ID setzen.
5. Neue Kinder anlegen, vorhandene ändern, mit `remove=1` markierte löschen, wenn `delete` erlaubt ist.
6. Audit-Zeilen mit derselben `operation_id` schreiben.
7. Commit.

Ein Fehler rollt alles zurück. Die Antwort bleibt auf dem Formular. `409` mit `conflict.stale` bedeutet, dass der Fingerabdruck nicht mehr zum Bestand passt. Ein stilles Überschreiben findet nicht statt.

Der Fingerabdruck ist ein prozesslokaler Hash der Eltern- und Kindfelder zum Renderzeitpunkt, versteckt als `basis`. Das ist keine Versionsspalte und kein frameworkweites Optimistic Locking. Ein Neustart des Prozesses kann denselben Bestand anders hashen. Zwei SQLite-Schreiber laufen wegen `max_connections(1)` nicht echt parallel. Die Prüfung ist die erneute Lesung in der Transaktion.

## Grenzen

- Eine Has-Many-Ebene. Kein Many-to-Many. Keine verschachtelten Inlines.
- Weitere Fremdschlüssel eines Kindes werden in dieser Transaktion nicht über `ensure_relations` aufgelöst. `OrderLine` hat nur den Elternschlüssel.
- Ein leeres boolesches `false` zählt als Inhalt. Eine sonst leere Zeile mit Häkchen wird nicht übersprungen.
- Eine unveränderte Kindzeile wird erneut geschrieben und auditiert. Ein zweites Absenden desselben gültigen Formulars kann deshalb eine weitere Audit-Zeile erzeugen, solange der Fingerabdruck noch passt.
- Entfernen ist die Checkbox vor dem Speichern. Es gibt keine eigene Wiederherstellen-Aktion. Ein abgewähltes Häkchen vor dem Absenden lässt die Zeile stehen.
- Kind-`DateTime` im Inline wird nicht in den Wert für `datetime-local` umgerechnet. Bestellpositionen haben kein solches Feld.
