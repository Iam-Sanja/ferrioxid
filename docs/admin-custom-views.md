# Custom admin pages

[Overview](admin-extension-api.md)

## Route

A custom page is served at `/admin/pages/{key}`. `key` must match `[a-z][a-z0-9_]{0,39}`. Reserved names are `admin`, `api`, `assets`, `audit`, `directory`, `login`, `logout`, `models`, `pages` and `preferences`. Passing a duplicate or reserved name to `view` returns an `ExtensionError`. An unknown page returns 404 for an authenticated staff user; a request without a session redirects to login.

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

Before calling `handle`, Ferrioxid checks the session, `is_staff` and `AuthService::holds` for the registered permission code. Superusers satisfy `holds` but still require `is_staff`. A POST without valid CSRF returns 403 without calling the handler. A page without `on_post` returns 405 for POST.

`ViewPage` provides data for the registered template. `notice` and `alert` are shortened and escaped by Tera. The template is chosen by the developer during registration, not the request.

The sidebar includes pages registered with `navigation(true)` only when the current user holds the required permission. This applies to the dashboard, lists, forms and custom pages.
