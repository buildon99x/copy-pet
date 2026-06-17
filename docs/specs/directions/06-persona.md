# 방향 6 — 부캐(페르소나) 시스템 "한 고양이, 여러 컨셉"

> ClipCat 고양이 캐릭터 업그레이드 7개 방향 중 #6.
> 본캐("생선 본위제 먹보 고양이", 부록 B)를 **불변**으로 두고 그 위에 텍스처 프리셋을 묶는 메타 레이어.

## What (정의)

부캐는 **새 정체성이 아니라, 본캐 위에 덧입히는 "텍스처 프리셋 묶음"**이다. ClipCat의 본캐 — *생선 본위제(복사한 텍스트를 생선으로 환산해 받아먹는) 게으른 먹보 고양이* — 의 톤·형태·세계관은 **불변(FIXED)**이며, 부캐는 그 고양이가 오늘 취하는 "서로 다른 생선 태도(독설/병맛/다정/츤데레/졸림)"를 한 번에 큐레이션해 묶은 데이터 레이어일 뿐이다. 한 부캐 = `{표정 풀 + 아이들 행동 가중치 + 시그니처 한 줄 세트 + 기본 룩(악세사리/컬러)}`의 번들. 정체성을 갈아끼우는 게 아니라, **같은 고양이의 기분 프리셋을 고르는 것**이다. 개별 표정/라인/아이들/악세사리 자산 자체는 방향2/3/5/1이 만들고, 방향6은 그 빌딩블록을 *묶어서 한 번에 전환*하는 메타 레이어 + 선택·추천·해금 UX를 담당한다.

## Why (기대효과)

- **"한 캐릭터 여러 세트" = 카카오 #1 성장엔진.** 망그러진곰(직장인/대학생/꼬질 에디션), 라이언 멀티에디션처럼, *고정된 본캐 하나 + 무한히 늘어나는 컨셉 세트*가 캐릭터 IP의 검증된 확장·수집 동력이다. 부캐 시스템은 새 캐릭터를 그리지 않고도 "신규 콘텐츠"를 계속 출시할 수 있게 한다 — `Persona` 데이터 한 항목만 추가하면 끝.
- **기분 선택 · 자기표현 · 수집 = 2030 여성 핵심 동인.** "오늘은 번아웃이라 직장인 부캐", "졸려서 몽실냥" 같은 *그날의 나를 투영하는 선택*은 SNS 프로필/카톡 테마 교체와 같은 자기표현 행위다. 해금된 부캐 컬렉션은 가챠·결제 없이도 수집 만족을 준다(하드 제약 준수).
- **무한 확장성 + 저비용.** 시즌 부캐(산타냥), 콜라보 부캐를 데이터로만 추가. 렌더 프리미티브 신규 개발 없음 → 유지보수·리스크 최소.
- **본캐 불변이 곧 일관성.** 사용자는 늘 "그 먹보 고양이"를 만난다. 부캐는 그 위의 분위기 변주라, IP 정체성이 흔들리지 않는다.

## 구현 아이디어 (구체화)

### 1. `Persona` 데이터 레이어 + 정적 카탈로그
- **무엇을:** 부캐 한 종 = 가중치/라인셋/룩을 묶은 순수 데이터 struct. `ACCESSORIES`(`src/state.rs:320`)와 똑같은 *정적 배열 카탈로그* 패턴을 그대로 차용.
- **어떻게:** `src/state.rs`에 `pub struct PersonaDef { unlock_level: u32, name_kr/name_en: &'static str, idle_bias: IdleWeights, line_set: PersonaLines, default_accessory: usize, theme: ColorTheme, face_tint: FaceTint }` + `pub const PERSONAS: [PersonaDef; N]`. 0번은 본캐(무해/순둥, `unlock_level: 1`, `default_accessory: 0`). `AccessoryDef::name(lang)`(`state.rs:310`)와 동일한 `name(lang)` 헬퍼 재사용. 시드 4~5종: 본캐(순둥), 직장인(번아웃 공감·독설 텍스처), 시크·새침(츤데레), 엉뚱(병맛), 몽실(졸림).
- **난이도:** 낮음(데이터 정의 + 기존 패턴 복제).
- **본캐 불변:** 부캐는 *표정 풀의 선택 가중치/룩/라인만* 바꾼다. `draw_face`(`render.rs:484`)의 형태(∩눈/ω입/blush)와 `Pet`의 애니메이션 상태머신은 그대로. 정체성이 아니라 표면 텍스처만 교체.

