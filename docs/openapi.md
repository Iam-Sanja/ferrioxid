# OpenAPI 3.1

[Overview](index.md) · **Implemented** in `crates/ferrioxid-api/src/openapi.rs`. The document is built from `ModelDescriptor`, `ColumnSpec`, `NumericBound`, `FieldContract` and `AdminFieldMeta`. It does not keep a second model description.

`App` publishes `GET /openapi.json` and, unless turned off, `GET /docs`. Admin routes are not included. `App::document` accepts an operation only when its path is already registered.

```rust
let app = ferrioxid::App::new(db)
    .register::<Product>()
    .openapi(ferrioxid::OpenApiOptions {
        enabled: true,
        json_path: "/openapi.json".to_owned(),
        ui_path: Some("/docs".to_owned()),
        title: "Shop".to_owned(),
    });
let router = app.into_router();
```

`without_openapi` omits both routes. A path that collides with a registered route is rejected.

## What the document contains

- Generated model routes: `GET` and `POST` on the collection, `GET`, `PATCH` and `DELETE` on `/{id}`. `PUT` is omitted because the generated handlers do not implement it.
- Create, update and response schemas. Readonly fields stay on create and leave the update schema. Primary keys and managed timestamps stay on the response.
- Required, nullable, decimal precision and scale, choice enums, `minLength`, `maxLength`, numeric `minimum` and `maximum`, and foreign-key relation metadata.
- The list response is `data`, `limit`, `next` and `has_more`. Query parameters are `limit` (1..=100, default 100), `cursor`, `sort` and repeated `filter`. Sort and filter names are the admin allow-list, plus `id` for sort. A `write_only` or secret-named field is absent from the response schema. `contains` is text only. Offset is not a parameter.
- Session cookie, anonymous cookie and `x-csrf-token`. Cookie writes say that Bearer authorization is not a session.
- Problem JSON for 401, 403, 404, 422 and 500 where those statuses are produced. 409 is not documented because the handlers do not return it.
- `/healthz` and the mounted `/auth` routes.

`App::document` adds `GET`, `POST`, `PUT`, `PATCH` or `DELETE` for a route the application registered itself. An unknown path, an `/admin` path, or a duplicated operation id fails when the router is built.

The explorer is a page served by the same process. It fetches the JSON document and can send a same-origin request. It does not load a third-party script.

## Example assertion

```rust
let document = client.get("/openapi.json").await?.json()?;
assert_eq!(document["openapi"], "3.1.0");
assert!(document["paths"].get("/admin").is_none());
assert_eq!(
    document["components"]["schemas"]["Offer"]["properties"]["price"]["x-ferrioxid-decimal"]["scale"],
    2
);
```

The executable copy is `crates/ferrioxid/tests/openapi_client.rs`.
