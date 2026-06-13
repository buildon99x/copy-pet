# 08. Codex / Claude Code Workflow

## Implementation order
1. Add tokens and primitive drawing helpers.
2. Refactor pet state machine without changing platform watchers.
3. Implement pet drawing and animation states.
4. Implement fish + nom queue.
5. Implement panel visual refresh with existing store/search behavior.
6. Implement keyboard/mouse edge cases.
7. Implement settings/tray labels and EN/KO strings.
8. Add preview frames for regression.
9. Run QA checklist.

## Agent rule
Never ask the agent to implement everything at once. Use milestone prompts from `prompts/` and require it to show:
- files changed
- behavior changed
- tests or preview command run
- screenshots/output artifacts

## Definition of done
- cargo fmt
- cargo clippy clean for target features available
- cargo test green
- preview example generates idle/typing/copy/panel frames
- no privacy regression
- no bundled fonts
- hotkey fallback reflected in UI
