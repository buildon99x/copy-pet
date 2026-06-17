# Plan: agent-workflow auto-review smoke test

Work request: buildon99x/copy-pet#38 · Date: 2026-06-17

## Context

Re-verify the agent dev-workflow end-to-end in **automated self-review** mode
(`require_plan_approval: false`), where `task-run` reviews its own plan instead of
waiting for a human. Uses a trivial, reversible change so the workflow machinery
is what's exercised.

## Approach

Add a single one-line note file. Nothing else changes — keeps verify fast and
cleanup trivial.

## Files to change

- `docs/plans/SMOKE-E2E-AUTO.md` — new file, one line marking the auto-review run.

## Verification

- Run the configured `verify_commands` (cargo build/clippy/test) at the Verify
  stage; a green run confirms the workflow drove verify correctly.
- Doc-only change cannot affect build/test behavior.
