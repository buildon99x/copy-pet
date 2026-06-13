# ClipCat Tier 1 — 편의성 파리티 구현 스펙

Status: spec · Owner: ClipCat · 작성일: 2026-06-13 · 평가축: ① e2e 완결성 ② 간편 사용성 ③ UX 무훼손

이 문서는 [`improvement-plan.md`](improvement-plan.md)의 **Tier 1**(유틸리티 파리티)
다섯 항목을 빌드 가능한 수준으로 구체화한다. 모든 사실은 트리에 대해 검증했다
(라인 번호는 작성 시점 기준). macOS = 포터블 백엔드 + `mac_input` 특수성.

## 0. 범위와 결정

대상 5개: **F1 자동 붙여넣기 · F2 평문 표면화 · F3 첫실행 온보딩+하단 힌트 ·
F4 검색 랭킹·다중토큰 · F5 단축키 프리셋 순환.**

확정된 설계 결정:
1. **단축키 = 프리셋 순환.** 자유 리바인드(키 캡처·충돌감지·라이브 재등록 UI)는
   "복잡 금지 / 최대 2뎁스 / 최소 입력" 기준과 충돌. 대신 안전 프리셋을 1클릭/1키로
   순환 — 입력 1회·1뎁스, 충돌 시 다음 프리셋, 라벨·토스트로 즉시 확인.
2. **하단 힌트 = 학습되면 숨김.** 첫 패널 오픈 전까지만 고양이 아래 1줄 힌트를 띄우고,
   이후엔 숨겨 잔소리를 없앤다(hover·메뉴로 재확인 가능).
3. **자동 붙여넣기 = opt-in, 기본 off.** 일부 사용자는 복사만 원하고, 일부 앱엔
   직접 붙여넣기가 부적절. 실패해도 클립보드엔 남아 수동 붙여넣기로 graceful degrade.

> 사전 검토: 단축키는 현재도 `state.json`의 `hotkey`로 변경 가능(파싱·검증·Windows
> 자동 폴백)하나 **런타임 UI가 없고**, 진짜 병목은 "사용자가 단축키를 *모른다*"는
> 발견성이다 → F3가 해결, F5가 "원하면 바꿀 수 있다"를 1뎁스로 보완.

## 1. 검증된 확장점

| 영역 | 사실 | 위치 |
|---|---|---|
| copy-back 계약 | `panel_click/panel_nav → Option<String>` → `run_action` | pet.rs:382,460,476 |
| 복사 액션 | `PanelAction::Copy`가 toast/sound/auto-close 후 텍스트 반환 | pet.rs:384-399 |
| 소비처(6) | `copy_back` ×3 / `set_clipboard` ×2 | windows.rs:1198,1306,1364 · portable.rs:401,531 |
| 포커스 | 패널 오픈 시 `WS_EX_NOACTIVATE` 해제 + `SetForegroundWindow(self.hwnd)` | windows.rs:288-297 |
| 포그라운드 | `GetForegroundWindow()` 이미 사용(소스앱 식별) | windows.rs:481 |
| 입력 합성 | `rdev` **이미 의존성** → `rdev::simulate`로 Ctrl+V, 신규 의존성 0 | Cargo.toml:29,74 |
| 검색 | 유일 변경점 `filtered_indices`, alloc-free `contains_ci` | clipboard.rs:314-336,103-122 |
| 뷰캐시 | `ViewCache` 키 = (version,query,source) — 스코어 변경 무관, 불변 | panel.rs:320-334 |
| 캔버스 | `take_window_shift/take_size_changed/canvas_size`가 성장 시 앵커 유지 | pet.rs:268-272 |
| 힌트 | `panel_hint`/`set_panel_hint`(라이브 핫키 라벨) 재사용 | pet.rs:91,174 |
| 오픈 훅 | `toggle_panel` | pet.rs:327 |
| 영속화 | `Persist` 필드는 `#[serde(default)]` → 새 필드 자동 마이그레이션 | state.rs |
| 포터블 단축키 | `ChordTracker`(hk:Hotkey + 캐시 main), 입력스레드서 1회 생성 | portable.rs:694-715,808,820 |
| 포터블 펌프 | `pump`가 `chord.on_event` 호출, 메인루프는 `Arc<AtomicBool>`로 토글 감지 | portable.rs:782-797,621 |
| 메뉴 | `build_menu`/`apply_menu_action`, `MenuOutcome`(ReregisterHotkey 없음) | pet.rs:817,933 · menu.rs:85-97 |

## 2. 교차 관심사 — copy-back 계약 확장 (F1이 소유, 원자적)

