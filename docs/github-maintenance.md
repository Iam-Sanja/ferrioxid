# GitHub maintenance

This page records repository settings, contributor workflow and CI maintenance for [Iam-Sanja/fexu-rs](https://github.com/Iam-Sanja/fexu-rs). Settings were checked through GitHub's API on **2026-11-08** against main at `5c9bd7c438179b53d02f36130871ac42f313ef5e`, when the repository was named `Iam-Sanja/ferrioxid`. They may change independently of Git; Markdown cannot enforce them.

The repository is already **public**. Use the [public release security checklist](public-release-checklist.md) to distinguish completed configuration from unresolved historical-secret, licensing, policy and contributor-flow reviews. Public source availability does not imply a stable crates.io release: Cargo `0.1.0` remains pre-release; use the workspace/path dependency as documented in the [README](../README.md).

## Tracked policies and workflows

- [CI](../.github/workflows/ci.yml) validates main and PRs on Ubuntu and Windows, with a separate disposable PostgreSQL 16 job. PostgreSQL runs fresh framework initialization first, then HTTP and migration roundtrips.
- [Security audit](../.github/workflows/security.yml) runs cargo-audit 0.22.2, preserves an unfiltered report and guards the existing RSA exception with an all-target enabled-dependency check.
- [Dependabot](../.github/dependabot.yml) proposes weekly Cargo and GitHub Actions updates; it does not merge them.
- [CODEOWNERS](../.github/CODEOWNERS) routes changes to `@Iam-Sanja`; it grants no permissions and the current ruleset does not require code-owner approval.
- [Issue forms](../.github/ISSUE_TEMPLATE/config.yml) disallow blank issues and link to the security policy; the [PR template](../.github/pull_request_template.md) requests validation and security considerations.
- [CONTRIBUTING](../CONTRIBUTING.md), [SECURITY](../SECURITY.md), [Code of Conduct](../CODE_OF_CONDUCT.md), [Governance](../GOVERNANCE.md) and [MIT license](../LICENSE-MIT) are present. Presence is not a complete accuracy, third-party-license or usable-contact audit. Known stale contributor/security wording is listed in the [open checklist tasks](public-release-checklist.md#1-publication-and-historical-exposure).

The former `.github/workflows/rust.yml` is historical; current instructions refer to `ci.yml` and `security.yml`. Current framework setup uses `fexu.toml`, `.fexu/`, `FEXU_*` and `_fexu_*`; see [the earlier name cleanup](ferrum-legacy-removal.md) for historical mappings and the intentional fresh-database requirement.

## Verified settings snapshot

| Setting | Observed state on 2026-11-08 |
| --- | --- |
| Visibility / default branch | Public / `main` |
| Issues / Discussions | Both enabled; moderation capacity not established by the setting |
| Merge methods | Squash enabled; merge commits and rebase disabled |
| Head-branch cleanup | Automatic deletion enabled |
| Main protection | Active `protect-main`, ID `26812021`, targeting the default branch; effective main rules confirmed |
| Bypass | No bypass actors; API reports current user cannot bypass |
| Required PR | Required; squash only, resolved review threads and linear history |
| Required checks | `Rust (ubuntu-latest)`, `Rust (windows-latest)`, `PostgreSQL`, `Cargo audit`; strict up-to-date policy, integration ID `15368` |
| Approval counts | Zero required approving reviews; code-owner and last-push approval not required |
| Main deletion / force pushes | Blocked by deletion and non-fast-forward rules |
| Private Vulnerability Reporting | Enabled; [private report form](https://github.com/Iam-Sanja/fexu-rs/security/advisories/new) |
| Dependabot alerts / security updates | Alerts enabled; updates enabled and not paused |
| Dependency graph completeness | Separate UI/data coverage review still open |
| Secret Scanning / Push Protection | Both enabled |
| Additional secret coverage | Non-provider patterns and validity checks disabled; assess separately |
| CodeQL | Default setup configured; weekly, default query suite, remote threat model; Rust, Actions and JavaScript/TypeScript language groups |
| CodeQL required for merge | Not in the four required status-check contexts; review its results separately |
| Repository Actions token | Read-only default; cannot approve PR reviews |
| Checked-in workflow permissions | Both workflows explicitly use `contents: read`; no `pull_request_target` execution or repository-secret references |
| Fork approval policy | `first_time_contributors`; not approval of every outside contributor's execution |
| Allowed Actions / global SHA policy | All Actions allowed; SHA pinning not required globally. The two checked-in workflows pin third-party Actions to full SHAs |

The single-maintainer approval configuration avoids requiring a nonexistent independent reviewer. It does not remove the need for the owner's final review. Add stronger reviewer requirements when a real independent reviewer is available; do not weaken checks or add bypasses as a workaround.

The normal merge of main into a topic branch does not violate main's squash-only/linear-history rule: the final PR merge into main must use the permitted method. No direct push or merge action on main is authorized by this document.

## Repeatable verification

Read these authenticated REST endpoints under `/repos/Iam-Sanja/fexu-rs`; follow pagination where the response is a list:

| Endpoint | Evidence |
| --- | --- |
| Repository root and `/branches/main` | Visibility, merge settings, security metadata and protected branch flag |
| `/rulesets?includes_parents=true`, `/rulesets/26812021`, `/rules/branches/main` | Enforcement, target refs, bypasses and actual required contexts |
| `/vulnerability-alerts` | HTTP 204 confirms Dependabot alerts enabled |
| `/automated-security-fixes` | Security-update enablement and pause state |
| `/private-vulnerability-reporting` | Private reporting enablement |
| `/code-scanning/default-setup` | Query suite, languages and schedule |
| `/actions/permissions`, `/actions/permissions/workflow` | Allowed Actions, default token and PR-review approval settings |
| `/actions/permissions/fork-pr-contributor-approval` | Public-fork contributor approval policy |
| `/commits/{sha}/check-runs`, `/actions/runs/{id}/jobs` | Exact latest-head checks, step execution and conclusions |
| `/code-scanning/analyses` with the relevant ref | Actual analyzed commit and categories; PR-diff analysis is not a full-history secret scan |

An unavailable endpoint is not proof of enablement or disablement. The private-repository fork-workflow settings endpoint rejects public repositories, so it cannot establish this repository's public-fork permissions. A successful configuration read does not replace a controlled failing-check exercise, signed-out visitor check or real fork-PR test. Those remain open in the checklist. Do not test protection by actually merging a failing change.

## Security and contributor boundaries

Vulnerabilities go to the [private report form](https://github.com/Iam-Sanja/fexu-rs/security/advisories/new), not public issues, discussions or PRs. `SECURITY.md` still uses conditional wording about the feature; this snapshot supplies the verified route. A GitHub username alone is not proof of a private messaging channel. Finish the policy/contact review before treating every published contact instruction as tested.

Public visibility grants no write access. Fork PRs must not receive privileged credentials or run outsider-controlled code through `pull_request_target`. Review workflow edits, artifact handling and dependency updates deliberately. The read-only default and source-level SHA pins do not establish a repository-wide Actions allowlist. Review returning contributors as well as first-time contributors; inspect the current approval policy rather than assuming all external builds require approval.

`@Iam-Sanja` is the maintainer recorded by Governance and CODEOWNERS. Adding a maintainer requires both permission changes and policy updates. Periodically audit collaborators, apps, repository/environment secrets, runner usage and security alerts. Discussions require moderation. Issue forms do not assign labels; maintain labels such as `dependencies` for Dependabot without assuming a label changes security enforcement.

## Integrated security and naming changes

[PR #27](https://github.com/Iam-Sanja/ferrioxid/pull/27) removed the environment-selected external database seeder. [PR #25](https://github.com/Iam-Sanja/ferrioxid/pull/25) preserved that removal and its regression for both historical/current environment names while completing the Fexu rename. Both are merged into the source baseline above. Do not restore known privileged fixture credentials in a persistent database. Reused databases touched by an earlier seeder need operator review; removing code cannot revoke existing accounts or sessions.

The [CodeQL review](security/codeql-review.md) and [94-alert ledger](security/codeql-alerts.csv) are pinned historical evidence with per-alert reasoning, including the PR #25 follow-up for false positive #95. They must not be interpreted as a current count, blanket suppression list or complete secrets audit. PR #25's final head passed the four required checks and CodeQL; each new PR still needs results for its own latest head.

The repository was private and unprotected during the original GitHub Free setup; that restriction is historical. Main is now protected by the active ruleset. [Issue #12](https://github.com/Iam-Sanja/ferrioxid/issues/12) still reports open despite the observed enforcement configuration; its administrative closure is separate from this documentation PR. The old private Actions billing blockage no longer describes current execution: public PR jobs have run successfully.

## Issue-to-pull-request workflow

Use one issue for a coherent bug or feature. Open a focused branch and PR for the implementation rather than committing directly to `main`.

1. Create or select an issue. Confirm scope, acceptance criteria, and ownership. An issue's assignee is the responsible GitHub account, **not** an indication that an AI assistant is a separate GitHub user.
2. From an up-to-date, clean `main`, create `feat/<slug>`, `fix/<slug>`, or `docs/<slug>`. If uncommitted work already exists on `main`, preserve it before switching; never discard it to make the working tree clean.
3. Commit focused changes in English with Conventional Commits. Include appropriate tests and documentation.
4. Push the topic branch, open a PR against `main`, and reference the issue with `Closes #NN` **only when the PR fully resolves it**.
5. Inspect the diff, run CI, obtain review, and squash-merge after the maintainer approves. Do not merge incomplete or failing PRs to make checks disappear.
6. Delete the merged topic branch when no longer needed. Keep only `main` long-lived; create `release/<version>` only when preparing a real release.

### Parallel work with Cursor and GitHub contributors

A local Cursor agent and a GitHub-connected assistant do **not** automatically share a working tree. GitHub edits become available locally only after an explicit fetch/pull by the developer or an authorized agent. Likewise, local uncommitted changes are invisible on GitHub.

- Give each task a separate issue, branch, and file ownership where possible. Avoid concurrent edits to the same source files.
- Have Cursor report its current branch and `git status` before starting. If it was forbidden from performing Git operations, it is **not** working on a feature branch unless someone created one separately.
- Do not pull, rebase, reset, switch branches, or force-push over uncommitted Cursor work without a deliberate integration plan.
- If CI fixes are committed remotely while Cursor is working locally, review and integrate them only after local work is safely committed on its topic branch.
- Repository renames change remote URLs and references; update local `origin` and policy links after the rename is finalized.

## CI troubleshooting baseline

Formatting runs first. A formatting failure skips later Clippy/tests/build steps; a skipped test step is not a failed Rust test. Inspect actual steps and logs, including rerun attempts, and reproduce the first relevant failure:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

The PostgreSQL job runs all three integration tests against its disposable database. Its success does not prove the Ubuntu or Windows jobs passed. Branch rules require the exact four names above; if workflow names change, maintainers must review the corresponding ruleset contexts rather than removing checks to bypass failures.

The unfiltered cargo-audit report can match optional lockfile packages. The sole existing `RUSTSEC-2023-0071` exception is tracked in [issue #8](https://github.com/Iam-Sanja/ferrioxid/issues/8), which remains open. The workflow must fail if RSA becomes reachable in the enabled workspace graph on any target, and every other advisory remains gated. Reassess the exception when features or SQLx change; never broaden it simply to obtain a green check.

Current CI does not prove complete secret-history scanning, third-party asset licensing, successful fork moderation or safety of all production deployments. Follow the remaining checklist items and report validation limits explicitly. This documentation update changes no security setting, workflow permission, dependency exception or protection rule.
