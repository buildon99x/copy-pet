# 방향 5 — 교감 & 위로 보이스 (감정 대리인 캐릭터성)

> ClipCat 고양이 캐릭터 업그레이드 7개 방향 중 #5.
> 본캐("생선 본위제 먹보 고양이의 속마음 + id-미러", 부록 B)의 **보이스 운영·작명·로컬 공유** 레이어.

## What (정의)

방향 5는 ClipCat 고양이를 단순한 "복사 마스코트"에서 **"내 편이 되어주는 감정 대리인"** 으로 끌어올리는 *보이스 운영·세계관 카피·로컬 공유* 레이어다. 핵심은 "생선 본위제 먹보 고양이의 속마음"이라는 세계관 위에서, 고정 표정 + 짧은 한 줄 = 완성된 짤이라는 전달 방식을 따르되, **무드를 텍스처로** 다룬다(집중→독설, 한가→병맛, 야간→다정, 상호작용→츤데레). 결정적 레버는 **id-미러 레지스터**: 어떤 라인은 생선 은유를 버리고 1인칭으로 사용자의 속마음을 대신 말한다("나 대신 빡쳐줄게", "오늘은 그냥 눕자, 생선은 내일"). 모든 라인은 *오직 로컬 활동 상태*(카운터, 로컬 시각, 일시정지 여부, 레벨, 클립 수, 생선 연속 폭주)만 입력으로 쓰며 **클립 내용·앱 제목은 절대 보지 않는다.**

**방향 2와의 경계 (명시):**
- **방향 2 = 메커니즘.** 표정 상태머신, 말풍선/토스트 *전달 표면*, 무드 텍스처를 *선택·전환하는 엔진*. 즉 "어떻게 띄우나".
- **방향 5 = 운영·작명·카드.** 그 엔진 위에 올라가는 **무해력 톤의 라인 풀(한/영), 고양이 작명/성격, 상황 공감 매핑, id-미러 카피, 로컬 명함 카드**. 즉 "무엇을 말하나 + 누구로서 말하나 + 어떻게 간직/공유하나".
- 부캐(성격 프리셋) *묶음 운영*은 방향 6. 방향 5는 단일 작명/단일 성격 필드 + 라인 풀까지만.

전달 표면은 이미 존재한다: 토스트 핀(`src/pet.rs:99` `toast` 필드, `set_toast` `src/pet.rs:1439`)과 렌더링되는 토스트 알약(`src/render.rs:452-462`). 방향 5는 이 표면에 *세계관 카피*를 흘려보내는 콘텐츠/선택 로직이다.

## Why (기대효과)

- **"내 속마음 대리인" = 2030 여성 핵심 동인.** 데스크탑 펫/위젯의 리텐션은 "유용성"보다 "정서적 동반"에서 나온다. id-미러("나 대신 빡쳐줄게")는 사용자의 감정을 *읽는 척*이 아니라 *대신 말해주는* 구조라, 프라이버시를 1mm도 침범하지 않으면서(클립 내용 미사용) 깊은 공감 착시를 만든다. 이게 93→97을 닫은 +2 레버였다.
- **작명 = 애착의 1차 스위치.** 이름을 붙이는 순간 "프로그램"이 "내 고양이"가 된다(다마고치·앱등이 효과). 한 번의 입력으로 리텐션 곡선이 꺾인다.
- **무해력 톤(당당·솔직 속마음 + 병맛 한 스푼) = 한국 여성 밈 문법.** 죄책감/추적/스트릭/순위표를 *배제*하는 것 자체가 차별점: 생산성 앱의 "오늘 3시간 낭비했어요" 잔소리와 정반대로, "오늘은 그냥 눕자"고 편들어준다.
- **로컬 명함 카드 = 자기표현 + 자발적 바이럴.** 현재 코디+레벨을 담은 카드를 *로컬 저장*만 해도, 사용자가 직접 캡처해 공유하는 행동을 유도한다(전송은 앱이 안 함 → 프라이버시 유지하면서 입소문). 꾸미기·수집 동인과 직결.

## 구현 아이디어 (구체화)

