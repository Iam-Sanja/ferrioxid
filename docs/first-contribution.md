# Your first contribution to Ferrioxid

Thanks for considering a contribution! Ferrioxid is an experimental Rust web framework. You can help without knowing the entire codebase.

## Start small

Good first contributions are a clear documentation correction, a reproducible test for an existing bug, a small example, or a focused improvement to diagnostics. Before starting larger features, [open a discussion](https://github.com/Iam-Sanja/ferrioxid/discussions) or comment on the issue to avoid duplicated work.

Browse [open issues](https://github.com/Iam-Sanja/ferrioxid/issues) and [good first issues](https://github.com/Iam-Sanja/ferrioxid/labels/good%20first%20issue).

## Local setup

1. Install the Rust toolchain specified by `rust-toolchain.toml`, including rustfmt and Clippy.
2. Fork and clone the repository, then create a short-lived branch from `main`.
3. Build and run the SQLite example; no PostgreSQL service is needed for the initial setup.

```bash
git clone https://github.com/YOUR-USERNAME/ferrioxid.git
cd ferrioxid
cargo build --workspace
cargo run -p ferrioxid-cli -- migrate up
cargo run -p myshop
```

Test the running example at `http://127.0.0.1:3000/healthz`.

Ferrioxid includes a local SQLx/SQLite patch. Keep the workspace's `[patch.crates-io]` configuration intact. Read [SQLite setup and consumer guidance](security/sqlite-rustsec-2026-0459.md) before copying workspace dependencies into a standalone project.

## Find the relevant code

| Task | Start here |
|---|---|
| Models, derives | `crates/ferrioxid-core/`, `crates/ferrioxid-macros/` |
| Queries and database behavior | `crates/ferrioxid-db/` |
| HTTP and routing | `crates/ferrioxid-api/` |
| Authentication | `crates/ferrioxid-auth/` |
| Admin pages | `crates/ferrioxid-admin/` |
| CLI and generator | `crates/ferrioxid-cli/` |
| Migrations, fixtures | `crates/ferrioxid-migrations/` |
| Documentation | `docs/` |
| Realistic example | `examples/myshop/` |

## A helpful pull request

- Explain **what problem** is solved, **what changes**, and **how you tested it**.
- Include a regression test when fixing behavior, particularly for migrations and authentication.
- If a feature affects database backends, mention SQLite and PostgreSQL coverage separately.
- Update the corresponding docs; do not claim tests were run if they were skipped.
- Keep the PR focused and link its issue when one exists.

Run before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

The default suite skips ignored PostgreSQL tests. These require an **isolated disposable** database and `DATABASE_URL`; never point destructive tests at an existing application.

Read the full [CONTRIBUTING.md](../CONTRIBUTING.md), [Code of Conduct](../CODE_OF_CONDUCT.md) and [security policy](../SECURITY.md). Security vulnerabilities should be reported privately, not as public issues.

## Not sure where to start?

An issue describing confusing setup instructions, a missing code example, or a precise reproduction of unexpected behavior is a genuinely useful contribution.

You are welcome to ask questions in [Discussions](https://github.com/Iam-Sanja/ferrioxid/discussions) before sending code.
