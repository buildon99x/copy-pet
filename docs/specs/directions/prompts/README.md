# 구현 작업 요청 프롬프트 (방향별 개별)

7개 방향을 각각 바로 구현에 착수할 수 있도록, 방향별로 분리한 **구현 작업 요청 프롬프트**다.
한 방향을 맡길 때 해당 파일 내용을 그대로 작업자(또는 새 Claude Code 세션/에이전트)에게 전달하면 된다.

| # | 프롬프트 | 방향 | 우선순위 |
|---|---|---|---|
| 1 | [01-accessories.prompt.md](01-accessories.prompt.md) | 악세사리 확장 | 받쳐주기 |
| 2 | [02-emotion-design.prompt.md](02-emotion-design.prompt.md) | 감정·표정 설계 | **★최우선** |
| 3 | [03-living-cat.prompt.md](03-living-cat.prompt.md) | 살아있는 고양이 | **★최우선** |
| 4 | [04-collection-season.prompt.md](04-collection-season.prompt.md) | 컬렉션·시즌 운영 | 후속 |
| 5 | [05-comfort-voice.prompt.md](05-comfort-voice.prompt.md) | 교감·위로 보이스 | 핵심 |
| 6 | [06-persona.prompt.md](06-persona.prompt.md) | 부캐 페르소나 | 후속 |
| 7 | [07-friend.prompt.md](07-friend.prompt.md) | 친구 관계 | 후속 |

## 공통 구조
각 프롬프트는 동일한 골격을 가진다: **역할 / 먼저 읽을 것(해당 스펙 + AGENTS.md) / 확정 캐릭터
정체성 / 이번 작업 범위 + 권장 1차 PR / 반드시 지킬 제약(골든룰) / 완료·검증 기준 / 작업 방식**.

## 사용법
1. 맡길 방향의 `.prompt.md` 내용을 작업자에게 전달.
2. 작업자는 `먼저 읽을 것`의 스펙 문서(`docs/specs/directions/0N-*.md`)에서 터치할 파일·함수
   (`file:line`)와 재사용 패턴을 확인한 뒤 구현.
3. 권장 착수 순서: **2·3(★최우선) → 5 → 1 → 4 → 6 → 7** (방향 2·3이 빌딩블록).

## 공통 가드레일 (모든 프롬프트에 포함)
프라이버시 로컬-only(입력은 카운터만, 키 내용·창 제목·타이밍 미사용, 네트워크는 `update.rs`뿐),
코드 드로잉 벡터아트, 코어 OS-비종속, i18n EN/KO, 두 백엔드 패리티, 신규 heavy dep 금지(ADR),
결제·가챠·스트릭·방치 죄책감 금지. 브랜치 `claude/vigilant-volta-q64b61`, PR은 명시 요청 전까지 금지.
