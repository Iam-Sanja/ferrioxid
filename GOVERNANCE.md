# Governance

Ferrioxid is maintained primarily by one developer, GitHub account `@Iam-Sanja`. There is no foundation, steering committee, or separate organization.

## Maintainer

The maintainer:

- Reviews pull requests and decides what is merged into `main`.
- Protects compatibility of existing generated projects and applied migrations.
- Publishes releases only when a release is intentionally prepared. Cargo `0.1.0` is still pre-release.
- Handles security reports received through [SECURITY.md](SECURITY.md).
- Applies the [Code of Conduct](CODE_OF_CONDUCT.md).

## Contributions

Anyone may propose a change by opening a pull request against `main`. A contribution does not grant commit access. The maintainer may ask for tests, documentation, or a smaller diff before merging.

Review is a conversation. The maintainer may merge, request changes, or decline work that conflicts with the framework's direction. Silence is not approval.

## Decisions

Ordinary fixes and documentation follow the pull-request process. A breaking change to public crate APIs, migrations, authentication, settings, or generated projects needs an explicit note in the pull request and the maintainer's agreement before merge.

Internal milestone numbers are planning labels. They do not by themselves authorize a Cargo version bump or a release.

## Releases

There is no stable release channel yet. A future release is prepared on a short-lived `release/<version>` branch and merged to `main`. Dependency updates opened by Dependabot are reviewed. They are not merged automatically.

## Security

The maintainer coordinates vulnerability response. There is no security team. If the maintainer is unavailable, a report may wait. The policy in [SECURITY.md](SECURITY.md) does not promise a deadline.

## Future maintainers

Additional maintainers can be added later by the current maintainer, with write access on GitHub and a corresponding update to [`.github/CODEOWNERS`](.github/CODEOWNERS). CODEOWNERS does not grant access by itself. GitHub repository permissions and branch protection are configured separately, as described in [docs/github-maintenance.md](docs/github-maintenance.md).
