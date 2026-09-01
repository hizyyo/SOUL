# Changelog

All notable changes to SOUL are documented in this file. The project follows [Semantic Versioning](https://semver.org/) for application builds.

## [0.2.0-alpha.1] - Unreleased

Status: active-development pre-release. No Git tag or GitHub Release has been published.

### Added

- Local-first desktop runtime built with Tauri, Rust, React, and TypeScript.
- Guided calibration, typed personal entities, candidate review, and activation flows.
- SQLCipher-backed local storage, FTS5 search, encrypted export, restore, and deletion.
- Deterministic context compilation with purpose, sensitivity, and token constraints.
- Local MCP integrations and a Chromium Browser Companion for supported AI clients.
- Blind preference evaluation, deterministic policy rules, and signed local receipts.
- Windows packaging, sidecar verification, clean-install smoke testing, and CI quality gates.

### Changed

- Reorganized public documentation around product, operations, validation, and development history.
- Replaced internal workflow terminology with descriptive milestone documentation.
- Added an official project logo, technology overview, CI status, and explicit pre-release labeling.

### Security

- Added ownership checks, bounded IPC, replay protections, signed capabilities, strict CSP, and fail-closed validation at privileged boundaries.
- Clarified that policy enforcement applies only to actions routed through a SOUL-controlled execution path.

### Not Yet Released

- Production Gateway connectors and credential isolation.
- Signed installers and secure updates.
- Cross-platform release validation.
- External P0 product validation and legal approval.
