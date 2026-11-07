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
| `templates/models/list.html` | Liste, Suche, Filter, Sortierung, Blättern, Sammelaktion |
| `templates/models/bulk.html` | Bestätigung einer Sammelaktion, registriert als `bulk.html` |
| `templates/audit.html` | Änderungshistorie |
| `templates/models/form.html` | Anlegen und Bearbeiten, Abschnitte, drei Speicherknöpfe |
| `templates/models/pick.html` | Fremdschlüssel-Auswahl, registriert als `pick.html` |
| `templates/partials/field.html` | Feldgruppe, registriert als `field.html` |
| `templates/models/detail.html` | Detail |
| `templates/models/delete.html` | Löschbestätigung |
| `templates/directory/users.html` | Benutzerliste |
| `templates/directory/user_form.html` | Benutzer anlegen und bearbeiten, Passwort |
| `templates/directory/user_detail.html` | Benutzerdetail |
| `templates/directory/roles.html` | Rollen |
| `templates/directory/permissions.html` | Berechtigungen |
| `templates/errors/error.html` | 403, 404, 422 und 429 |

`rendering.rs` registriert diese Dateien unter festen Namen. `base.html` ist die Elternvorlage. Unterseiten füllen den Block `content`.

## Kontext

Jede Seite erhält `locale`, `theme`, `title`, Branding, den Katalog `i18n`, `signed_in`, `superuser`, `user_email`, `csrf_token`, `current_path`, `notice` und `alert`. Zusätzliche Felder kommen aus dem Handler, zum Beispiel `sections`, `rows` oder `models`. Modellformulare gruppieren Felder in `sections`. Benutzerformulare liefern `email`, `email_error`, `password_error` und `role_checks`. Passwortfelder bekommen keinen Wert.

Meldungen nach einem POST stehen nicht als freier Text in der URL. Erlaubt sind die Codes `created`, `saved`, `deleted`, `updated` und `changed`. `changed` verlangt zusätzlich eine Zahl `changed` von 0 bis 100. Der Handler übersetzt den Code.

## Escaping

`.html`-Vorlagen sind bei Tera automatisch escaped. Werte aus der Datenbank, Fehlermeldungen und der Katalog laufen durch dieses Escaping. `primary_color` ist vorher auf Hex beschränkt. Es gibt keine `|safe`-Ausgabe für Datensatzfelder.

Boolesche Felder senden zuerst ein verstecktes `false` und danach die Checkbox `true`. Der Parser behält den letzten Wert. Ein gesetztes Häkchen bleibt damit `true`, ein fehlendes `false`.

## Formularfelder

Der Handler baut Felder aus `AdminFieldMeta` und `ColumnSpec`. Verborgen und sensibel erscheinen nicht. Nur-lesen und `id` erscheinen als Text und werden nicht gesendet. Fremdschlüssel werden zu `select`, wenn das Zielmodell registriert ist, die Liste nicht blockiert ist und der Benutzer lesen darf. Die aktuelle ID ist `selected`. Pflicht-Fremdschlüssel haben keine leere Option. Ein Validierungsfehler setzt `aria-invalid` und einen Text unter dem Feld.

Die Rollenzuweisung sendet `email` und `role` aus Auswahllisten. Der Handler weist nur eine Rolle zu, die `list_roles` bereits kennt.

## Bekannte Lücken

`errors/error.html` zeigt bei einer Sitzung ein POST-Abmeldeformular. Ohne Sitzung verlinkt sie auf die Anmeldung. `directory/users.html` weist Rollen über ein `<select>` zu. Die Werkzeugleiste trennt Suche und Filter von „Neu“. Feldgruppen liegen in `partials/field.html`. Eingebaute Namen bleiben fest. Eine Anfrage darf keinen Vorlagenpfad wählen. Ein Projekt kann zusätzliche Namen registrieren, beschrieben in [admin-extension-api.md](admin-extension-api.md). Diese Vorlagen erweitern `base.html`, überschreiben sie nicht und dürfen `|safe` nicht verwenden.
