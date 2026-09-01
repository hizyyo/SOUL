# Search and Context Compiler

## Objective

Select the smallest relevant set of approved personal context for a specific task.

## Delivered

- SQLite FTS5 indexing and bounded lexical search.
- Deterministic context ranking across entity type, relevance, confidence, and recency.
- Scope, status, time, and sensitivity filters.
- Supersession and explicit conflict handling.
- Standard and hard token budgets with serialized-pack overhead included.
- Stable state and policy versions for caching and receipts.

## Verification

Tests covered order independence, stale-search races, budget enforcement, sensitivity defaults, conflict limits, duplicate suppression, and deterministic serialization.

## Product Boundary

The compiler does not send an entire profile when a narrower task-specific disclosure is sufficient.
