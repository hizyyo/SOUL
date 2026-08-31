# Interactive Calibration and Candidate Inbox

**Date:** 2026-07-31

## What was built

### Rust Backend
- `db.rs`: added `get_calibration`, `save_calibration`, `activate_soul`, `is_soul_activated`, `update_entity`
- `lib.rs`: new Tauri commands `get_calibration_cmd`, `save_calibration_cmd`, `activate_soul_cmd`, `update_entity_cmd`; `SoulInfo` extended with `activated` and `calibration_step` fields

### Frontend Data
- `src/data/calibration.ts`: 25 calibration questions across 4 steps (12 binary, 5 multiple-choice, 5 text, 3 writing samples)

### Frontend Pages
- `src/components/Nav.tsx`: 5-tab navigation (Home, Inbox, Tests, Context, Settings) with candidate badge
- `src/pages/Home.tsx`: state-driven CTA (create soul → start calibration → continue/activate → connect AI client)
- `src/pages/Calibration.tsx`: multi-step wizard with progress bar, binary/multiple-choice/text/writing inputs, per-step save, entity creation on completion
- `src/pages/Inbox.tsx`: candidate list with Confirm/Reject buttons
- `src/pages/Tests.tsx`, `Context.tsx`, `Settings.tsx`: placeholder stubs
- `src/App.tsx`: rewired to orchestrate all pages, calibration flow, and Tauri commands

## Verification
- `cargo check` — clean, no warnings
- `pnpm typecheck` — clean
- `pnpm lint` — clean
- `pnpm test` — 12/12 tests pass

## Design Decisions
- Calibration answers saved per-step to avoid data loss; entity creation happens only on final completion
- Inbox shows entities with `status = 'candidate'`; Confirm sets `active`, Reject sets `rejected`
- `exactOptionalPropertyTypes: true` on TS config required explicit `| undefined` on optional props
- `useCallback` hooks moved before early return to satisfy React hooks rules
