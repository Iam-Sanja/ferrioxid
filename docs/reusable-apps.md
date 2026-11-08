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
