# Admin and developer workflow

This branch extends the existing admin, forms, app registry, views and CLI. It does not add a second admin engine and it does not load plugins at runtime.

## Issue status

| Issue | Delivered here | Still open |
|---|---|---|
| #79 | `ModelAdmin` checks list, search, filter, ordering, readonly, hidden, fieldsets, page size and an extra action permission against `AdminModelMeta`. `seal` rejects a configured foreign key whose table is not registered. Reads and writes use the sealed field list. | No browser or screen-reader run. Compile-time rejection of a bad `ModelAdmin` list still happens in `check`, not in the type system. Per-record audit stays on the separate admin branch. |
| #80 | `lookup_page` covers 0, 1, 100 and 1000 rows, search, and `QueryScope::None` without returning foreign rows. `shape_of` distinguishes a foreign key from a unique one-to-one column. `replace_links` replaces many-to-many rows in one transaction and a rollback test keeps the previous row. `bulk_targets` rejects an id outside the visible set. The HTML select says when only the first 100 matches are shown and links to the existing paged picker. | The HTML admin does not yet edit a many-to-many widget. One-to-one is enforced by the unique column, not by a separate editor. No mobile or screen-reader pass. PostgreSQL runtime is ignored unless `DATABASE_URL` points at an approved test database. |
| #84 | URL-encoded and JSON bodies use `bind_submission`. Duplicates, forged readonly names, Unicode length, relation markup and image byte checks are tested. `plan_formset` counts creates, updates and deletes. | The plan does not open the transaction; the caller does. There is no image processing pipeline and no remote storage. |
| #86 | Versioned `AppConfig`, ordered `requires`, `upgrade_plan` and `uninstall_plan`. The same app type installs into two `App` values. Uninstall does not drop tables or rows. | Packaging is a Cargo crate plus `install`, not a published version archive. Two generated projects are not compiled in this change. Downgrade of applied SQL stays a manual migration. |
| #89 | `scoped_list`, `scoped_get`, local pagination links and consume-once session flashes in `_ferrioxid_flashes`. Templates escape the text. | No sitemap, robots, redirects or conditional HTTP responses. Flash storage is explicit; `dev` does not create the table. |
| #91 | `doctor` warns on a busy loopback port and reports declared apps. `dev` stops when that port is taken and prints that it does not migrate. | No timed clean-room comparison against Cot, Runique or Loco. The Ubuntu clean-room job is not added here. Generated projects are still covered by the existing CLI tests. |

## Dependencies on open pull requests

PRs #97–#100 are not merged into this branch. This change does not copy their commits.

- #97 embeds auth migrations. A future flash migration, if moved into auth, must be added to that embed list. This branch creates `_ferrioxid_flashes` only when the application calls `create_flash_table`.
- #98 adds `load_one_to_one`. This branch classifies a unique foreign key locally and does not call that function.
- #99 is the per-record audit timeline. Object permissions here stay on `QueryScope` and `holds`.
- #100 accepts one legacy project file. App registration here still reads the current `ferrioxid.toml` apps list.

## Tests

SQLite covers the lookup sizes, many-to-many rollback, one-to-one uniqueness, flash consume-once, form rejection and two in-memory app installs. PostgreSQL tests for lookup and flash are `#[ignore]` and use `DisposablePostgres` when `DATABASE_URL` is an approved administrator database.
