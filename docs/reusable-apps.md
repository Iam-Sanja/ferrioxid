# Reusable apps

[Overview](index.md) · Apps: [apps.md](apps.md) · **Implemented:** `examples/notes`.

An app can be in its own crate. It is attached to `ferrioxid`, not to `examples/myshop` and not to private framework modules.

```toml
[dependencies]
ferrioxid = { path = "../ferrioxid/crates/ferrioxid" }
notes = { path = "../notes" }
```

```rust
let app = ferrioxid::App::new(database)
    .install(notes::NotesApp)?
    .seal()?;
```

`examples/notes` registers `notes.list` under `/notes/items`. The `examples/notes/tests/reuse.rs` test also installs a second app and shows that `install` and `seal` do not create a table `_ferrioxid_migrations`. The answer of `/notes/items` is JSON.

## Limitations

- The crate must occur in the binary. Ferrioxid does not load a Dynamic Library and does not read the Crate name from `ferrioxid.toml`.
- Models use `FerrioxidModel` and `registry.model`. A table name remains the name from the model. Two models with the same table are rejected.
- Relationships between apps remain normal foreign keys. The target table must exist. A qualified model name does not change the SQL name.
- App settings are `[custom.<app>]` and `ProjectConfig::custom_settings`. `Registration::project` provides the configuration only if `with_settings` was called before `install`. Framework keys are not replaced.
- External app migrations live in their own directory and are registered through `registry.migrations` and declared in `[app.<name>]`. Version identity comes from the file ID and checksum rather than a separate version field.
- `install` does not change the database.

## Versions, upgrades and removal

`AppConfig::version("1.2.0")` records `major.minor.patch`. `requires("shop", "1.0.0")` refuses installation until that app is already installed at that version or newer. `upgrade_plan` refuses a downgrade and does not apply migrations or write rows. Run `ferrioxid migrate` for schema changes. `uninstall_plan` tells the next build to stop calling `install`. It does not drop tables, rows or migration history, and it does not unload a running process. Removing data needs an explicit migration written for that purpose. The same app type can be installed into two separate `App` values. Ferrioxid still does not load a dynamic library.

```rust
let plan = app.upgrade_plan("catalog", Some("1.2.0"), "1.3.0")?;
assert!(!plan.applies_migrations);
let removal = app.uninstall_plan("catalog")?;
assert!(!removal.drops_tables && !removal.drops_rows);
```
