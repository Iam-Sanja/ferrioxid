# Extensibility and plugins – design (NOT implemented)

[For overview](index.md) · **Status:** Only REQUIRED.] No `ferrioxid-plugins` crate, no hooks or runtime plugin engine in v0.1.

## Objective and design decision

**Prefer compile-time registered Rust extensions**. No dynamically loadable binary plugins without a specific, proven need: ABI stability and runtime security would be unnecessarily complex.

## Possible Extension Points (reference)

| Extension Point | Zweck | Sicherheitsregel |
|---|---|---|
| `RouteExtension` |Additional HTTP routes|Defining policy explicitly|
| `ModelHook` |Pre- and post-actions at CRUD| Transaktionssemantik dokumentieren |
| `Validator` |Domain Rules|Return error structured|
| `AdminWidget` | UI-Komponenten |No circumvention of backend policy|
| `BackgroundJob` | asynchrone Tasks | explizite Retry-/Idempotenzregel |
| `StorageProvider` |Files/blob memory|Enforce tenant/ACL boundaries|

## Implementation order

1. Create stable core modules and public API.
2. Select a specific extension case (e.g. Custom Route).
3. Define the smallest suitable trait, with an understandable lifecycle.
4. Failure handling and tracing.
5. Testing that an extension works with **only** the public ferrioxide crate.

## Not in the MVP

Plugin marketplace, dynamic `.so`/`.dll` loading, automatic code sandbox, network packet installation, new programming/schema language. No plugins promise before the corresponding contract is tested.