`Option<String>` → `Option<ClipPick>`로 전환. 6개 소비처를 한 번에 변경.

```rust
// pet.rs (run_action 근처, 신규)
pub struct ClipPick {
    pub text: String,
    pub paste: bool, // true => 백엔드가 이전 앱에 붙여넣기
}
```

- `run_action`(pet.rs:382), `panel_click`(:460), `panel_nav`(:476) 반환 타입 변경.
- `PanelAction::Copy`(pet.rs:384-399)는 `Some(ClipPick { text, paste: self.st.paste_on_select })`
  반환 — toast/sound/auto-close 로직 불변.
- 소비처는 `if let Some(text)` → `if let Some(pick)`로 바뀌어 구조체를 그대로 전달:
  - Windows: `a.copy_back(pick)` (windows.rs:1198,1306,1364)
  - 포터블: `self.set_clipboard(pick)` (portable.rs:401,531)
- 백엔드: `copy_back(pick)` / `set_clipboard(pick)` — 항상 클립보드 쓰기,
  `pick.paste`면 붙여넣기 시퀀스 실행. (포터블 `set_clipboard`는 `&self`로 충분 —
  붙여넣기 대상은 읽기 전용.)

## 3. 기능별 설계

### F1. 자동 붙여넣기 (최고 가치 · 최고 위험)

**파일/타입**
- `state.rs`: `paste_on_select: bool`(`#[serde(default)]`, 기본 false).
- `pet.rs`: `ClipPick`; `toggle_paste_on_select()`(+토스트, `toggle_panel_autoclose`
  pet.rs:448 미러); `paste_on_select()` getter(메뉴 체크 상태용).
- `menu.rs`/`pet.rs`: `MenuAction::TogglePasteOnSelect` → `build_menu`(pet.rs:817)에
  `checked = st.paste_on_select` 리프, `apply_menu_action`(pet.rs:933) → `Handled`.

**Windows**
- `App`에 `paste_target: HWND`. 패널 오픈 포커스-탈취 분기(windows.rs:292)에서
  `SetForegroundWindow(self.hwnd)` **직전** `GetForegroundWindow()` 저장. 이 분기는
  hotkey·middle-click·tray 오픈을 모두 거치므로 한 곳에서 통일 캡처.
- `copy_back`에서 `pick.paste`면: `set_clipboard_text` → `SetForegroundWindow(paste_target)`
  → `SendInput`(VK_CONTROL↓, 'V'↓, 'V'↑, VK_CONTROL↑). auto-close가 `run_action` 안에서
  먼저 동기 실행되어 패널이 포그라운드를 이미 양보 → paste는 그 뒤 실행(순서 보장).

**포터블** (`rdev::simulate`, 신규 의존성 0)
- `set_clipboard`에서 `pick.paste`면 `cb.set_text` 후 `rdev::simulate`로
  ControlLeft↓, KeyV↓, KeyV↑, ControlLeft↑ (macOS는 MetaLeft = Cmd+V). 합성 간 최소 간격.
- 포커스 복원: Windows-portable·X11은 패널 자동닫힘 시 OS가 이전 앱에 포커스 반환 →
  `simulate`가 그 앱에 입력. **macOS는 이전 앱 복원이 백엔드에서 불안정 → best-effort**
  (패널 숨김 후 frontmost에 붙여넣음). 동일 플래그 뒤 한계로 문서화.

**규칙 준수**: 출력 이벤트(Ctrl+V)만 합성, 키 내용 비취급(골든룰1). `rdev::simulate`는
출력 전용이라 macOS TIS listen 크래시(LNR-0005) 무관. 신규 의존성 0(rdev 기존 + Win32
SendInput은 OS API). 양 백엔드 동일 플래그·메뉴·동작 패리티, macOS는 best-effort 명시.

**e2e 흐름**: 패널 오픈(이전 앱 캡처) → 클립 선택 → `run_action`이 복사+자동닫힘 →
백엔드 클립보드 쓰기 → 이전 앱 포커스 복원 → 합성 Ctrl+V → 앱에 텍스트 안착.
일반 복사 대비 추가 동작 0(켜져 있으면 자동).

**테스트**: `run_action(Copy)`가 `paste_on_select`에 따라 `ClipPick.paste` 반환;
토글+토스트; `build_menu` 체크 상태; `apply_menu_action(TogglePasteOnSelect)` 반전→
`Handled`. e2e.rs에 패널 선택→`ClipPick.paste` 단언. SendInput/simulate는 플랫폼
부작용이라 코어 결정 로직으로 커버.

### F2. 평문 붙여넣기 표면화 (거의 무료)