### 2. `Persist`에 `active_persona` 추가 + 하위호환
- **무엇을:** 현재 선택 부캐를 영속화. 기존 `state.json`이 필드 없이 로드돼도 자동으로 본캐.
- **어떻게:** `Persist`(`src/state.rs:13`, `#[serde(default)]`)에 `pub active_persona: usize` + `Default`(`state.rs:69`)에서 `0`. `serde(default)` 덕분에 구버전 JSON은 자동으로 0(본캐). `old_state_json_still_deserializes` 테스트(`state.rs:373`)에 `assert_eq!(st.active_persona, 0)` 한 줄 추가로 보증. 해금 풀(여러 부캐 보유 상태)은 별도 비트마스크 불필요 — `unlock_level <= 현재 level`로 매번 계산(악세사리와 동일 방식, `pet.rs:1269`).
- **난이도:** 매우 낮음.
- **본캐 불변:** 기본값=0=본캐가 곧 "갈아끼움 아님"의 영속 보증.

### 3. 메뉴 부캐 피커 서브메뉴 (테스트 가능, 레벨 게이트)
- **무엇을:** 악세사리 서브메뉴와 똑같은 구조의 "부캐" 라디오 서브메뉴. 잠긴 부캐는 회색 처리 + "(LV n 달성 시)".
- **어떻게:** `src/menu.rs`에 `MenuAction::SetPersona(usize)` 추가. `build_menu`(`pet.rs:1201`)에서 악세사리 서브메뉴 빌드 로직(`pet.rs:1262~1283`)을 그대로 복제 — `unlocked = level >= persona.unlock_level`, 잠금 라벨은 `i18n::accessory_locked` 패턴을 본뜬 `i18n::persona_locked` 헬퍼. `apply_menu_action`(`pet.rs:1342`)에 `SetPersona(id)` 처리: 악세사리 가드(`pet.rs:1358`)와 동일하게 `PERSONAS.get(id).is_some_and(|p| self.level() >= p.unlock_level)`로 잠금 검증 후 `self.st.active_persona = id; self.apply_persona_default_look(); self.dirty = true`.
- **난이도:** 낮음.
- **본캐 불변:** 본캐(index 0)는 항상 `unlock_level: 1`이라 영구 선택 가능 — 사용자는 언제든 "원래 그 고양이"로 돌아올 수 있음.

### 4. 부캐 → 표정/룩/아이들/라인 *한 번에* 전환
- **무엇을:** 부캐 선택이 (a) face 텍스처 (b) 기본 악세사리 (c) idle 가중치 (d) 시그니처 라인을 동시에 바꿈.
- **어떻게:**
  - **표정 텍스처:** `draw_face`(`render.rs:484`)에 `Scene`(`render.rs:172`)으로 넘어온 `persona_tint`(눈 크기/입 곡률/blush 강도 같은 스칼라 셋)를 곱한다. 형태 상수는 유지, 미세 변조만 — 직장인=눈 반쯤 감김+blush↓, 몽실=눈 더 닫힘, 새침=입 곡률↓. `Scene` 구성은 `draw()`(`pet.rs:1105`)에서 `persona: PERSONAS[self.st.active_persona]` 한 줄로 주입.
  - **기본 룩:** 부캐 선택 시 `default_accessory`를 `st.accessory`에 *제안 적용*(이미 해금된 악세사리면). `Accessory::from_id`(`render.rs:47`) / `draw_accessory`(`render.rs:647`) 그대로 재사용.
  - **아이들 가중치:** `advance`(`pet.rs:855`)의 idle 분기(블링크 간격 `pet.rs:913`, zzz `pet.rs:936`, tail_phase `pet.rs:919`)에 부캐 `idle_bias`를 곱셈 적용 — 몽실=sleep_target 빨리/zzz 잦게, 엉뚱=tail 흔들림↑. `rng`(`pet.rs:111`)·`rand_f`(`pet.rs:52`)는 그대로.
  - **시그니처 라인:** 5번 항목.
- **난이도:** 중간(여러 기존 함수에 *읽기 전용 데이터 곱하기* — 신규 렌더 프리미티브 0개).
- **본캐 불변:** 모든 변조는 본캐 형태 상수에 *곱해지는 작은 계수*. 부캐 비활성(본캐) 시 계수=1.0 → 픽셀 동일.

