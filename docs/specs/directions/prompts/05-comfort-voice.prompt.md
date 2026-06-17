# 구현 요청 — 방향 5: 교감 & 위로 보이스

공통 규칙(역할·정체성·제약·검증·워크플로)은 [README](README.md) 참조.

**스펙**: `docs/specs/directions/05-comfort-voice.md` (메커니즘은 방향2, 여기는 운영·작명·카드) — 무해력 톤·id-미러.

## 범위
1. 작명 + 단일 성격 필드: `Persist`에 `cat_name`/`personality`(serde default). 메뉴 `SetPersonality` + `PromptCatName`(기존 다이얼로그 경로 재사용). i18n 라벨.
2. 상황 공감 라인 풀: advance에서 로컬 신호(로컬 시 hour leaf, sleep/idle, rate/excite, fish_queue, copies_today, clip_capture)로 무드 선택 → `set_toast`. id-미러 혼합, 쿨다운. 한글 폭 `sysfont::measure`/`truncate`.
3. 무해력 변주 카피: nom/after_pick/level_up에서 확률적 변주, EN/KO.
4. 미니 리액션: 픽/단축키 시 하트·sparkle + 짧은 미러 라인.
5. 로컬 명함 카드: `render_card`에 이름/레벨/한 줄 캡션, `save_png` **디스크 저장만(전송 0)**, `MenuAction::ExportCard`.

## 권장 1차 PR
**#1(작명) + #2(라인 풀)**. #3·#4 후속, #5(명함)는 별도 PR. 라인 톤은 방향2와 합의.

## 이 방향 특이 주의
- **프라이버시 화이트리스트**: 로컬 시·sleep/idle·rate·카운터·clip_capture·level·accessory·cat_name만. 클립 텍스트·창 제목·키 식별/타이밍 금지. 명함은 업로드 API 금지.
- 라인 풀 무드별 전수 테스트. 톤·신호 화이트리스트는 경량 ADR 권장.
