# Listenkonfiguration

[Zur Übersicht](admin.md) · Vergleich: [admin-django-parity.md](admin-django-parity.md)

**Stand:** v0.17. Die Cargo-Version bleibt `0.1.0`.

Die Liste liest dieselben `AdminFieldMeta`- und `AdminModelMeta`-Werte wie Formular und JSON-API. Es gibt keine zweite Konfiguration.

| Django-Begriff | Ferrioxid-Attribut | Wirkung |
|---|---|---|
| `list_display` | `#[admin(list_display)]` | Spalten in Struct-Reihenfolge. `id` bleibt immer eine Spalte. Ohne Markierung: erstes sichtbares Textfeld, sonst ein anderes sichtbares Feld, plus `id` |
| `list_display_links` | `#[admin(list_link)]` | Diese Spalten verlinken auf das Detail. `list_link` verlangt `list_display`, außer bei `id`. Ohne Markierung verlinkt die erste gelistete Spalte, die nicht `id` ist, sonst `id` |
| `ordering` | `admin_ordering = "name"` oder `"-name"` | Standard, wenn die Anfrage kein `sort` sendet. Das Minus sortiert absteigend. Die Spalte muss `sortable` oder `id` sein. Ein gesendetes `sort` ersetzt die Vorgabe. Ein unbekannter Name ist 422 |
| `list_per_page` | `admin_per_page = 25` | HTML-Seitengröße von 1 bis 100. Vorgabe 25. Der Handler deckelt erneut bei 100. Es gibt keinen Parameter `limit` und kein „alle zeigen“ |
| `list_filter` | `#[admin(filterable)]` | GET-Feld `filter.{spalte}` |
| `search_fields` | `#[admin(searchable)]` | Parameter `q`, nur String-Spalten |

`hidden` und sensible Namen fallen aus Anzeige, Suche, Sortierung und Filter. Sortierung und Filter bauen keine SQL-Fragmente aus der Anfrage. Die JSON-Liste bleibt bei höchstens 100 Zeilen und nutzt `admin_per_page` nicht.

Gültige Suche und Filter bleiben in den Folgelinks. Ein neuer Filter setzt die Seite zurück, weil `page` nicht mitgeschickt wird.
