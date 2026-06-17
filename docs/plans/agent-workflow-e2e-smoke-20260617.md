# Plan: agent-workflow E2E smoke test

Work request: buildon99x/copy-pet#34 · Date: 2026-06-17

## Context

A throwaway task to E2E-verify the new agent dev-workflow skills (`task-new` /
`task-run`). It exercises the full machinery — plan → human approval → implement →
verify → completion report → PR → conflict check — with a trivial, reversible
change so the workflow itself is what's under test, not any product code.

## Approach

Add a single one-line note file. Nothing else changes. This keeps the verify
stage fast and the cleanup trivial (delete the file / close the PR / delete the
branch).

## Files to change

- `docs/plans/SMOKE-E2E.md` — new file, one line noting it's an E2E smoke marker.

## Verification

- Project verify commands from `.claude/agent-task.json` run at the Verify stage.
- The change is doc-only, so it cannot affect build/test behavior; a green run
  confirms the workflow drove verify correctly.
