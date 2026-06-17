# 구현 작업 요청 — 방향 6: 부캐(페르소나) 시스템 "한 고양이, 여러 컨셉"

## 역할
너는 ClipCat(러스트 초경량 클립보드 매니저 데스크탑 펫) 저장소에서 작업하는 시니어 러스트
엔지니어다. 아래 기획 문서를 바탕으로 본캐 위에 텍스처 프리셋을 묶는 부캐 시스템을 구현한다.

## 먼저 읽을 것 (반드시 정독)
- `AGENTS.md` (골든룰·아키텍처·검증 절차 — 단일 진실 공급원)
- `docs/specs/directions/README.md` (7개 방향 개요 + 확정 캐릭터 정체성)
- `docs/specs/directions/06-persona.md` ← 이번 작업 핵심(터치할 파일·함수 file:line, 재사용 패턴)
- (빌딩블록 의존) 02-emotion-design.md / 03-living-cat.md / 05-comfort-voice.md / 01-accessories.md
새 코드 전에 기존 패턴(ACCESSORIES 정적 카탈로그, Persist serde default, build_menu 서브메뉴, draw_face, advance idle 분기)을 먼저 재사용하라.

## 확정 캐릭터 정체성 (척추 — 절대 흔들지 말 것)
"생선 본위제 먹보 고양이". **부캐 = 새 정체성이 아니라 본캐 위 '텍스처 프리셋 묶음'**
(`{표정 풀 + 아이들 가중치 + 시그니처 라인 + 기본 룩}`). 본캐의 톤·형태·세계관은 불변, 부캐는
"서로 다른 생선 태도"의 큐레이션일 뿐 — 정체성 교체가 아니다. 본캐(index 0) 렌더는 현행과 픽셀 동일해야 한다.

## 이번 작업 범위
1. [#1] `PersonaDef` 데이터 레이어 + 정적 카탈로그 `PERSONAS[N]`(ACCESSORIES 패턴 복제). 시드: 순둥(본캐)/직장인/시크·새침/엉뚱/몽실.
2. [#2] `Persist.active_persona`(serde default 0=본캐) + 하위호환 테스트(`assert_eq!(active_persona,0)`).
3. [#3] 메뉴 부캐 피커(`MenuAction::SetPersona`, 레벨 게이트, 잠금 회색): build_menu 악세사리 서브메뉴 복제 + apply_menu_action 가드 동형.
4. [#4] 부캐 → 표정/룩/아이들/라인 동시 전환: draw_face에 `persona_tint`(형태 상수에 곱하는 작은 계수, 본캐=1.0), 기본 악세사리 제안, advance idle 분기에 `idle_bias` 곱.
5. [#5] 부캐별 시그니처 라인: i18n 부캐×무드 풀(`persona_line`), 어휘 베이스 "생선/먹보" 고정.
6. [#6] 부캐별 컬러 테마(팔레트 상수 선택 오버라이드, 본캐=현행 색).
7. [#7] 로컬 활동 기반 "추천 부캐"(1회성 힌트 토스트, **자동 전환 금지** — 항상 수동 선택).
8. [#8] (확장) 시즌 부캐(산타냥): `PersonaDef.season`, `today_string` 월 비교.

### 권장 1차 PR
**#1·#2·#3(데이터 레이어 + Persist + 메뉴 피커)** → 부캐 골격. #4(동시 전환)·#5(라인)은 후속,
#7(추천)·#8(시즌)은 그다음. 빌딩블록(방향2/3/5/1)이 충분히 있어야 부캐가 묶을 자산이 풍부하니 순서 유의.

## 반드시 지킬 제약 (위반 시 작업 무효)
- 프라이버시: 입력 후킹은 카운터만. "추천 부캐"는 카운터·로컬 시각·active_min_today만 — 키 내용·창
  제목·타이밍 미사용, 자동 전환 금지. 새 네트워크 금지(허용은 `update.rs`뿐).
- 에셋은 전부 tiny-skia 코드 드로잉. 번들 이미지·새 heavy dependency 금지(필요 시 ADR 먼저 제안).
- 코어 OS-비종속. 모든 문구 i18n(영/한 동시). 결제/가챠/스트릭/방치 죄책감 금지.
- **양 백엔드 패리티(의도적 split)**: 메뉴 동작은 `apply_menu_action`(테스트)에서 보증. 렌더는 macOS
  NSMenu가 MenuEntry 자동 반영, **Windows 트레이는 부캐 서브메뉴를 손으로 빌드 추가 필요**, Linux=단축키.

## 완료·검증 기준 (전부 통과해야 done)
1. `cargo build --release` / `cargo clippy --release` (+ `--features portable`) — 경고 0.
2. `cargo test --release` — `tests/e2e.rs` 악세사리 게이트 테스트를 본뜬 `SetPersona` 테스트(잠금 no-op,
   범위 밖 인덱스 패닉 없음) + 하위호환 assert. i18n 패리티 + 부캐×무드 라인 전수 테스트.
3. `cargo run --release --example preview` — 부캐별 표정/룩/컬러/한글 시그니처 라인 PNG. **본캐(index 0) PNG가 현행과 픽셀 동일**한지 diff 가드.
4. `CHANGELOG.md` `[Unreleased]`에 사용자 관점 한 줄(영문) 추가.

## 작업 방식
- 브랜치 `claude/vigilant-volta-q64b61`에서 작업. 다른 브랜치 푸시 금지. PR은 명시 요청 전까지 금지.
- 논리 단위 커밋(데이터 레이어 / 메뉴 / 전환 / 라인 …). Persist 스키마·새 의존성은 진행 전 질문.
- 끝나면 변경 요약 + 검증 결과(본캐 픽셀 동일 포함) + 직접 실행 못 한 항목(Windows/macOS 런타임)을 정직히 구분 보고.
