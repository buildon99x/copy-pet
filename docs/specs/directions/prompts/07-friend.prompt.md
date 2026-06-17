# 구현 작업 요청 — 방향 7: 친구·관계 시스템 "둘이라서 더 귀여운"

## 역할
너는 ClipCat(러스트 초경량 클립보드 매니저 데스크탑 펫) 저장소에서 작업하는 시니어 러스트
엔지니어다. 아래 기획 문서를 바탕으로 로컬 두 번째 캐릭터(친구)와 관계 연출을 구현한다.

## 먼저 읽을 것 (반드시 정독)
- `AGENTS.md` (골든룰·아키텍처·검증 절차 — 단일 진실 공급원)
- `docs/specs/directions/README.md` (7개 방향 개요 + 확정 캐릭터 정체성)
- `docs/specs/directions/07-friend.md` ← 이번 작업 핵심(터치할 파일·함수 file:line, 캔버스 여백 분석, 재사용 패턴)
- (연계) `docs/specs/directions/04-collection-season.md` — 커플/베프 세트 룩
새 코드 전에 기존 함수(draw_scene/draw_fish/draw_accessory 프리미티브, advance 스케줄러 패턴, on_copy_rich, level_up, build_menu/menu.rs 토글, Persist serde default)를 먼저 재사용하라.

## 확정 캐릭터 정체성 (척추 — 흔들지 말 것)
"생선 본위제 먹보 고양이". 친구 = **로컬 두 번째 그림**(네트워크 아님, 멀티플레이어 아님). 모든 관계는
"생선을 둘이 나눈다"는 세계관으로 수렴(나눠 먹기 컷, 선물=생선 꾸러미, 생선 사이드킥).

## 이번 작업 범위
1. [#1] 친구 저빈도 가장자리 등장: advance에 등장 스케줄러(blink/zzz 패턴, 8~20분 ±랜덤) + friend_phase ease.
   **본캐 좌우 여백(x≈0–64/176–240, CANVAS_W=240) 안에 작게(scale~0.55)** 그려 캔버스/window-shift 불필요. draw_scene 프리미티브 재호출.
2. [#2] 둘이 생선 나눠 먹기 깜짝 컷: on_copy_rich에서 친구 on + 저확률(~8~12%) 시 nom 지점에서 친구도 한 입. id-미러 한 줄("반만… 줄게.").
3. [#3] 커플/베프 세트 룩(방향4 연계): ACCESSORIES + Accessory/from_id/draw_accessory 항목 추가, 친구도 같은 draw_accessory. 친구 없을 때도 본캐 상시 착용.
4. [#4] 관계 이벤트(선물=깜짝 해금): level_up 로직 복제한 `friend_gift()`. 트리거는 #1 스케줄러의 한 컷.
5. [#5] 친구 토글: `MenuAction::ToggleFriend` + `Persist.friend_on: bool`(serde default false, 하위호환). 메뉴 코어 모델 → GUI 없이 테스트.
6. [#6] (선택) 라이벌 강아지 깜짝(병맛 1컷). / [#7] (선택) 생선 친구 사이드킥(본캐 감정 에코).

### 권장 1차 PR
**#5(토글) + #1(등장) + #2(나눠 먹기)** — 핵심 관계 연출. #3(세트 룩)·#4(선물)는 후속, #6·#7은 그다음.
**캔버스/window-shift는 1차에서 건드리지 않는다**(여백 안 작게 그리기). 큰 친구가 필요해지면 별도 ADR + 양 백엔드 resize 검증.

## 반드시 지킬 제약 (위반 시 작업 무효)
- 프라이버시: 친구 시스템은 **로컬 상태 + 난수 스케줄러만**. 입력 후킹 카운터 의미 불변, 키 내용·창
  제목·타이밍 미접근. 새 네트워크 금지(허용은 `update.rs`뿐).
- 에셋은 전부 tiny-skia 코드 드로잉. 번들 이미지·새 heavy dependency 금지(필요 시 ADR 먼저 제안).
- 코어 OS-비종속(pet.rs/render.rs/menu.rs/state.rs/i18n.rs) → 양 백엔드 자동 패리티, 새 OS 코드 0.
- 모든 문구 i18n(영/한 동시). 결제/가챠/스트릭/방치 죄책감 금지.
- **성능 핵심**: 친구는 저빈도·옵션·기본 off. 평소 draw/스케줄러 비용 0, 등장 중에만 작은 draw 1~2회.

## 완료·검증 기준 (전부 통과해야 done)
1. `cargo build --release` / `cargo clippy --release` (+ `--features portable`) — 경고 0.
2. `cargo test --release` — 친구 토글 동작·Persist 라운드트립(`old_state_json_still_deserializes` 확장) +
   **"친구 on이어도 canvas_size·take_window_shift 불변"**을 toggle_panel_changes_canvas_size 패턴으로 단언.
3. `cargo run --release --example preview` — 친구 등장/나눠 먹기/세트 룩/선물 컷 PNG 육안 확인.
4. **CPU/RAM은 release에서만**: 기본 off로 신규 비용 0, 등장 중 회귀를 release에서 측정.
5. `CHANGELOG.md` `[Unreleased]`에 사용자 관점 한 줄(영문) 추가.

## 작업 방식
- 브랜치 `claude/vigilant-volta-q64b61`에서 작업. 다른 브랜치 푸시 금지. PR은 명시 요청 전까지 금지.
- 논리 단위 커밋(토글 / 등장 스케줄러 / 나눠 먹기 …). **캔버스 확장·새 입력·새 의존성은 진행 전 반드시 질문**.
- 끝나면 변경 요약 + 검증 결과(캔버스 불변·CPU 포함) + 직접 실행 못 한 항목(Windows/macOS 런타임)을 정직히 구분 보고.
