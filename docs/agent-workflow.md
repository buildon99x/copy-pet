# Agent dev-workflow (Claude Code + routine)

A small, reusable structure that lets a **Claude Code routine** drive a whole dev
task — from a GitHub *work-request* issue all the way to a PR — by running **one
skill** on a schedule. State lives entirely in GitHub, so it works in the
routine's fresh-clone-every-run cloud environment.

```
작업 요청 이슈  →  계획(docs/plans/…)  →  계획 리뷰(사람 승인)  →  구현  →  검증
   work request      plan                    review (human)        implement   verify
                                                                                  │
        완료 보고 이슈(스샷)  ←─────────────────────────────────────────────────┘
        completion report      →  PR  →  충돌 해결 / conflict resolution  →  done
```

## Pieces

| File | Role |
|---|---|
| `.claude/skills/task-new/SKILL.md` | **Register** a work-request issue (human entry point). |
| `.claude/skills/task-run/SKILL.md` | **Driver** — a routine runs this; advances **one** task **one** run. |
| `.claude/agent-task.json` | The only project-specific file (all keys optional). |
| `docs/plans/` | Where each task's plan doc is written. |

## How a task flows

1. Someone runs **`/task-new`** (interactively) describing the work. It opens an
   issue labelled `agent-task` + `agent:queued` with a workflow checklist.
2. The routine fires and runs **`/task-run`**. It picks the one task, creates a
   `claude/<slug>` branch, writes `docs/plans/<slug>-<date>.md`, comments the
   plan, sets `agent:needs-approval`, and **stops** (human gate).
3. A maintainer reviews the plan and comments **`/approve`** (or just `approve`)
   — or asks for changes. Matching is normalized (case- and punctuation-
   insensitive), so the leading-slash form survives API escaping.
4. The next routine run resumes: **implement → verify (+ screenshots) → completion
   report issue → PR → conflict check → `agent:done`**. Every stage posts a
   progress comment on the work-request issue; the report issue and the PR
   cross-link back to it.

### Status labels (the state machine)

`agent-task` marks a workflow issue (vs. a normal issue). Exactly one status at a
time: `agent:queued` → `agent:in-progress` → `agent:needs-approval` →
(`agent:blocked`) → `agent:done`. The completion report gets `agent-report`.
Fine-grained progress is the **checklist in the issue body**; `task-run` resumes
at the first unchecked box, so a crashed/interrupted run is safe to re-run.

> Labels are auto-created when the first issue is filed with them. If your repo
> doesn't auto-create labels, make them once in **Settings → Labels** — the
> workflow still functions without them thanks to the `<!-- agent-task:v1 -->`
> body marker, but the board reads better with labels.

## Anti-runaway guardrails

- Only ever touches issues carrying the `agent-task` label/marker.
- Never auto-acts on `blocked` / `done` / closed issues.
- **One** task advanced per run; **one** `in-progress` task at a time.
- A **human approval gate** before any code is written (toggle with
  `require_plan_approval`).
- An **attempt cap** (`max_attempts`, default 3): repeated verify failures or
  replans flip the issue to `agent:blocked` and stop.
- Only commits to the task's `claude/<slug>` branch; never to the base branch.
- Any unexpected error just comments and exits, leaving status `in-progress` so
  the next run resumes — it never loops in place.

## Set up the routine (Claude Code web/desktop)

1. **Merge these skills to your default branch** (`main`). A routine clones the
   default branch each run, so the skills must live there to be found.
2. Create a routine bound to this repo with:
   - the **GitHub connector** enabled (the workflow uses GitHub MCP tools),
   - network access **Trusted** (default),
   - pushes allowed to `claude/` branches (the default; that's why the branch
     prefix is `claude/`),
   - **prompt:** `/task-run`,
   - **schedule:** hourly or daily.
3. File work with `/task-new` whenever you have a task. The routine drains the
   queue one task per run.

That's it — the routine just keeps calling `/task-run`; the GitHub issue state
tells it what to do next.

## Configuration — `.claude/agent-task.json`

All keys are optional; the skills fall back to these defaults:

| Key | Default | Meaning |
|---|---|---|
| `plans_dir` | `docs/plans` | Where plan docs are written. |
| `base_branch` | `main` | PR base; branches fork from here. |
| `branch_prefix` | `claude/` | Work-branch prefix (keep `claude/` for routine push rules). |
| `verify_commands` | `["cargo build --release", "cargo clippy --release", "cargo test --release"]` | Run in order at the Verify stage; any failure blocks. |
| `screenshot` | *(see below)* | `{command, src_glob, commit_dir}` — optional; omit to skip screenshots. |
| `max_attempts` | `3` | Verify/replan failures before `agent:blocked`. |
| `require_plan_approval` | `true` | Human gate after planning. |
| `approval_keyword` | `/approve` | The comment that approves a plan. |
| `labels` | see below | Override any label name. |

`screenshot` collects "tested" images for the completion report: it runs
`command`, then copies files matching `src_glob` into `commit_dir` (committed to
the branch and embedded in the report by commit-pinned raw URL). This repo uses
the headless preview renderer:

```json
"screenshot": {
  "command": "cargo run --release --example preview",
  "src_glob": "/tmp/clipcat-preview/*.png",
  "commit_dir": "docs/plans/screenshots"
}
```

`labels` defaults: `task=agent-task`, `queued=agent:queued`,
`in_progress=agent:in-progress`, `needs_approval=agent:needs-approval`,
`blocked=agent:blocked`, `done=agent:done`, `report=agent-report`.

## Reuse in another project

1. Copy `.claude/skills/task-new/` and `.claude/skills/task-run/` into the repo.
2. Drop an `.claude/agent-task.json` with that project's `verify_commands` (and
   `screenshot`, if it can produce images headlessly). Everything else can stay
   default; `owner/repo` is auto-detected from `git remote`.
3. Merge to the default branch and create the routine as above.

The skill instructions contain **no** ClipCat specifics — only the config does —
so the same two skills drive any repo's workflow.
