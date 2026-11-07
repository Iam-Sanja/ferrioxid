# Admin-Vorlagen

[Zur Übersicht](admin-native.md) · Gestaltung: [admin-design-system.md](admin-design-system.md)

## Aufgabe

Tera rendert den Admin aus festen Vorlagen im Crate. Eine Anfrage kann keinen Dateipfad und keinen Vorlagennamen wählen. Unbekannte Namen schlagen serverseitig fehl. Die Antwort ist dann ein kurzer Text `Request rejected` mit Status 500. Der Fehlertext aus Tera geht ins Log, nicht in den Browser.

## Dateien

| Vorlage | Seite |
|---|---|
| `templates/base.html` | Dokument, Seitenleiste, Sprache, Theme, Abmelden |
| `templates/login.html` | Anmelden |
| `templates/dashboard.html` | Übersicht |
| `templates/models/list.html` | Liste, Suche, Filter, Sortierung, Blättern |
| `templates/models/form.html` | Anlegen und Bearbeiten |
| `templates/models/detail.html` | Detail |
| `templates/models/delete.html` | Löschbestätigung |
| `templates/directory/users.html` | Benutzer |
| `templates/directory/roles.html` | Rollen |
| `templates/directory/permissions.html` | Berechtigungen |
| `templates/errors/error.html` | 403, 404, 422 und 429 |

`rendering.rs` registriert diese Dateien unter festen Namen. `base.html` ist die Elternvorlage. Unterseiten füllen den Block `content`.

## Kontext

Jede Seite erhält `locale`, `theme`, `title`, Branding, den Katalog `i18n`, `signed_in`, `superuser`, `user_email`, `csrf_token`, `current_path`, `notice` und `alert`. Zusätzliche Felder kommen aus dem Handler, zum Beispiel `fields`, `rows` oder `models`.

Meldungen nach einem POST stehen nicht als freier Text in der URL. Erlaubt sind nur die Codes `created`, `saved`, `deleted` und `updated`. Der Handler übersetzt den Code.

## Escaping

`.html`-Vorlagen sind bei Tera automatisch escaped. Werte aus der Datenbank, Fehlermeldungen und der Katalog laufen durch dieses Escaping. `primary_color` ist vorher auf Hex beschränkt. Es gibt keine `|safe`-Ausgabe für Datensatzfelder.

Boolesche Felder senden zuerst ein verstecktes `false` und danach die Checkbox `true`. Der Parser behält den letzten Wert. Ein gesetztes Häkchen bleibt damit `true`, ein fehlendes `false`.

## Formularfelder

Der Handler baut Felder aus `AdminFieldMeta` und `ColumnSpec`. Verborgen, nur lesbar, sensibel und `id` erscheinen nicht als Eingabe. Fremdschlüssel werden zu `select`, wenn das Zielmodell registriert ist, die Liste nicht blockiert ist und der Benutzer lesen darf. Die aktuelle ID ist `selected`. Pflichtfelder sind markiert. Ein Validierungsfehler setzt `aria-invalid` und einen Text unter dem Feld.
