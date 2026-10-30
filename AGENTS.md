# AGENTS.md — ClipCat 작업 가이드

이 문서는 이 저장소에서 작업하는 에이전트와 기여자의 공통 지침이다.
`CLAUDE.md`도 이 파일을 가져온다. 구조나 개발 명령이 바뀌면 함께 갱신한다.
이전 문서는 [2026-10-19 아카이브](docs/archive/AGENTS-2026-10-19.md)에 원문으로 보관한다.
현재 작업 지침은 이 파일을 기준으로 한다.

## 프로젝트와 작업 원칙

ClipCat은 Rust로 만든 가벼운 데스크톱 고양이 겸 클립보드 관리자다.
복사한 내용을 로컬 히스토리에 저장하고, 복사 이벤트로 고양이 모션을 구동한다.
입력 횟수는 통계·성장에만 사용하며 키·마우스 입력으로 외형·소리·기상을 유발하지 않는다.
영어와 한국어를 지원하며, 주 대상은 Windows 네이티브 앱이다.
macOS와 Linux는 같은 코어를 사용하는 portable 백엔드로 동작한다.

- 먼저 `git status --short`로 기존 변경을 확인하고 사용자 작업을 보존한다.
- 요청 범위에 집중하고 관련 없는 리팩터링·의존성·생성 파일 변경을 섞지 않는다.
- 기존 체크아웃을 사용한다. 클라우드 작업은 이미 격리되어 있으므로 사용자가 요청하지 않으면 Git worktree를 만들지 않는다.
- 관련 코드와 테스트를 먼저 읽는다. 파일 위치는 [저장소 맵](.context/repo-map.md), 설계 이유는 [ADR](.context/kb/adr/README.md)을 참고한다.
- 렌더링·전역 입력·플랫폼 의존성·macOS 경로를 다룰 때는 [LNR](.context/kb/lnr/README.md)도 먼저 읽는다.
- 완료 보고에는 변경 내용, 실제 검증 결과, 남은 제한을 적는다. 실행하지 않은 검사나 다른 OS의 동작을 통과했다고 표현하지 않는다.

## 개인정보와 의존성

- 전역 입력 훅은 `src/input.rs`의 활동 카운터만 갱신한다. 허용되는 키코드 처리는 핫키 조합 비교 후 즉시 폐기하는 처리와 자동 반복을 막기 위한 held/released 상태뿐이다. 입력 내용·창 제목·입력 타이밍을 기록하거나 전송하지 않는다.
- 클립보드 내용과 서식은 사용자 설정 디렉터리에만 저장한다. 수집 일시정지와 재개 시 동기화 동작을 유지한다. 검증에는 합성한 테스트 문구를 사용한다.
- 커서 기반 플라이아웃은 위치·크기만 조회한다. 접근성 API로 요소의 텍스트나 값을 읽지 않는다.
- 앱의 유일한 네트워크 기능은 `src/update.rs`의 GitHub 릴리스 확인·다운로드다. 시스템 `curl`을 사용하고 `auto_update`로 끌 수 있다. 다른 네트워크 기능을 추가하지 않는다. 이 규칙은 앱 동작에 관한 것이며 개발 의존성 설치와 구분한다.
- 무거운 의존성이나 자산 파이프라인을 추가하려면 ADR을 작성한다. 아이콘·소리는 코드로 생성하고 UI 글꼴은 OS에서 읽는다. 글꼴을 번들하지 않는다.

## Architecture

