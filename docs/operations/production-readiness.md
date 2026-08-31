# Production Readiness

SOUL is an early-stage prototype, not a production release. This document defines the remaining release gates and separates implemented controls from planned work.

## Implemented Foundation

- SQLCipher-backed local storage with recovery-aware migration behavior.
- Signed local packages and receipts with fail-closed verification.
- Ownership checks for entities, evaluations, context, and integration access.
- Purpose-scoped context compilation with sensitivity filters and token limits.
- Deterministic policy evaluation and replay-resistant local capabilities.
- Restricted bridge and MCP database access, bounded IPC, and a strict production CSP.
- Windows release assembly with sidecar verification and clean-install smoke testing.
- Automated TypeScript, Rust, browser-integration, security, and performance checks.

## Open Release Gates

### Product Validation

- Complete the approved external P0 validation program.
- Record day-0, day-7, and day-28 evidence from independent participants.
- Confirm that SOUL-assisted responses outperform the reviewed baseline often enough to justify continued investment.
- Record an explicit `GO`, `ITERATE`, or `KILL` product decision.

Billing, hosted services, and broader P1 development remain blocked until this gate is complete.

### Distribution Security

- Sign Windows binaries with an organization-controlled Authenticode certificate.
- Add macOS signing and notarization before distributing a macOS build.
- Publish signed update metadata through an organization-controlled HTTPS channel.
- Test clean installation, upgrade, rollback, and interrupted-update recovery.
- Generate and review an SBOM and dependency-license report for each release.

### Platform Coverage

- Add maintained macOS and Linux CI jobs.
- Move platform secrets to Keychain and Secret Service where applicable.
- Run native sidecar, browser-integration, packaging, and recovery smoke tests on each supported platform.
- Resolve or formally accept relevant transitive desktop dependency advisories.

### Security Operations

- Establish a private vulnerability-reporting channel and response policy.
- Add opt-in diagnostics that redact personal content and secrets.
- Document incident triage, severity, embargo, notification, and patch timelines.
- Publish backup, restore, and key-loss guidance.
- Complete a manual accessibility and security review of release candidates.

### Privacy and Legal

- Publish a versioned privacy notice describing local processing and every external disclosure path.
- Approve consent, retention, deletion, and support language.
- Review regional obligations before enabling accounts, sync, telemetry, or hosted Gateway services.

### Production Gateway

The current Gateway is a local simulation. A production Gateway additionally requires:

- destination credentials isolated from agents and renderers;
- real provider connectors with bounded scopes;
- signed requests and verified responses;
- server-side replay protection and idempotency;
- connector-specific audit receipts;
- tested revocation, rotation, timeout, and recovery behavior.

No release should imply universal agent control. Enforcement applies only to actions routed through a SOUL-controlled execution path.

## Windows Release Evidence

The existing Windows x64 release gate:

1. Builds the Rust application and sidecars with the pinned MSVC toolchain.
2. Verifies the staged sidecar payloads against final Cargo outputs.
3. Runs TypeScript, Rust, browser, security, and release-only tests.
4. Builds the NSIS installer.
5. Performs a silent clean installation.
6. Verifies installed application and sidecar hashes.
7. Exercises real MCP and native-messaging responses from the installed directory.

This evidence demonstrates packaging integrity for the tested environment. It does not replace signing, updater, cross-platform, privacy, or external product-validation gates.

## Release Checklist

- [x] TypeScript and Rust quality checks pass in CI.
- [x] Windows sidecars are assembled and hash-verified.
- [x] Windows NSIS clean-install smoke test exists.
- [ ] External P0 validation is complete and accepted.
- [ ] Release artifacts are signed and timestamped.
- [ ] Secure update and rollback paths are verified.
- [ ] SBOM and license review are complete.
- [ ] Privacy notice and support contact are published.
- [ ] Backup, restore, and key-loss documentation is published.
- [ ] Vulnerability response and incident procedures are operational.
- [ ] Every advertised platform passes native release validation.

## Source of Status

The validation status is maintained in `../validation/P0_VALIDATION_REPORT.md`. Owner-controlled prerequisites are listed in `../EXTERNAL_BLOCKERS.md`.
