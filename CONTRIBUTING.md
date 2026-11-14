# Contributing to Fexu

Fexu is a pre-release Rust full-stack web framework. The current name is Fexu. Workspace crates keep names such as `fexu`, `fexu-api`, and `fexu-cli`. The Cargo version is `0.1.0`. Milestone labels such as v0.27.1 describe internal progress. They are not Cargo releases and they are not an installable crate named `fexu`.

Product documentation in `docs/` is currently written in German. New repository policy, GitHub templates, and contributor instructions are written in English.

## Expected behavior

- Be respectful. The [Code of Conduct](CODE_OF_CONDUCT.md) applies to issues, pull requests, and discussions.
- Report security vulnerabilities through [SECURITY.md](SECURITY.md). Do not file them as public issues.
- Keep changes focused. Do not rename the framework or rewrite unrelated modules in a contribution.
- Do not commit secrets, local databases, `.env` files, or generated `target/` output.
- Do not modify another person's application database, including databases outside this repository.

## Local setup

Install the stable Rust toolchain. This repository pins it in `rust-toolchain.toml` and includes `rustfmt` and `clippy`.

```bash
rustup show
cargo --version
```

From the repository root:

```bash
cargo build --workspace
cargo test --workspace
```

SQLite is the default database. No database server is required for `cargo test --workspace`. PostgreSQL is optional. The ignored tests `postgres_http_roundtrip` and `postgres_migration_roundtrip` run only when `DATABASE_URL` points at a disposable PostgreSQL database and the tests are selected with `--ignored`.

On a machine that already has the dependency cache, the same checks can use `--offline`. A clean environment must be allowed to download crates. Do not put `--offline` in GitHub Actions.

Windows generator tests start nested Cargo builds. Those child processes cap `CARGO_BUILD_JOBS` at 2. Leave that cap in place. An unlimited nested release build has crashed `rustc` 1.98.1 on Windows.

## Checks before a pull request

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

`cargo fmt --all` rewrites formatting. Run the check form before opening a pull request. Clippy denies warnings for the workspace and all targets.

## Repository map

| Path | Role |
|---|---|
| `crates/fexu` | Public framework facade |
| `crates/fexu-api` | HTTP application, routes, views, and app registry |
| `crates/fexu-cli` | `fexu` developer command |
| `crates/fexu-db` | SeaQuery SQL generation, rusqlite SQLite driver and sqlx-postgres execution |
| `crates/fexu-migrations` | Project settings and migrations |
| `crates/fexu-auth` | Accounts, sessions, and password reset |
| `crates/fexu-admin` | Server-rendered admin |
| `crates/fexu-jobs` | Persistent background jobs |
| `crates/fexu-mail` | Mail delivery |
| `examples/myshop` | Worked example application |
| `examples/notes` | Small application and worker example |
| `docs/` | Product documentation, currently German |

`Cargo.lock` is source-controlled. Do not delete it.

## Branches

`main` is the only long-lived branch. Do not add a permanent `develop` branch.

Create a branch from `main`:

- `feat/<description>` for features
- `fix/<description>` for bug fixes
- `docs/<description>` for documentation
- `refactor/<description>` for internal changes
- `test/<description>` for tests
- `release/<version>` only while preparing an actual release
- `hotfix/<description>` for an urgent fix to a released version

There is no stable release yet, so `hotfix/` and `release/` should be rare.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/) for new commits. Do not rewrite old German commit messages.

```text
feat(admin): implement app-based sidebar navigation
fix(jobs): handle worker lease expiration
docs: improve application registry guide
test(cli): cover migration errors
ci: add Windows and Linux checks
```

## Pull requests

1. Branch from `main`.
2. Implement the change and run the checks above.
3. Open a pull request against `main`.
4. Wait for CI and review.
5. The maintainer squash-merges accepted work.
6. The merged branch is deleted.

GitHub branch protection is configured in the GitHub UI, not by a file in this repository. See [docs/github-maintenance.md](docs/github-maintenance.md).

## Bug reports and feature proposals

Use the GitHub issue forms. A bug report needs the framework revision, operating system, Rust version, database backend, reproduction steps, and sanitized logs. A feature request needs the problem, the proposed developer experience, alternatives, and compatibility impact.

## Security

Follow [SECURITY.md](SECURITY.md). Do not publish exploit details in an issue or pull request.

## Database migrations

- Add a new migration file. Do not edit a migration that has already been applied in a real project.
- `fexu migrate down` is not implemented. Do not document it as available.
- `fexu migrate up` applies migrations. It is not a database reset.
- Tests must use temporary SQLite files or an explicitly disposable PostgreSQL database.
- Do not point tests or examples at a developer's real database.
- Existing generated projects must keep working. A contribution that changes scaffold output needs a compatibility note.

## Compatibility

Preserve public crate names and the Cargo version unless the maintainer accepts an explicit breaking-change proposal. Internal milestones are not semver releases. Call out behavior changes for settings, routes, migrations, authentication, and jobs in the pull request.

## Getting started with the CLI

The commands below match the current `fexu-cli` binary. They are not a published install of a crate named `fexu`.

```bash
cargo run -p fexu-cli -- new demo --template fullstack --fexu-path .
cargo run -p fexu-cli -- migrate up
cargo run -p fexu-cli -- doctor
cargo run -p fexu-cli -- dev
```

`fexu dev` does not apply migrations and does not start a background worker. Create an admin user with `fexu admin create-user`. The password is prompted. There is no default password.
