# Control Center and Receipts

## Objective

Turn the desktop shell into a state-aware control center with one clear next action.

## Delivered

- Home-state model covering creation, calibration, review, activation, and integration readiness.
- Single primary call to action derived from current local state.
- Secondary actions for candidate review, profile improvement, and receipt inspection.
- Local receipt viewer with bounded parsing and corruption tolerance.
- Responsive states and explicit loading and failure feedback.

## Verification

Tests covered state transitions, active-profile behavior, receipt size limits, malformed receipt files, inaccessible actions, and safe rendering.

## Follow-up

Context-disclosure receipts were added with [Local MCP Runtime and Client Integrations](08-local-mcp-runtime-and-client-integrations.md).
