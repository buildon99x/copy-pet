# 구현 작업 요청 — 방향 3: "살아있는 고양이" 아이들 미세행동 & 매력 ★최우선

## 역할
너는 ClipCat(러스트 초경량 클립보드 매니저 데스크탑 펫) 저장소에서 작업하는 시니어 러스트
엔지니어다. 아래 기획 문서를 바탕으로 "가만히 둬도 살아있는" 고양이의 모션·행동 디테일을 구현한다.

## 먼저 읽을 것 (반드시 정독)
- `AGENTS.md` (골든룰·아키텍처·검증 절차 — 단일 진실 공급원)
- `docs/specs/directions/README.md` (7개 방향 개요 + 확정 캐릭터 정체성)
- `docs/specs/directions/03-living-cat.md` ← 이번 작업 핵심(터치할 파일·함수 file:line, 재사용 패턴)
새 코드 전에 기존 골격(happy/sleep/squash/tail_phase float, blink ±랜덤, 호흡 bob, fish/nom 시퀀스, rand_f)을 먼저 재사용하라.

## 확정 캐릭터 정체성 (척추 — 흔들지 말 것)
"생선 본위제 먹보 고양이". 게으른 먹보 본캐 불변 — idle일수록 졸림·늘어짐, 먹이 오면 달려듦.
모든 모션은 "고정 정체성 × 예측 불가 표현(±랜덤 타이밍)"의 갭모에(볼매)를 반복 피로 없이 굴린다.

## 이번 작업 범위
1. [#1] 아이들 행동 풀(소형 상태기계): 그루밍/기지개/하품/두리번/꼬리 쫓기, idle·sleep 낮을 때 수 초 ±랜덤.
   기존 frame/rng/last_event 기반. Scene에 head_yaw/stretch 등 파생 필드 합성.
2. [#2] 보조동작: 귀 씰룩·수염 떨림(hover)·호흡 bob 강화 — 전부 phase+sin.
3. [#3] 팔로스루/오버랩: 몸 squash 후 꼬리·귀 15~25% 지연(1차 lerp 관용구).
4. [#5] 클릭/더블클릭 반응 확장: 쓰다듬기 wiggle + 하트, 연타 점증 반응(반응 강화는 코어만 → 자동 패리티).
5. [#4] 이스터에그(황금 생선): on_copy에서 rand_f 저확률 게이트로 골드 badge, nom 시 과장 환희(기존 fish/파티클 재사용).
6. [#6] 로컬 일주기: `today_string`과 같은 패턴의 `local_hour()` leaf(시만 반환) 추가, 밤엔 더 졸리게 + 은은한 틴트 1장.

### 권장 1차 PR
코어만으로 자동 패리티인 **#1·#2·#3(아이들·보조동작·팔로스루)**. #5(이스터에그)·#6(일주기)은 후속.
#5에서 **새 입력 제스처(hold-to-pet 등)를 도입하면 양 백엔드(windows.rs/portable.rs) 동일 임계로 구현** 필요 — 도입 전 질문.

## 반드시 지킬 제약 (위반 시 작업 무효)
- 프라이버시: 입력 후킹은 카운터만. 키 내용·창 제목·타이밍 읽기/저장/로그/전송 금지. 행동·연출
  선택은 로컬 상태(감정 float, rng, 로컬 시각 hour)만 — clips/source 문자열을 분기에 쓰지 말 것. 새 네트워크 금지(허용은 `update.rs`뿐).
- 에셋은 전부 tiny-skia 코드 드로잉. 번들 이미지·새 heavy dependency 금지(필요 시 ADR 먼저 제안).
- 코어 OS-비종속(pet.rs/render.rs/state.rs). 두 백엔드 패리티 — 항목 5의 새 제스처만 양쪽 구현.
- 모든 사용자 문구 i18n(영/한 동시). 결제/가챠/스트릭/방치 죄책감 금지.

## 완료·검증 기준 (전부 통과해야 done)
1. `cargo build --release` / `cargo clippy --release` (+ `--features portable`) — 경고 0.
2. `cargo test --release` — 기존 스모크(render_panel_open_smoke, fish_flies_and_gets_eaten) 회귀 + 신규
   idle 상태기계/일주기 임계/황금 생선 게이트(rng만 보는지)에 결정적 단위 테스트.
3. `cargo run --release --example preview` — idle 행동/틴트/황금 생선 프레임 PNG 육안 확인.
4. **CPU는 release에서만 측정**(디버그 ~10× 느림): idle 시 수 %, RAM ~12–16MB 유지. 깊은 수면(sleep>0.9)
   프레임 스킵 로직 보존, idle 행동은 sleep 낮을 때만.
5. `CHANGELOG.md` `[Unreleased]`에 사용자 관점 한 줄(영문) 추가.

## 작업 방식
- 브랜치 `claude/vigilant-volta-q64b61`에서 작업. 다른 브랜치 푸시 금지. PR은 명시 요청 전까지 금지.
- 논리 단위 커밋(아이들 풀 / 보조동작 / 팔로스루 …). 새 입력 제스처·캔버스 변경은 진행 전 질문.
- 끝나면 변경 요약 + 검증 결과(빌드/clippy/test/preview/CPU) + 직접 실행 못 한 항목(Windows/macOS 런타임)을 정직히 구분 보고.