### 1. 고양이 작명 + 단일 성격 필드 (로컬·i18n)
- **무엇을:** 사용자가 고양이 이름을 정하고(예: "참치"), 성격 톤을 하나 고른다(예: 다정/독설/병맛 — 무드 텍스처의 *기본 가중치*만 바꿈, 부캐 묶음 아님).
- **어떻게:**
  - `src/state.rs` `Persist`에 `#[serde(default)]` 보장 하에 옵션 필드 추가: `cat_name: String`(빈 문자열=미설정), `personality: u8`(0=균형 기본). `Default`(`src/state.rs:69`)에 빈 값/0 추가. **백워드 호환은 기존 테스트 `old_state_json_still_deserializes`(`src/state.rs:373`)가 그대로 보증** — `#[serde(default)]` 컨테이너 어트리뷰트(`src/state.rs:12`) 덕분에 구버전 JSON이 새 필드를 default로 채운다.
  - 메뉴 연결: `MenuAction`(`src/menu.rs:18`)에 `SetPersonality(u8)`, 작명은 `MenuOutcome`에 `PromptCatName`(백엔드가 OS 텍스트 입력 다이얼로그 — `mac_dialogs.rs` 패턴 재사용, Windows는 기존 reset/about 다이얼로그 경로 재사용) 추가. `build_menu`(`src/pet.rs:1201`)에 성격 서브메뉴를 사이즈/액세서리 서브메뉴와 동일 패턴(`src/pet.rs:1252-1294`)으로 추가. `apply_menu_action`(`src/pet.rs:1342`)에서 순수 상태변경만 처리.
  - i18n: `Msg`에 `MenuCatName`, `MenuPersonality`, 성격 라벨들 추가(`src/i18n.rs`). **`every_message_has_both_translations` 테스트(`src/i18n.rs:406`)가 한/영 누락을 컴파일+테스트로 강제** → 새 Msg를 그 배열에 넣어야 통과.
- **사용 LOCAL 신호:** 없음(순수 설정). 이름은 라인 템플릿에 `{name}`로만 주입.
- **난이도: EASY.**
- **id-미러 연결:** 이름이 있으면 라인이 1인칭에서 더 자연스러워진다("참치가 대신 빡쳐줄게" 같은 3인칭/1인칭 혼용 톤 가능).

### 2. 상황 공감 멘트 풀 (LOCAL 상태 기반 + 무드 텍스처 + id-미러)
- **무엇을:** "지금 상황"에 맞는 한 줄을 토스트로 띄운다. 단, *상황은 오직 로컬 신호*에서 추론한다.
- **어떻게:**
  - 라인 풀을 `src/i18n.rs`에 새 함수군으로 추가(예: `vibe_line(lang, mood, id_mirror_roll) -> &str`, 또는 `&[&str]` 풀 + 인덱스). i18n이 이 방향의 심장이므로 모든 풀은 여기 한/영 쌍으로.
  - 선택 로직은 `Pet::advance`(`src/pet.rs:855`)에 얹는다. advance는 이미 모든 신호에 접근 가능:
    - **야간:** `state::today_string`(`src/state.rs:257`)와 같은 per-OS leaf 패턴으로 *로컬 시(hour)* 만 읽는 작은 함수 추가(분/초/날짜 내용 저장 안 함) → 야간이면 *다정* 무드. (시각은 이미 사용 중인 sanctioned leaf 범주.)
    - **장시간/한가:** `self.sleep`/`idle_secs`(`src/pet.rs:908-911`) → 한가하면 *병맛*.
    - **집중(폭타):** `self.rate`/`excite`(`src/pet.rs:905-906, 918`) 높음 → *독설*.
    - **대량 복사:** `fish_queue` 폭주 또는 `copies_today` 임계 → 전용 풀.
    - **일시정지:** `self.st.clip_capture == false` → 전용 위로 풀.
  - 띄우기는 기존 `set_toast`(`src/pet.rs:1439`) 그대로 호출. 빈도는 쿨다운 타이머(필드 1개)로 절제 — 잔소리 금지 원칙.
  - **긴 한글 한 줄 대응:** 토스트 알약 폭은 `sysfont::measure`(`src/sysfont.rs:183`)로 자동 산정(`src/render.rs:455`)되므로 한글 라인도 깨지지 않음. 너무 길면 `truncate_to_width`(`src/sysfont.rs:189`)로 컷.
