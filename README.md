# 🐱 ClipCat — the desktop cat that eats your clipboard

*[한국어는 아래에 ↓](#-clipcat--클립보드를-먹는-데스크탑-고양이)*

A clipboard manager with a heartbeat. A small cat sits at the bottom of your
screen and types along with you (à la Bongo Cat) — and **every time you copy
something, anywhere, it eats a fish badged with the app you copied from** and
files the clip into its history. Pick a clip to paste it back **with its
original formatting** (or as plain text). Pin, search, delete — all inside one
tiny (~1 MB) native binary.

**Windows · macOS · Linux** — fully transparent click-through native build on
Windows; a portable build sharing the same core on macOS/Linux.

![ClipCat](assets/screenshot.png)

## Features

- **Clipboard history** — every text copy is captured system-wide and stored
  locally (up to 100 clips + pinned clips that never expire). Rich text keeps
  its original formatting (bold, colors, links) on Windows and macOS.
- **The fish** 🐟 — each copy sends a fish flying into the cat's mouth, tinted
  and badged with the source app (its real icon on Windows, an initial
  elsewhere). Copying feeds the cat: +5 XP per clip.
- **Panel** — `Win+Shift+V` on Windows (configurable; auto-falls back to
  `Ctrl+Shift+V` if taken), `Cmd+Shift+V` on macOS, `Super+Shift+V` on
  Linux — or middle-click / the tray / `C`. The hotkey pops the list up **at
  your text caret** in the app you're typing in (Win+V parity); middle-click
  shows it by the cat. Type to search (Korean included), pick a clip to paste
  it back, ★ to pin, **...** for per-clip actions, 🗑 to clear, ⏸ to pause
  capture (privacy pause). Clips show as roomy **cards** by default; toggle the
  compact **list** view from the header (your choice is remembered).
- **Paste as text** — each row's **...** menu (or selecting the row and pressing
  **→**) reveals **Paste as text** and **Delete**; "Paste as text" strips the
  formatting and pastes the clean text. `Ctrl/Cmd+Enter` does the same for the
  selected clip.
- **Auto-paste on select** (off by default) — turn on "Paste on select" and
  picking a clip pastes it straight into the app you were just in, instead of
  only copying it (Windows returns focus and sends Ctrl+V; macOS/Linux are
  best-effort).
- **Filter by app** — the funnel button (or `Tab`) cycles through the apps
  you copied from; the active app shows as a chip in the search box and
  combines with the (multi-word, relevance-ranked) text search.
- **Arrange the panel your way** — drag its header to move it (the cat
  stays put), drag the bottom-right grip to resize it; both persist.
  **Ctrl+0–9** instantly copies one of the badged top ten rows, and
  "Close panel after copy" can be switched off to grab several clips.
- **Window modes** — **Always on top** (default), **Normal** (the pet can sit
  behind other windows) or **Hide** (it returns from the tray icon or the
  clipboard hotkey).
- **Stay up to date** — once a day ClipCat checks GitHub for a newer release
  and toasts when one exists. On Windows pick "Update to vX.Y.Z and restart";
  on macOS/Linux press `U` to open the download page. On by default, switchable
  off ("Check for updates automatically" / `auto_update`); it sends nothing but
  the request.
- **English + Korean** — full UI in both, switchable at runtime. All text
  renders in your **system font** (Segoe UI / Malgun Gothic on Windows —
  read from the OS, never bundled).
- **Bongo-cat core loop** — global input *counts* drive paw taps, XP and
  levels: 2 XP/key, 1 XP/click & scroll, 5 XP/copy. Level-ups unlock
  accessories (scarf, glasses, beanie, headphones, crown, wizard hat).
- **Today's stats bubble** — hover the cat: keys, clicks, copies, active time.
- **Alive** — breathing, blinking, tail, sweat when you type fast, sleep
  after 75 s idle, hearts when petted.
- **Unobtrusive & light** — always-on-top without stealing focus, transparent
  pixels click through (Windows), single exe, no installer, ~12-16 MB RAM.

## Controls

