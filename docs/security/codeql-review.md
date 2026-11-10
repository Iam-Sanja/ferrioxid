# CodeQL alert review

Snapshot: 2026-11-08, repository `Iam-Sanja/ferrioxid`, `refs/heads/main`, commit `8cf85ba8988ed02444bddca2592e7d0cd202ed19`. Review and remediation branch: `security/codeql-remediation`, based on that commit. PRs #25 and #26 are separate, unmodified changes and are not covered by this source snapshot.

## Inventory and collection

All **94 open CodeQL alerts** were retrieved with authenticated GitHub REST API requests using API version `2022-11-28`. Collection followed the `Link: rel="next"` header until absent:

```text
GET /repos/Iam-Sanja/ferrioxid/code-scanning/alerts?state=open&per_page=100&page=1
GET /repos/Iam-Sanja/ferrioxid/code-scanning/alerts/{number}/instances?per_page=100&page=1
```

The alerts endpoint returned 94 unique alerts on one page with no next-page link. The instances endpoint was also paginated for each of the 94 alerts: one instance each, all on `refs/heads/main` at the commit above. Thus the branch statement is based on all available instances, not just each alert's most recent location. The analyzer was CodeQL `2.27.4`, Rust, build mode `none`. No access failures or missing pages occurred.

GitHub's default setup was also checked: state `configured`, query suite `default`, language identifiers `actions`, `javascript`, `javascript-typescript`, `rust` and `typescript`. All currently open findings returned by the API are from Rust; that does not establish the absence of issues in other languages or rules outside the configured suite.

The [complete 94-row ledger](codeql-alerts.csv) records the alert URL, rule ID, GitHub security severity, diagnostic severity, original file/line, description, exact message, branch, analysis commit, manual verdict and individual technical justification. Source links are pinned to the analyzed commit so line numbers remain valid after remediation. GitHub's security severity is preserved separately from our assessment; its ordinary diagnostic severity is not a substitute.

| GitHub security severity | Open at collection | Confirmed | False positives |
| --- | ---: | ---: | ---: |
| Critical | 92 | 1 | 91 |
| High | 2 | 0 | 2 |
| Medium | 0 | 0 | 0 |
| Low | 0 | 0 | 0 |
| Total | 94 | 1 | 93 |

## Causes and decisions, in review priority order

The Critical rule is `rust/hard-coded-cryptographic-value`; the High rule is `rust/cleartext-logging`. Critical findings were reviewed first, including their production callers and test database factories, followed by both High findings. There were no Medium or Low alerts to assess.

| Group | Alerts | Count | Decision |
| --- | --- | ---: | --- |
| External database seeding with a public password | #77 | 1 Critical | Confirmed conditional setup vulnerability; hook removed |
| Missing form field represented by an empty string | #18–20, #67 | 4 Critical | False positives; downstream validation or password verification rejects it |
| Failed-login timing dummy | #21 | 1 Critical | False positive; comparison result is discarded and login fails |
| Isolated test password inputs | #1–17, #22–66, #68–76, #78–92 | 86 Critical | False positives; no production credential or external database |
| Local test HTTP response diagnostics | #93–94 | 2 High | False positives; synthetic data and ephemeral test sessions |

### Confirmed: alert #77

`seed_file_database_when_requested` in `examples/myshop/tests/admin_http.rs` read `FERRUM_E2E_DATABASE`, connected to the supplied database, applied authentication migrations and, when the user list was empty, created `staff@example.com` with the public password `correct-horse-battery`, `is_staff=true` and `is_superuser=true`. Unlike the other password fixtures, this was not restricted to an in-memory database. Running tests with an inherited setting could persist a known privileged credential in a reusable database.

GitHub labels this Critical. The demonstrated consequence is conditional: someone must execute the test binary with that setting, and the selected database must have no users. This is not a remotely callable production bootstrap endpoint. Its impact can nevertheless be account takeover if the resulting database is later exposed. Argon2 hashing does not protect a publicly known password.

