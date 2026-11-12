# Release readiness

Checked on 2026-11-12 from branch `work/api-quality`. Nothing in this branch was published to crates.io.

## Versions

| Name | Value | Meaning |
|---|---|---|
| Crate version | `0.1.0` | Cargo package version for every workspace member. Experimental. |
| HTTP contract | OpenAPI `info.version` = `1` | Generated list, problem JSON and read/write schemas described in [api.md](api.md). |
| Template version | unchanged | `ferrioxid upgrade` does not rewrite an existing project to match a new crate. |

Before 1.0, a new `0.1.x` may change Rust APIs, migration history and HTTP fields. The HTTP document version moves only when a generated request or response changes in a way an existing client cannot ignore. This branch moved the list body from `truncated` to `next` and `has_more`. That is a breaking HTTP change inside contract version `1` because no stable crate has been published. Callers of the old list body need to read `has_more`.

## Crate graph and publication order

Publication has to follow the path dependencies. A later crate cannot be published before the crates it uses.

1. `ferrioxid-core`
2. `ferrioxid-macros`
3. `ferrioxid-db`
4. `ferrioxid-static`, `ferrioxid-middleware`, `ferrioxid-storage`, `ferrioxid-mail`
5. `ferrioxid-migrations`, `ferrioxid-forms`, `ferrioxid-jobs`, `ferrioxid-auth`
6. `ferrioxid-admin`
7. `ferrioxid-api`
8. `ferrioxid`
9. `ferrioxid-cli`

`examples/myshop`, `examples/notes` and `examples/desk` are `publish = false`. They keep path dependencies.

Every framework crate still depends on its siblings with `path`. `cargo publish` from this workspace cannot succeed until those dependencies are version requirements for crates that already exist on crates.io. That rewrite is a release commit. It is not in this branch. `cargo package` was not treated as a green publish check for that reason.

Workspace metadata now sets `description`, `repository` and `homepage`. License is MIT. There is no `rust-version` key. The toolchain file selects stable Rust. A minimum supported version was not measured here.

## Install

Until a crate is published, install from a checkout:

```powershell
cargo install --path crates/ferrioxid-cli
```

```bash
cargo install --path crates/ferrioxid-cli
```

The same command is the Linux and macOS form. `cargo install ferrioxid-cli` is not a supported command yet because the crate is not on crates.io.

## Upgrade

There is no previous crates.io release and no git tag that represents a published `0.1.0`. An upgrade test from an older published crate cannot be run. `ferrioxid upgrade` still only plans project-file changes and does not migrate a database. Schema changes stay `ferrioxid migrate up`, run by hand. Rollback of a migration is `ferrioxid migrate down` where that command already exists, on a disposable database.

## What this branch does not do

- It does not publish, tag or sign release artifacts.
- It does not label the project 1.0 or production-ready.
- It does not merge pull requests 97, 98, 99 or 100. Those branches are independent. See the dependency note below.
- It does not start the Compose stack when Docker is absent.

## Dependencies on pull requests 97–100

| PR | Branch | What this branch needs from it | What this branch does instead |
|---|---|---|---|
| 97 | `fix/runtime-security-docker` | Peer allow-list for `X-Forwarded-For`, embedded auth/job migrations | `check --deploy` fails when `FERRIOXID_TRUST_PROXY=1` because this branch trusts every peer. Migrations are still read from the existing files. |
| 98 | `feat/orm-migrations-lifecycle` | Grouped reports, JSON columns, `migrate plan` | Not included. List filters use the existing `Select`. |
| 99 | `feat/native-admin` | Per-record audit timeline | Not included. The Playwright test only opens `/admin/login`. |
| 100 | `feat/developer-apps` | One project file and conflicting env aliases | Not included. `ferrioxid.toml` behavior stays as on main. |

Do not merge those branches into this one to make the checks pass. The integration order is documented on the runtime branch, not here.
