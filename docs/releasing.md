# 개발과 출시 브랜치

2026-10-31부터 다음 규칙을 사용한다.

- `main`: 병합된 최신 개발 코드. 출시되지 않은 변경이 포함될 수 있다.
- `production`: 최종 공개된 GitHub 정식 릴리스 태그가 가리키는 **동일한 커밋**.
- `main → production` 승격은 출시다. 별도 변경이나 머지 커밋을 production에 만들지 않는다.
- MS Store 제출은 별도의 배포 채널이다. 이 자동화는 Store 제출이나 심사 완료를 의미하지 않는다.
  Store 패키지는 production의 정확한 태그/커밋에서 만들고 제출 기록에 남긴다.

## 최초 생성

기존 정식 릴리스 태그의 커밋에서 production을 만든다. 아직 출시하지 않은 main의
HEAD에서 production을 만들어 출시된 것처럼 표시하지 않는다.

## 출시하기

main에 변경을 병합하고 원격 변경을 반영한 깨끗한 상태에서 실행한다.

```bash
git switch main
git pull --ff-only origin main
bash scripts/release.sh minor       # 또는 patch / major / 명시적 X.Y.Z
```

이 명령이 버전과 CHANGELOG를 갱신하고 main의 출시 준비 커밋과 주석 태그를 만든다.
main과 태그를 atomic push한다. 이때 production은 아직 바뀌지 않는다.
`--no-push`는 로컬 준비 전용이며 출시 완료를 의미하지 않는다.
`--allow-any-branch`는 지원하지 않는다. feature 브랜치나 production에서 출시하지 않는다.
`--skip-checks`는 로컬 검사만 생략하며 원격 CI 검사는 생략하지 않는다.

CI는 다음 순서로 동작한다.

1. 태그의 버전과 Cargo.toml 일치, main 포함 여부, production으로부터 fast-forward 및 버전 증가를 검사한다.
2. Windows·macOS 빌드, Clippy, 테스트 및 macOS 앱 스모크 검사를 통과한다.
3. 기존 공개 릴리스와 production의 일치를 확인한 후 두 OS의 배포 파일을 모아 GitHub Release를 공개한다.
   이전 공개의 승격이 실패한 상태면 같은 태그 복구만 허용하고 다른 출시는 차단한다.
4. 공개된 최신 정식 릴리스에 필요한 파일이 있는지 다시 확인하고 production을 정확히 그 커밋으로 승격한다.
5. production과 공개 태그의 커밋 일치를 재검사한다. 동시 출시는 release 작업에서 직렬화한다.

버전 태그 푸시는 **출시 승격 요청**이고, release 작업과 production 검증 성공이 출시 완료다.
태그만 있거나 CI가 실행 중이라는 이유로 출시 완료라고 보고하지 않는다.
production의 직접 push, force-push, 별도 PR 머지로 이 절차를 우회하지 않는다.

## 실패·재실행

빌드/공개 전 실패는 production을 변경하지 않는다. GitHub Release 공개와 Git ref 변경은
서로 다른 API 작업이라 한 원자적 트랜잭션으로 묶이지 않는다. 공개 직후 ref 갱신이 실패하면
CI는 실패하고 production 불일치를 알린다. 해당 태그의 CI release 작업을 재실행해 복구한다.
공개 바이너리를 교체하지 않으며, 다음 출시보다 이 복구를 먼저 수행한다.
이후 개발 커밋을 production에 추가해서 맞추려 하지 않는다.

인증된 GitHub CLI 환경에서 확인/복구할 수도 있다.

```bash
python3 scripts/production.py audit
python3 scripts/production.py check vX.Y.Z
python3 scripts/production.py promote vX.Y.Z
```

check는 참조를 조회하고 검사만 한다. promote는 GitHub 최신 정식 릴리스 확인 후에만
원격 production을 이동한다. 미공개·초안·prerelease·누락 파일·과거 버전·main 밖의 태그는 거부한다.
현재 production SHA에 대한 lease를 사용하므로 동시에 바뀐 브랜치를 덮어쓰지 않는다.
네트워크/API 권한이 없으면 우회하지 말고 GitHub CI 재실행으로 복구한다.

## 저장소 권한

production을 변경하는 CI에는 `contents: write`가 필요하다. 브랜치 보호나 ruleset을 설정할 때는
production 직접 쓰기를 제한하되 승격 작업의 쓰기는 허용해야 한다. 이 저장소의 워크플로와
로컬 스크립트만으로 관리자·강제 push까지 서버에서 차단할 수는 없다.
보호 규칙 변경에는 저장소 관리자 권한과 별도 설정이 필요하다.