- ClipCat는 `CF_UNICODETEXT`만 저장(clipboard.rs:171) → **copy-back은 이미 평문.**
  데이터 경로 변경 없음.
- `i18n.rs`: `Msg::PlainTextNote`(온보딩/about/토스트 프레이밍), En/Ko 동기화.
- F3와 묶어 진행. 규칙: 문자열 전용, 전부 `i18n::t` 경유.
- 테스트: i18n 완전성 테스트에 새 변형 추가(양 언어 해석 단언).

### F3. 첫 실행 온보딩 + 하단 단축키 힌트 (학습되면 숨김)

**파일/타입**
- `state.rs`: `onboarded: bool`(`#[serde(default)]`, 기본 false).
- `pet.rs`: `!onboarded`일 때 하단 힌트 표시. `toggle_panel`(pet.rs:327) **첫 오픈 시**
  `st.onboarded = true; dirty = true`("학습"); 이후 숨김, hover 시 재표시(`Option`화).
  힌트 텍스트는 기존 `panel_hint`(라이브 핫키 라벨, 백엔드가 set) 재사용 — 신규 라벨
  배선 없음.
- `render.rs`: `Scene.hotkey_hint: Option<&str>`. 고양이 하단에 1줄 스트립(캔버스 높이
  ~10px 증가). 성장은 기존 `take_size_changed`/`take_window_shift` 경로 그대로 사용
  (고양이 앵커 유지) — 신규 지오메트리 금지. 스타일은 토스트(render.rs:440-450) 참고.

**규칙**: 입력 내용 무관(핫키 라벨만). 신규 의존성 0(tiny-skia 드로잉). 양 백엔드 동일
코어 경로(둘 다 `take_size_changed` 존중, 패널 오픈 사이즈 변화로 이미 검증됨). 온보딩
문구는 `i18n::t`, chord는 `Hotkey::display()`.

**e2e 흐름**: 신규 설치(`onboarded=false`) → "패널 열기: {hotkey}" 스트립 → hotkey 1회
→ 패널 오픈 → `onboarded=true` 영속 → 이후 프레임서 자동 숨김. 이후 hover/메뉴로 재표시.

**테스트**: `onboarded=false` 렌더 Scene에 `hotkey_hint=Some`; 첫 `toggle_panel` 후
`onboarded=true` & `hotkey_hint=None`(hover 제외); 힌트 표시 시 캔버스 높이↑ &
`take_size_changed`/`take_window_shift` 플래그 단언(고양이 앵커); hover 재표시; e2e로
첫 오픈 시 `onboarded` 영속 반전.

### F4. 검색 랭킹 + 다중 토큰

**유일 변경점** `filtered_indices`(clipboard.rs:314-336):
- `q`를 공백으로 토큰화(borrowed `&str`, 토큰 경계 작은 `Vec`만).
- **모든 토큰**이 `c.text` 또는 `c.source`에 `contains_ci` → 매치(토큰 간 AND,
  토큰별 text|source).
- 스코어(alloc-free 유지, fold/`contains_ci`): 접두/단어경계 매치 > 이른 위치 >
  짧은 텍스트, 토큰 합산. `to_lowercase()` 같은 per-clip String 복사 금지
  (clipboard.rs:100-102 불변식).
- 정렬: 핀 먼저(기존 `row(true)`/`row(false)` 구조 유지) → 그룹 내 스코어 내림차순,
  동점은 최신순(안정 정렬이 삽입 순서=최신 보존). `ViewCache` 키 변경 불필요.

**규칙**: 신규 의존성 0, per-clip alloc-free, 플랫폼 무관(백엔드 변경 0).

**테스트**(clipboard.rs:422 / e2e.rs:175 확장): 다중토큰 AND("foo bar"가 "foo bar"
포함 클립 매치, "foo"만 있는 클립 거부); 토큰 text+source 교차 매치; 랭킹(접두/단어시작
> 중간, 구절 > 분산); 한국어/유니코드 다중토큰; 핀 우선 불변; per-clip alloc 무회귀
(리뷰 게이트).

### F5. 단축키 프리셋 순환

**파일/타입**
- `hotkey.rs`: `PRESETS: &[&str] = &["win+shift+v","ctrl+shift+v","alt+shift+v",
  "ctrl+shift+c"]`(DEFAULT 우선, 전부 `from_spec` 파싱 가능); `next_preset(current)
  -> &'static str`(현재 인덱스 다음, 랩; 비프리셋 → `PRESETS[0]`).
- `pet.rs`: `cycle_hotkey() -> &str` — `st.hotkey` 갱신, `dirty=true`,
  `set_panel_hint(Hotkey::from_spec(new).display())`, 토스트, 새 spec 반환(백엔드 재등록용).
