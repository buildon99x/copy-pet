# 구현 요청 — 방향 4: 컬렉션 & 시즌 운영

공통 규칙(역할·정체성·제약·검증·워크플로)은 [README](README.md) 참조.

**스펙**: `docs/specs/directions/04-collection-season.md` — 수집·완성·시즌 메타. 컬렉션=본캐의 "장부".

## 범위
1. 컬렉션 북: 패널 3번째 뷰(`panel.view`/`Persist.panel_view` 이미 u8 → 값 2, 스키마 무변경). `draw_panel`/`card_fields`/click에 `view==2` 분기, 셀은 `draw_accessory` 미니 재호출.
2. 생선 도감(출처=어종): `ClipStore::sources()` 파생, 배지 색/글자를 생선에. 영구 기록 원하면 `Persist.seen_sources`(serde default). 초기엔 분모 없는 카운트형.
3. 테마 세트 + 완성 보너스: `ACCESSORIES`에 `set`/`rarity`(컴파일 상수), 완성 판정 + level_up 연출 복제.
4. 희귀도 티어: `rarity`로 `spawn_sparkles` 수/색/문구 차등.
5. 시즌/한정: `today_string` 월 파싱 → 표시/숨김만(영속 변경 0). "제철" 프레이밍, FOMO·처벌 금지.
6. "오늘의 코디" 셔플: `MenuAction::TodaysLook` + 해금 풀 rng 선택.

## 권장 1차 PR
스키마 가벼운 **#6 + #4**. #1(패널 뷰)·#2(생선 도감)는 panel/Persist 변경 → 별도 PR, 레이아웃·스키마안 먼저 질문.

## 이 방향 특이 주의
- 패널 회귀 테스트(`default_layout_matches_legacy_geometry`, `click_routes_rows_and_zones`) 유지.
- 도감은 출처 앱 **이름만**(내용·제목·타이밍 미사용). 해금은 활동 XP/누적 클립만.
