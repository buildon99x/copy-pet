# 구현 작업 요청 — 방향 4: 컬렉션 & 시즌 운영 (덕질·완성욕, 결제 없이)

## 역할
너는 ClipCat(러스트 초경량 클립보드 매니저 데스크탑 펫) 저장소에서 작업하는 시니어 러스트
엔지니어다. 아래 기획 문서를 바탕으로 수집·완성·시즌 운영 메타 레이어를 구현한다.

## 먼저 읽을 것 (반드시 정독)
- `AGENTS.md` (골든룰·아키텍처·검증 절차 — 단일 진실 공급원)
- `docs/specs/directions/README.md` (7개 방향 개요 + 확정 캐릭터 정체성)
- `docs/specs/directions/04-collection-season.md` ← 이번 작업 핵심(터치할 파일·함수 file:line, 재사용 패턴)
새 코드 전에 기존 함수(panel.view, ACCESSORIES, level_up/spawn_sparkles, ClipStore::sources, today_string, build_menu)를 먼저 재사용하라.

## 확정 캐릭터 정체성 (척추 — 흔들지 말 것)
"생선 본위제 먹보 고양이". 컬렉션을 본캐의 "장부(帳簿)"로 프레이밍 — 생선 도감(출처 앱=어종),
명대사 로그(id-미러/무드 명대사 수집), 악세사리 도감. 출처 앱은 *이름만* 쓰고 내용·제목은 절대 미사용.

## 이번 작업 범위
1. [#1] 컬렉션 북: 패널 3번째 뷰(`panel.view`/`Persist.panel_view` 이미 u8 → 값 2 추가, 스키마 무변경).
   `draw_panel`/`card_fields`/click 라우팅에 `view==2` 분기, 셀은 draw_accessory 미니 재호출.
2. [#2] 생선 도감(출처=어종): `ClipStore::sources()`로 파생, 배지 색/글자를 생선에. 영구 기록 원하면
   `Persist.seen_sources`(serde default). 초기엔 분모 없는 "잡은 어종 N" 카운트형.
3. [#3] 테마 세트 + 완성 보너스: `ACCESSORIES`에 `set`/`rarity`(컴파일 상수, 마이그레이션 0), 완성 판정 + level_up 연출 복제.
4. [#4] 희귀도 티어: `rarity`로 spawn_sparkles 수/색/문구 차등(ParticleKind 신규 0).
5. [#5] 시즌/한정 해금: `today_string`로 월 파싱 → 표시/숨김만(영속 변경 0). "제철 생선" 프레이밍, FOMO·처벌 금지.
6. [#6] "오늘의 코디" 셔플: `MenuAction::TodaysLook` + 해금 풀에서 rng 선택.

### 권장 1차 PR
스키마 변경이 가벼운 **#6(오늘의 코디) + #4(희귀도 티어)**부터. #1(패널 뷰)·#2(생선 도감)는
panel.rs/Persist 변경이 커서 별도 PR, 진행 전 패널 레이아웃·스키마 변경안을 먼저 질문하라.

## 반드시 지킬 제약 (위반 시 작업 무효)
- 프라이버시: 입력 후킹은 카운터만. 생선 도감은 출처 앱 *이름*만(내용·제목·키 내용·타이밍 미사용).
  명대사 로그는 "본 라인 ID 비트셋"만. 시즌은 `today_string`만. 새 네트워크 금지(허용은 `update.rs`뿐).
- 에셋은 전부 tiny-skia 코드 드로잉. 번들 이미지·새 heavy dependency 금지(필요 시 ADR 먼저 제안).
- 코어 OS-비종속. 두 백엔드 패리티(도감 입력도 기존 PanelAction/NavKey 경로). 모든 문구 i18n(영/한 동시).
- **결제/가챠/스트릭/리더보드/방치 죄책감 금지** — 해금은 활동 XP/누적 클립만, 시즌은 로컬 날짜 공개일 뿐.

## 완료·검증 기준 (전부 통과해야 done)
1. `cargo build --release` / `cargo clippy --release` (+ `--features portable`) — 경고 0.
2. `cargo test --release` — 세트 완성 판정·오늘의 코디 선택·시즌 분기·`view==2` 히트테스트를 순수 함수로
   단위 테스트. 패널 회귀(default_layout_matches_legacy_geometry, click_routes_rows_and_zones) 유지.
   하위호환: 구버전 state.json 디시리얼라이즈 테스트 갱신. i18n 패리티 테스트 통과.
3. `cargo run --release --example preview` — 도감 뷰/세트/티어/시즌 컷 PNG 육안 확인.
4. `CHANGELOG.md` `[Unreleased]`에 사용자 관점 한 줄(영문) 추가.

## 작업 방식
- 브랜치 `claude/vigilant-volta-q64b61`에서 작업. 다른 브랜치 푸시 금지. PR은 명시 요청 전까지 금지.
- 논리 단위 커밋. 패널 변경/Persist 스키마/새 의존성은 진행 전 질문.
- 끝나면 변경 요약 + 검증 결과(빌드/clippy/test/preview) + 직접 실행 못 한 항목(Windows/macOS 런타임)을 정직히 구분 보고.
