# Windows Release Hardening

## Objective

Verify that a clean Windows build can assemble, install, and execute the application and its sidecars reproducibly.

## Delivered

- Pinned Node, pnpm, and Rust toolchains.
- Windows GitHub Actions quality workflow.
- Tauri sidecar preparation with final-output hash verification.
- Recovery-aware client configuration replacement and rollback.
- DPAPI protection for Windows private key and local MCP capability material.
- NSIS installer assembly and silent clean-install smoke testing.
- Installed-binary checks and real MCP and Native Messaging responses.

## Verification

The release gate runs frontend checks, Rust formatting and linting, unit and integration tests, release-only tests, sidecar verification, installer assembly, installation, and installed-payload validation.

## Remaining Gates

Code signing, secure updates, SBOM and license review, macOS and Linux validation, privacy approval, and external product validation remain incomplete. This milestone does not represent a production release.
