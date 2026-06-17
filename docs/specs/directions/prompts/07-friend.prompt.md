# 구현 요청 — 방향 7: 친구·관계 시스템

공통 규칙(역할·정체성·제약·검증·워크플로)은 [README](README.md) 참조.

**스펙**: `docs/specs/directions/07-friend.md` — 친구 = **로컬 두 번째 그림**(네트워크 아님). 관계는 "생선 나눔"으로 수렴.

## 범위
1. 친구 등장 스케줄러: advance에 등장(blink/zzz 패턴, 8~20분 ±랜덤) + friend_phase ease. **본캐 좌우 여백(x≈0–64/176–240) 안에 작게(scale~0.55)** → 캔버스/window-shift 불필요. `draw_scene` 프리미티브 재호출.
2. 둘이 생선 나눠 먹기: on_copy_rich에서 친구 on + 저확률(~8~12%) 시 nom 지점에서 친구도 한 입. id-미러 한 줄.
3. 커플/베프 세트 룩(방향4 연계): ACCESSORIES + draw_accessory 항목, 친구도 같은 호출. 친구 없어도 본캐 상시 착용.
4. 관계 이벤트(선물=깜짝 해금): level_up 복제한 `friend_gift()`.
5. 친구 토글: `MenuAction::ToggleFriend` + `Persist.friend_on`(serde default false). 메뉴 코어 모델 → GUI 없이 테스트.
6. (선택) 라이벌 강아지 / (선택) 생선 사이드킥.

## 권장 1차 PR
**#5(토글) + #1(등장) + #2(나눠 먹기)**. #3·#4 후속. **캔버스/window-shift는 1차에서 미변경**(여백 안 작게). 큰 친구 필요 시 별도 ADR.

## 이 방향 특이 주의
- **성능**: 친구는 저빈도·옵션·기본 off. 평소 비용 0, 등장 중에만 작은 draw 1~2회(release에서 측정).
- 테스트: "친구 on이어도 `canvas_size`·`take_window_shift` 불변"을 `toggle_panel_changes_canvas_size` 패턴으로 단언.
