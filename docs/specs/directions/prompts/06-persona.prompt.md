# 구현 요청 — 방향 6: 부캐(페르소나) 시스템

공통 규칙(역할·정체성·제약·검증·워크플로)은 [README](README.md) 참조.

**스펙**: `docs/specs/directions/06-persona.md` — 부캐 = 본캐 위 **텍스처 프리셋 묶음**(정체성 교체 아님). 본캐(index 0)는 현행과 픽셀 동일.

## 범위
1. `PersonaDef` 데이터 레이어 + 정적 카탈로그 `PERSONAS[N]`(ACCESSORIES 패턴 복제). 시드: 순둥(본캐)/직장인/시크·새침/엉뚱/몽실.
2. `Persist.active_persona`(serde default 0) + 하위호환 테스트.
3. 메뉴 부캐 피커(`MenuAction::SetPersona`, 레벨 게이트·잠금 회색): build_menu 복제 + apply_menu_action 가드 동형.
4. 동시 전환: `draw_face`에 `persona_tint`(형태 상수에 곱하는 계수, 본캐=1.0), 기본 악세사리 제안, advance idle 분기에 `idle_bias` 곱.
5. 시그니처 라인: i18n 부캐×무드 풀(`persona_line`), 어휘 베이스 "생선/먹보" 고정.
6. 컬러 테마(팔레트 상수 선택 오버라이드, 본캐=현행).
7. "추천 부캐"(1회 힌트, **자동 전환 금지** — 항상 수동). 카운터·로컬 시각·active_min_today만.
8. (확장) 시즌 부캐(산타냥): `PersonaDef.season`, `today_string` 월 비교.

## 권장 1차 PR
**#1·#2·#3(데이터 레이어 + Persist + 메뉴 피커)**. #4·#5 후속. 빌딩블록(방향2/3/5/1)이 있어야 묶을 자산 충분.

## 이 방향 특이 주의
- **본캐(index 0) PNG가 현행과 픽셀 동일**한지 diff 가드.
- 양 백엔드 패리티(의도적 split): 동작은 `apply_menu_action`(테스트) 보증. macOS NSMenu 자동 반영, **Windows 트레이는 서브메뉴 수동 빌드 추가 필요**, Linux=단축키.