### 5. 부캐별 시그니처 한 줄 (가나디式 짤 멘트)
- **무엇을:** 부캐마다 고정 톤의 짧은 한 줄 세트(focused→독설, idle→병맛, late-night→다정, interaction→츤데레의 "텍스처" 변주). 본캐 세계관(생선) 어휘는 공통.
- **어떻게:** `src/i18n.rs`에 부캐별 라인 풀을 추가(기존 `Msg` enum + `t()` `i18n.rs:111` 패턴, 또는 부캐×무드 인덱싱 헬퍼 `persona_line(lang, persona, mood) -> &str`). 표시 경로는 기존 `toast`(`pet.rs:99`, `set_toast` `pet.rs:1439`)와 `draw_bubble`을 그대로 재사용 — `nom()`(`pet.rs:1002`)·`pet()`(`pet.rs:1154`)·`level_up`(`pet.rs:1419`) 같은 이벤트 훅에서 본캐 고정 멘트 대신 `persona_line(...)` 호출. 무드는 *이미 있는 로컬 상태*(focused=`self.rate`/`excite`, idle=`self.sleep`, late-night=`today_string`/로컬 시각, interaction=pet/click)로만 분기.
- **난이도:** 낮음~중간(주로 카피라이팅 + EN/KO 쌍).
- **본캐 불변:** 어휘 베이스가 "생선/먹보"로 고정 → 라인이 바뀌어도 같은 고양이의 말투 변주로 읽힘.

### 6. 부캐별 컬러 테마 (파스텔/모노)
- **무엇을:** 부캐가 blush/배경/strap 등 *팔레트 톤*도 함께 전환(새침=모노, 몽실=파스텔블루).
- **어떻게:** `render.rs`의 팔레트 상수(`BLUSH` `render.rs:23`, `EAR_PINK` `render.rs:22` 등)를 부캐 `theme`로 *선택적 오버라이드*. `Scene`에 `theme: ColorTheme` 추가, `draw_face`의 `fade(BLUSH, ...)`(`render.rs:531`)를 `fade(theme.blush, ...)`로. 본캐 테마 = 현행 상수 그대로.
- **난이도:** 낮음(상수→데이터 치환).
- **본캐 불변:** 본캐 테마는 현재 색을 그대로 담아 시각적 무변화.

### 7. 로컬 활동 기반 "추천 부캐" 슬쩍 제안
- **무엇을:** 키 *내용 미사용*, 오직 로컬 활동 신호로 부캐를 부드럽게 추천(예: 늦은 시각 + 긴 세션 → 직장인 부캐 제안 토스트, 1회성, 강요 없음).
- **어떻게:** 입력은 `input.rs`의 **카운터만**(KEYS/CLICKS/WHEEL, `input.rs:13`) + `today_string()`(`state.rs:258`) 기반 로컬 시각 + `active_min_today`(`state.rs:66`)/세션 길이. `advance`에서 신호 충족 시 `set_toast`(`pet.rs:1439`)로 "지금은 직장인 부캐 어때요?" 류 1회 힌트만. 자동 전환 절대 금지 — 항상 사용자가 메뉴(3번)에서 직접 선택. 죄책감/스트릭/추적 문구 배제.
- **난이도:** 낮음~중간(임계값 튜닝).
- **본캐 불변 & 프라이버시:** 추천은 *제안*일 뿐 본캐를 바꾸지 않음. 신호는 집계 카운터/로컬 시각만 — 키 내용·창 제목·타이밍 절대 미사용(하드 제약 1, `input.rs` 주석/ADR-0008 그대로 준수).

### 8. (확장) 시즌 부캐 — 산타냥
- **무엇을:** 날짜 기반으로만 노출되는 한정 부캐(12월 산타냥).
- **어떻게:** `PersonaDef`에 `season: Option<(u8,u8)>`(월/일 범위) 필드. `today_string()`(`state.rs:258`)로 현재 월 파싱해 시즌 외엔 메뉴에서 숨김/회색. 데이터 한 줄 추가 = 신규 시즌 콘텐츠.
- **난이도:** 낮음.
- **본캐 불변:** 산타냥도 *생선 먹보 본캐 + 산타 룩/멘트 텍스처*일 뿐, 새 캐릭터 아님.

## 여성 타깃 근거

