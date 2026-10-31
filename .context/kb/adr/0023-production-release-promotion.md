# ADR-0023: 출시된 커밋만 production으로 승격

날짜: 2026-10-31. 상태: Accepted (사용자 지정 브랜치 계약).

main에는 최신 개발 작업을 병합하고 production은 최종 공개 정식 릴리스 태그와 같은
커밋을 가리킨다. production 선이동 후 빌드하면 실패 시 미출시 커밋이 남으므로 채택하지 않는다.
main의 주석 버전 태그를 승격 요청으로 사용하고 기존 Windows/macOS CI를 재사용한다.
빌드·검사·공개 확인 후 CI가 production을 fast-forward하고 SHA 일치를 검사한다.
공개와 ref 갱신은 원자적이지 않으므로 중간 실패는 실패로 보고하며 동일 태그 재실행으로 복구한다.
직접 쓰기 차단은 별도 GitHub 브랜치 보호/ruleset 권한이 필요하다.
MS Store는 별도 채널이며 그 제출 패키지도 production 기준으로 만든다.

세부 운영: [출시 지침](../../../docs/releasing.md).
