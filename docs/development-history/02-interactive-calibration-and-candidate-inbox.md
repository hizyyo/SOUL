# Interactive Calibration and Candidate Inbox

## Objective

Create a useful initial profile without requiring chat-history imports or hidden background collection.

## Delivered

- Guided calibration flow with explicit progress and resumable state.
- Typed candidate generation from user-selected answers.
- Candidate inbox with confidence, provenance, sensitivity, and scope.
- Confirm, edit, reject, and defer actions.
- Local-only persistence for incomplete calibration state.
- Guardrails preventing inferred boundaries from becoming active without confirmation.

## Verification

Tests covered deterministic candidate generation, invalid answers, interrupted progress, duplicate submissions, and confirmation requirements.

## Product Boundary

Calibration creates reviewable candidate state. It does not claim to infer a complete personality or predict future decisions.
