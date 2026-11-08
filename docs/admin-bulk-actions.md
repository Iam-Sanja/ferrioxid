# Sammelaktionen

[Zur Übersicht](admin.md) · Eigene Aktionen: [admin-custom-actions.md](admin-custom-actions.md)

## Oberfläche

Die Modellliste zeigt, wenn mindestens eine erlaubte Aktion existiert:

- eine Checkbox `ids` je Zeile
- eine Checkbox `scope=page` für die aktuelle Seite
- ein `select` mit dem Aktionsnamen
- einen Submit

`scope=page` ist keine Skript-Auswahl. Der Server liest die Seite mit dem Schreib-Scope erneut und ignoriert gesendete IDs. Die Seitengröße bleibt 1 bis 100. Die ganze Tabelle wird nicht gewählt.

## Ablauf

1. `POST /admin/models/{model}/bulk` prüft CSRF, den registrierten Namen, UUID-IDs, die Obergrenze und für jede ID Lesezugriff plus Objektregel. Ein Fehlschlag speichert keinen Entwurf und ändert keine Zeile.
2. Die erlaubte Auswahl wird als Entwurf mit Zweck `bulk` gespeichert. Die Antwort ist 303 auf `GET /admin/models/{model}/bulk/{draft}`. Dieses GET schreibt nicht.
3. `POST` derselben Adresse ohne `confirm=1` zeigt die Bestätigung erneut und schreibt nicht.
4. Mit `confirm=1` prüft der Server das Recht erneut. Ein inzwischen entzogenes Recht ist 403 und ändert nichts.
5. Eine Transaktion liest jede ID erneut, führt die Operation aus, schreibt je Zeile ein Audit und committet. Danach wird der Entwurf gelöscht.

## Atomarität

`Delete` und `SetBool` sind atomar: jede ausgewählte Zeile wird geändert oder keine. Ein Datenbankfehler, einschließlich `ON DELETE RESTRICT`, rollt die Transaktion zurück. Die Fehlerseite entsteht erst nach dem Ende der Transaktion, damit die einzige SQLite-Verbindung nicht blockiert.

Doppelte IDs zählen einmal. Eine unbekannte Aktion, eine Nicht-UUID oder eine überschrittene Grenze ist 422. Fehlendes CSRF ist 403. Eine fremde oder fehlende ID verweigert die ganze Auswahl.

`App::bootstrap_schema` erzeugt keine Fremdschlüssel. Der Rollback-Test baut die Produkt-Tabelle deshalb mit `ON DELETE RESTRICT` neu. Produktiv gelten die Fremdschlüssel der Migration.

Code: `crates/ferrioxid-admin/src/actions.rs`, `html.rs` `stage_bulk`, `show_bulk`, `apply_bulk`. Vorlage: `templates/models/bulk.html`.
