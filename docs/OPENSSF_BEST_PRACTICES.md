# OpenSSF Best Practices evidence

This document helps maintainers prepare the
[OpenSSF Best Practices Badge](https://www.bestpractices.dev) application for
mnemo. It gathers, in one place, the evidence that already exists in the
repository so the form can be filled in quickly and honestly.

> **This document does not claim that mnemo has already earned the badge.**
> The badge is granted only after a maintainer completes the official
> questionnaire on <https://www.bestpractices.dev> and the criteria are actually
> met. No badge is added to the README until the status is genuinely granted.

## Evidence available in mnemo

| Area | Evidence in mnemo | File or workflow |
|---|---|---|
| Project homepage & description | README with purpose, features, usage | [README.md](../README.md) |
| OSS license | MIT license, OSI-recognized | [LICENSE](../LICENSE) |
| Version control | Public Git repository on GitHub | `https://github.com/Vesperis-group/mnemo` |
| Release process & versioning | Semantic versioning, automated signed releases | [docs/RELEASE_APP.md](RELEASE_APP.md), [.github/workflows/release.yml](../.github/workflows/release.yml) |
| Security policy | Private vulnerability reporting process | [SECURITY.md](../SECURITY.md) |
| Threat model | Documented security assumptions | [docs/THREAT_MODEL.md](THREAT_MODEL.md) |
| Contribution process | Contributor guide and expectations | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Issue & PR templates | Structured bug/feature/PR intake | [.github/ISSUE_TEMPLATE/](../.github/ISSUE_TEMPLATE), [.github/pull_request_template.md](../.github/pull_request_template.md) |
| CI tests | Rust unit/integration tests run on PR and `main` | [.github/workflows/ci.yml](../.github/workflows/ci.yml) |
| Static analysis (SAST) | CodeQL on every commit | [.github/workflows/codeql.yml](../.github/workflows/codeql.yml) |
| Linting | `clippy -D warnings`, `actionlint`, `ShellCheck` | [.github/workflows/ci.yml](../.github/workflows/ci.yml), [.github/workflows/lint.yml](../.github/workflows/lint.yml) |
| Dependency update tool | Dependabot (cargo, npm, github-actions) | [.github/dependabot.yml](../.github/dependabot.yml) |
| Dependency audit & policy | `cargo-audit`, `cargo-deny`, `cargo-machete` | [.github/workflows/audit.yml](../.github/workflows/audit.yml), [deny.toml](../deny.toml) |
| Secrets scanning | `gitleaks` on PR and push | [.github/workflows/audit.yml](../.github/workflows/audit.yml) |
| Fuzzing | `cargo-fuzz` baseline with three targets | [fuzz/](../fuzz), [.github/workflows/fuzz.yml](../.github/workflows/fuzz.yml), [docs/FUZZING.md](FUZZING.md) |
| Pinned dependencies | GitHub Actions pinned by full commit SHA | `.github/workflows/*.yml` |
| Release integrity | SHA-256 checksums, cosign signatures, SLSA provenance, SBOM | [.github/workflows/release.yml](../.github/workflows/release.yml), [docs/THREAT_MODEL.md](THREAT_MODEL.md) |
| Post-release verification | Installation smoke tests on published releases | [.github/workflows/release-smoke.yml](../.github/workflows/release-smoke.yml) |
| Branch protection | Documented repository rules and rulesets | [docs/REPOSITORY_RULES.md](REPOSITORY_RULES.md) |
| Security posture tracking | OpenSSF Scorecard workflow and report | [.github/workflows/scorecard.yml](../.github/workflows/scorecard.yml), [docs/SCORECARD.md](SCORECARD.md) |

## Mapping to common Best Practices criteria

The OpenSSF Best Practices questionnaire is organized into *passing*, *silver*
and *gold* tiers. The items below summarize where mnemo already has concrete
evidence; the maintainer remains responsible for the authoritative answers in
the form.

- **Basics** — public repository, MIT license, README describing the project,
  and a documented contribution process (`CONTRIBUTING.md`).
- **Change control** — all changes go through pull requests on dedicated
  branches; direct pushes to `main` are forbidden by convention and enforced via
  rulesets (see [docs/REPOSITORY_RULES.md](REPOSITORY_RULES.md)).
- **Reporting** — a security policy with a private reporting channel
  (`SECURITY.md`) and structured issue templates.
- **Quality** — automated build, tests, formatting and linting on every PR; a
  documented local quality gate (`make check`, `make security-full`).
- **Security** — SAST (CodeQL), dependency audit (`cargo-audit`/`cargo-deny`),
  secrets scanning (`gitleaks`), fuzzing (`cargo-fuzz`), pinned dependencies, and
  signed, provenance-attested releases.
- **Analysis** — static analysis and fuzzing are run in CI; dynamic analysis is
  partially covered by the fuzzing baseline (see [docs/FUZZING.md](FUZZING.md)).

## Known gaps to disclose honestly in the form

- The project is young: the **Maintained** signal and a long contribution
  history will only build over time.
- **Multi-organization contributors** are not present yet; this must not be
  faked (see [docs/SCORECARD.md](SCORECARD.md)).
- Some criteria (e.g. cryptographic signing of every individual artifact) are
  partially met; answer them accurately rather than optimistically.

## Maintainer action required

The maintainer must manually complete the OpenSSF Best Practices Badge form at
<https://www.bestpractices.dev> and only add a badge to the README **once the
status is actually granted**. Until then, this file exists solely to prepare the
application and must not be read as proof that the badge has been earned.
