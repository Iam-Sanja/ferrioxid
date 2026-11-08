# GitHub maintenance

This page documents repository settings, the contributor workflow, and CI maintenance. Settings must be verified in the GitHub UI or API; documentation alone cannot enable them.

The repository has been pushed to GitHub. The local development checkout uses `origin` to track its GitHub remote. GitHub settings and CI results can change independently of this document.

The intended repository is <https://github.com/Iam-Sanja/ferrioxid>. The framework name remains Ferrioxid. `ferrioxid` is not a Rust crate.

## Repository files

These files are tracked on GitHub's default branch:

- `.github/workflows/ci.yml` validates `main` and pull requests on Linux and Windows, and runs a separate PostgreSQL job.
- `.github/workflows/security.yml` runs `cargo audit`.
- `.github/dependabot.yml` opens weekly Cargo and GitHub Actions update requests. It does not merge them.
- `.github/CODEOWNERS` names `@Iam-Sanja`. It does not require that review.
- `.github/ISSUE_TEMPLATE/` and `.github/pull_request_template.md` supply forms after they are on the default branch.
- `SECURITY.md` describes disclosure. It does not turn on Private Vulnerability Reporting.

The previous workflow file `.github/workflows/rust.yml` is replaced by `ci.yml`. It checked only Ubuntu, used an incorrect `cargo fmt` invocation, and did not start PostgreSQL.

## Settings to configure manually

| Setting | Intended value | Status in Git |
|---|---|---|
| Default branch | `main` | Verify in GitHub |
| Issues | Enabled | Verify in GitHub |
| Discussions | Enabled, if the plan allows it | Verify in GitHub |
| Pull requests | Allowed into `main` | Verify in GitHub |
| Merge method | Squash merge only | Verify in GitHub |
| Merge commits and rebase merge | Disabled | Verify in GitHub |
| Delete head branches | Enabled after merge | Verify in GitHub |
| Branch protection for `main` | Enabled | Verify in GitHub |
| Required pull request review | At least one review, including code owners if desired | Verify in GitHub |
| Required status checks | `Rust (ubuntu-latest)`, `Rust (windows-latest)`, `PostgreSQL`, and `Cargo audit` after they have run once | Verify in GitHub |
| Force pushes to `main` | Disabled | Verify in GitHub |
| Branch deletion of `main` | Disabled | Verify in GitHub |
| Private Vulnerability Reporting | Enabled | Not enabled by `SECURITY.md` |
| Secret scanning | Enabled when GitHub offers it | Verify in GitHub |
| Dependabot alerts | Enabled | The Dependabot config file does not enable alerts by itself |
| Visibility | Private until a public release is intentional | Private at initial setup; verify before publishing |
| ChatGPT GitHub integration | Grant access only if that integration should read this repository | Verify in GitHub |

Required status checks should use the exact names reported by GitHub Actions. Select them only after GitHub has recorded the checks; resolve failures before requiring them for merge.

## Access

`@Iam-Sanja` is the current maintainer. Adding another maintainer requires both a GitHub permission change and an update to `GOVERNANCE.md` and `.github/CODEOWNERS`.

## Labels

Issue forms do not assign labels. Create `dependencies` before expecting Dependabot to apply it. Missing labels do not block the update itself.

## What this repository does not do

- It does not create the GitHub repository, push commits, or change visibility.
- It does not publish crates.
- It does not turn milestone labels such as v0.27.1 into releases.
- It does not rewrite Git history. A secret already present in history would remain after `.gitignore` changes.

## Branch protection availability

The repository is currently private. GitHub API queries during the initial setup returned a plan restriction: private-repository rulesets require an eligible paid GitHub plan. GitHub Free supports branch protection on public repositories; paid plans also support private repositories. Do **not** change repository visibility just to satisfy a branch-protection checklist.

Track eligibility and eventual enforcement in [issue #12](https://github.com/Iam-Sanja/ferrioxid/issues/12). Until protection is available, maintainers should voluntarily use pull requests and avoid direct pushes or force pushes to `main`. Once eligible, configure protection in GitHub Settings and verify it actually applies; documentation is not enforcement.

Avoid requiring another person's approval in a single-maintainer repository unless a separate reviewer is available. CI and review rules should be enabled only when the corresponding checks and reviewers exist.

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

The CI workflow runs formatting first. A formatting error causes the subsequent Clippy, tests, and build steps in that job to be skipped; a failed job does **not** mean the skipped Rust tests failed.

Use the job steps and logs to identify the first failure. Reproduce locally with:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

The PostgreSQL integration job runs separately against a disposable database. A green PostgreSQL job does not prove that the whole CI workflow passed.

The Security audit uses `cargo audit` on `Cargo.lock`. Advisory matches can involve transitive or lockfile-only packages; investigate the dependency and feature tree before removing packages or considering a documented exception. Do not hide findings merely to turn a check green.

As of the initial GitHub setup, see the independent tracking issues for [admin formatting](https://github.com/Iam-Sanja/ferrioxid/issues/7) and [RSA dependency audit](https://github.com/Iam-Sanja/ferrioxid/issues/8). These links are historical references; check their current status rather than assuming they are still open.