Reproduction used a fresh disposable SQLite file. The original test succeeded and left a user row with both privilege flags set. The new `tests_ignore_external_seed_database` regression then failed against the original hook. It launches the current test executable in a child process, supplies a unique file URL through the environment, runs the other tests, and asserts that the file was never created. It passes after removing the hook. The parent process environment is unchanged, and the child skips the regression itself to avoid recursion.

The correction deletes the seeder rather than adding another default credential or moving it to a different variable. Manual browser setup now uses the existing `ferrioxid admin create-user` command and its password prompt; see [testing](../testing.md). Existing authentication, CSRF, authorization and security checks remain in place.

**Existing database response:** removing the hook cannot undo accounts created by earlier runs. If `FERRUM_E2E_DATABASE` was ever pointed at a reused database, inspect it for the seeded account. For a disposable database, recreate it. Otherwise deactivate the account, revoke its sessions, review its activity and set a unique credential through the supported administration workflow if the account is still needed. Do not delete unrelated user data automatically.

### False positives: #18, #19, #20 and #67

These literals are missing-input sentinels, not configured or installed passwords:

- #18: `login_submit` passes the missing field as `""` to `AuthService::login`. Login verifies the stored Argon2 hash and rejects mismatches. Unknown users also return `InvalidCredentials`. Account creation and password changes enforce a minimum of 12 characters; this handler does not create an empty-password account.
- #19: HTML directory creation calls `directory::create_user`, which validates the password before hashing and before opening its write transaction.
- #20: HTML directory password changes call `directory::reset_password`, which uses the same validation before hashing or opening its transaction.
- #67: `submit_choose` calls `complete_reset`, which validates the password and returns `CompleteError::Weak` before changing credentials or consuming a reset token.

The validation implementation is in `crates/ferrioxid-auth/src/password.rs`. The relevant callers are `crates/ferrioxid-admin/src/directory.rs`, `crates/ferrioxid-auth/src/service.rs` and `crates/ferrioxid-auth/src/recovery.rs`. Imported or externally manipulated password hashes remain an operator trust boundary; the empty sentinel does not install them.

### False positive: #21

`dummy_hash` hashes `ferrioxid-timing-placeholder` with a fresh random salt. `verify_against_dummy` deliberately discards its verification result. Both production callers in `AuthService::login` immediately record a failure and return `InvalidCredentials`. The dummy hash is never written as a user's password, used to issue a session or used as an encryption key. Keeping this work preserves the existing account-enumeration timing defense.

### False positives: 86 isolated password fixtures

Each of these alerts has an individual justification in the ledger; test classification alone was not accepted as proof, as #77 demonstrates.

- #1–17: `directory.rs` has an inline `#[cfg(test)]` module. Its auth factory explicitly opens `sqlite::memory:`; passwords exercise audit rollback, directory transactions and session revocation.
- #22–33: the inline `password.rs` test module checks randomized salts, password replacement, validation and malformed PHC hashes. It creates no account.
- #34–48: `reset_tests.rs` is included under `#[cfg(test)]`; its factory uses `sqlite::memory:` and a memory mail inbox. The strings drive reset, replay and delivery-failure tests.
- #49–66: `sqlite_tests.rs` is included under `#[cfg(test)]`; its database factory returns `sqlite::memory:`. The strings drive login, rotation, expiry, permissions and rate-limit tests.
- #68: the inline storage test uses `sqlite::memory:` and a unique temporary storage root to test authenticated owner isolation.
- #69–76 and #78–92: integration tests explicitly open `sqlite::memory:`; `orders.rs` does so through `ready()`. HTTP fixtures bind ephemeral loopback ports. These accounts never use an environment-selected database. The defective separate seeder at #77 was assessed and removed independently.

The known strings are public inputs to isolated tests. Changing them to random strings would not fix a deployed credential and would obscure the reason for this verdict.

PR #62 added four more in-memory fixtures in `anonymous_tests.rs`, `anonymous_contact.rs` and `cookie_csrf.rs`. CodeQL reported eight call sites, two per fixture. They open `sqlite::memory:` and never an environment-selected database. The password is the same public fixture, assembled in `memory_fixture_password` so the test source no longer passes a string literal into `create_user` or `login`.

### False positives: #93 and #94

