# Contributing to SOUL

SOUL is currently developed as a private, early-stage product. Issues and focused pull requests are welcome, but maintainers may decline changes that expand the product beyond its validated scope.

## Development Setup

```bash
pnpm install
pnpm dev
```

The repository pins Node, pnpm, and Rust versions. Use the versions declared in `.nvmrc`, `package.json`, and `rust-toolchain.toml` to keep local and CI results consistent.

## Before Opening a Pull Request

Run the checks relevant to your change:

```bash
pnpm typecheck
pnpm lint
pnpm test
pnpm build
pnpm build:companion
```

Rust changes should also pass:

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Use `pnpm release:check` when changing packaging, sidecars, installation behavior, security-sensitive runtime code, or release configuration.

## Engineering Standards

- Keep changes small, explicit, and limited to one concern.
- Validate untrusted input at system boundaries.
- Add regression tests for parsers, persistence, policy decisions, and security controls.
- Preserve user ownership, local-first behavior, and purpose-scoped context disclosure.
- Do not claim enforcement for actions that do not pass through a SOUL-controlled execution path.
- Never commit secrets, personal exports, local databases, generated extensions, installers, or build artifacts.

## Pull Requests

Describe the problem, the chosen behavior, tests performed, security implications, and known limitations. Link architectural or product decisions when a change modifies a public contract.

Historical implementation records are archived in `docs/development-history/`; they are not the current contribution process or normative specification.
