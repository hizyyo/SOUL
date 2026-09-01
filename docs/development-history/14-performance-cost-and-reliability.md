# Performance, Cost, and Reliability

## Objective

Keep local context and policy paths fast, bounded, and observable without adding model calls to the hot path.

## Delivered

- Context cache keyed by state, policy, and task inputs.
- Import deduplication and monotonic state revisions.
- Incremental context packing and single-pass entity parsing.
- Input-token and estimated-cost metadata in disclosure receipts.
- Usage aggregation without raw prompts or personal content.
- Release-only benchmarks and regression thresholds.

## Verification

Release checks measured policy and context p95 latency across representative data sizes and verified cache invalidation after state or policy changes.

## Product Boundary

SOUL does not call a model in the local compiler. Cost figures estimate the external input context added to a supported AI client.
