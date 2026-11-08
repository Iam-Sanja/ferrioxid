# Datum und Uhrzeit

**Stand:** v0.19. Die Cargo-Version bleibt `0.1.0`.

Ferrioxid unterscheidet drei Typen. Sie werden nicht ineinander umgerechnet.

| Rust | `ColumnKind` | Bedeutung | Kanonische Zeichenkette |
|---|---|---|---|
| `chrono::NaiveDate` | `Date` | Kalendertag ohne Uhrzeit und ohne Zone | `2026-11-07` |
| `chrono::NaiveTime` | `Time` | Tageszeit ohne Datum | `14:30:00` |
| `chrono::DateTime<Utc>` | `DateTime` | Absoluter Zeitpunkt | `2026-11-07T13:30:00Z` |

`Option<T>` bleibt nullable. Ein `NaiveDate` springt nicht auf den Nachbartag. `DateTime` muss `DateTime<Utc>` sein. Andere Zeitzonenparameter lehnt das Makro ab.

## JSON

```json
{
  "event_date": "2026-11-07",
  "start_time": "14:30:00",
  "created_at": "2026-11-07T13:30:00Z"
}
```

`+00:00` ist beim Lesen erlaubt und wird als `Z` ausgegeben. Jeder andere Offset ist `validation.datetime_offset`. Ein ungültiges Datum, einschließlich `2023-02-29`, ist `validation.date`. Eine ungültige Uhrzeit ist `validation.time`. Ein unlesbarer Zeitpunkt ist `validation.datetime`.

## HTML

| Typ | Eingabe |
|---|---|
| Date | `<input type="date">` |
| Time | `<input type="time">` |
| DateTime | `<input type="datetime-local">` |

`datetime-local` enthält keine Zone. Ferrioxid behandelt den Wert als UTC und sagt das im Hinweis. `2026-11-07T13:30` wird `2026-11-07T13:30:00Z`. Es gibt keine Umrechnung in die Zeitzone des Browsers und damit keine Sommerzeit-Lücke.

## Modell

```rust
#[ferrioxid(table = "events", api = "/api/v1/events", admin = true, admin_date_hierarchy = "event_date")]
pub struct Event {
    pub event_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub created_at: DateTime<Utc>,
}
```

Die Typen liegen in `ferrioxid::prelude`. Bestehende Modelle müssen sie nicht verwenden.

## Grenzen

- Keine Dauer und kein `DateTime<Local>`.
- Kein stilles Ändern einer Textspalte in ein Datum.
- PostgreSQL-Spaltentypen sind implementiert. Ein Lauf gegen einen PostgreSQL-Server ist in dieser Version nicht ausgeführt.