- `menu.rs`/`pet.rs`: `MenuAction::CycleHotkey`(라벨 = 현재 chord, `build_menu`의 hotkey
  파라미터 사용); `MenuOutcome::ReregisterHotkey(String)` 추가(menu.rs:85) →
  `apply_menu_action`이 반환, 백엔드가 OS 재등록.

**Windows(native)**: `UnregisterHotKey(hwnd, HOTKEY_ID)` → `register_panel_hotkey`
(windows.rs:656, Ctrl+Shift+V 자동 폴백 재사용 → 충돌 프리셋도 graceful), `hotkey_label`
갱신(트레이 메뉴 재빌드가 읽음 windows.rs:1014,1035).

**포터블(핵심 난점 — 입력스레드 런타임 재등록)**
- `ChordTracker`(portable.rs:694)는 입력 스레드에서 1회 생성(`spawn_global_input`
  808/820). 메인루프는 `Arc<AtomicBool>`로 토글만 감지.
- `ChordTracker.hk: Hotkey` → `Arc<Mutex<Hotkey>>`로 교체. `on_event`(portable.rs:719)에서
  main 키 후보가 들어오는 분기에서만 잠금→현재 hotkey의 기대 main(`rdev_key_of(hk.key)`)
  +modifier 비교→해제. **캐시 main에 의존하지 말 것**(스펙 변경 시 미갱신). 잠금 범위는
  해당 분기로 최소화(전역 입력 스레드 정지 방지), poison 시 `lock().ok()`로 패닉 금지.
- 구성: `let shared_hk = Arc::new(Mutex::new(Hotkey::from_spec(&st.hotkey)))`,
  `spawn_global_input(hk: Hotkey)` → `hk: Arc<Mutex<Hotkey>>`(808/820 양쪽), `App`도
  클론 보유. 순환 시 메인스레드가 `*shared_hk.lock() = Hotkey::from_spec(new)` →
  리스너가 다음 이벤트에 반영(스레드 재시작/채널 불필요). macOS도 동일
  (`mac_input::listen`이 동일 `pump`/`ChordTracker` 사용, 재탭 불필요).
- 포터블 단일키 진입점 1개 추가(예: `K`)로 `cycle_hotkey()`+뮤텍스 스왑 → Windows
  트레이와 패리티.

**규칙**: 신규 의존성 0(프리셋은 문자열, `Hotkey::parse/from_spec/display` 재사용).
공유 상태는 설정 chord뿐(관측 키 아님) → ADR-0008 불변. 메뉴 라벨/토스트는 `i18n::t`,
chord는 `display()`.

**e2e 흐름**: 메뉴 "Hotkey: Win+Shift+V" 클릭(또는 포터블 K) → 코어가 다음 프리셋으로 →
토스트 "Ctrl+Shift+V" → Windows 재등록 / 포터블 공유 Hotkey 스왑 → 새 chord 누르면 패널
오픈; 하단 힌트·about·트레이 라벨 모두 갱신.

**테스트**: `next_preset` 순환/랩/비프리셋→[0], 각 프리셋 `from_spec().display()` 파싱·
비공백; `cycle_hotkey`가 `st.hotkey`/dirty/hint/토스트/반환 갱신; `build_menu` 현재 chord
라벨; `apply_menu_action(CycleHotkey)` → `ReregisterHotkey(new)`; **ChordTracker 런타임
스왑 테스트**(`Arc<Mutex<Hotkey>>`로 구성, chord A 합성 시퀀스 발화 → 뮤텍스 B 스왑 →
B 발화·A 미발화, 창 없이). 백엔드 재등록은 런타임/CI로.

## 4. 구현 시퀀스 (의존성 고려)

1. **F4 검색** — 완전 자기완결(함수 1개, 백엔드/계약 변경 0), 즉시 테스트. 워밍업.
2. **F3+F2 온보딩/힌트/평문** — 코어/렌더/i18n만, `Persist` 마이그레이션 패턴 공유,
   `panel_hint` 재사용, 계약 변경 없음.
3. **F5 단축키 프리셋** — auto-paste와 독립. 포터블 `Arc<Mutex<Hotkey>>` 메커니즘 +
   `MenuAction/MenuOutcome` 배선 도입(F1 메뉴 항목이 이 위에 탑승).
4. **F1 자동 붙여넣기** — 최후/최고 위험. `ClipPick` 계약(6곳) 원자 변경 +
   `paste_on_select` + Windows 포커스/타이밍을 마지막에 고립.

