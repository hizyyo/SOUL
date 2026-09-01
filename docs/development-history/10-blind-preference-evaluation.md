# Blind Preference Evaluation

## Objective

Measure whether SOUL-assisted responses are preferred over a reviewed baseline without revealing assignment before selection.

## Delivered

- Twenty-round local blind evaluation flow.
- Host-assigned randomized response slots.
- Reviewed baseline profile and task-specific SOUL context variants.
- Choice options for response A, response B, or neither.
- Assignment reveal only after the choice is persisted.
- Aggregate statistics and live refresh after mutation.

## Verification

Tests covered assignment secrecy, randomization, persistence, deletion cascades, aggregate calculations, baseline isolation, and style-normalized prompt construction.

## Product Boundary

The evaluation measures response preference. It does not prove that SOUL predicts a user's future real-world decisions.
