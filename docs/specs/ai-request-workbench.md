# AI request workbench / AI 요청 작업대

The workbench prepares text locally for an AI application you choose. It does
not call an AI provider or add any network destination. Open the existing
clipboard panel and click **AI workbench** in its footer, press **Ctrl+B**
(**Cmd+B** on macOS), or use the Windows tray / macOS context-menu entry.

## Build and reuse a request

1. Choose **Request**, type or paste text, and press Enter to save. Ctrl/Cmd+A
   selects the entire draft; Ctrl/Cmd+V pastes; Ctrl/Cmd+Enter inserts a newline;
   Backspace edits at the end. Esc cancels that edit. Read the entire draft with
   the wheel, Up/Down, PageUp/PageDown, Home or End (Ctrl+Home/End also works).
   These keys move the preview, not the insertion point. New typing, pasting or
   deletion follows the draft end again. A scrollbar and visible-row range show
   your position. This is a small append/backspace editor, not a full text editor.
2. Choose **Select clips from history**. Click clips or use arrows and Enter to
   add snapshots, without copying or pasting. Keep selecting; Ctrl/Cmd+B or the
   footer returns to the tray. Search and app/tag filters remain available.
3. Select a tray clip. Left/Right or Enter changes its section: Request,
   Related code, Errors, Constraints. Ctrl/Cmd+P moves it up; Ctrl/Cmd+Z moves it
   down; Delete removes it. These keys affect the selected tray item only.
   Within each section, the prompt follows tray order. Removing a snapshot
   leaves the original history entry intact. History eviction does not remove
   the tray's text. Images produce an explicit **attach separately** reminder;
   image bytes are never inserted into the prompt, and images are not attached.
4. To make a favorite, put its body in Request, choose **Save request as
   template**, enter a name, and save. Use variables such as `{task}`, `{file}`,
   `{check}`, or `{작업 내용}`. Variable names contain letters/digits, spaces,
   underscores or hyphens, up to 40 characters. Other code braces are literal.
   Select a ★ favorite to load its original body, then select each **Fill
   variable** row. Values are literal, including JSON/braces, and repeated
   placeholders share one value. Select a filled row to change its value.
   All variables must be filled before review. Delete on a favorite removes it.
   Loading a favorite replaces the saved Request and its filled values.
5. Choose **Review & prepare safe copy**. Inspect replacement records and the
   complete wrapped output using arrows, PageUp/PageDown, or the wheel. Only
   **Confirm: copy reviewed text** writes to the clipboard. It always copies
   plain text and never automatically pastes into another application, including
   from the Windows hotkey flyout. Esc/Back cancels review. Originals remain.

The workbench uses compact rows independently of the history card/list setting.
Its seven main actions fit the default panel, and closing it restores the history
layout without changing your saved view preference.

## Projects and saved drafts

**Switch project** or the header's **P** button opens the explicit project list.
Create a project, then use **Rename project**. No active-window/title watcher
chooses a project for you. Each project keeps Request, filled variables, clip
snapshots, favorites, default tags, and its unfinished editor independently.
Switching away while editing and returning restores that draft. The default
project tags apply to newly added tray snapshots, with original tags preferred
and the existing eight-tag limit; they do not retag clipboard history.

`requests.json` lives beside `state.json` and `clips.json` in the application's
local configuration directory. The existing 30-second autosave and normal exit
save include unfinished drafts and the selected project. Review copies and UI
selection are session-only. Files are local plaintext, not encrypted storage;
redaction affects the outgoing copy, not stored originals. A crash before
an autosave can lose recent edits. A failed save retains dirty state for retry.

Limits: 24 projects, 40 clip snapshots and 24 favorites per project; 32 KiB per
Request/editor/template/expanded request and 256 KiB of **all text combined** per
project. Oversized changes are refused. The 64 MiB file-read limit exceeds the
maximum serialized payload, including JSON escapes and metadata. Invalid saved
models are not used. Existing `state.json` and `clips.json` formats are unchanged.

## Local sensitive-text check and limits

This check applies to **AI workbench output**. Ordinary clipboard-history
picks keep their existing rich-format and automatic-paste behavior.

The scanner checks plausible `sk-`, `ghp_`, `github_pat_`, `AKIA`, `xoxb-`,
`xoxp-` tokens; Bearer values; assignments/JSON keys ending in `api_key`,
`apikey`, `api-key`, `token`, `secret`, `password`, `passwd`, `access_key`, or
`private_key`; and PEM private-key blocks. Matching values become numbered
`[REDACTED_n]` placeholders. The review records kind and replacement label,
without echoing detected values. It logs no clipboard/request contents.

This is a heuristic, not a complete secret detector. Custom/short/encoded keys,
nonstandard assignment syntax, multiple assignments on a line, secrets in
images, filenames, rich-only formats and unknown providers can be missed;
example strings can also match. Review the output yourself before sharing.
The app does not inspect global keystroke text or window titles.

## 한국어 사용 안내

클립보드 패널 아래 **AI 작업대** 또는 Ctrl+B로 열고, Windows 트레이에서도
진입할 수 있습니다. **요청**을 선택해 작성하거나 Ctrl+V로 붙여넣으세요.
Enter 저장, Ctrl+Enter 줄바꿈, Ctrl+A 전체 선택, Esc 편집 취소입니다.
긴 초안은 휠·위/아래·PageUp/PageDown·Home/End로 전체를 읽을 수 있습니다.
입력 위치는 끝에 유지되며 새 입력·붙여넣기·삭제 후에는 끝부분으로 돌아옵니다.
스크롤바와 현재 표시 범위로 위치를 확인하세요. 작업대는 히스토리 보기 설정과
별개로 작은 행을 사용하므로 기본 크기에서도 주요 작업이 모두 표시됩니다.

**히스토리에서 클립 선택**을 누르고 필요한 항목을 여러 번 선택한 뒤 Ctrl+B로
돌아오세요. 트레이 항목을 선택해 ←/→로 요청·관련 코드·오류·제약 섹션을
바꾸고, Ctrl+P로 위로 / Ctrl+Z로 아래로 이동하며 Delete로 제거합니다.
이미지는 별도 첨부 안내만 포함하며 실제 이미지 첨부는 사용자가 해야 합니다.

요청 본문에 `{작업 내용}`, `{대상 파일}`, `{검증 명령}`을 넣고 **요청을
템플릿으로 저장**하면 이름을 붙여 즐겨찾기에 보관합니다. ★ 항목을 다시
선택하고 각 **변수 채우기** 행에서 값을 입력하세요. 변수 값의 JSON 중괄호는
다시 변수로 해석하지 않습니다. **검사 후 안전한 복사 준비**에서 치환 내역과
전체 사본을 읽은 뒤 **확인: 검토한 텍스트 복사**를 눌러야 복사됩니다.
자동 붙여넣기는 하지 않습니다. 원본 클립과 템플릿은 그대로 보존합니다.

헤더 **P** 또는 **프로젝트 전환**으로 작업을 명시적으로 바꿉니다. 프로젝트별
클립·요청·즐겨찾기·기본태그·입력 중인 초안이 저장되며, 창 제목을 감시해
프로젝트를 추측하지 않습니다. 한 프로젝트의 모든 텍스트 합계는 256 KiB,
항목은 40개, 즐겨찾기는 24개입니다. 원문은 로컬 JSON에 평문으로 남습니다.
검사는 위 패턴만 다루므로 누락과 오탐이 있을 수 있습니다. 공유 전 직접
확인하세요. 일반 히스토리 선택은 이 검사 대상이 아니며 기존 동작을 유지합니다.
