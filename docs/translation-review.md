# Translation review

All 28 protected-source entries from the initial translation pass have been translated and reviewed. No entries remain open. Code fences, Rust identifiers, CLI argument names, legacy configuration keys, database names and migration IDs were retained. The README setup commands were corrected separately to create a standalone project outside the framework workspace.

| Entry | Reviewed page | Resolution |
|---|---|---|
| 1 | [admin-audit-log.md](admin-audit-log.md) | Resolved: Sensitive-field exclusions and audit-summary limits. |
| 2 | [admin-bulk-actions.md](admin-bulk-actions.md) | Resolved: Overview and custom-action navigation. |
| 3 | [admin-custom-views.md](admin-custom-views.md) | Resolved: Custom page keys, reserved names and authentication behavior. |
| 4 | [admin-design-system.md](admin-design-system.md) | Resolved: Color roles and contrast requirements. |
| 5 | [admin-templates.md](admin-templates.md) | Resolved: Shared template context and password-field handling. |
| 6 | [admin-ux-backlog.md](admin-ux-backlog.md) | Resolved: Responsive layout and accessibility requirements. |
| 7 | [architecture.md](architecture.md) | Resolved: Physical crates and dependency boundaries. |
| 8 | [architecture.md](architecture.md) | Resolved: Named routes, extensions and reserved paths. |
| 9 | [constraints.md](constraints.md) | Resolved: Model metadata and cross-model validation. |
| 10 | [database.md](database.md) | Resolved: Column metadata and temporal SQL types. |
| 11 | [database.md](database.md) | Resolved: SQLite tests and explicit PostgreSQL prerequisites. |
| 12 | [forms.md](forms.md) | Resolved: Form values, helpers and unknown-field handling. |
| 13 | [forms.md](forms.md) | Resolved: Stable validation codes and localized messages. |
| 14 | [implementation-guide.md](implementation-guide.md) | Resolved: Required reading and migration safety. |
| 15 | [implementation-guide.md](implementation-guide.md) | Resolved: Module responsibilities and implementation limits. |
| 16 | [mail.md](mail.md) | Resolved: Legacy mail environment-variable overrides. |
| 17 | [migrations.md](migrations.md) | Resolved: Operation tags and snapshot validation. |
| 18 | [migrations.md](migrations.md) | Resolved: Operations permitted in a baseline chain. |
| 19 | [migrations.md](migrations.md) | Resolved: Live-schema comparison and default normalization. |
| 20 | [project-templates.md](project-templates.md) | Resolved: Generator markers and duplicate-model behavior. |
| 21 | [settings.md](settings.md) | Resolved: Configuration sections and default values. |
| 22 | [settings.md](settings.md) | Resolved: Production runtime validation. |
| 23 | [staticfiles.md](staticfiles.md) | Resolved: Static route and response headers. |
| 24 | [testing.md](testing.md) | Resolved: Job, worker and generator coverage. |
| 25 | [transactions.md](transactions.md) | Resolved: Commit and rollback behavior. |
| 26 | [urls.md](urls.md) | Resolved: Reserved framework routes. |
| 27 | [views.md](views.md) | Resolved: HTML/JSON content negotiation and validation details. |
| 28 | [views.md](views.md) | Resolved: Locale selection, error redaction and logging. |

German text inside preserved examples and quoted localization values is intentional. This review records documentation changes; it does not certify unrelated runtime behavior or repeat the repository audit.
