---
name: task-run
description: Drive one agent work-request issue forward by one run through the dev workflow (plan, human-approved review, implement, verify, completion-report issue, PR, conflict resolution). This is the entry point a Claude Code routine invokes on a schedule. Advances exactly one task per run and is safe to re-run.
disable-model-invocation: true
allowed-tools: Bash(git *), Bash(cargo *), Bash(mkdir *), Bash(cp *), Bash(ls *), mcp__github__issue_write, mcp__github__issue_read, mcp__github__add_issue_comment, mcp__github__search_issues, mcp__github__list_issues, mcp__github__create_pull_request, mcp__github__list_pull_requests, mcp__github__pull_request_read, mcp__github__update_pull_request_branch, mcp__github__list_branches
---

# task-run — agent dev-workflow driver

Picks **exactly one** agent task and advances it **one run**. Designed to be run
unattended by a Claude Code routine (prompt: `/task-run`) but also runnable by
hand. All state lives in GitHub (labels + issue body) because a routine starts
from a fresh clone each run — never rely on local files surviving between runs.

Project specifics come from `.claude/agent-task.json` (optional; defaults below).
Read `docs/agent-workflow.md` for the full design.

## Golden guardrails (anti-runaway — obey before doing anything)

- **Only agent tasks.** Touch an issue only if it carries the `<labels.task>`
  label **or** the `<!-- agent-task:v1 -->` body marker. Never touch a normal issue.
- **Never** auto-act on a `blocked`, `done`, or closed issue.
- **One task per run.** Select one, advance it, stop.
- **One active task.** If any task is already `in-progress`, you may only resume
  *that* one — never start a second.
- **Attempt cap.** Read `attempts:` from the issue's `agent-meta`. If it is
  `>= max_attempts` (default 3), set the issue `blocked`, comment why, and stop.
- **Review gate before code.** Always gate implementation behind a plan review.
  By default (`require_plan_approval` false) this is an **automated self-review**
  (stage 2): the driver critiques its own plan and only proceeds if it passes.
  Set `require_plan_approval: true` to instead require an explicit **human**
  approval — the run stops after planning until a maintainer approves.
- **No base-branch writes.** Only ever commit/push to the task's
  `<branch_prefix><slug>` branch. Never push to `<base_branch>`.

## 0. Resolve context

1. **owner/repo** — `git remote get-url origin`, last two path segments, strip `.git`.
2. **config** — read `.claude/agent-task.json` or use defaults:
   `plans_dir=docs/plans`, `base_branch=main`, `branch_prefix=claude/`,
   `verify_commands=["cargo build --release","cargo clippy --release","cargo test --release"]`,
   `screenshot={command,src_glob,commit_dir}` (optional — skip stage screenshots if absent),
   `max_attempts=3`, `require_plan_approval=false`, `approval_keyword="/approve"`,
   `labels={task,queued,in_progress,needs_approval,blocked,done,report}`.
3. `git fetch origin` so branch/PR checks are current.

## 1. Select the one task

Query open agent tasks: `list_issues` with `labels:[<labels.task>] state:OPEN`
(fall back to `search_issues … in:body "agent-task:v1"` if the label is absent).
Then choose, in this priority order:

1. **Resume** an `<labels.in_progress>` task → go to its first unchecked stage.
2. An `<labels.needs_approval>` task **that was approved**: read its comments
   (`issue_read get_comments`). Approved = a comment **after** the plan comment,
   from a repo collaborator (the issue author counts), that matches the approval
   keyword. **Match by normalizing both sides** — lowercase and strip every
   non-alphanumeric character, then check the comment contains the keyword's core
   (default `approve`). This normalization is required: a leading-slash token like
   `/approve` is escaped to e.g. `·/·a·pprove` when read back through the API, so a
   raw substring match would miss it. → resume at **Implement**.
   - If instead the latest reviewer comment requests changes (its normalized text
     contains `replan` / `changes` / `재계획`), bump `attempts` and go back to **Plan**.
   - Otherwise it is still waiting → **exit** ("awaiting approval on #N").
3. The **oldest** `<labels.queued>` task → **start** it (only if no task is
   `in-progress`; the order above already guarantees this).
4. Nothing actionable → **exit cleanly** ("no agent task to run").

Once selected, parse the body: the **Workflow state** checklist (first unchecked
box = next stage) and the `agent-meta` block (`slug, attempts, branch, plan,
report_issue, pr`). Helper to update meta/checklist: re-`issue_write update` the
**whole body** with the edited markers (GitHub has no partial-body edit).

If `attempts >= max_attempts` → set `<labels.blocked>`, comment, stop.

## 2. Stages

Run stages **in order from the first unchecked box**, posting a progress comment
and ticking the box after each. Stop at the approval gate, at completion, or on
an unrecoverable error. Wrap every stage in error handling (see §3).

**Start (queued → in-progress).** Swap `<labels.queued>` for
`<labels.in_progress>` (`issue_write update labels:[…]`). Comment: `▶️ 작업 시작 ·
Started — picking up this task.`

