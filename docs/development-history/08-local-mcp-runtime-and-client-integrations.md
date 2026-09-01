# Local MCP Runtime and Client Integrations

## Objective

Expose purpose-scoped SOUL context to supported coding clients without moving work into a separate chat interface.

## Delivered

- Rust implementation of the deterministic context compiler.
- Local MCP sidecar with bounded line input and read-only database access.
- Client detection and configuration for supported coding tools.
- Safe configuration backup, replacement, rollback, and recovery.
- Disclosure receipts written before context leaves the runtime.
- Connection-state refresh and binary integration tests.

## Verification

Golden tests kept Rust and TypeScript compiler output aligned. Integration tests covered detection, connection, rollback, missing prior configuration, write failure, database location, and real sidecar responses.

## Security Boundary

Receipts contain disclosure metadata, not raw prompts, claims, evidence, or secrets.
