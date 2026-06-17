# 구현 요청 — 방향 1: 악세사리 확장

공통 규칙(역할·정체성·제약·검증·워크플로)은 [README](README.md) 참조.

**스펙**: `docs/specs/directions/01-accessories.md`

## 범위
1. 신규 코드드로잉 악세사리 8~12종(리본·꽃·토끼귀·베레모·후드·담요 망토·안대·**생선 모자**): `Accessory`+`from_id`+`draw_accessory` arm + `ACCESSORIES` 행 + preview.
2. 컬러 변형: `draw_accessory` 색 파라미터, `lighten`/`darken` 파생, `Persist.accessory_tint`(serde default 0).
3. 앞발(paw) 소품: `draw_paw` 좌표 기준 미니 생선/커피/하트 팻말.
4. 시즌 룩: `today_string` 로컬 날짜로 메뉴 노출만(해금 후 영구 착용, FOMO·streak 금지).
5. 멀티 슬롯(hat/face/neck/paw): `Persist` 신규 필드(serde default), `SetAccessory`→`SetSlot` 일반화.
6. preview·i18n(신규 메뉴 라벨 EN/KO).

## 권장 1차 PR
스키마 변경 없는 **#1 + #6**. #5(멀티 슬롯)·#2(컬러)는 `Persist`/메뉴 변경 → 별도 PR, 스키마안 먼저 질문.

## 이 방향 특이 주의
- `ACCESSORIES` 테스트(`every_accessory_has_a_reachable_level`)가 신규 행을 자동 검증 — 레벨·이름만 맞추면 통과.
- 생선 모자=셀프 패러디, 앞발 미니 생선=화폐 단위, 하트 팻말=id-미러 소품화로 정체성 연결.