- **사용 LOCAL 신호:** 로컬 시(hour), `sleep`/`idle_secs`, `rate`, `fish_queue` 길이, `copies_today`, `clip_capture`, `level`. **클립 텍스트·앱 제목·키 내용 0%.**
- **난이도: EASY–MEDIUM.**
- **id-미러 연결:** 풀 안에 일정 비율로 1인칭 미러 라인 섞기. 결정론적 셔플은 기존 `rand_f(&mut self.rng)`(`src/pet.rs:52`) 재사용.

**라인 예시 (한/영):**

| 상황(LOCAL 신호) | 무드 | 한국어 | English |
|---|---|---|---|
| 야간(로컬 시 ≥ 1시) | 다정 | "오늘은 그냥 눕자. 생선은 내일." | "Let's just lie down. Fish can wait till tomorrow." |
| 야간 + id-미러 | 다정 | "잘했어 진짜. 이제 자도 돼." | "You did enough. You're allowed to rest now." |
| 폭타(rate 높음) | 독설 | "손가락 부러지겠다, 천천히 해 인간." | "Your fingers are gonna snap. Ease up, human." |
| 한가(idle 장시간) | 병맛 | "...생선 안 와. 나 그냥 누워있을게." | "...no fish incoming. I'll just be a puddle." |
| 대량 복사 폭주 | 츤데레 | "또 복사야? 뭐 어차피 내가 다 생선으로 바꿔줄 거지만." | "Copying again? Fine, I'll turn it all to fish. Not for you though." |
| 일시정지 중 | 다정 | "수집 꺼놨어. 비밀은 비밀로." | "Capture's off. Your secrets stay secret." |
| id-미러(분노 대리) | — | "나 대신 빡쳐줄게. 너는 쉬어." | "I'll be mad on your behalf. You rest." |
| 레벨업 직후 | 병맛 | "레벨 올랐다고 생선 더 주는 거 아니지? ...아 줘." | "Leveling up doesn't mean more fish, right? ...give me fish." |

### 3. 무해력 토널리티 카피라이팅 (이벤트 라인 한/영 동시)
- **무엇을:** 기존 *기능 토스트*(복사됨/붙여넣음/레벨업)를 "당당·솔직 속마음 + 병맛 한 스푼" 톤으로 다듬고, 이벤트별 *변주 풀*을 만들어 매번 다른 한 줄이 나오게 한다.
- **어떻게:**
  - 현재 단일 문자열인 메시지들(`Msg::ToastCopied` 등, `src/i18n.rs:184`)은 그대로 두고(다른 호출부 호환), `nom`(`src/pet.rs:1002`, 생선 먹는 순간)과 `after_pick`(`src/pet.rs:685`, 클립 픽)에서 **확률적으로** 변주 풀의 라인을 `set_toast`로 덮어쓰는 분기 추가.
  - 레벨업/액세서리 라인(`level_up`/`new_accessory`, `src/i18n.rs:272,279`)도 무해력 변주 풀 추가. `level_up`(`src/pet.rs:1419`)에서 선택.
  - 모든 신규 풀은 `src/i18n.rs`에 한/영 쌍. 톤 가이드는 ADR로 1줄 기록(아래 리스크 참고).
- **사용 LOCAL 신호:** 이벤트 종류(copy/nom/levelup) + 성격 필드 가중치. 내용 미사용.
- **난이도: EASY.**
- **id-미러 연결:** 픽/먹는 순간 라인에 가끔 "오늘 이거 하나 건졌네. 잘했어." 같은 사용자-칭찬 미러 삽입.

**라인 예시 (한/영):**
- 생선 먹음(nom): "냠. 이건 좀 살이 통통하네." / "Nom. This one's nice and plump."
- 복사 픽: "가져가. 내가 지키고 있었어." / "Take it. I was guarding it for you."
- 레벨업: "레벨 업. 근데 생선은 언제 줘?" / "Level up. So... when's the fish?"