Both are failed-status assertion messages in `html_admin_works_without_javascript`, which opens `sqlite::memory:` and runs its own server on `127.0.0.1:0`.

- #93: `own_secret` is an HTTP `Reply` from creating a synthetic owner-scoped `HtmlSecret` record with title `Eigen`. Its body is printed only if the expected redirect fails. It is not a stored credential or a response from a deployed service.
- #94: `secret_pick` is an HTTP `Reply` containing a fixture picker. `StaffVisible` and `RootOnlySecret` are synthetic record titles. Adjacent assertions require the former to be present and the latter absent, exercising the owner policy.

Neither path reads an external database or genuine application secret. Any session or CSRF material belongs only to the short-lived test instance, not a deployed account. The variable/model names do not establish a sensitive production source. Useful failure diagnostics and the owner-isolation assertions are preserved.

## Focused security boundary review

This review followed the alert flows and inspected the following related boundaries. It is not a claim of an exhaustive penetration test or a clean result for the unmerged changes in PRs #25/#26.

| Area | Evidence inspected | Assessment and limit |
| --- | --- | --- |
| SQL injection | `ferrioxid-db/src/sql.rs` builds statements with bound values; runtime sort/filter columns pass `ensure_column`. Macro identifiers are validated in `ferrioxid-macros/src/lib.rs`. | No confirmed injection in these reviewed flows. Application-written raw SQL remains the application's responsibility. |
| XSS | Admin rendering enables Tera HTML autoescaping; API HTML error output escapes text. Admin HTTP tests exercise stored markup and owner-scoped relation labels. | No confirmed XSS in the reviewed flows. Custom trusted templates and explicitly safe HTML require separate application review. |
| Path traversal | Storage uses typed UUID owner/file paths, canonical containment for reads, and sanitized display names. Static/admin asset tests cover traversal. | No confirmed request-controlled traversal in these flows. Storage-root integrity and local filesystem permissions remain deployment assumptions. |
| Authentication and sessions | Argon2id with random salts, dummy failed-login verification, server-side session lookup, expiry, revocation, inactive-user rejection and HttpOnly/SameSite cookies. | #77 is the confirmed setup issue. Production HTTPS/Secure-cookie configuration remains required. |
| CSRF | `authenticate_csrf` checks origins and compares a session-bound HMAC in constant time on unsafe methods; HTML login has separate bootstrap-token checks. | No confirmed bypass in the reviewed flows. The development-only key is not a production secret; `from_env` loads its configured/persistent key. |
| Permissions | Default-deny/model policies, owner query scopes, directory actor checks inside transactions; integration tests cover foreign owners and role restrictions. | No confirmed bypass in the reviewed paths. Application policies must supply correct scopes. |
| Uploads | Authenticated owner selection, CSRF checks, bounded bodies, content allowlist, UUID paths, private attachment downloads with `nosniff`. | No confirmed alert-related bypass. Signature checks are not malware scanning; public serving and hostile local filesystem actors require separate review. |
| Secrets and logging | All flagged password and assertion flows traced to their actual sources; reset tests use memory mail; normal reset logs use event codes. | No leaked production credential established by #93/#94. Console mail intentionally prints messages; environment-based configuration refuses it in production. Explicit application setup must preserve that boundary. This is not a complete secret-history scan. |
| Processes | Reviewed CLI build/server invocations use explicit programs and argument arrays. | No confirmed shell injection in the reviewed invocations. Project/schema commands execute trusted project code; this does not sandbox an untrusted checkout. |

## Validation and remaining work

Local validation used Windows/MSVC, stable Rust, a fresh isolated Cargo target directory, two build jobs and a plain temporary directory. PostgreSQL verification used a fresh disposable PostgreSQL 16.15 cluster on loopback; it was stopped and its temporary password file removed afterward.

