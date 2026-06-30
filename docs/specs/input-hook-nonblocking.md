# ClipCat — 전역 입력 훅 논블로킹 스펙 (키/마우스 입력 지연 방지)

Status: implemented (unreleased) · Owner: ClipCat · 작성일: 2026-06-22 ·
관련: PR #54 · commit `771dde2` ·
[LNR-0008](../../.context/kb/lnr/0008-windows-ll-hooks-dedicated-thread.md)

> 라인 번호는 작성 시점 기준. `windows.rs`는 본 변경으로 이동했고, `mac_input.rs`·
> `portable.rs`는 미변경(검토 시점 기준).

## 1. 요약

전역 입력 훅(키보드/마우스 **활동 카운팅**)이 시스템 전역 입력을 지연시킬 수 있는
경로를 세 백엔드 모두 검토하고, 네이티브 Windows 백엔드의 리스크를 제거했다. 훅
콜백 자체는 본래 `input.rs`의 atomic 카운터만 만져 가벼웠으나, 네이티브 Windows는
`WH_*_LL` 훅이 클립보드·렌더·paste를 처리하는 **UI 스레드에 얹혀** 있어, 그 스레드가
바쁠 때 훅 서빙이 밀리면 `LowLevelHooksTimeout`(~300ms) 한도까지 전역 키·마우스
입력이 끊길 수 있었다. 해법은 훅을 **전용 스레드**로 옮겨, portable 백엔드가 이미
가진 격리를 네이티브에도 적용한 것이다.

## 2. 배경 / 제약

- **프라이버시 골든룰 1**: 전역 입력 훅은 `input.rs`의 atomic 카운터만 증가 가능
  (ChordTracker·KeyGate가 유일 예외). 이 카운터가 atomic인 이유가 곧 **"어느
  스레드에서든 값만 올리면 된다"는 격리 의도**다 → 훅을 UI 스레드에 둘 필요가 없다.
- 코어는 OS 무관, 백엔드별로 정확히 하나가 컴파일됨(`platform/mod.rs`). 본 변경은
  `all(windows, not(feature="portable"))` 게이트의 네이티브 백엔드에만 해당.

## 3. 검토 결과 — 3개 백엔드

| 백엔드 | 입력 경로 | 블로킹 리스크 | 판정 | 근거 (file:line) |
|---|---|---|---|---|
| macOS | CGEventTap (listen-only) | 수동 탭이라 입력 지연 불가 + 타임아웃 시 자가 재활성화 | ✅ 안전 | `mac_input.rs:106-128,181-201` |
| portable (rdev) | `rdev::listen`, 전용 스레드 | 콜백=atomic+ChordTracker+KeyGate, 클립보드는 별도 스레드 | ✅ 안전 | `portable.rs:1329-1353,1385-1397` |
| 네이티브 Windows | `WH_KEYBOARD_LL`/`WH_MOUSE_LL` | 훅이 클립보드·렌더·paste와 **스레드 공유** | ⚠️→수정 | (§3.3) |

### 3.1 macOS — 안전
- 탭이 `kCGEventTapOptionListenOnly`(`mac_input.rs:184`). 설계상 입력 차단·지연 불가
  — 콜백이 느려도 포그라운드 앱 입력을 못 막는다.
- `raw_callback`이 `TapDisabledByTimeout`/`ByUserInput` 수신 시 탭 재활성화
  (`mac_input.rs:114-120`). 콜백 내부는 atomic + ChordTracker + KeyGate, 락 없음.

### 3.2 portable — 안전
- `rdev::listen`이 **전용 스레드**(`portable.rs:1391-1394`). 콜백 `pump`은 atomic
  카운터 + ChordTracker(락 없음) + KeyGate(`Vec`, 64키 상한). 패널 토글은 채널이
  아니라 atomic bool store(`portable.rs:1351`).
- 클립보드 폴링은 **별도 스레드**(`portable.rs:1407`)라 입력 콜백을 막지 않음.
  (`suppress` Mutex 경합은 보유 시간 마이크로초로 무시 수준.)
- Windows-portable에서 rdev가 LL 훅을 쓰지만 **자기 전용 스레드**에 설치 → 렌더/
  클립보드와 격리됨(네이티브보다 오히려 잘 격리).

### 3.3 네이티브 Windows — 리스크 (수정 대상)
- `SetWindowsHookExW`(변경 전 `windows.rs:2300-2301`)가 `GetMessageW` 루프를 펌프하는
  **메인 스레드에 설치** → 같은 스레드가 동시에:
  - **WM_CLIPBOARDUPDATE**: `read_clipboard_rich`의 `Sleep(15)`×5 ≈ 최대 60ms 재시도
    + `clipboard_source`의 아이콘/프로세스명 추출(`OpenProcess`/
    `QueryFullProcessImageNameW`/`ExtractIconExW`).
  - **WM_TIMER**: 매 ~33ms `pet.render` tiny-skia 래스터화.
  - **paste 경로**: `AttachThreadInput` + `SetForegroundWindow` + `SendInput`.
- LL 훅은 *설치한 스레드 컨텍스트*에서 동기 호출되고 메시지 펌프가 필요. 그 스레드가
  위 핸들러에서 길게 머물면 Windows가 이벤트당 `LowLevelHooksTimeout`(기본 ~300ms)까지
  대기 후 훅을 **스킵**(구버전은 **해제**) → 전역 입력 끊김, 카운트 누락, 최악 시 훅
  사망으로 펫 반응 정지.

