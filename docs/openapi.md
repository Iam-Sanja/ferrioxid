# OpenAPI and API contracts – draft (NOT implemented)

[For overview](index.md) · **Status:** Only REQUIRED.] v0.1 does not use `utoipa` and does not provide an OpenAPI specification.

## Ziel

Create a correct OpenAPI description from the same model definition and the HTTP routes generated. Not just copy DB fields: request, response and rights can have different views.

## Geplanter Generator

1. Macro generates DTO metadata (`Create`, `Update`, `Response`) and validation rules.
2. Route Registry verknüpft Methode, Pfad, Request-/Response-Typ, Statuscodes und Policy.
3. OpenAPI layer composes this data into deterministic schemes.
4. Output is compared with actually registered routes during build/integration tests.

### Vertrag pro Endpoint

- Method, path, tags, summary (if available).
- Request Body and Content Types.
- Success: 200/201 and JSON scheme.
- Error: 401/403/404/409/422/500 after implemented semantics, no invented status codes.
- Pagination scheme only document when implemented.
- Define Security Schemes only after implemented Auth.

## Designregeln

- `required`/min/max length traceable from field metadata.
- Hidden/Readonly fields in the correct DTO.
- No API consumer may derive a write permission from a DB schema.
- Versioned API schema, stable operation IDs and deterministic sorting.

## Abnahmetests

OpenAPI-JSON can be parsed/validated; Sample payloads correspond to Rust DTOs; documented and registered routes match; prohibited response fields do not appear. Sources: [api.md](api.md), [validation.md](validation.md), [auth.md](auth.md)].
