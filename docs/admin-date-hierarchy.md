# Datumshierarchie

**Stand:** v0.19.

```rust
#[ferrioxid(admin = true, admin_date_hierarchy = "event_date")]
```

Das Feld muss ein sichtbares `NaiveDate` oder `DateTime<Utc>` sein. `NaiveTime` ist nicht erlaubt.

Die Liste zeigt Jahr, dann Monat, dann Tag als Links. Die Parameter heißen `year`, `month` und `day`. Ein Monat ohne Jahr oder ein Tag ohne Monat ist 422 und schreibt nichts.

Der Filter ist ein parametrisierter Bereich:

- `Date`: inklusive Anfangstag, exklusiv der folgende Tag, Monat oder das Folgejahr.
- `DateTime`: dieselben Grenzen als UTC-Mitternacht.

Die Gruppierung zählt höchstens 100 Werte und verwendet denselben Query-Scope, dieselbe Suche und dieselben Filter wie die Liste. Ein Datensatz außerhalb des Scopes erscheint nicht in der Zählung.

Für `DateTime` ist der Kalender der UTC-Tag der kanonischen Zeichenkette, nicht die lokale Zone des Browsers.

Die Navigation ist HTML. Sie braucht kein JavaScript.