## 4. 설계 결정 — 전용 입력 스레드 `InputHooks`

핵심: 두 LL 훅을 **메시지 펌프 + atomic 증가만** 하는 전용 스레드로 이전.
사용자가 제안한 "키/마우스 입력 비동기 처리"의 올바른 구현이다.

> 뉘앙스: LL 훅 콜백 **자체**는 비동기화 불가(OS가 동기 호출, 타임아웃 안에 반환
> 대기). "비동기"는 **훅 스레드를 렌더/클립보드 스레드와 분리**해 달성한다. 콜백
> (`kbd_hook`/`mouse_hook`, `windows.rs:1152-1180`)·`input.rs`는 무변경.

- 패널 핫키는 `RegisterHotKey`→`WM_HOTKEY`로 메인 스레드 유지(`windows.rs:1086`) —
  **LL 훅은 카운팅 전용**이라 chord 상태를 안 들고, 분리에 **추가 스레드 간 통신
  (채널·락) 0**. (`input.rs`의 lock-free atomic 카운터가 유일한 출력.)

### 4.1 `InputHooks` 수명주기 (`windows.rs:1191-1248`)
1. `start()`(`:1199`): 스레드 `spawn` → 그 스레드에서 두 훅 설치 → `PeekMessage`로
   메시지 큐 강제 생성(이후 `WM_QUIT` 유실 방지) → `GetCurrentThreadId`를 채널로
   메인에 반환 → `GetMessage` 루프. 메인은 설치 완료까지 짧게 대기(`rx.recv`).
2. `stop()`(`:1233`, WM_DESTROY `windows.rs:2164`에서 호출):
   `PostThreadMessageW(tid, WM_QUIT)`로 종료 신호 → `join`. 훅 해제
   (`UnhookWindowsHookEx`)는 **설치 스레드 자신**이 수행(설치/해제 동일 스레드).
3. 생성자(`windows.rs:2364`)는 `input_hooks: InputHooks::start()` 한 줄. `App`의
   기존 `kbd_hook`/`mouse_hook: HHOOK` 필드는 `input_hooks: InputHooks`로 교체.

### 4.2 불변식
- 훅 콜백은 `input.rs`의 lock-free atomic(`KEYS/CLICKS/WHEEL/KEY_HELD`)만 증가 →
  어느 스레드에서든 안전, 프라이버시 골든룰 1 유지.
- UI 스레드는 매 tick 그 카운터를 드레인(기존과 동일).
- 훅 활성 타이밍은 기존(생성자 시점)과 동일 — 동작 회귀 없음.
- 비정상 종료로 `stop()`이 안 불려도 프로세스 종료 시 OS가 훅 자동 정리(기존과 동일).

## 5. 동작 (변경 후)

복사 폭주, 타앱의 클립보드 점유(`Sleep` 재시도), 무거운 렌더가 있어도 훅 스레드는
그와 무관하게 즉시 카운팅 → **전역 키·마우스 입력이 끊기지 않는다**. 사용자 체감:
클립보드 활동 중 타이핑/마우스가 매끄럽다 (CHANGELOG `[Unreleased] → Fixed` 항목).

## 6. 검증

- `cargo check`/`cargo clippy --target x86_64-pc-windows-msvc`: 클린(헤드리스 Linux
  에서 Windows 코드 타입 체크).
- `cargo test --release`: 통과(입력 게이트 테스트 `held_key_counts_once_through_the_input_gate`
  포함, 코어 무영향).
- `cargo run --release --example preview`: 렌더 회귀 없음. `scripts/release.sh verify`: OK.
- **CI(PR #54)**: `windows-latest`·`macos-latest`·`changelog lint`·`detect changes`
  전부 ✅ — 실 MSVC 러너에서 네이티브 백엔드 빌드 통과(크로스 체크 이상의 검증).
- **미실측(정직)**: Windows 런타임 실측 — 복사 폭주 중 입력 매끄러움, 핫키(Win+Shift+V)/
  미들클릭 패널, 종료 시 행 없는 `join` — 은 헤드리스 환경상 불가 → Windows 개발 머신
  확인 필요(CI 빌드 + 코드리뷰로 대체).

## 7. 변경 파일

- `src/platform/windows.rs` — `InputHooks` 전용 스레드 신설, `App` 필드 교체,
  생성자·`WM_DESTROY` 갱신, 모듈 doc 보정.
- `CHANGELOG.md` — `[Unreleased] → Fixed` 1줄.
- `.context/kb/lnr/0008-windows-ll-hooks-dedicated-thread.md` — 회귀 방지 LNR + 인덱스.

## 8. 비범위 / 후속 (선택)

- 보조 격리: WM_CLIPBOARDUPDATE 핸들러의 아이콘/프로세스명 추출(`clipboard_source`)을
  워커 스레드로 미뤄 메인 스레드 정체 자체를 줄이는 안. §4가 훅 격리를 이미 완결하므로
  미적용 — 필요 시 별도 작업.
- macOS/portable: 변경 없음(이미 안전, §3.1–3.2).
