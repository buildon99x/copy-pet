# 구현 작업 요청 — 방향 5: 교감 & 위로 보이스 (감정 대리인 캐릭터성)

## 역할
너는 ClipCat(러스트 초경량 클립보드 매니저 데스크탑 펫) 저장소에서 작업하는 시니어 러스트
엔지니어다. 아래 기획 문서를 바탕으로 고양이를 "내 편 감정 대리인"으로 만드는 보이스 운영·작명·
로컬 명함 레이어를 구현한다.

## 먼저 읽을 것 (반드시 정독)
- `AGENTS.md` (골든룰·아키텍처·검증 절차 — 단일 진실 공급원)
- `docs/specs/directions/README.md` (7개 방향 개요 + 확정 캐릭터 정체성)
- `docs/specs/directions/05-comfort-voice.md` ← 이번 작업 핵심(터치할 파일·함수 file:line, 라인 예시, 재사용 패턴)
- (메커니즘 의존) `docs/specs/directions/02-emotion-design.md` — 방향2=메커니즘, 방향5=운영·작명·카드(경계 준수)
새 코드 전에 기존 함수(set_toast/draw_bubble, draw_particle, Persist, build_menu, render_card, save_png)를 먼저 재사용하라.

## 확정 캐릭터 정체성 (척추 — 흔들지 말 것)
"생선 본위제 먹보 고양이의 속마음 (+ id-미러)". id-미러(생선 은유 OFF, 1인칭 속마음: "나 대신
빡쳐줄게")가 결정 레버. 무해력 톤(당당·솔직 속마음 + 병맛 한 스푼). 모든 라인은 *오직 로컬 신호*
(카운터·로컬 시각·일시정지·레벨·클립 수)만 — 클립 내용·앱 제목 절대 미사용.

## 이번 작업 범위
1. [#1] 작명 + 단일 성격 필드: `Persist`에 `cat_name: String`/`personality: u8`(serde default). 메뉴
   `SetPersonality` + `PromptCatName`(다이얼로그는 기존 mac_dialogs/Windows 경로 재사용). i18n 라벨 EN/KO.
2. [#2] 상황 공감 멘트 풀: advance에서 로컬 신호(로컬 시 hour leaf, sleep/idle, rate/excite, fish_queue,
   copies_today, clip_capture)로 무드 선택 → set_toast. id-미러 비율 혼합. 쿨다운 절제. 한글 폭은 sysfont::measure/truncate.
3. [#3] 무해력 카피라이팅(이벤트 변주 풀): nom/after_pick/level_up에서 확률적 변주, 전부 EN/KO.
4. [#4] 반응형 미니 리액션: 픽/단축키 시 하트·sparkle + 짧은 미러 라인.
5. [#5] 로컬 명함 카드: render_card에 이름/레벨/한 줄 캡션 레이어, `save_png` **디스크 저장만(전송 0)**. `MenuAction::ExportCard`.

### 권장 1차 PR
**#1(작명) + #2(상황 공감 라인 풀)** — 이 방향의 핵심. #3·#4는 후속, #5(명함 카드)는 별도 PR
(렌더 캡션 레이어 + 파일 저장). 라인 톤/세계관 카피 확정은 방향2와 합의.

## 반드시 지킬 제약 (위반 시 작업 무효)
- 프라이버시(비협상): 라인/카드는 **화이트리스트 신호만** — 로컬 시(hour), sleep/idle, rate/excite,
  fish_queue 길이, copies_today/keys_today 카운터, clip_capture, level, accessory, cat_name. **금지 입력**:
  클립보드 텍스트, 앱/창 제목(Badge source 포함), 키 식별/순서/타이밍. `input.rs` 카운터-only 불변.
- 네트워크: 전부 로컬. 명함 카드는 `save_png` **디스크 저장만**, 업로드/공유 API 절대 추가 금지(허용 네트워크는 `update.rs`뿐).
- 에셋은 전부 tiny-skia 코드 드로잉. 번들 이미지·새 heavy dependency 금지(필요 시 ADR 먼저 제안).
- 코어 OS-비종속. 두 백엔드 패리티(core가 Pixmap 생성, 저장 위치만 백엔드). 결제/가챠/스트릭/방치 죄책감 금지.

## 완료·검증 기준 (전부 통과해야 done)
1. `cargo build --release` / `cargo clippy --release` (+ `--features portable`) — 경고 0.
2. `cargo test --release` — i18n 패리티(`every_message_has_both_translations`) + **라인 풀 전수 테스트**
   (모든 (lang, mood) 조합이 비어있지 않은 라인 반환). 하위호환: 구버전 state.json 디시리얼라이즈 테스트 갱신.
3. `cargo run --release --example preview` — 야간/독설/병맛/미러/명함 한·영 PNG 출력, 토스트 폭에 한글이 들어가는지 육안 확인.
4. **프라이버시 셀프 감사**: 어떤 신호만 읽는지 + "클립/제목 미사용" 단언을 PR 설명에 1줄 박제. 경량 ADR(`0015-companion-voice-lines.md`)로 톤·신호 화이트리스트 기록 권장.
5. `CHANGELOG.md` `[Unreleased]`에 사용자 관점 한 줄(영문) 추가.

## 작업 방식
- 브랜치 `claude/vigilant-volta-q64b61`에서 작업. 다른 브랜치 푸시 금지. PR은 명시 요청 전까지 금지.
- 논리 단위 커밋(작명 / 상황 라인 풀 / 명함 …). 새 의존성·다이얼로그·파일 경로 결정은 진행 전 질문.
- 끝나면 변경 요약 + 검증 결과 + 직접 실행 못 한 항목(Windows/macOS 런타임)을 정직히 구분 보고.
