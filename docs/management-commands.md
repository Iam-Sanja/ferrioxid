# Management commands

[Overview](index.md) · **Implemented in this pull request:** `crates/ferrioxid-migrations/src/management.rs`, `ferrioxid manage`, and the project binary `ferrioxid-manage`.

Rust project code cannot be loaded into the already compiled `ferrioxid` binary. A project therefore compiles its own commands into `src/bin/ferrioxid-manage.rs`. `ferrioxid manage <name>` checks that `<name>` is a lowercase identifier such as `seed-shop` and then starts:

```text
cargo run --quiet --bin ferrioxid-manage -- <name> ...
```

`cargo` and `ferrioxid-manage` are fixed arguments. The command name and its options are separate arguments, not one shell string. There is no plugin loader and no execution of fixture contents.

## Registry

```rust
use ferrioxid::async_trait::async_trait;
use ferrioxid::management::{
    ArgSpec, CommandError, CommandSpec, Invocation, ManagementCommand, Registry,
};

struct SeedShop;

#[async_trait]
impl ManagementCommand for SeedShop {
    fn spec(&self) -> CommandSpec {
        CommandSpec {
            name: "seed-shop",
            about: "Upserts the catalog fixture.",
            args: &[ArgSpec {
                name: "file",
                help: "Fixture path inside this project.",
                value: true,
                required: false,
            }],
        }
    }

    async fn run(&self, invocation: &Invocation<'_>) -> Result<(), CommandError> {
        let db = invocation.database().await?;
        let _ = db;
        Ok(())
    }
}

let mut registry = Registry::new();
registry.register(SeedShop)?;
let code = registry.execute(&project, &["seed-shop".into()]).await;
```

`register` rejects a duplicate name and a name that is not a lowercase identifier. `execute` prints help for no arguments, `--help`, or `-h`, and returns `0`. An unknown command prints help and returns `2` without opening the database. A missing required option also returns `2` before `run`. A command returns `1` when its own work fails.

`invocation.database()` connects only when the command calls it. `--database-url` overrides `ferrioxid.toml` and `DATABASE_URL` for that process. The URL is not written to the error text. Other options are those declared on the command. Positional arguments are rejected.

Exit codes from the project binary are the exit codes of `ferrioxid manage`. A name such as `../secret` is rejected by the CLI with exit code `2` and does not start `cargo`.

## Generated projects

`ferrioxid new project` writes `src/project/management.rs` and `src/bin/ferrioxid-manage.rs`. The sample command is `about`. It prints the project name and does not open the database.

```powershell
ferrioxid manage
ferrioxid manage about
ferrioxid manage about --name Shop
```

An existing project does not gain this binary by itself. Add the two files, or generate a new project and copy them. `examples/notes` has no management binary.

## myshop

`examples/myshop` registers `seed-shop`. It applies the myshop migrations and then upserts `fixtures/catalog.json`. A relative fixture path is resolved from the myshop crate directory and cannot contain `..`. An absolute path is accepted only when its parent is the system temp directory, so tests can use a unique file without writing into the source tree. Running the command again updates the same primary keys and does not add rows. `--insert` switches to the insert-only fixture mode, so an existing primary key fails and the transaction rolls the file back. `--database-url` selects the database for that run. In this workspace, `ferrioxid.toml` lives at the repository root and points its migrations directory at `examples/myshop/migrations`.

```powershell
ferrioxid manage seed-shop
ferrioxid manage seed-shop --file fixtures/catalog.json
ferrioxid manage seed-shop --insert --file fixtures/catalog.json
```

`examples/myshop/tests/management_command.rs` runs the registry twice against a temporary SQLite file, checks that a second run keeps two products, checks that a conflicting `--insert` leaves the category and product counts unchanged, and reads the rows again after the connection is closed. `postgres_fixture_roundtrip` remains the ignored PostgreSQL fixture test. This command was not run against PostgreSQL locally.