### 4. 반응형 미니 리액션 (붙여넣기/단축키 격려 비주얼)
- **무엇을:** 사용자가 클립을 픽/붙여넣기 하거나 패널 단축키를 쓸 때, 짧은 격려 파티클 + 깜짝 한 줄.
- **어떻게:**
  - 파티클 시스템 완전 재사용: `draw_particle`(`src/render.rs:761`)의 Heart/Sparkle, 스폰 헬퍼 `spawn_sparkles`(`src/pet.rs:1462`)/하트 스폰(`pet`의 `src/pet.rs:1158-1171` 패턴). `after_pick`(`src/pet.rs:685`)은 이미 `happy` 범프 + pop 사운드를 함 → 여기에 하트 1~2개 + 격려 라인 추가.
  - 단축키로 패널 열 때(`open_flyout`/`open_panel`, `src/pet.rs:497,518`)는 작은 sparkle 한 번. 과하지 않게.
- **사용 LOCAL 신호:** 사용자 액션 이벤트(픽/단축키)만. 클립 내용 미사용.
- **난이도: EASY.**
- **id-미러 연결:** "역시 너야." / "Knew you could." 같은 짧은 미러 라인을 리액션에 동봉.

### 5. 고양이 명함 / 스크린샷 카드 (현재 코디+레벨, 로컬 저장만)
- **무엇을:** 현재 고양이(이름+레벨+착용 액세서리+무드 한 줄)를 담은 예쁜 "명함" PNG를 한 번에 디스크에 저장. **전송 기능 일절 없음.**
- **어떻게:**
  - 렌더 파이프 재사용이 핵심: `render_card`(`src/pet.rs:1057`)는 이미 불투명 `Pixmap`에 `Scene`을 래스터한다(`src/render.rs:306`). 명함은 이 위에 *작은 캡션 레이어*(이름/레벨/한 줄)만 얹은 새 `render_card_export` 정도. 텍스트는 `Cv::ui_text`(`src/render.rs:265`)로 한글 그대로 렌더.
  - 저장은 `examples/preview.rs`가 이미 증명한 `pm.save_png(path)`(`examples/preview.rs:38-40`) 패턴 그대로. 경로는 `state::config_dir`(`src/state.rs:128`) 하위 또는 OS 사진/다운로드 폴더. 디스크 쓰기는 `write_atomic`(`src/state.rs:135`) 인접 패턴 참고하되 PNG는 단순 저장으로 충분.
  - 메뉴 트리거: `MenuAction::ExportCard`, `MenuOutcome::ExportCard`(백엔드가 저장 경로 토스트). `build_menu`/`apply_menu_action`에 추가. i18n에 `MenuExportCard` + 저장완료 토스트 한/영.
  - **양 백엔드 패리티:** core가 PNG 바이트/Pixmap을 만들고, 저장 위치 결정만 백엔드가 하면 macOS/Windows/Linux 모두 동일 경험.
- **사용 LOCAL 신호:** 레벨, 액세서리, 이름, (선택) 무드 한 줄. 클립 내용 미사용.
- **난이도: MEDIUM** (렌더 파이프·save_png·preview 패턴이 다 있어 신규 위험 낮음).
- **id-미러 연결:** 카드 하단 카피를 미러 톤으로("내 고양이, 내 편." / "My cat. On my side.").

## 여성 타깃 근거

- **정서적 동반 > 기능 효용.** 2030 여성 사용자층에서 데스크탑 펫/위젯·캐릭터 앱의 장기 리텐션은 "내 편 같은 존재감"에서 나온다. id-미러는 사용자의 감정을 *대신 발화*해 "이 앱은 나를 안다"는 착시를 만들되, 입력은 로컬 카운터/시각뿐이라 안전하다.
- **작명·꾸미기·수집 = 검증된 애착 루프.** 이름 짓기(1번)와 명함 카드(5번)는 "내 캐릭터를 내 손으로 만들고 자랑"하는 자기표현 동인을 직접 자극한다.
- **반(反)생산성 위로 포지셔닝.** 죄책감/스트릭/순위표를 *명시적으로 배제*(하드 제약)하고 "오늘은 그냥 눕자"로 편들어주는 톤은, 잔소리형 트래커에 피로한 사용자에게 차별적 안식처가 된다.
- **무해력 밈 문법 적합.** "당당·솔직 속마음 + 병맛 한 스푼"은 한국 여성 커뮤니티의 짤·밈 톤과 정합 → 명함 카드의 자발적 공유로 이어질 개연성이 높다.

