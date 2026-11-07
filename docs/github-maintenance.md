# GitHub maintenance

This page lists settings that live in GitHub, not in Git. Nothing in this file has been enabled remotely. The local repository also has no Git remote configured until one is added manually.

The intended repository is <https://github.com/Iam-Sanja/neravol>. The framework name remains Ferrum. `neravol` is not a Rust crate.

## Repository files

These files are part of the working tree and do nothing until they are committed and GitHub reads them:

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
| Default branch | `main` | Not configured remotely from this task |
| Issues | Enabled | Not configured remotely |
| Discussions | Enabled, if the plan allows it | Not configured remotely |
| Pull requests | Allowed into `main` | Not configured remotely |
| Merge method | Squash merge only | Not configured remotely |
| Merge commits and rebase merge | Disabled | Not configured remotely |
| Delete head branches | Enabled after merge | Not configured remotely |
| Branch protection for `main` | Enabled | Not configured remotely |
| Required pull request review | At least one review, including code owners if desired | Not configured remotely |
| Required status checks | `Rust (ubuntu-latest)`, `Rust (windows-latest)`, `PostgreSQL`, and `Cargo audit` after they have run once | Not configured remotely |
| Force pushes to `main` | Disabled | Not configured remotely |
| Branch deletion of `main` | Disabled | Not configured remotely |
| Private Vulnerability Reporting | Enabled | Not enabled by `SECURITY.md` |
| Secret scanning | Enabled when GitHub offers it | Not configured remotely |
| Dependabot alerts | Enabled | The Dependabot config file does not enable alerts by itself |
| Visibility | Private until a public release is intentional | Not changed remotely |
| ChatGPT GitHub integration | Grant access only if that integration should read this repository | Not configured remotely |

Required status checks cannot be selected until GitHub has recorded at least one run of each job. Do not mark a workflow as required before it has succeeded.

## Access

`@Iam-Sanja` is the current maintainer. Adding another maintainer requires both a GitHub permission change and an update to `GOVERNANCE.md` and `.github/CODEOWNERS`.

## Labels

Issue forms do not assign labels. Create `dependencies` before expecting Dependabot to apply it. Missing labels do not block the update itself.

## What this repository does not do

- It does not create the GitHub repository, push commits, or change visibility.
- It does not publish crates.
- It does not turn milestone labels such as v0.27.1 into releases.
- It does not rewrite Git history. A secret already present in history would remain after `.gitignore` changes.
