# Security policy

Ferrum is a pre-release framework. The Cargo version is `0.1.0`. There is no stable release and no commitment to provide security patches for older revisions.

## Reporting a vulnerability

Do not report an exploitable vulnerability in a public issue, discussion, or pull request.

Use GitHub Private Vulnerability Reporting if the repository owner has enabled it. This file does not enable that feature. Enabling it is a GitHub setting, described in [docs/github-maintenance.md](docs/github-maintenance.md).

Until that setting is confirmed, contact the maintainer privately through GitHub account `@Iam-Sanja`. No security email address is published.

Include the affected revision, the component, and a description that lets the maintainer reproduce the issue. Give the maintainer time to investigate before any public disclosure. There is no bounty program and no guaranteed response time.

## Supported versions

| Version | Security maintenance |
|---|---|
| Git `main` at Cargo `0.1.0` | Best effort while the project is in pre-release development |
| Any older milestone label | Not a supported release |
| Future numbered releases | None exist yet |

Milestone labels such as v0.27.1 are development notes. They are not supported releases.

## Areas to treat carefully

These notes describe the current design. They are not a promise that every deployment is safe.

- **Authentication.** Passwords use Argon2id. There is no default admin password. `allow_anonymous = true` is an explicit public grant and is appropriate only for local demos.
- **Sessions.** Sessions are server-side and use an HttpOnly cookie. CSRF protection applies to cookie-authenticated writes. Anonymous routes do not receive that cookie check.
- **Password reset.** Reset tokens are not stored in background-job payloads. A reset message must not be placed on the job queue.
- **Migrations.** The framework does not provide `migrate down`. Do not edit a migration that a real project has already applied. Tests must use a temporary or disposable database.
- **Background jobs.** Delivery is at least once, not exactly once. `ferrum jobs inspect` hides payloads unless `--show-payload` is passed. A requested payload can still contain personal data. The lease token is not printed.
- **Bindings.** Generated projects and the example server bind to loopback by default. Do not publish a demo with anonymous model access on a public interface.

The sample PostgreSQL password `ferrum_dev_only` in `.env.example` and `compose.yaml` is a local development default. It is not a production credential.