## 검증 & 리스크

**빌드/품질 게이트:**
- `cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test` (i18n 패리티·persist 호환 테스트가 자동 가드).
- **프리뷰 PNG로 한글 라인 육안 검증:** `cargo run --release --example preview`. 신규 라인 풀·명함 카드를 `examples/preview.rs`(이미 한글 토스트/버블/카드 프레임을 PNG로 출력함, `examples/preview.rs:88-104, 118-133`)에 한 프레임씩 추가해 *야간/독설/병맛/미러/명함* 각각을 한·영 PNG로 확인. 토스트 폭 자동 산정(`render.rs:455`)으로 긴 한글이 알약 밖으로 넘치지 않는지 시각 확인.

**프라이버시 감사 (골든룰 1, 비협상):**
- **쓰는 신호 화이트리스트만 사용:** 로컬 시(hour, per-OS leaf — `today_string` 동급 범주), `sleep`/`idle_secs`, `rate`/`excite`, `fish_queue` 길이, `copies_today`/`keys_today` 등 카운터, `clip_capture`(일시정지), `level`, `accessory`, `cat_name`.
- **절대 금지 입력:** 클립보드 텍스트, 앱/창 제목(`Badge`의 source 문자열 포함), 키 식별자/순서/타이밍. `input.rs`(`src/input.rs`)는 카운터-only 유지 — 라인 선택이 `input.rs`를 건드리지 않음을 확인.
- **네트워크:** 신규 라인/카드/작명은 *전부 로컬*. 유일 sanctioned 네트워크는 `update.rs`. 명함 카드는 `save_png`로 **디스크 저장만**, 업로드/공유 API 절대 추가 금지.
- 감사 체크리스트를 PR 설명 또는 ADR에 1줄로 박제(어떤 신호만 읽는지 + "클립/제목 미사용" 단언).

**i18n 누락 리스크:**
- 새 `Msg` enum 항목은 `every_message_has_both_translations`(`src/i18n.rs:406`)의 배열에 등록해야 통과 → 한/영 누락이 테스트로 막힘. 단, *풀 함수*(`vibe_line` 등)는 이 테스트가 자동 커버하지 않으므로, 풀 함수용 전용 테스트(모든 (lang, mood) 조합이 비어있지 않은 라인을 반환)를 추가해야 함.

**명함 카드 로컬 저장 리스크:**
- 저장 경로 권한/디스크 실패는 무해하게 처리(실패 토스트), 임시파일 누수 방지(rename/cleanup 패턴 `state.rs:135` 참고). **전송 코드가 실수로라도 들어가지 않도록** 카드 모듈은 파일시스템 외 의존성 0으로 유지.

**의존성 리스크 (골든룰 3):**
- 위 5개 모두 **신규 heavy dep 0** — tiny-skia/ab_glyph/serde/기존 렌더·파티클·토스트·persist·preview 파이프 재사용만으로 구현 가능 → ADR 불필요. 단, *톤 가이드 + 프라이버시 신호 화이트리스트*는 경량 ADR(예: `0015-companion-voice-lines.md`)로 1장 남겨 라인 추가 시 규칙이 유지되게 할 것.

## 핵심 수정 파일
- `src/i18n.rs` (라인 풀의 심장: 한/영 변주 풀 + 신규 Msg, 패리티 테스트)
- `src/pet.rs` (advance `:855` 의 상황 선택 + nom `:1002`/after_pick `:685` 변주 + build_menu `:1201`/apply_menu_action `:1342` + render_card `:1057` 명함)
- `src/state.rs` (Persist 에 cat_name/personality optional 필드, 백워드 호환 + config_dir/save 패턴)
- `src/render.rs` (toast 알약 `:452`, draw_particle `:761`, render_card `:306` 재사용·명함 캡션 레이어)
- `examples/preview.rs` (한글 라인·명함 카드 PNG 검증 — save_png 패턴)
