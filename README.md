# 🐱 DeskCat — 데스크탑 타이핑 컴패니언

키보드와 함께 자라는 데스크탑 고양이. Bongo Cat처럼 여러분의 타이핑을 따라 치고,
열심히 일할수록 레벨이 올라 새 액세서리를 잠금해제합니다.

**Windows · macOS · Linux** 지원 — Windows는 완전 투명·클릭 통과 네이티브 빌드,
macOS/Linux는 동일한 코어를 공유하는 portable 빌드로 동작합니다.

![DeskCat](assets/screenshot.png)

## 특징

- **봉고캣 코어 루프** — 키보드/마우스 입력을 전역으로 감지해 고양이가 앞발로 키보드를 따라 칩니다.
- **성장 시스템** — 키 입력 1회 = 2 XP, 클릭/스크롤 = 1 XP. 레벨업하면 액세서리가 잠금해제되고 자동 장착됩니다.
- **생산성 통계** — 고양이에 마우스를 올리면 오늘의 키 입력 / 클릭 / 활동 시간이 말풍선으로 표시됩니다.
- **살아있는 애니메이션** — 호흡, 깜박임, 꼬리 흔들기, 빠른 타이핑 시 땀방울, 75초 이상 자리를 비우면 잠들기(Zzz), 더블클릭으로 쓰다듬기(하트).
- **방해 없는 디자인** — 항상 위에 떠 있지만 포커스를 훔치지 않고, 투명한 부분은 클릭이 통과합니다.
- **가벼움** — 단일 exe (~600KB), 메모리 ~11MB, CPU ~3% (유휴 시 더 낮음). 설치 불필요.

## 레벨 보상

| 레벨 | 잠금해제 |
|-----|---------|
| 2  | 빨간 목도리 |
| 3  | 동그란 안경 |
| 5  | 파란 비니 |
| 7  | 헤드폰 |
| 10 | 황금 왕관 |
| 15 | 마법사 모자 |

## 조작법

| 동작 | 효과 |
|------|------|
| 드래그 | 위치 이동 |
| 클릭 | 콩— (살짝 눌림) |
| 더블클릭 | 쓰다듬기 (+10 XP, 하트) |
| 마우스 올리기 | 오늘의 통계 말풍선 |
| 우클릭 (Windows 네이티브) | 설정 메뉴 (크기 · 액세서리 · 소리 · 위치 잠금 · 자동 실행 · 초기화) |
| 트레이 아이콘 클릭 (Windows) | 고양이 숨기기/보이기 |

### portable 빌드 (macOS / Linux) 키보드 단축키

시스템 트레이 대신 키보드로 설정합니다 (창을 클릭해 포커스를 준 뒤):

`S` 크기 · `A` 액세서리(잠금해제된 것만) · `M` 소리 · `B` 통계 고정 · `L` 위치 잠금 · `Q`/`Esc` 종료

## 플랫폼별 차이

| | Windows (네이티브) | macOS / Linux (portable) |
|---|---|---|
| 창 | 완전 투명 · 클릭 통과 (레이어드 윈도우) | 불투명 "카드" 위에 표시 (softbuffer는 픽셀 단위 투명 미지원) |
| 설정 UI | 트레이 우클릭 메뉴 | 키보드 단축키 |
| 소리 | winmm 합성음 | (v1은 무음) |
| 전역 입력 | `WH_*_LL` 훅 | `rdev` (macOS는 손쉬운 사용 권한 필요, Linux는 X11에서만) |

자세한 설계 배경은 [`.context/kb/adr/`](.context/kb/adr/), 스펙은
[`docs/specs/deskcat-spec.md`](docs/specs/deskcat-spec.md) 참고.

## 빌드

```bash
# 기본 백엔드 (Windows=네이티브, macOS/Linux=portable)
cargo build --release          # 실행 파일: target/release/deskcat[.exe]

# Windows에서 portable 백엔드를 테스트
cargo build --release --features portable

# 아이콘 재생성 (render.rs 아트 변경 후)
cargo run --bin gen_icon
```

요구 사항: Rust (Windows는 MSVC 툴체인). macOS/Linux는 portable 스택용 시스템
라이브러리가 필요합니다 — 정확한 목록은 [CI 워크플로](.github/workflows/ci.yml)의
*Install Linux system dependencies* 단계를 참고하세요. 세 OS 모두에서의 빌드는
CI(GitHub Actions)가 검증합니다.

## 데이터

통계와 설정은 OS별 설정 디렉터리의 `state.json`에 저장됩니다:
Windows `%APPDATA%\DeskCat`, macOS `~/Library/Application Support/DeskCat`,
Linux `$XDG_CONFIG_HOME/DeskCat`. Windows의 "시작 시 자동 실행"은 `HKCU\...\Run`
레지스트리 키를 사용합니다.

## 기술 노트

- GUI 프레임워크 없음 — [tiny-skia](https://github.com/linebender/tiny-skia) 벡터 렌더링 + 각 OS API 직접 사용
- 플랫폼 분리: 네이티브 Win32 백엔드(레이어드 윈도우·트레이) / portable 백엔드(`winit` + `softbuffer` + `rdev`), 공통 코어(`pet`, `render`, `state`)
- 전역 입력은 키 내용을 읽지 않고 **횟수만** 셉니다 (`src/input.rs`의 원자 카운터)
- 효과음·아이콘·폰트 모두 코드에서 생성 — 번들 에셋 없음 (단일 바이너리)

## 프라이버시

DeskCat은 어떤 키가 눌렸는지 **기록하지 않습니다**. 입력 훅 콜백은 이벤트 횟수를
세는 원자 카운터만 증가시키며, 네트워크 통신이 전혀 없습니다.
