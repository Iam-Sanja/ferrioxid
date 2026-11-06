# Admin anpassen

[Zur Übersicht](admin.md) · Makro: [macros.md](macros.md)

## Zweck

Die Anpassung geschieht am Rust-Modell. Es gibt keinen Laufzeit-Baukasten und kein zweites Schema.

## Attribute

```rust
#[ferrum(
    table = "products",
    api = "/api/v1/products",
    allow_anonymous = true,
    admin = true,
    admin_label = "Produkte",
    admin_permission = "products"
)]
#[admin(list_display, searchable, sortable, filterable, readonly, hidden, label = "Name")]
```

`admin = true` ist nötig, bevor Feldattribute erlaubt sind. `admin_label` ist 1 bis 80 Zeichen und sonst der Tabellenname. `admin_permission` ist entweder `authenticated` oder ein Bezeichner. Fehlt es, gilt das vorhandene `permissions`. Fehlen beide, ist das Modell im Admin nur für Superuser sichtbar.

## Feldregeln

- `id` erscheint in der Liste und ist nicht editierbar.
- `readonly` und der Primärschlüssel lassen sich über den Admin nicht schreiben.
- `hidden` verträgt sich nicht mit Liste, Suche, Sortierung oder Filter.
- `searchable` verlangt `String`.
- Sensible Feldnamen müssen `hidden` sein.
- Ein unbekanntes Attribut ist ein Compilerfehler mit der Liste der erlaubten Namen.

`allow_anonymous` und `permissions` bleiben gemeinsam ein Compilerfehler. `allow_anonymous` mit `admin` und `admin_permission` ist erlaubt: die öffentliche Route bleibt öffentlich, der Admin verlangt die genannte Ressource und zusätzlich Staff.

## Registrierung

```rust
AdminRegistry::new().add::<Product>().add_policy::<InternalNote, NotePolicy>()
```

`add` ignoriert Modelle ohne `admin = true`. Eine Objektregel muss mit `add_policy` gebunden werden. Ohne Bindung antwortet der Einzelzugriff 403 mit `object policy is not bound`.

## myshop

`Product` nutzt `admin_permission = "products"`, `Category` nutzt `categories`, `InternalNote` übernimmt `notes` aus `permissions`. Die Migrationen `0001`, `0002` und `0003` bleiben unverändert, weil keine Spalte hinzukommt.

## Sicherheit

`permissions = "authenticated"` zusammen mit Staff erlaubt jedem Staff-Benutzer das Modell. Das ist ausdrücklich und sollte nur für interne Werkzeuge verwendet werden.

## Fehler

Compilerfehler bei unbekannten Attributen, verstecktem `id`, durchsuchbarer Nicht-Textspalte und sichtbaren sensiblen Feldern. Laufzeitfehler stehen in [admin-backend.md](admin-backend.md).

## Tests

Der Admin-HTTP-Test prüft `AdminProduct::admin_meta`, die Registry und ein nicht registriertes Modell.

## Grenzen

Keine eigenen Widget-Namen, keine berechneten Spalten, keine Mehrfachbeziehungen in der Auswahl. Has-Many bleibt eine Rust-Beziehung und wird im Formular nicht als Untertabelle gezeigt.

## Später

Deklarierte Widget-Namen und ein SQL-Scope als eigenes Attribut, sobald die Abfrage das sicher kann.