**1 · Plan.** Create the branch off base if missing:
`git fetch origin && git switch -c <branch_prefix><slug> origin/<base_branch>`
(or `git switch <branch>` if it already exists). Write
`<plans_dir>/<slug>-<yyyymmdd>.md` covering: Context, Approach, Files to change,
Verification. Commit + push (`git push -u origin <branch>`). Record `branch` and
`plan` in `agent-meta`; tick box 1. Comment a short plan summary + a link to the
file on the branch.
- If `require_plan_approval` is **true** (human gate): set `<labels.needs_approval>`,
  comment `🧐 계획 리뷰 요청 · Plan review — reply with \`<approval_keyword>\` to approve,
  or ask for changes.` and **exit**. (A later run resumes after approval.)
- Else (default): continue to the automated self-review.

**2 · Plan review.**
- **Human-gate mode** (reached only when resumed after a maintainer `<approval_keyword>`):
  tick box 2; comment `✅ 계획 승인됨 · Plan approved — implementing.`
- **Automated self-review** (default): critique your own plan against a short
  rubric — (a) does it cover every acceptance criterion? (b) is the scope bounded
  and the change reversible? (c) any obvious feasibility risk or missing step?
  Keep it lightweight (no heavy tooling). Post the verdict as a comment
  (`🤖 자동 계획 리뷰 · Self-review`: PASS + 1–3 bullets, or the blockers found).
  - **PASS** → tick box 2; proceed to Implement.
  - **Blocking issues** → revise the plan doc to address them, commit + push,
    bump `attempts`, and re-review. If `attempts >= max_attempts`, set
    `<labels.blocked>` with the open concerns and stop.

**3 · Implement.** Make the changes described in the plan. Commit + push to the
branch. Tick box 3; comment a 1–3 line summary of what changed.

**4 · Verify.** Run each `verify_commands` entry. On **any** failure: bump
`attempts` in meta, comment the failing command + the tail of its output, and —
if `attempts >= max_attempts` set `<labels.blocked>` and stop, else stop and let
the next run retry. On **success**: if `screenshot` is configured, run
`screenshot.command`, `mkdir -p <screenshot.commit_dir>`, copy `screenshot.src_glob`
there, commit + push. Tick box 4; comment the verify results (commands that
passed) and note screenshots if any.

**5 · Completion report.** Create a **new** issue (`issue_write create`,
`labels:[<labels.report>]`) titled `✅ 완료 보고 · Done: <task title> (#<task#>)`.
Body: summary of changes, the verify command results, and — if screenshots exist
— embed them with **permanent commit-pinned raw URLs**
`https://raw.githubusercontent.com/<owner>/<repo>/<commit-sha>/<path>` (use the
SHA from `git rev-parse HEAD` on the pushed branch, **not** the branch name, so
the images survive branch deletion). End with `관련 작업 요청 · Work request: #<task#>`.
Record `report_issue` in meta; tick box 5; comment on the **work-request** issue:
`📄 완료 보고 · Completion report: #<report#>`.

**6 · PR.** If a PR already exists for the branch (`list_pull_requests
head:<owner>:<branch>`), reuse it; else `create_pull_request` base=`<base_branch>`
head=`<branch>`. Title = task title. Body must reference **both**:
`Closes #<task#>` and `완료 보고 · Report: #<report#>`. Record `pr` in meta; tick
box 6; comment `🔀 PR: #<pr#>` on the work-request issue.

**7 · Conflicts.** Read PR mergeability (`pull_request_read get` →
`mergeable`/`mergeable_state`). If clean/`mergeable`, tick box 7. If behind or
conflicting, try `update_pull_request_branch` (merges base into the branch); if
GitHub reports conflicts it cannot auto-merge, resolve locally:
`git fetch origin && git merge origin/<base_branch>` on the branch, fix the
conflicts faithfully, commit + push, re-check. If still unresolvable → comment
the conflicting files and set `<labels.blocked>`; stop. On success tick box 7.

**8 · Done.** Set `<labels.done>` (remove `in_progress`). Final comment on the
work-request: `🎉 완료 · Done — Report #<report#>, PR #<pr#>.` Stop.

## 3. Error handling

- Treat any GitHub API or git/command error as a **soft failure for this run**:
  comment the error on the work-request issue, leave the status `in-progress`
  (so the next run resumes the same stage), and **exit** — do not loop or retry
  in-process. The `attempts` counter (bumped only on **verify**/**replan**)
  bounds runaway; everything else simply resumes next run.
- Idempotency: before each side-effecting step, check whether it was already done
  (branch exists, plan file committed, report issue recorded in meta, PR exists)
  and skip rather than duplicate.
- If selection found nothing, exit quietly with a one-line status. A routine run
  with no work is a success, not an error.

## 4. Output

End with a compact status line: the task issue #, the stage you advanced to (or
"exited at gate" / "no work"), and links to the report issue / PR if created.
