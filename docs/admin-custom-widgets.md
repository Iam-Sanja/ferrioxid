# Eigene Admin-Widgets

[Zur Übersicht](admin-extension-api.md)

## IST

`AdminWidget` ändert die HTML-Darstellung eines sichtbaren Feldes. Es ändert `AdminFieldMeta` nicht. Verborgen, sensibel, Primärschlüssel und Fremdschlüssel werden bei `seal` abgelehnt. Ein Feld hat höchstens ein Widget.

| Art | HTML | Zusätzliche Prüfung beim POST |
|---|---|---|
| `text` | `input type="text"` | die Modellregeln |
| `textarea` | `textarea`, 1 bis 4000 Zeichen | Zeichengrenze des Widgets und die Modellregeln |
| `email` | `input type="email"` | `ferrum_core::is_email` |
| `url` | `input type="url"` | `ferrum_core::is_http_url` |
| `number` | `input type="number"` | Unter- und Obergrenze, danach die Modellregeln |
| `readonly` | Text, kein absendbares Feld | ein mitgesendeter Wert ist `field is not writable` |

```rust
registry
    .widget(ferrum::admin::AdminWidget::textarea("orders", "customer", 120))?
    .widget(ferrum::admin::AdminWidget::email("people", "email"))?
    .widget(ferrum::admin::AdminWidget::number("products", "price_cents", Some(0), Some(1_000_000)))?
    .widget(ferrum::admin::AdminWidget::readonly("orders", "status"))?;
```

Tera escaped den Wert. Ein Fehler bleibt am Formular und schreibt nicht. Entwürfe, CSRF und die drei Speicherziele bleiben die des normalen Formulars. Boolesche Felder und Fremdschlüssel behalten ihre eingebauten Kontrollen.