## 5. 리스크

1. **Windows 포커스/타이밍 (高)** — `SetForegroundWindow`는 포그라운드 락 규칙에 막힐 수
   있음. 완화: 탈취 *전* 타깃 저장(이미), auto-close 후 paste(포그라운드 자유), 최후
   수단 `AllowSetForegroundWindow`/ALT 넛지. 순서: 클립보드 쓰기 → SetForegroundWindow
   → SendInput. 실패해도 클립보드엔 남아 수동 붙여넣기로 degrade. 사용자가 물리 modifier를
   잡고 있을 때 합성 충돌 방지(역순 해제, 시퀀스 짧게).
2. **포터블 런타임 재등록 (中)** — `ChordTracker.main` 캐시 vs 스왑: 비교 시점에 잠긴
   hotkey에서 main 재도출. 잠금 범위 최소, poison 비패닉. macOS는 `mac_input::listen`
   클로저가 `Arc<Mutex<Hotkey>>`를 move 캡처(현재 chord move와 동일) — OK.
3. **캔버스 지오메트리 vs 윈도우 시프트 (中)** — 힌트 표시/숨김 높이 토글은 반드시 기존
   `take_size_changed`/`take_window_shift`/`canvas_size` 경로로(고양이 점프/미리사이즈
   방지). hover 깜빡임 시 위치 thrash 방지(나타남/사라짐 시에만 시프트). Windows DIB
   재생성(windows.rs:280-285)은 w/h 변화로 이미 트리거.
4. **macOS auto-paste best-effort (低)** — 이전 앱 포커스 복원 불가 가능 → 플래그 뒤
   문서화, 기능 차단 금지.
5. **검색 alloc 회귀 (低)** — 다중토큰+스코어를 fold/`contains_ci`로 alloc-free 유지
   (200클립/키스트로크). 리뷰 게이트.

## 6. ADR / 스펙 갱신

- **ADR-0005 개정 필요** — "no paste-automation" 비목표를 뒤집는 superseding ADR:
  근거(최상위 요구·유일 치명적 파리티 공백), 범위(opt-in/기본 off), 프라이버시(출력 전용
  합성, 키 비취급), 백엔드 전략(Win32 SetForegroundWindow+SendInput / rdev::simulate /
  macOS best-effort), **신규 의존성 없음** 명시(무거운 의존성 규칙 선제 차단).
- **ADR-0008** — 규칙 불변, 단 포터블 단축키가 `Arc<Mutex<Hotkey>>`로 런타임 가변(설정
  chord만 보유, 관측 키 아님) + `rdev::simulate`는 출력 전용임을 주석 추가.
- **LNR-0005** — `rdev::simulate`는 출력 전용이라 macOS TIS listen 경로와 무관 주석.
- **신규 의존성 ADR 불필요**(rdev 기존, SendInput은 OS API) — auto-paste ADR에 명시.
- **clipcat-spec.md** — 프리셋 목록 + 온보딩 "학습"(첫 패널 오픈이 `onboarded` 설정) 의미
  반영.

## 7. 검증 (e2e)

- 빌드 게이트: `cargo build --release`, `cargo clippy --release`(+`--features portable`),
  `cargo test --release`, `cargo run --release --example preview`로 하단 힌트/검색/패널
  PNG(En/Ko·한글 샘플) 육안 확인.
- 코어 단위/e2e는 각 기능 절(§3)에 명시. 플랫폼 부작용(SendInput/simulate/재등록)은 코어
  결정 로직 + Windows/포터블 런타임 + CI 빌드/리뷰로 honest 검증(헤드리스 한계 명시;
  CI에 Linux 잡 없음 → Linux 영향 변경은 로컬 빌드/테스트).

## 8. 평가축 매핑

1. **e2e 완결성** — 각 기능을 트리거 → 코어 → 양 백엔드 → 렌더/OS → 테스트까지 완결
   흐름으로 기술. 플랫폼 부작용은 코어 로직 테스트 + 런타임/CI로 보강(헤드리스 한계 명시).
2. **간편 사용성**(입력 1~3 · 최대 2뎁스 · 기억하기 좋음) — F1 켜면 추가입력 0(자동) ·
   F5 1클릭/1키 1뎁스 · F3 첫 성공 후 자동 소멸 · F4 추가 타이핑 0. 모두 최대 1뎁스,
   라벨/토스트가 기억 보강.
3. **UX 무훼손** — auto-paste 기본 off + graceful degrade, 힌트 학습 후 숨김(잔소리 없음),
   검색 핀 우선 유지, 단축키 충돌 자동 폴백, 캔버스 thrash 방지.
