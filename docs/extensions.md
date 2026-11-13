# Extensibility and plugins – design (NOT implemented)

[Overview](index.md) · **Planned:** a general plugin-hook API. There is no `fexu-plugins` crate or dynamic plugin engine. Typed apps, admin extensions, storage backends and job handlers already have the specific APIs described in their own guides.

## Objective and design decision

**Prefer compile-time registered Rust extensions**. No dynamically loadable binary plugins without a specific, proven need: ABI stability and runtime security would be unnecessarily complex.

## Possible Extension Points (planned)

| Extension Point | Purpose | Security rule |
|---|---|---|
| `RouteExtension` |Additional HTTP routes|Defining policy explicitly|
| `ModelHook` | Before/after CRUD hooks | Document transaction semantics |
| `Validator` |Domain Rules|Return error structured|
| `AdminWidget` | UI-Components |No circumvention of backend policy|
| `BackgroundJob` | Asynchronous tasks | Explicit retry and idempotency rules |
| `StorageProvider` | File/blob storage | Enforce tenant and ACL boundaries |

## Implementation order

1. Create stable core modules and public API.
2. Select a specific extension case (e.g. Custom Route).
3. Define the smallest suitable trait, with an understandable lifecycle.
4. Failure handling and tracing.
5. Testing that an extension works with **only** the public Fexu crate.

## Not in the MVP

Plugin marketplace, dynamic `.so`/`.dll` loading, automatic code sandbox, network packet installation, new programming/schema language. No plugins promise before the corresponding contract is tested.