- 공통 코어는 `src/*.rs`, OS 연동은 `src/platform/`에 둔다. 코어에 OS API를 도입하지 않는다. 기존 날짜·언어 감지와 업데이트의 작은 `cfg` 분기는 유지한다.
- `src/platform/mod.rs`가 백엔드 하나만 선택한다. Windows 기본은 Win32, macOS·Linux 및 Windows의 `--features portable`은 portable이다.
- `Pet`이 시뮬레이션·클립보드·패널을 조율하고 렌더링 장면을 만든다. 백엔드는 공개 `Pet` API로 이벤트를 전달하고 창·입력·클립보드를 처리한다.
- `cat.rs`는 12종의 고정 ID·개별 모션·입 위치를 정의한다. `Pet`이 선택한 한 마리의 `CatArt`를 소유하며 종류별 전역 상세 캐시를 만들지 않는다. 액세서리와 레벨 해금은 사용하지 않는다. 배포 아틀라스 제작·메모리 기준은 ADR-0018을 따른다.
- `ClipPick`은 텍스트, 서식, 붙여넣기 의도, 일반 텍스트 여부를 백엔드에 전달한다. Windows·macOS는 HTML/RTF를 보존하고 Linux는 일반 텍스트로 동작한다. Windows 네이티브는 PNG·DIB 이미지도 캡처하고 다시 복사한다. 검색·미리보기·중복 판정은 일반 텍스트를 기준으로 한다.
- Windows 네이티브와 macOS의 핫키 패널은 별도 커서 기반 플라이아웃 창이다. 가운데 클릭은 고양이 창의 내장 패널을 사용한다. Linux·Windows portable은 내장 패널을 사용한다.
- macOS 전역 입력은 `mac_input.rs`의 CoreGraphics 이벤트 탭을 사용한다. macOS 키보드 경로를 `rdev`로 되돌리지 않는다. macOS 투명 표시는 CALayer, Linux·Windows portable 표시는 불투명 softbuffer 카드다.
- macOS UI는 논리 포인트로 계산하며 `viewport::Density`를 통해 창·입력 경계에서만 물리 픽셀과 변환한다. 폰트도 배율을 적용한 해상도로 직접 래스터화한다. TTC의 첫 서체를 Regular로 가정하지 않는다(ADR-0019).

### 변경 시 지켜야 할 계약

- 앱이 클립보드에 다시 쓴 내용은 suppression marker로 정확히 한 번 건너뛴다. Windows는 클립보드 시퀀스 번호를 사용해 이미지도 처리한다. 클립 선택이 새 복사 이벤트나 생선 애니메이션을 반복 생성하면 안 된다.
- 내장 패널의 크기가 바뀌면 `canvas_size()`와 `take_window_shift()`를 함께 적용해 고양이 위치를 유지한다. 별도 플라이아웃은 고양이 창의 크기·위치를 바꾸지 않는다.
- 고양이 창은 불필요하게 포커스를 가져오지 않는다. 패널 검색에 필요한 포커스 전환과 붙여넣기 대상 앱 복원 순서를 유지한다.
- 드래그·클릭·패널 탐색·빠른 복사·크기 조절은 두 백엔드의 동작을 함께 검토한다. 메뉴는 Windows 트레이, macOS NSMenu, Linux·Windows portable 단축키의 차이를 고려한다. 공통 메뉴 항목과 동작은 `menu.rs` 및 `Pet::{build_menu,apply_menu_action}`에 둔다.
- 업데이트 자산 이름은 `.github/workflows/ci.yml`과 `src/update.rs`의 계약이다. 변경할 때 양쪽과 `Cargo.toml`의 저장소 주소를 함께 확인한다.

## 개발 환경과 명령

저장소 루트에서 실행한다. Rust stable과 Clippy가 필요하며 Windows는 MSVC를 사용한다.
Linux의 portable 빌드는 다음 시스템 패키지가 필요하다.

```bash
apt-get install libx11-dev libxi-dev libxtst-dev libxkbcommon-dev libxkbcommon-x11-dev pkg-config
```

이 클라우드 환경에 `/workspace/.clipcat-env/env.sh`가 있으면 새 셸에서 먼저 불러온다.
가상 X11 화면과 앱은 `bash /workspace/.clipcat-env/start.sh`로 시작한다.
이 경로들은 클라우드 설정용이며 저장소 자체의 필수 경로가 아니다.

