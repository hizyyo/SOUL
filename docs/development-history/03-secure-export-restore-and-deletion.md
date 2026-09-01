# Secure Export, Restore, and Deletion

## Objective

Make user ownership operational through portable backup, verified restore, and complete local deletion.

## Delivered

- Versioned `.soul` package envelope with a signed manifest and encrypted payload.
- Password-based key derivation with bounded resource parameters.
- Preview-before-restore flow with schema and signature validation.
- Atomic restore that preserves existing data when validation fails.
- Full local deletion with a receipt that excludes personal content.
- Size, version, duplicate, and malformed-package rejection.

## Verification

Tests covered tampering, wrong passwords, unsupported versions, oversized input, duplicate identifiers, interrupted restore, and deletion completeness.

## Follow-up

Fine-grained entity review and deletion controls were added in [Entity Review Control Center](04-entity-review-control-center.md).
