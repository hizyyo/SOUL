# Core Data Model and Local Storage

## Objective

Define stable contracts for user-owned personal state and persist them locally.

## Delivered

- Typed entities for preferences, decisions, boundaries, goals, and facts.
- Lifecycle, sensitivity, stability, provenance, scope, and confidence metadata.
- Local SQLite schema for profiles, entities, events, and metadata.
- Event-chain integrity fields and deterministic identifiers.
- Rust commands for creating, listing, and mutating local state.
- Shared TypeScript schemas for frontend and integration boundaries.

## Verification

Schema validation, invalid-input handling, CRUD behavior, event ordering, and restart persistence were covered by automated tests.

## Follow-up

Guided calibration was added in [Interactive Calibration and Candidate Inbox](02-interactive-calibration-and-candidate-inbox.md). Encryption at rest was introduced later in [Simulated Gateway and Storage Hardening](12-simulated-gateway.md).
