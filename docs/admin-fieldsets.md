# Formularabschnitte

[Zur Übersicht](admin.md) · Vorlagen: [admin-templates.md](admin-templates.md)

**Stand:** v0.17. Die Cargo-Version bleibt `0.1.0`.

Ohne `admin_fieldset` bleibt das Formular eine einzelne unbenannte Gruppe.

Ein Modell deklariert die Abschnitte. Jedes sichtbare Feld nennt genau einen davon.

```rust
#[ferrum(
    admin = true,
    admin_fieldset = "Allgemein|Name und Kategorie",
    admin_fieldset = "Preis und Verfügbarkeit",
    admin_fieldset = "System|Nur lesen|optional"
)]
struct Offer {
    #[admin(fieldset = "System")]
    id: Uuid,
    #[admin(fieldset = "Allgemein")]
    title: String,
}
```

Die drei Teile sind Name, optionale Beschreibung und das Wort `optional`. Der Name hat 1 bis 80 Zeichen, die Beschreibung höchstens 160. Namen sind eindeutig. Ein leerer Abschnitt und ein Feld ohne Abschnitt sind Übersetzungsfehler, sobald das Modell Abschnitte hat. Verborgene Felder brauchen keinen Abschnitt. Ein Feld darf nur einmal vorkommen.

Die Vorlage zeichnet einen benannten Abschnitt als `fieldset` mit `legend`. `optional` ist `details` und `summary` und bleibt ohne JavaScript zu. Leere Abschnitte, zum Beispiel die ID beim Anlegen, werden nicht gezeichnet. Die Feldreihenfolge ist die Struct-Reihenfolge innerhalb des Abschnitts.

`readonly` und `id` bleiben Text. `hidden` wird nicht gesendet. Rechte, CSRF und `reject_locked` gelten unverändert.
