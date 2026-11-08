# Erweiterbarkeit und Plugins – Entwurf (NICHT implementiert)

[Zur Übersicht](index.md) · **Status:** ausschließlich SOLL. Keine `ferrioxid-plugins`-Crate, keine Hooks oder Laufzeitplugin-Engine in v0.1.

## Ziel und Designentscheidung

**Compile-time registrierte Rust-Erweiterungen** bevorzugen. Keine dynamisch ladbaren Binärplugins ohne konkreten, belegten Bedarf: ABI-Stabilität und Runtime-Sicherheit wären unnötig komplex.

## Mögliche Extension Points (SOLL)

| Extension Point | Zweck | Sicherheitsregel |
|---|---|---|
| `RouteExtension` | Zusätzliche HTTP-Routen | Policy ausdrücklich definieren |
| `ModelHook` | Vor-/Nachaktionen bei CRUD | Transaktionssemantik dokumentieren |
| `Validator` | Domänenregeln | Fehler strukturiert zurückgeben |
| `AdminWidget` | UI-Komponenten | keine Umgehung von Backend-Policy |
| `BackgroundJob` | asynchrone Tasks | explizite Retry-/Idempotenzregel |
| `StorageProvider` | Dateien/Blob-Speicher | Tenant-/ACL-Grenzen durchsetzen |

## Implementierungsreihenfolge

1. Stabile Kernmodule und Public API herstellen.
2. Konkreten Erweiterungsfall auswählen (z. B. Custom Route).
3. Kleinstes passendes Trait definieren, mit verständlichem Lifecycle.
4. Failure-Handling und Tracing festlegen.
5. Testen, dass eine Extension mit **nur** der öffentlichen Ferrioxid-Crate funktioniert.

## Nicht im MVP

Plugin-Marktplatz, dynamisches `.so`/`.dll`-Laden, automatische Code-Sandbox, Netzwerkpaketinstallation, neue Programmier-/Schema-Sprache. Keine Plugins versprechen, bevor der entsprechende Vertrag getestet ist.
