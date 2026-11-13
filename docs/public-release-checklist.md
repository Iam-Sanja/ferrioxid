# Public release security and contributor setup

This checklist records verified settings and remaining work for [Iam-Sanja/fexu-rs](https://github.com/Iam-Sanja/fexu-rs). The repository is already **public**. Checked items are evidence observed on **2026-11-08**, when the repository was named `Iam-Sanja/ferrioxid`, not a permanent security certification. Settings can change independently of Git. No repository setting was changed during this documentation update, and the separate historical `fexu` repository was not modified.

Current source baseline: `main` at `5c9bd7c438179b53d02f36130871ac42f313ef5e`. See [GitHub maintenance](github-maintenance.md) for the settings snapshot and repeatable verification endpoints.

## 1. Publication and historical exposure

- [x] Confirm public visibility and default branch `main` through the repository API. Visibility is an owner decision that has already been completed; do not repeat the visibility-change procedure.
- [ ] Complete and record a full content/history secret review covering all fetched branches and tags, previous test projects, relevant PR discussions, Actions logs and downloadable artifacts. Assess separately any available unreachable/local objects and external copies. A clean working tree or `.gitignore` does not establish that these sources are free of secrets.
- [ ] Run a dedicated history-aware scanner against a current full clone, record the tool version, exact refs and exclusions, and triage findings. Use the scanner's current supported syntax; do not publish unredacted reports. Native Secret Scanning is a second check, not proof that every historical or unsupported secret was covered.
- [ ] Revoke and rotate any real exposed credentials and record remediation. Deleting a file or adding an ignore pattern cannot revoke a credential. Coordinate any necessary history removal; do not casually rewrite public history or assume it removes forks and downloaded copies.
- [x] Verify that [LICENSE-MIT](../LICENSE-MIT), [SECURITY.md](../SECURITY.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md), [GOVERNANCE.md](../GOVERNANCE.md), [CODEOWNERS](../.github/CODEOWNERS), issue forms and the PR template exist on current main.
- [ ] Complete the license/third-party asset review, including fonts, screenshots, templates, JS/CSS and copied examples. The MIT file's presence does not establish permission for every asset.
- [ ] Finish policy-text and contact-route review. `CONTRIBUTING.md` still describes product documentation as German and lists only two of the three PostgreSQL integration tests; `SECURITY.md` still describes Private Vulnerability Reporting conditionally. Use the verified reporting route below. The Code of Conduct has no published reporting email; verify a usable private contact route.
- [ ] Review repository/environment Actions secrets, variables, collaborators and installed apps for necessity and scope. Do not expose credentials to untrusted code. This documentation review did not inventory secret values or certify all environments.
- [x] Verify current development defaults and naming: generated projects use `fexu.toml`, `.fexu/`, `FEXU_*` and `_fexu_*`; local demo credentials are not production defaults. See [the earlier name cleanup](ferrum-legacy-removal.md).
- [ ] Review previously publicized workflow logs/artifacts and any earlier disclosure. Publication has already happened; remaining review is incident-prevention/remediation work, not a claim that a pre-publication audit was completed.

## 2. Repository and main-branch protection

Repository API and effective branch-rule checks confirmed:

- [x] Issues and Discussions enabled; [issue configuration](../.github/ISSUE_TEMPLATE/config.yml) disables blank issues and routes security reports to the policy. Having Discussions enabled does not prove it is actively moderated.
- [x] Squash merge enabled; merge commits and rebase merge disabled; automatic head-branch deletion enabled. Normal merges of main into topic branches remain possible; the restriction applies to merging PRs into protected main.
- [x] Active [protect-main ruleset](https://github.com/Iam-Sanja/fexu-rs/rules/26812021), ID `26812021`, applies to the default branch, currently `main`; no excluded refs or bypass actors. The API reports `main.protected=true` and the current user cannot bypass the ruleset.
- [x] Require a PR, resolved review conversations and linear history; allow squash only into main. Block deletion and non-fast-forward pushes. Do not push directly to main or bypass these controls.
- [x] Require the four exact GitHub Actions check names: **Rust (ubuntu-latest)**, **Rust (windows-latest)**, **PostgreSQL**, **Cargo audit**. Strict status-check policy requires the branch to be up to date. Each check is associated with GitHub Actions integration ID `15368`.
- [x] Configure zero mandatory approving reviews, no mandatory code-owner review and no last-push approval, consistent with the current single-maintainer setup. This does not replace the maintainer's final review. Reassess approval requirements when an independent reviewer is available.
- [ ] Record a controlled enforcement exercise showing that a deliberately failing required check blocks merging, without actually merging a failing PR or weakening protection. API configuration and successful PRs do not by themselves prove this negative case.

CodeQL is enabled but is **not** one of the four required check contexts. Review its results before merging; do not silently claim the ruleset requires CodeQL.

## 3. Security features

- [x] **Private Vulnerability Reporting** enabled (`GET /private-vulnerability-reporting` returns `enabled=true`). Report vulnerabilities using the [private advisory form](https://github.com/Iam-Sanja/fexu-rs/security/advisories/new), which may require sign-in. Do not post exploit details in public issues, discussions or PRs.
- [x] **Dependabot alerts** enabled (vulnerability-alerts endpoint returns HTTP 204); **security updates** enabled and not paused. The alert listing was accessible. This establishes alert availability, not perfect dependency coverage.
- [x] Verify [Dependabot configuration](../.github/dependabot.yml): weekly Cargo and GitHub Actions updates. Configuration does not auto-merge updates or prove the timing of every scheduled run.
- [ ] Independently verify the Dependency graph UI/data coverage, including any dependencies outside the Cargo lockfile and Actions configuration. Alert enablement alone is not a complete inventory audit.
- [x] **Secret Scanning** and **Push Protection** enabled in repository `security_and_analysis` metadata. Coverage is limited to supported patterns and available features; neither setting proves that all historic material was scanned.
- [ ] Assess whether additional Secret Scanning coverage is appropriate: non-provider patterns and validity checks were both reported **disabled**. No setting was changed by this PR.
- [x] **CodeQL default setup** configured, weekly schedule, default query suite, remote threat model, with language identifiers `actions`, `javascript`, `javascript-typescript`, `rust` and `typescript`. This is not a claim that every security query suite is enabled.
- [x] Retain the [94-alert review](security/codeql-review.md) and [per-alert ledger](security/codeql-alerts.csv) from PR #27, plus the documented PR #25 follow-up for false positive #95. Individual technical assessments do not authorize blanket alert dismissal or query suppression.
- [x] Preserve the [audit workflow](../.github/workflows/security.yml): `cargo audit` runs without an advisory ignore. The job also fails if RSA becomes reachable in the enabled graph. Do not add an ignore to make a check green. The former `RUSTSEC-2023-0071` ignore is not still in force; see [security-rsa.md](security-rsa.md).

## 4. Actions, fork PRs and external contributions

- [x] Actions enabled. Repository default `GITHUB_TOKEN` permissions are read-only and Actions cannot approve PR reviews. Both checked-in workflows explicitly set `permissions: contents: read`, pin third-party `uses:` entries to full commit SHAs, and do not execute `pull_request_target` code or reference repository secrets.
- [x] Fork workflow approval policy is **first_time_contributors**. This is the observed policy, not approval of every outside contributor's workflow execution.
- [ ] Review whether broader approval of outside contributors is needed. Exercise a real fork PR, inspect token/secret handling, artifacts and permission boundaries, and require deliberate review of workflow changes. Do not assume a returning contributor is trusted.
- [ ] Decide whether to restrict Actions at repository level: the current policy permits **all** actions and does **not** require SHA pinning globally. The checked-in SHA pins are a separate, narrower source-control safeguard.
- [ ] Inspect usage/billing, runner types and environments. The old private-repository Actions billing blockage is historical; public-repository Actions now execute. No larger/self-hosted runner or external service cost audit was performed.

Contributor flow remains fork + focused feature branch → tests and documentation → PR template → all required checks → maintainer review → squash merge. Public visibility does not grant write access. CODEOWNERS names `@Iam-Sanja` for routing but is not required approval under the current ruleset. Never execute outsider-controlled code with elevated credentials, remove existing protections or auto-merge failed dependency updates.

Releases require an intentional version/tag, successful checks, changelog and migration guidance. Development milestone labels do not establish a stable Cargo release. See [Contributing](../CONTRIBUTING.md) and [Governance](../GOVERNANCE.md).

## 5. Merged security and naming work

- [x] [PR #27](https://github.com/Iam-Sanja/ferrioxid/pull/27) merged as `d48dbecc731b2b83f9529cf74bd13d187ee6f82c`: the external database test-seeder is deleted. The regression `tests_ignore_external_seed_database` sets `FEXU_E2E_DATABASE` and the retired environment names. Those retired names are a negative security test, not a runtime alias.
- [x] [PR #25](https://github.com/Iam-Sanja/ferrioxid/pull/25) merged as `5c9bd7c438179b53d02f36130871ac42f313ef5e`: Fexu-only configuration/schema names, generated-project secret exclusions and the seeder deletion are present on main. No compatibility alias or automatic data conversion was introduced.
- [x] PR #25's final head `eb3389ffc3169d433b0604168ae23699f2eb3f96` passed Ubuntu, Windows, PostgreSQL, Cargo Audit and CodeQL before merge. The recorded local validation passed 180 Rust tests plus all three PostgreSQL integration tests. The five built-in migration seals and both generated project builds passed. These are pinned historical results, not a claim about future commits.
- [ ] If an earlier external-seeder run touched a reused database, inspect the known privileged account, revoke sessions and replace compromised credentials as described in the [security review](security/codeql-review.md). Code deletion does not repair previously seeded databases. No operator database audit is claimed here.

The schema/configuration rename intentionally requires fresh databases. Existing migration histories and checksums must not be silently reused. Current instructions use Fexu names. Historical audit records, mapping tables, original license attribution and negative tests may still spell the retired names.

## 6. Verification and ongoing work

- [ ] Exercise a separate contributor/fork PR, including approvals, required checks, issue forms and review behavior.
- [ ] Inspect repository pages and documentation as a signed-out visitor; verify the private reporting form through an appropriately signed-in reporter account.
- [ ] Complete the unresolved secret-history, license and policy/contact reviews above; do not mark them complete based on enabled scanners.
- [ ] Periodically recheck rules, collaborators, apps, credentials, dependency coverage and security alerts. Refresh this dated snapshot when settings change.

Before merging PR #26, inspect the checks attached to its **latest head**, including Ubuntu, Windows, PostgreSQL, Cargo Audit and CodeQL. The review agent must leave the PR open for the owner's decision. A green PR-diff CodeQL analysis is not a full-history secret scan or a statement that unrelated baseline alerts were closed.

## Evidence and historical limits

The current snapshot comes from authenticated read-only repository, effective branch-rule/ruleset, vulnerability-alerts, automated-security-fixes, private-vulnerability-reporting, CodeQL default-setup and Actions permission APIs, plus inspection of tracked policies/workflows. [GitHub maintenance](github-maintenance.md) lists the routes. The private-repository-only fork-settings endpoint is not applicable to this public repository; it was not treated as evidence about public fork permissions.

The earlier private GitHub Free inspection reported unprotected main and unavailable rulesets; that observation is superseded by the active public ruleset above. Its filename-based inspection of 23 default-branch commits and selected snapshots, and limited token-prefix searches, did **not** establish a full content/history secret scan. This update makes no such claim either. Publication, enabled scanners and green CI cannot retroactively certify historical disclosure safety.
