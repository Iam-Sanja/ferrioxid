# Public release security and contributor setup

This checklist applies to `Iam-Sanja/ferrioxid`. It must be completed **before** making the repository public and immediately afterward where GitHub Free only enables the feature for public repositories. A README or policy file alone does **not** enable GitHub protection settings.

## 1. Before changing visibility

- [ ] Confirm that **only ferrioxid** will become public; the separate legacy `ferrum` repository must remain untouched.
- [ ] Review the entire repository, full Git history (including orphaned/reachable objects, all PR branches and tags), closed PR discussions, CI logs, and downloadable artifacts for tokens, credentials, private keys, database dumps, personal information and proprietary material. Inspect any previous test projects if they were ever committed. `.gitignore` does not remove an already committed secret.
- [ ] Run a dedicated history-aware scanner (for example `gitleaks git --log-opts="--all"` on an up-to-date **full** clone, using the current supported gitleaks syntax). Triage findings; never upload unredacted results. GitHub's native secret scanning should be enabled as a second independent check when available.
- [ ] Rotate and revoke any actual credentials found **before** publishing. Removing a file from the latest commit is not sufficient. Do not rewrite published Git history casually; assess disclosure and coordinate any required removal.
- [ ] Review license and third-party asset permissions. `LICENSE-MIT` exists; check third-party fonts, screenshots, templates, JS/CSS and copied examples independently.
- [ ] Confirm `SECURITY.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `GOVERNANCE.md`, `.github/CODEOWNERS`, issue forms and PR template remain accurate.
- [ ] Inspect Settings → Secrets and variables → Actions and all environments for unneeded credentials; grant no repository secret to untrusted pull-request builds.
- [ ] Document that release/demo credentials are **not** production defaults; keep sample databases bound to loopback and do not deploy anonymously writable demo endpoints to the internet.
- [ ] Review existing Actions workflow logs: GitHub makes their history and logs public when repository visibility changes.
- [ ] Record current settings and plan the post-public hardening steps below. Branch `main` was **not protected** during the private GitHub Free audit; do not assume its protection will automatically appear.

## 2. Change visibility (owner-only action)

In repository **Settings → General → Danger Zone → Change repository visibility**, choose **Public** for `ferrioxid` only. Confirm the GitHub warning about public history, issues, forks and Actions logs. Visibility change is a separate explicit owner decision, not a CI fix.

GitHub may disable existing push rulesets when a repository becomes public. Re-check enforcement immediately afterward.

## 3. Immediately harden public repository settings

In **Settings → General**:

- [ ] Keep Issues enabled and use the existing bug/feature forms; do not allow blank issues unless deliberately chosen.
- [ ] Allow **squash merge only**, disable merge commits and rebase merge, and enable **Automatically delete head branches**.
- [ ] Enable Discussions only if there is time to moderate and answer questions; direct support questions there instead of treating every question as a defect.

In **Settings → Branches / Rules → Rulesets**, for `main`:

- [ ] Require a pull request before merging; prevent direct pushes to `main`.
- [ ] Require successful checks: **Rust (ubuntu-latest)**, **Rust (windows-latest)**, **PostgreSQL**, and **Cargo audit**, using the **exact names** exposed by Actions after they have run on the public repo. If the names change, update the rule. Include checks of recent PR branch commits as appropriate.
- [ ] Block force pushes and deletion; include administrators in enforcement / do not permit routine bypass.
- [ ] Require conversations to be resolved before merging; allow only fast-forward/squash-compatible history where supported.
- [ ] Do **not** require one additional approving reviewer while the project has only one maintainer. A mandatory external approval can lock out the sole maintainer. Add code-owner review requirements only after a real second reviewer is available.
- [ ] Test the protection with a harmless PR; verify it **actually blocks** a failed-check merge.

In **Settings → Security / Advanced Security**:

- [ ] Enable **Private vulnerability reporting**. `SECURITY.md` alone does not create a safe confidential reporting channel.
- [ ] Enable **Dependency graph**, **Dependabot alerts**, and security updates where offered.
- [ ] Enable **Secret scanning** and **push protection** for supported secret types where available; verify actual on/off state.
- [ ] Enable **Code scanning/CodeQL** if available and configure a suitable Rust analysis; check false positives and alerts before adding it as a required check.
- [ ] Review the known `RUSTSEC-2023-0071` exception in `.github/workflows/security.yml` and the tracked dependency issue. Never broaden that exception without checking reachability.
- [ ] Ensure token permission defaults stay minimal. Keep each workflow's `permissions: contents: read` or narrower; avoid `pull_request_target` for execution of outsider-submitted code.

In **Settings → Actions**:

- [ ] Verify fork pull-request approval policy; require approval for workflows from first-time or otherwise untrusted contributors where available.
- [ ] Keep GitHub Actions `GITHUB_TOKEN` read-only by default; no privileged secrets in fork PR jobs.
- [ ] Restrict reusable actions to trusted sources or immutable SHAs where practical. Current CI and security workflows pin their third-party `uses:` to full commit SHAs.
- [ ] Review Actions usage and limits; regular GitHub-hosted standard runners are free for public repositories, but self-hosted/larger runners and other services can have different terms.

## 4. Contributor and maintenance process

- **Issues:** bug reports use `.github/ISSUE_TEMPLATE/bug_report.yml` with version, OS, Rust version, backend, reproduction, expected/actual behavior and sanitized logs. Features use `feature_request.yml`. Security reports use private vulnerability reporting—not public issues.
- **Pull requests:** fork + feature branch → focused changes → tests + updated docs → PR template → CI → maintainer review → squash merge. Users have no direct write access merely because the repo is public.
- Never use contributor-supplied `pull_request_target` code execution or expose secrets to fork workflows. Verify all workflow changes carefully in code review.
- **Dependabot:** review semver-major changes and Rust/Windows/PostgreSQL CI. Never auto-merge failed dependency updates.
- **Code owners:** `.github/CODEOWNERS` routes responsibility but is not an enforcement mechanism without GitHub rules.
- **Labels/triage:** maintain `bug`, `enhancement`, `documentation`, `security`, `dependencies`, `good first issue` as applicable; avoid posting exploit details in public security issues.
- **Releases:** publish tagged versions only after successful checks, a changelog, migration guidance and a verified reproducible build. Do not represent development milestone numbers as stable Cargo releases.

## 5. Verification after publication

- [ ] Make a harmless PR from a separate contributor/fork, confirm Actions permissions, required checks, issue forms and review workflow.
- [ ] Open the repository as a signed-out visitor and verify links and documentation.
- [ ] Confirm that private vulnerability reporting offers a private report form.
- [ ] Confirm branch/ruleset enforcement and that GitHub Actions can actually start after the visibility change.
- [ ] Re-run PR #25 only once billing/access is fixed; do not merge a PR with missing execution just because its workflows report an accounting failure.
- [ ] Periodically audit collaborators, GitHub apps, credentials and repository security alerts.

## Inspection notes (2026-11-08)

Read-only GitHub checks confirmed a private `main` branch with `protected: false`; the GitHub rulesets API reported that GitHub Pro or public visibility is required. Issue forms, PR template, CODEOWNERS, MIT license, security and contributor policies, pinned GitHub Actions, and explicit read-only workflow tokens are present.

A **filename-based** check of all 23 commits on the default branch, current repository file list and selected PR head snapshots found no obvious committed `.env`, private key or database dump files. Targeted current-code token searches found no matches for some common token prefixes. These checks do **not** constitute a complete content- or history-wide secrets scan, including all branches, unreachable objects, CI artifacts or nonstandard secret encodings. No public-safety certification is implied.