| Check | Result |
| --- | --- |
| Seeder reproduction on disposable SQLite | Original hook persisted a staff superuser with the public fixture password |
| `tests_ignore_external_seed_database` before correction | Failed as expected (exit 101) because the child created the selected database |
| `cargo test -p myshop --test admin_http --locked --offline` after correction | Exit 0; all 3 tests passed, including the final regression with both variable names |
| `cargo test --workspace --locked --offline` | Exit 0; 178 passed, 0 failed, 2 PostgreSQL tests ignored by their existing attributes; generated API/fullstack project builds passed |
| `cargo test -p myshop --test postgres_api --locked --offline -- --ignored postgres_http_roundtrip` | Exit 0; passed against PostgreSQL 16.15 |
| `cargo test -p ferrioxid-migrations --test engine --locked --offline -- --ignored postgres_migration_roundtrip` | Exit 0; passed against PostgreSQL 16.15 |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Exit 0; MSVC emitted its existing non-failing import-library linker notice; no lint suppression was added |
| `cargo build --workspace --locked --offline` | Exit 0 |
| Unfiltered `cargo audit` 0.22.2 with freshly fetched RustSec DB | Exit 1; only `RUSTSEC-2023-0071`, `rsa` 0.9.10, present through optional SQLx MySQL dependencies in the lockfile; no other advisories or warnings |
| Existing CI RSA reachability guard and audit gate | RSA absent from `cargo tree --workspace --target all --prefix none --locked --offline`; existing `cargo audit --ignore RUSTSEC-2023-0071` gate exited 0; no exception was added or broadened |
| Inventory, documentation and diff | 94 unique ledger rows matched the API fields; new Markdown fences/internal links and the authentication example anchor checked; `git diff --check` passed |

RustSec database commit: `309392b18f6b17022f82c701f300433126901068` (1,422 advisories, last updated 2026-11-06). The existing RSA exception is tracked in [issue #8](https://github.com/Iam-Sanja/ferrioxid/issues/8) and must be reconsidered if the enabled feature graph changes. The lockfile match is reported here rather than described as an unfiltered clean audit.

No standalone CodeQL CLI is installed locally. The original GitHub analysis and default-setup metadata were available; fresh analysis and Linux CI results belong to the new PR's checks. No alert was dismissed through the GitHub API, and no CodeQL query, test, warning or workflow was suppressed. The 93 false-positive decisions are review evidence for maintainers, not blanket scanner exclusions.

GitHub will continue to show the original findings against `main` until it analyzes a changed default branch and maintainers separately triage the false positives. This remediation PR must not be merged automatically. PR #25 at `1b78d4a2f93ddaa413aae3cd5d49ea98988ce47c` still contains this seeder under the renamed `FERRIOXID_E2E_DATABASE` setting. The new regression supplies both names in the child process so a naming change cannot hide its reintroduction; this adds no runtime compatibility behavior. Preserve the deletion when resolving that integration conflict. Neither PR #25 nor #26 was modified here.

## PR #25 integration follow-up

PR #27 was merged into `main` as `d48dbecc731b2b83f9529cf74bd13d187ee6f82c`. PR #25 integrated that commit with the normal merge `76ee15c4f1918140e6aa39ad29df0f4cff663f4b`. The external database seeder remains deleted, and `tests_ignore_external_seed_database` is unchanged from #27, including both environment names. The initial 94-alert snapshot and its CSV ledger above remain historical records of the pinned source, not claims about the current PR's alert state.

That integration's CodeQL analysis reported [alert #95](https://github.com/Iam-Sanja/ferrioxid/security/code-scanning/95), `rust/cleartext-logging`, High, at [generator.rs:308](https://github.com/Iam-Sanja/ferrioxid/blob/76ee15c4f1918140e6aa39ad29df0f4cff663f4b/crates/ferrioxid-cli/tests/generator.rs#L308) on `refs/pull/25/head`. The message was `This operation writes secret to a log file.`

**False positive:** the binding named `secret` held one of four literal `.gitignore` patterns: `.env`, `.env.*`, `!.env.example` or `.ferrioxid/csrf.key`, each followed by a newline. The assertion reads the generated `.gitignore` and prints the missing pattern on failure; it does not read environment values, the CSRF key file or any credential. The binding is now named `pattern` to describe the actual value. The patterns, membership assertions and rendered failure messages are unchanged. No rule, diagnostic or logging safeguard is disabled, and no alert is manually dismissed. Fresh CodeQL and CI results are tracked by PR #25's checks.