| Gesture | Effect |
|---------|--------|
| Copy anywhere (Ctrl+C) | Cat eats a fish; clip saved to history (formatting kept) |
| `Win+Shift+V` (`Cmd+Shift+V` on macOS) | Open the panel at your text caret |
| Middle-click | Open the panel by the cat |
| Pick a clip (click / Enter) | Paste it back (keeps formatting; or auto-paste if enabled) |
| `Ctrl/Cmd+Enter` (panel open) | Paste the selected clip as plain text |
| `Ctrl/Cmd+0`–`9` (panel open) | Quick-copy the row with that digit badge |
| ★ on a row / `Ctrl/Cmd+P` | Pin / unpin the clip |
| Row **...** menu (or `→`/`←`) | Reveal/hide **Paste as text** + **Delete** |
| `Del` / `Ctrl+Z` (panel open) | Delete the selected clip / undo a delete |
| Funnel button / `Tab` (panel open) | Cycle the source-app filter |
| Type while panel is open | Search (arrows, Home/End, Enter; Esc clears query → filter → closes) |
| Drag the panel header / corner grip | Move / resize the panel (cat stays put) |
| Drag | Move the pet (unless position-locked) |
| Click | Boop (squash bounce, +1 XP) |
| Double-click | Pet it (+10 XP, hearts) |
| Hover | Today's stats bubble |
| Right-click (Windows tray / macOS) | Settings menu (see below) |
| Tray icon click (Windows) | Hide/show the cat |

The settings menu (Windows tray, macOS right-click) holds: clipboard panel,
pause capture, close-after-copy, paste-on-select, panel hotkey (cycles through
safe presets), show-stats, Size / Accessory / Sound / Window submenus, lock
position, language, run at login, automatic updates, reset stats, About,
GitHub, quit.

### Portable build (macOS / Linux) keyboard shortcuts

Global (works everywhere): `Cmd+Shift+V` (macOS) / `Super+Shift+V` (Linux)
opens the clipboard panel. With the window focused (and the panel closed):
`C` clipboard panel · `V` list/card view · `O` close-after-copy · `K` panel
hotkey preset · `S` size · `A` accessory · `M` sound · `B` stats bubble ·
`L` lock · `G` language · `U` update page (when one is found) · `Q`/`Esc` quit.
(macOS also has the full right-click menu.)

## Platform differences

| | Windows (native) | macOS / Linux (portable) |
|---|---|---|
| Window | fully transparent, click-through | opaque "card" (softbuffer has no per-pixel alpha) |
| Clipboard watch | `WM_CLIPBOARDUPDATE` listener | `arboard` polling (~0.4 s) |
| Fish badge | real app icon | colored initial |
| Paste-back format | rich formats kept | kept on macOS · plain text on Linux |
| Panel hotkey | global `Win+Shift+V`, opens at caret (configurable, `Ctrl+Shift+V` fallback) | global `Cmd/Super+Shift+V`; caret flyout on macOS, embedded panel on Linux |
| Settings UI | tray menu | right-click NSMenu (macOS) · keyboard shortcuts (Linux) |
| Sound | winmm synth | silent |
| Global input | `WH_*_LL` hooks | `rdev` (Linux, X11 only) · CoreGraphics event tap (macOS) |

Design history lives in [`.context/kb/adr/`](.context/kb/adr/), the spec in
[`docs/specs/clipcat-spec.md`](docs/specs/clipcat-spec.md).

## Build

```bash
# default backend (Windows=native, macOS/Linux=portable)
cargo build --release          # binary: target/release/clipcat[.exe]

# test the portable backend on Windows
cargo build --release --features portable

# regenerate the icon / preview frames after art changes
cargo run --bin gen_icon
cargo run --release --example preview
```

Requires Rust (MSVC toolchain on Windows). Linux needs system libraries for
the portable stack — on Debian/Ubuntu: `apt-get install libx11-dev libxi-dev
libxtst-dev libxkbcommon-dev libxkbcommon-x11-dev pkg-config`. CI builds
Windows and macOS.

## Data & privacy