- **자기표현 도구로서의 캐릭터:** 2030 여성에게 카톡 이모티콘·프로필·테마는 *그날의 기분을 고르는* 일상 의식이다. 부캐 선택은 동일한 행위를 데스크톱 펫으로 옮긴 것 — "번아웃이라 직장인냥", "다정한 밤이라 몽실냥".
- **수집 + 무해한 성취:** 레벨 해금으로 부캐가 늘어나는 컬렉션 만족(가챠·결제·랭킹 없이). 비교·경쟁 메커닉을 배제해 *부드럽고 안전한* 진행감만 제공(하드 제약 1: 죄책감/스트릭/리더보드 배제).
- **검증된 IP 패턴 차용:** 망그러진곰·라이언의 "한 캐릭터 여러 에디션"은 한국 여성 캐릭터 시장에서 실증된 공식. 본캐 불변 + 텍스처 변주는 이 공식을 그대로 따르되 정체성 일관성을 지킨다.
- **공감 페르소나:** 직장인(번아웃)·몽실(졸림)·새침(츤데레)은 타깃의 정서적 상태에 *공감*하는 거울. id-미러 1인칭 톤과 결합해 "내 속마음 같은 고양이" 경험.

## 검증 & 리스크

**빌드/품질 게이트 (기존 워크플로 그대로):**
- `cargo build` / `cargo clippy -- -D warnings` / `cargo test` — 데이터+선택 로직이라 컴파일·린트 통과 용이.
- **e2e 메뉴 테스트 확장:** `tests/e2e.rs`의 악세사리 게이트 테스트(`e2e.rs:618~633` — 잠금 비활성/해금 후 checked/범위 밖 인덱스 no-op-패닉 없음)를 *그대로 본뜬* `SetPersona` 테스트 추가. `menu_find`(`e2e.rs:34`) 헬퍼 재사용. 잠긴 부캐 선택이 가드 no-op인지, `SetPersona(9999)`가 패닉 없이 무시되는지 검증.
- **하위호환 테스트:** `state.rs:373`의 구 JSON 디시리얼라이즈 테스트에 `assert_eq!(st.active_persona, 0)` 추가 — *Persist active_persona 기본값=본캐* 보증.

**프리뷰(부캐별 표정/룩 PNG):**
- `examples/preview.rs`는 `Scene`을 직접 구성해 PNG를 뽑는 헤드리스 도구(`preview.rs:13` `base_scene`). 부캐별로 `persona`/`theme`/`accessory`를 바꾼 `Scene`을 추가해 부캐 표정·룩·컬러를 한 장씩 렌더 → 카피라이터·디자이너 검토 + 회귀 확인. `Lang::Ko`로 시그니처 라인까지 같이 확인.

**리스크 & 완화:**
- **본캐 불변 위반 리스크:** 부캐 변조 계수가 본캐에서 1.0/동일 팔레트가 아니면 "그 고양이"가 깨진다 → 본캐(index 0) 프리뷰가 현행과 픽셀 동일한지 PNG diff로 가드.
- **"추천 부캐"가 프라이버시 신호만 쓰는지:** 추천 로직이 `input.rs` 카운터·로컬 시각·`active_min_today`만 참조하고 키 내용/창 제목/타이밍은 절대 안 건드리는지 코드 리뷰로 확인. 네트워크는 `update.rs`만(ADR-0008/0009). 자동 전환 금지 — 항상 수동 선택.
- **양 백엔드 패리티 (의도적 split):** 메뉴 모델(`src/menu.rs`)은 OS-무관이지만 *렌더링은 백엔드마다 다름* — macOS `mac_menu.rs`는 `MenuEntry` 트리를 그대로 렌더(`SetPersona` 자동 반영), **Windows 트레이는 서브메뉴를 손으로 빌드**(`windows.rs:1361~1387` 악세사리처럼 부캐 서브메뉴 코드를 *추가로 작성*해야 함 — 누락 시 패리티 깨짐), **Linux(portable)는 단축키 경로**라 메뉴 노출 방식이 다름. → 부캐 피커를 세 백엔드에 모두 노출하되, 핵심 동작은 `apply_menu_action`(테스트 대상)에 두어 *동작 패리티*는 코어가 보증하고, *렌더링*만 백엔드별로 맞춘다.
- **신규 의존성:** 전부 기존 `tiny-skia`/`serde`/i18n 위에서 데이터로 해결 → **새 heavy dep 0개, ADR 불필요**(하드 제약 4 준수).

## 핵심 수정 파일
- `src/state.rs` (PersonaDef + PERSONAS 카탈로그, Persist.active_persona, 하위호환)
- `src/pet.rs` (build_menu/apply_menu_action/level_up/advance/draw에 부캐 주입·선택·해금·추천)
- `src/render.rs` (draw_face 텍스처 틴트, 팔레트 테마, Scene에 persona/theme)
- `src/menu.rs` (MenuAction::SetPersona)
- `src/i18n.rs` (부캐 이름/잠금 라벨/시그니처 라인 EN+KO)
