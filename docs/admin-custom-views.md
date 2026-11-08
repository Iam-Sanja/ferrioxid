# Eigene Admin-Seiten

[Zur Übersicht](admin-extension-api.md)

## Route

Eine Seite heißt `/admin/pages/{key}`. `key` passt auf `[a-z][a-z0-9_]{0,39}`. Reserviert sind `admin`, `api`, `assets`, `audit`, `directory`, `login`, `logout`, `models`, `pages` und `preferences`. Ein doppelter oder reservierter Name macht `view` zu einem `ExtensionError`. Eine unbekannte Seite ist 404, sobald eine Sitzung mit Staff vorliegt. Ohne Sitzung folgt die Weiterleitung zur Anmeldung.

## Handler

```rust
struct OrderReport;

#[ferrioxid::async_trait::async_trait]
impl ferrioxid::admin::ViewHandler for OrderReport {
    async fn handle(
        &self,
        request: &ferrioxid::admin::ViewRequest<'_>,
    ) -> Result<ferrioxid::admin::ViewPage, ferrioxid::admin::ExtensionError> {
        let listed = Order::list(request.db).await.map_err(|_| {
            ferrioxid::admin::ExtensionError::new("extension.view", "orders could not be read")
        })?;
        let mut data = serde_json::Map::new();
        data.insert("rows".to_owned(), serde_json::json!(listed.data.len()));
        Ok(ferrioxid::admin::ViewPage::new(data))
    }
}
```

Ferrioxid prüft vor `handle` die Sitzung, `is_staff` und `AuthService::holds` für den registrierten Code. Ein Superuser besteht `holds`, bleibt aber an `is_staff` gebunden. POST ohne gültiges CSRF ist 403 und ruft den Handler nicht auf. Fehlt `on_post`, ist POST 405.

`ViewPage` liefert Daten für die registrierte Vorlage. `notice` und `alert` werden gekürzt und von Tera escaped. Die Vorlage wählt der Entwickler bei der Registrierung, nicht die Anfrage.

Die Seitenleiste zeigt `navigation(true)` nur für Codes, die der aktuelle Benutzer hält. Das gilt auf der Übersicht, in Listen, Formularen und auf der eigenen Seite.
