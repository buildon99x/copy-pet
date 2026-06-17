---
name: task-new
description: Register a work-request (agent task) issue for the agent dev-workflow. Creates a GitHub issue tagged with the agent-task label, a queued status, and a workflow checklist so the task-run driver (or a routine) can pick it up. Use when asked to file/queue/register a task, work request, or agent task.
allowed-tools: Bash(git *), mcp__github__issue_write, mcp__github__search_issues, mcp__github__list_issues
---

# task-new — register an agent work request

Creates a **work-request issue** that the `task-run` driver can pick up. This is
the one human-facing entry point of the workflow. Keep it small: gather the task
details, build the body, create the issue.

This skill is **project-agnostic**. Everything specific to a repo lives in
`.claude/agent-task.json` (optional — sensible defaults below). See
`docs/agent-workflow.md` for the whole system.

## 0. Resolve context

1. **owner/repo** — run `git remote get-url origin`, take the **last two path
   segments**, strip a trailing `.git`. (Works for `git@github.com:o/r.git`,
   `https://github.com/o/r`, and proxied `http://…/git/o/r` URLs.)
2. **config** — read `.claude/agent-task.json` if present; otherwise use the
   defaults. Only `labels.task`, `labels.queued`, and `plans_dir` matter here.
   Defaults: `labels.task = "agent-task"`, `labels.queued = "agent:queued"`.

## 1. Gather the task

From the user's request (or the skill arguments) collect:
- **title** — one line, imperative.
- **description** — what to build/change and why.
- **acceptance criteria** — a short checklist of "done" conditions. If the user
  gave none, infer 2–4 obvious ones and say you inferred them.

Derive a **slug**: lowercase the title, keep `[a-z0-9]`, collapse the rest to
single `-`, trim to ~50 chars. Example: "Fix panel scroll on Windows" →
`fix-panel-scroll-on-windows`.

## 2. Guard against duplicates

Search open agent tasks with the same slug before creating:
`search_issues` query `repo:<owner>/<repo> is:issue is:open label:<labels.task> in:body "slug: <slug>"`.
If a match exists, **stop** and link the existing issue instead of making a second one.

## 3. Build the body

Use this exact structure (the HTML markers are machine-read by `task-run` — keep
them). Content is bilingual KO/EN to match the repo; the markers/headings are not.

```
<!-- agent-task:v1 -->
## 작업 요청 · Work request

<description>

## 완료 기준 · Acceptance criteria

- [ ] <criterion 1>
- [ ] <criterion 2>

## Workflow state

- [ ] 1. Plan (`<plans_dir>/<slug>-<yyyymmdd>.md`)
- [ ] 2. Plan review (human approval)
- [ ] 3. Implement
- [ ] 4. Verify
- [ ] 5. Completion report
- [ ] 6. PR
- [ ] 7. Conflicts resolved

<!-- agent-meta
slug: <slug>
attempts: 0
branch:
plan:
report_issue:
pr:
-->
```

## 4. Create the issue

Call `issue_write` with `method:"create"`, the title, the body above, and
`labels: [<labels.task>, <labels.queued>]`.

**Label fallback (robustness):** if the create fails citing the labels (a repo
where issue-create does not auto-create missing labels), retry the create with
**no `labels`** — the `<!-- agent-task:v1 -->` marker still makes the issue
discoverable — and warn the user that the status labels
(`agent-task`, `agent:queued`, …) should be created once in repo Settings →
Labels (or by an admin) so the board reads cleanly.

## 5. Report

Print the new issue number + URL and a one-line note that `task-run` (or the
routine) will pick it up from `queued`. Do **not** start working the task here —
that is `task-run`'s job.