Everything stays on your machine. ClipCat makes **no network connection except
one**: a once-a-day update check to github.com releases (ADR-0009), which sends
nothing beyond the request and can be turned off ("Check for updates
automatically" / `auto_update` in `state.json`). There is no other network code.

- Input hooks count keystrokes/clicks only; **which** key is pressed is never
  read, stored or transmitted (`src/input.rs` atomic counters).
- Clipboard history is stored locally in `clips.json`; capture can be paused
  any time (⏸ in the panel / tray menu), clips > 256 KB are ignored, and
  stats/settings live in `state.json` next to it:
  Windows `%APPDATA%\ClipCat`, macOS `~/Library/Application Support/ClipCat`,
  Linux `$XDG_CONFIG_HOME/ClipCat`. A pre-2.0 `DeskCat` config dir is
  migrated automatically.

## Tech notes

- No GUI framework — [tiny-skia](https://github.com/linebender/tiny-skia)
  vector rendering + direct OS APIs per backend.
- One platform-agnostic core (`pet`, `clipboard`, `panel`, `render`, `state`,
  `i18n`, `menu`, `update`) + two backends: native Win32 (layered window, tray,
  clipboard listener, caret-anchored flyout) and portable (`winit` +
  `softbuffer` + `rdev`/CoreGraphics + `arboard`).
- Icon and sounds are generated from code — no bundled assets, single
  binary. All text renders in the OS's own fonts (loaded at runtime via
  `ab_glyph`, ADR-0007/0011).
- User-facing changes are tracked in [CHANGELOG.md](CHANGELOG.md); releases
  are cut with `scripts/release.sh`.

---

# 🐱 ClipCat — 클립보드를 먹는 데스크탑 고양이

살아있는 클립보드 매니저. 화면 아래에 앉은 고양이가 봉고캣처럼 타이핑을 따라
치고 — **어디서든 복사(Ctrl+C)할 때마다 복사한 앱의 뱃지가 붙은 생선을 냠**
하고 먹으며 클립을 히스토리에 저장합니다. 클립을 고르면 **원래 서식 그대로**
(또는 일반 텍스트로) 다시 붙여넣어지고, 고정·검색·삭제까지 — 전부 ~1MB짜리
네이티브 바이너리 하나로.

## 특징

- **클립보드 히스토리** — 시스템 전역의 텍스트 복사를 감지해 로컬에 저장
  (히스토리 100개 + 만료되지 않는 고정 클립). Windows·macOS에서는 리치
  텍스트의 서식(굵게·색·링크)이 그대로 보존됩니다.
- **생선** 🐟 — 복사할 때마다 출처 앱의 아이콘(Windows) 또는 이니셜 뱃지가
  붙은 생선이 고양이 입으로 날아갑니다. 복사 1회 = +5 XP.
- **패널** — `Win+Shift+V`(Windows 기본, 변경 가능 — 다른 앱이 선점하면
  `Ctrl+Shift+V`로 자동 대체), macOS `Cmd+Shift+V`, Linux `Super+Shift+V`,
  또는 휠클릭·트레이 메뉴·`C` 키로 열기. 핫키를 누르면 **입력 중인 텍스트
  커서 위치**에 목록이 뜨고(Win+V와 동일), 휠클릭은 고양이 옆에 띄웁니다.
  타이핑으로 검색(한글 지원), 클릭으로 재붙여넣기, ★ 고정, **...** 행별
  동작, 🗑 비우기, ⏸ 수집 일시정지(프라이버시 모드). 클립은 기본적으로
  넉넉한 **카드**로 보이며, 헤더에서 **리스트** 보기로 전환할 수 있습니다
  (선택은 기억됩니다).
- **텍스트로 붙여넣기** — 각 행의 **...** 메뉴(또는 행 선택 후 **→**)에서
  **텍스트로 붙여넣기**·**삭제**가 펼쳐집니다. "텍스트로 붙여넣기"는 서식을
  벗기고 깔끔한 텍스트만 붙여넣습니다. 선택한 클립에는 `Ctrl/Cmd+Enter`도
  동일하게 동작합니다.
- **선택 시 자동 붙여넣기**(기본 꺼짐) — "선택 시 붙여넣기"를 켜면 클립을
  고를 때 방금 쓰던 앱에 바로 붙여넣어집니다(Windows는 포커스를 되돌려
  Ctrl+V 전송, macOS·Linux는 최선 노력).
- **출처 앱 필터** — 깔때기 버튼(또는 `Tab`)으로 복사해 온 앱별로 클립을
  걸러 봅니다. 활성 필터는 검색창의 칩으로 표시되고, 여러 단어·관련도 순으로
  정렬되는 텍스트 검색과 함께 적용됩니다.
- **패널을 내 마음대로** — 헤더를 드래그해 패널만 이동(고양이는 제자리),
  오른쪽 아래 그립을 드래그해 크기 조절 — 둘 다 기억됩니다. **Ctrl+0~9**로
  뱃지가 붙은 상위 10개 클립을 즉시 복사하고, "복사 후 패널 자동 닫기"를
  꺼서 여러 클립을 연달아 가져올 수도 있습니다.
- **창 모드** — **항상 위**(기본), **일반**(다른 창 뒤에 놓일 수 있음),
  **숨김**(트레이 아이콘이나 클립보드 핫키로 복귀).
- **자동 업데이트** — 하루에 한 번 GitHub에서 새 릴리스를 확인하고 있으면
  토스트로 알립니다. Windows는 "vX.Y.Z로 업데이트 후 재시작", macOS·Linux는
  `U` 키로 다운로드 페이지를 엽니다. 기본 켜짐이며 끌 수 있고("자동 업데이트
  확인" / `auto_update`), 요청 외에는 아무것도 전송하지 않습니다.
- **영어 + 한국어** — 런타임에 전환 가능한 완전한 양국어 UI. 통계 말풍선을
  포함한 모든 텍스트가 **시스템 폰트**(Windows: 맑은 고딕/Segoe UI — OS에서
  읽어 오며 번들하지 않음)로 렌더링됩니다.
- **봉고캣 코어 루프** — 키 1회 = 2 XP, 클릭/스크롤 = 1 XP, 복사 = 5 XP.
  레벨업하면 액세서리가 잠금해제됩니다 (목도리·안경·비니·헤드폰·왕관·마법사 모자).
- **오늘의 통계** — 마우스를 올리면 키 입력 / 클릭 / 복사 / 활동 시간 표시.
- **살아있는 애니메이션** — 호흡, 깜박임, 꼬리, 빠른 타이핑 시 땀방울, 75초
  방치 시 잠들기, 더블클릭 쓰다듬기.
- **가벼움** — 포커스를 훔치지 않는 항상 위 창, 투명 부분 클릭 통과(Windows),
  단일 exe, 설치 불필요, 메모리 ~12-16MB.

## 조작법

| 동작 | 효과 |
|------|------|
| 어디서든 복사 (Ctrl+C) | 고양이가 생선을 먹고 클립 저장 (서식 유지) |
| `Win+Shift+V` (macOS `Cmd+Shift+V`) | 텍스트 커서 위치에 패널 열기 |
| 휠클릭 | 고양이 옆에 패널 열기 |
| 클립 선택 (클릭 / Enter) | 재붙여넣기 (서식 유지, 자동 붙여넣기 켜면 바로 붙여넣기) |
| `Ctrl/Cmd+Enter` (패널 열림) | 선택한 클립을 일반 텍스트로 붙여넣기 |
| `Ctrl/Cmd+0`~`9` (패널 열림) | 해당 숫자 뱃지 행을 바로 복사 |
| 행의 ★ / `Ctrl/Cmd+P` | 고정 / 고정 해제 |
| 행의 **...** 메뉴 (또는 `→`/`←`) | **텍스트로 붙여넣기** + **삭제** 펼치기/접기 |
| `Del` / `Ctrl+Z` (패널 열림) | 선택 클립 삭제 / 삭제 취소 |
| 깔때기 버튼 / `Tab` (패널 열림) | 출처 앱 필터 순환 |
| 패널 열고 타이핑 | 검색 (방향키·Home/End·Enter, Esc는 검색어 → 필터 → 닫기 순) |
| 패널 헤더 / 모서리 그립 드래그 | 패널만 이동 / 크기 조절 (고양이는 제자리) |
| 드래그 | 위치 이동 (잠금 시 제외) |
| 클릭 | 콩— (+1 XP) |
| 더블클릭 | 쓰다듬기 (+10 XP, 하트) |
| 마우스 올리기 | 오늘의 통계 말풍선 |
| 우클릭 (Windows 트레이 / macOS) | 설정 메뉴 (아래 참조) |
| 트레이 아이콘 클릭 (Windows) | 고양이 숨기기/보이기 |

설정 메뉴(Windows 트레이, macOS 우클릭)에는 다음이 들어 있습니다: 클립보드
패널, 수집 일시정지, 복사 후 자동 닫기, 선택 시 붙여넣기, 패널 핫키(안전한
프리셋 순환), 통계 고정, 크기 / 액세서리 / 소리 / 창 모드 하위 메뉴, 위치
잠금, 언어, 자동 실행, 자동 업데이트, 통계 초기화, 정보, GitHub, 종료.

### portable 빌드 (macOS / Linux) 단축키

전역(어디서나): macOS `Cmd+Shift+V` / Linux `Super+Shift+V`로 패널 열기.
창에 포커스를 준 뒤 (패널이 닫힌 상태에서):
`C` 클립보드 패널 · `V` 리스트/카드 보기 · `O` 복사 후 자동 닫기 ·
`K` 패널 핫키 프리셋 · `S` 크기 · `A` 액세서리 · `M` 소리 · `B` 통계 고정 ·
`L` 위치 잠금 · `G` 언어 · `U` 업데이트 페이지(새 버전이 있을 때) ·
`Q`/`Esc` 종료. (macOS는 우클릭 메뉴도 제공합니다.)

## 플랫폼 차이

| | Windows (네이티브) | macOS / Linux (portable) |
|---|---|---|
| 창 | 완전 투명, 클릭 통과 | 불투명 "카드" (softbuffer는 픽셀별 알파 불가) |
| 클립보드 감지 | `WM_CLIPBOARDUPDATE` 리스너 | `arboard` 폴링 (~0.4초) |
| 생선 뱃지 | 실제 앱 아이콘 | 색상 이니셜 |
| 재붙여넣기 서식 | 서식 유지 | macOS 유지 · Linux 일반 텍스트 |
| 패널 핫키 | 전역 `Win+Shift+V`, 커서 위치에 열림 (변경 가능, `Ctrl+Shift+V` 대체) | 전역 `Cmd/Super+Shift+V`; macOS는 커서 플라이아웃, Linux는 내장 패널 |
| 설정 UI | 트레이 메뉴 | 우클릭 NSMenu (macOS) · 키보드 단축키 (Linux) |
| 소리 | winmm 합성 | 무음 |
| 전역 입력 | `WH_*_LL` 훅 | `rdev` (Linux, X11 전용) · CoreGraphics 이벤트 탭 (macOS) |

설계 기록은 [`.context/kb/adr/`](.context/kb/adr/)에, 스펙은
[`docs/specs/clipcat-spec.md`](docs/specs/clipcat-spec.md)에 있습니다.

## 데이터 & 프라이버시

모든 데이터는 이 PC에만 저장됩니다. ClipCat은 **단 하나의 예외만 제외하면
네트워크 연결을 하지 않습니다**: 하루 한 번 github.com 릴리스로 보내는 업데이트
확인(ADR-0009)으로, 요청 외에는 아무것도 전송하지 않으며 끌 수 있습니다("자동
업데이트 확인" / `state.json`의 `auto_update`). 그 외 네트워크 코드는 없습니다.

- 입력 훅은 횟수만 셉니다. 어떤 키인지는 절대 읽지/저장하지/전송하지
  않습니다 (`src/input.rs`의 원자 카운터).
- 클립 히스토리는 로컬 `clips.json`에 저장되며 언제든 수집을 일시정지할 수
  있습니다 (패널 ⏸ / 트레이 메뉴). 256KB 초과 텍스트는 무시됩니다.
  저장 위치: Windows `%APPDATA%\ClipCat`, macOS
  `~/Library/Application Support/ClipCat`, Linux `$XDG_CONFIG_HOME/ClipCat`.
  기존 DeskCat(1.x) 설정은 자동 마이그레이션됩니다.

## 기술 노트

- GUI 프레임워크 없음 — [tiny-skia](https://github.com/linebender/tiny-skia)
  벡터 렌더링 + 백엔드별 직접 OS API.
- 하나의 플랫폼 독립 코어 (`pet`, `clipboard`, `panel`, `render`, `state`,
  `i18n`, `menu`, `update`) + 두 백엔드: 네이티브 Win32(레이어드 창, 트레이,
  클립보드 리스너, 커서 플라이아웃)와 portable(`winit` + `softbuffer` +
  `rdev`/CoreGraphics + `arboard`).
- 아이콘과 소리는 코드로 생성 — 번들 에셋 없는 단일 바이너리. 모든 텍스트는
  OS의 폰트를 런타임에 읽어 렌더링합니다 (`ab_glyph`, ADR-0007/0011).
- 사용자 대상 변경은 [CHANGELOG.md](CHANGELOG.md)에 기록되며, 릴리스는
  `scripts/release.sh`로 생성합니다.

## 빌드

```bash
cargo build --release                       # 실행 파일: target/release/clipcat[.exe]
cargo build --release --features portable   # Windows에서 portable 백엔드 테스트
cargo run --bin gen_icon                    # 아트 변경 후 아이콘 재생성
cargo run --release --example preview       # 렌더링 프리뷰 PNG 생성
```

요구 사항: Rust (Windows는 MSVC). Linux는 portable 스택용 시스템 라이브러리가
필요합니다 — Debian/Ubuntu: `apt-get install libx11-dev libxi-dev libxtst-dev
libxkbcommon-dev libxkbcommon-x11-dev pkg-config`.
