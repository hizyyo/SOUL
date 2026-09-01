# P0 Validation and Release Gates

## Objective

Convert product validation and release readiness into explicit, auditable gates.

## Delivered

- [Validation playbook](../validation/P0_VALIDATION_PLAYBOOK.md) for recruitment, consent, day-0 testing, and day-7/day-28 follow-up.
- [Validation report](../validation/P0_VALIDATION_REPORT.md) as the single source of validation status.
- Release checks covering frontend, Rust, sidecars, performance, packaging, and installation behavior.
- Explicit block on billing and broader product expansion before external evidence and an owner decision.

## Decision Rule

Internal tests and synthetic data cannot satisfy the external product-validation gate. Advancement requires independent participants, complete evidence, and a recorded `GO`, `ITERATE`, or `KILL` decision.

## Current State

Validation remains incomplete and therefore does not authorize a production release.
