# P0 Product Specification

## Purpose

SOUL is a local-first runtime for a user-owned representation of preferences, decisions, goals, facts, boundaries, and delegated authority. It integrates with AI clients the user already uses and does not provide a general-purpose chat interface of its own.

## Product Objective

The P0 prototype must let a user:

1. Create a useful local SOUL through guided calibration without importing external history.
2. Inspect, correct, confirm, or reject typed information before it becomes active.
3. Compile a minimal task-specific context package.
4. Connect that context to supported coding clients and web chats.
5. Compare personalized output against a reviewed baseline in a blind evaluation.
6. Demonstrate deterministic policy decisions and a locally mediated action receipt.
7. Export, restore, and delete their data.

## Product Boundaries

- The local store is the source of truth.
- Export and deletion remain available regardless of future pricing.
- Imported and model-generated content is untrusted until validated.
- Sensitive inferred information requires explicit confirmation.
- A standard context package should remain within 400-900 tokens, with an absolute limit of 3,000 tokens.
- Policy evaluation should remain deterministic on the normal execution path.
- The prototype must not claim to predict a person or govern arbitrary external agents.
- Enforcement applies only when an action is executed through a SOUL-controlled path.

## Functional Scope

| Capability                           | P0 requirement |
| ------------------------------------ | -------------- |
| Local profile creation               | Required       |
| Guided calibration                   | Required       |
| Typed entities and provenance        | Required       |
| Candidate review and correction      | Required       |
| Encrypted local persistence          | Required       |
| Full-text search                     | Required       |
| Context compilation                  | Required       |
| MCP client integration               | Required       |
| Chromium Browser Companion           | Required       |
| Blind preference evaluation          | Required       |
| Deterministic policy engine          | Required       |
| Simulated local Gateway              | Required       |
| Signed export, restore, and deletion | Required       |
| Feature flags and release checks     | Required       |

## Explicit Non-Goals

P0 does not include:

- a foundation model;
- autonomous continuous learning;
- arbitrary code execution inside a `.soul` package;
- browser automation;
- passive desktop, browser, email, or microphone surveillance;
- unencrypted cloud memory;
- production financial or destructive actions;
- a marketplace, social network, mobile application, or enterprise administration console;
- production claims for the simulated Gateway.

## Interaction Model

The desktop application is a control center for calibration, review, integrations, context disclosure, evaluations, permissions, export, and deletion. Users continue to write prompts in their existing AI clients.

Primary navigation:

```text
Home
Inbox
Tests
Context
Settings
```

## Data Model

SOUL stores typed entities such as:

- preferences;
- decisions and rationale;
- boundaries;
- goals;
- facts;
- communication style;
- permissions and policy state;
- provenance, confidence, sensitivity, and lifecycle metadata.

## Context Compilation

For each task, the compiler selects relevant active entities, applies sensitivity and policy constraints, ranks evidence, enforces a token budget, and emits a structured context package with disclosure metadata.

The compiler must not send an entire profile when a narrower disclosure is sufficient.

## Integrations

P0 supports two integration paths:

- a local MCP server for supported coding clients;
- a Chromium Browser Companion backed by native messaging for supported web chats.

Each integration must expose what context was selected and preserve a user-controlled approval boundary where required.

## Evaluation

The Blind Preference Test compares two responses under matched provider settings. Assignment remains hidden until the user selects response A, response B, or neither. Results measure preference between responses; they do not prove future decision prediction.

## Policy and Gateway

The policy engine evaluates typed rules and returns `allow`, `deny`, or `require_confirmation`. The P0 Gateway demonstrates a mediated local execution flow with scoped capabilities and signed receipts.

Production enforcement requires SOUL to control the destination credentials and execution channel. That boundary is not part of P0.

## Quality Gates

Changes must preserve:

- type safety and lint compliance;
- unit and integration coverage for changed behavior;
- migration and restart tests for persistence changes;
- hostile-input tests for parsers and security boundaries;
- fail-closed behavior for high-impact actions;
- no raw personal content or secrets in logs, telemetry, fixtures, or commits;
- local policy p95 below 5 ms and local context-path p95 below 75 ms in release checks.

## Release Gate

P0 remains a prototype until external validation and the release requirements in [Production Readiness](../operations/production-readiness.md) are complete. Billing and broader P1 scope are intentionally blocked until validation produces an explicit product decision.
