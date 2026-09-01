# Security Hardening and Threat Testing

## Objective

Attack the implemented P0 prototype across storage, imports, authorization, IPC, browser integration, and policy execution, then fix actionable findings.

## Delivered

- Scoped ownership checks for entities, evaluations, and imported state.
- Bounded KDF parameters, chain verification, query limits, and line limits.
- Atomic import wipe and duplicate detection.
- Trusted-sender validation in the browser relay.
- Strict production CSP and removal of an unnecessary shell integration.
- Export escaping and hostile-content regression fixtures.
- Property tests for policy and capability behavior.

## Verification

Coverage included prompt-injection text, malicious markup, oversized archives, duplicate events, cross-profile access, malformed signatures, unsupported schemas, replayed capabilities, path abuse, and secret-like content.

## Outcome

Imported text remains data rather than executable instruction, and privileged boundaries reject invalid state without partially applying it.
