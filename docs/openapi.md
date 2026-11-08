# OpenAPI and API contracts – draft (NOT implemented)

[Overview](index.md) · **Planned:** Ferrioxid does not generate an OpenAPI specification and does not depend on `utoipa`.

## Goal

Create a correct OpenAPI description from the same model definition and the HTTP routes generated. Not just copy DB fields: request, response and permissions can have different views.

## Planned generator

1. Macro generates DTO metadata (`Create`, `Update`, `Response`) and validation rules.
2. A route registry associates each method and path with its request/response types, status codes and policy.
3. OpenAPI layer composes this data into deterministic schemas.
4. Output is compared with actually registered routes during build/integration tests.

### Contract per endpoint

- Method, path, tags, summary (if available).
- Request Body and Content Types.
- Success: 200/201 and JSON schema.
- Error: 401/403/404/409/422/500 after implemented semantics, no invented status codes.
- Pagination schema only document when implemented.
- Define security schemes from the implemented authentication contract.

## Design rules

- `required`/min/max length traceable from field metadata.
- Hidden/Readonly fields in the correct DTO.
- No API consumer may derive a write permission from a DB schema.
- Versioned API schema, stable operation IDs and deterministic sorting.

## Acceptance tests

OpenAPI-JSON can be parsed/validated; Sample payloads correspond to Rust DTOs; documented and registered routes match; prohibited response fields do not appear. Sources: [api.md](api.md), [validation.md](validation.md), [auth.md](auth.md).
