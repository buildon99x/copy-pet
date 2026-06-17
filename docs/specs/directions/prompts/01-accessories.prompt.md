# 구현 작업 요청 — 방향 1: 악세사리 확장

## 역할
너는 ClipCat(러스트 초경량 클립보드 매니저 데스크탑 펫) 저장소에서 작업하는 시니어 러스트
엔지니어다. 아래 기획 문서를 바탕으로 고양이 악세사리 확장을 실제 코드로 구현한다.

## 먼저 읽을 것 (반드시 정독)
- `AGENTS.md` (골든룰·아키텍처·검증 절차 — 단일 진실 공급원)
- `docs/specs/directions/README.md` (7개 방향 개요 + 확정 캐릭터 정체성)
- `docs/specs/directions/01-accessories.md` ← 이번 작업 핵심(터치할 파일·함수 file:line, 난이도, 재사용 패턴)
새 코드를 만들기 전에 문서에 적힌 기존 함수(draw_accessory, ACCESSORIES, build_menu 등)를 먼저 재사용하라.

## 확정 캐릭터 정체성 (척추 — 흔들지 말 것)
"생선 본위제 먹보 고양이의 속마음 (+ id-미러)". 본캐(게으른 먹보·세상을 생선으로 환산)는 불변.
악세사리는 본캐 형태·톤을 유지한 채 외형만 확장한다. 생선 모자=셀프 패러디, 앞발 미니 생선=화폐
단위, 하트 팻말=id-미러 소품화로 정체성과 연결.

## 이번 작업 범위
1. [#2] 신규 코드드로잉 악세사리 8~12종(리본/보우·꽃·토끼귀·베레모·후드·담요 망토·안대·생선 모자):
   `Accessory` enum + `from_id` + `draw_accessory` arm + `ACCESSORIES` 행 + preview 컷.
2. [#3] 컬러 변형/스킨: `draw_accessory`에 색 파라미터, `lighten`/`darken`로 파생, `Persist.accessory_tint`(serde default 0).
3. [#4] 앞발(paw) 소품: `draw_paw` 좌표 기준 미니 생선/커피/하트 팻말.
4. [#5] 시즌 한정 룩: `state::today_string` 로컬 날짜로 메뉴 노출만(streak/FOMO 금지, 해금 후 영구 착용).
5. [#1] 멀티 슬롯화(hat/face/neck/paw): `Persist`에 신규 필드(serde default), `SetAccessory`→`SetSlot` 일반화.
6. [#6] 프리뷰·i18n 정비(신규 메뉴 라벨 EN/KO, 신규 조합 PNG 컷).

### 권장 1차 PR
스키마 변경이 없는 **#2(신규 악세사리) + #6(preview/i18n)**부터. #1 멀티 슬롯과 #3 컬러는 `Persist`
스키마/메뉴 변경이 있어 별도 PR로 분리하고, 진행 전 스키마 변경안을 먼저 질문하라.

## 반드시 지킬 제약 (위반 시 작업 무효)
- 프라이버시: 입력 후킹은 카운터만. 키 내용·창 제목·타이밍 읽기/저장/로그/전송 금지. 기능은 로컬
  상태(레벨·착용 인덱스·로컬 날짜)만. 새 네트워크 금지(허용은 `update.rs`뿐).
- 에셋은 전부 tiny-skia 코드 드로잉. 번들 이미지·새 heavy dependency 금지(필요 시 ADR 먼저 제안).
- 코어 OS-비종속(render.rs/state.rs/pet.rs/i18n.rs). OS 코드는 `src/platform/`만. 두 백엔드 패리티.
- 모든 사용자 문구 i18n(영/한 동시). 결제/가챠/스트릭/방치 죄책감 금지.

## 완료·검증 기준 (전부 통과해야 done)
1. `cargo build --release` / `cargo clippy --release` (+ `--features portable`) — 경고 0.
2. `cargo test --release` — `every_accessory_has_a_reachable_level` 등 기존 테스트 통과 + 신규 메뉴 잠금/슬롯 테스트.
   하위호환: 구버전 state.json 디시리얼라이즈 테스트 갱신.
3. `cargo run --release --example preview` — 신규 악세사리/색/시즌/슬롯 조합 PNG 육안 확인.
4. `CHANGELOG.md` `[Unreleased]`에 사용자 관점 한 줄(영문) 추가.

## 작업 방식
- 브랜치 `claude/vigilant-volta-q64b61`에서 작업. 다른 브랜치 푸시 금지. PR은 명시 요청 전까지 금지.
- 논리 단위 커밋(무엇을·왜). 스키마 변경/새 의존성은 진행 전 질문.
- 끝나면 변경 요약 + 검증 결과(빌드/clippy/test/preview) + 직접 실행 못 한 항목(Windows/macOS 런타임)을 정직히 구분 보고.