```bash
cargo build --locked --release
cargo run --locked --release --bin clipcat
cargo clippy --locked --release
cargo test --locked --release
cargo run --locked --release --example preview  # /tmp/clipcat-preview에 PNG 생성
bash scripts/release.sh verify                 # 변경 이력 형식·버전 검사
```

Windows에서는 `cargo clippy --locked --release --features portable`도 실행한다.
잠금 파일 변경이 작업 범위에 포함되지 않으면 `--locked`를 유지한다.
아이콘 아트를 변경했을 때만 `cargo run --locked --bin gen_icon`으로 아이콘을 재생성한다.
성능 측정은 반드시 릴리스 빌드로 한다.

## 검증 기준

- UI 기능 구현을 마치면 실제 앱에서 사람이 사용하는 순서로 직접 사용하고 리뷰한다. 발견한 불편·오류는 바로 개선하고 영향을 받는 흐름을 다시 검증한다. 주요 화면 스크린샷과 사용 결과를 함께 보고하며, 실제 실행 화면과 렌더링 프리뷰 및 검증하지 못한 OS를 구분한다.
- 코드 변경은 릴리스 빌드·Clippy·기존 테스트를 실행한다. 문서만 바꾼 경우에는 링크·명령·차이를 확인하고 불필요한 빌드는 생략할 수 있다. 기존 경고와 이번 변경으로 생긴 문제를 구분하고 검사를 약화해 통과시키지 않는다.
- UI·렌더링 변경은 preview를 생성하고 실제 PNG를 확인한다. 고양이·생선·아이콘·한글과 영어 패널·글꼴 표시를 확인한다.
- 런타임 변경은 가능한 OS에서 앱을 직접 실행한다. Linux 헤드리스 환경은 Xvfb로 창과 합성 클립보드 문구의 수집·표시·저장을 확인할 수 있다.
- 클립 선택·붙여넣기·패널 설정 변경은 Windows에서 핫키 플라이아웃과 가운데 클릭 패널을 각각 확인한다. 설정이 Windows 트레이에서도 도달 가능한지 확인하고, 서식 유지·일반 텍스트 붙여넣기·포커스 복원·자기 복사 억제를 검증한다. 관련 함정은 LNR-0006~0008을 참고한다.
- CI는 Windows·macOS 빌드와 테스트를 수행하며 Linux 빌드 작업은 없다. Linux 관련 변경은 로컬 Linux 검증이 필요하다. 교차 타깃 `cargo check`는 해당 타깃 설치 후 사용할 수 있지만 실제 OS 실행 검증을 대신하지 않는다.

## 코드와 문서 관리

- 주변 스타일을 따르고 모듈의 `//!` 문서에 설계 이유를 남긴다. 큰 테스트 모듈은 `pet_tests.rs`처럼 형제 파일로 분리할 수 있다.
- 사용자에게 보이는 문자열은 `i18n.rs`의 번역 헬퍼를 사용한다. 영어·한국어를 렌더러나 백엔드에 직접 넣지 않는다.
- 플랫폼 분기는 `#[cfg(...)]`로 처리한다. `unsafe`는 플랫폼 FFI와 기존의 작은 바이너리 버퍼 생성 코드에 한정하고 안전 조건을 설명한다.
- `state.json`·`clips.json` 변경은 기존 저장 데이터와의 호환성을 유지한다.
- `CHANGELOG.md`에는 사용자에게 보이는 기능·동작 변경·버그 수정만 기록한다. 개발 환경·리팩터링·문서 변경은 넣지 않는다.
- 중요한 설계 결정은 ADR과 인덱스, 재발하기 쉬운 디버깅 교훈은 LNR, 제품 동작은 `docs/specs/`에 기록한다. 날짜는 절대 날짜를 사용한다.
- 릴리스 스크립트는 버전·변경 이력·커밋·태그·푸시를 변경할 수 있다. 릴리스 요청이 있을 때만 사용하고 일반 검증에는 `verify`를 사용한다.
