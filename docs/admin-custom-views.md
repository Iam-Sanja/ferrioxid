# Eigene Admin-Seiten

[For an overview](admin-extension-api.md)]

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

Ferrioxide checks the session before `handle`, `is_staff` and `AuthService::holds` for the registered code. A superuser exists `holds`, but remains bound to `is_staff`. POST without valid CSRF is 403 and does not call the trader. If `on_post` is missing, POST 405 is.

`ViewPage` provides data for the registered template. `notice` and `alert` are shortened and escaped by Tera. The template is chosen by the developer during registration, not the request.

The sidebar shows `navigation(true)` only for codes that the current user holds. This applies on the overview, in lists, forms and on the own page.
