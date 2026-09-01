# Initial SOUL Compilation and Activation

## Objective

Compile reviewed calibration answers into a deterministic initial profile and require explicit activation.

## Delivered

- Deterministic mapping from calibration state to typed entities.
- Human-readable preview before activation.
- Explicit confirmation for boundaries and sensitive candidates.
- Idempotent activation and legacy-data deduplication.
- Reset behavior when the previewed state changes.
- Activation receipts without raw personal content.

## Verification

Tests covered repeat compilation, ordering stability, duplicate prevention, stale confirmations, boundary review, and recovery from partial state.

## Product Boundary

Activation confirms a user-reviewed local profile. It does not make generated inferences authoritative without consent.
