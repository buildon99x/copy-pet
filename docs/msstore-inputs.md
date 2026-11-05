# Microsoft Store 입력 데이터

2026-11-06 상품 페이지 최적화 제출에 사용한 입력값이다. Partner Center에 다시
입력하거나 문구를 검토할 때 이 문서와 원본 JSON을 함께 사용한다.

- [정확한 언어별 입력값 JSON](releases/msstore-aso-2026-11-06/listings.json)
- [저장 검증·제출 상태 기록](releases/msstore-aso-2026-11-06/receipt.json)
- [패키지·아이콘 제출 기록](releases/msstore-c-icon-2026-11-04/submission.json)
- [스크린샷 3장 교체 등록 기록](releases/msstore-images-2026-11-06/receipt.json)
- [출시 절차](releasing.md)

언어별 문구는 `listings.json`이 기준이며, 아래 내용은 같은 값의 읽기용 사본이다.
다음 변경 때 JSON과 이 문서를 함께 갱신하고 실제 저장값을 검증한다.

## 공통 설정과 자산

| Partner Center 항목 | 입력값 / 기준 |
|---|---|
| Product name | ClipCat |
| Product ID | 9NZBR8X5T677 |
| Package identity | PIXO.ClipCat |
| Publisher display name / Developed by | PIXO |
| Primary / secondary category | Productivity / Personalization |
| Website | https://github.com/buildon99x/copy-pet |
| Support | https://github.com/buildon99x/copy-pet/issues |
| Privacy policy | https://github.com/buildon99x/copy-pet/blob/main/docs/privacy-policy.md |
| Languages | en-US / ko-KR |
| Publishing hold | 심사 통과 후 자동 게시 |
| Existing package | clipcat-store-x64-2.5.4.msix / 2.5.4.0 |
| Platform | Windows.Desktop / x64 / minimum Windows 10.0.19041.0 |
| Store app tile | 선택한 C 아이콘, 300×300 PNG |
| Screenshots | 최신 이미지 업데이트 제출 기준 언어별 3장. 아래 교체 기록 참조 |

가격은 무료, 배포 대상은 전 세계(최초 설정 시 240개 시장)다. 이 두 항목은 기존
제출 기록 기준이며 2026-11-06 최적화에서 변경하지 않았다. 카테고리, 지원 URL,
개인정보처리방침 URL, 심사 통과 후 자동 게시 설정은 해당 작업에서 확인했다.

C 아이콘 제출 파일:
[store-c-300.png](releases/msstore-c-icon-2026-11-04/store-c-300.png).
기존 Windows 검증 화면:
[windows-native-panel.png](releases/msstore-c-icon-2026-11-04/windows-native-panel.png).
한국어 캡션을 제공하지만 해당 실행 화면 자체의 UI는 영어다.

## 필드 대응

| Partner Center | listings.json | CSV export |
|---|---|---|
| Short description | `short` | ShortDescription |
| Description | `description` | Description |
| Product features | `features` | Feature1–Feature6 |
| Keywords | `keywords` | SearchTerm1–SearchTerm7 |
| Screenshot caption | `caption` | DesktopScreenshotCaption1 |

JSON의 `en`은 CSV의 `en-us`, `ko`는 `ko-kr` 열에 대응한다.

## 영어 — en-US

### 짧은 설명

```text
Find and reuse copied text and images without leaving your workflow. Search clipboard history, pin useful clips, and paste with a shortcut—all with a quiet desktop cat.
```

### 상세 설명

```text
Find what you copied and get back to work—with a quiet desktop cat by your side.

ClipCat is a Windows clipboard manager that keeps text and image history on your device. Reuse a message, a code snippet, or an image without copying it again from the original window.

COPY. FIND. PASTE.
1. Copy text or an image as usual. ClipCat saves it to local history while your cat reacts.
2. Press Win+Shift+V to open the clipboard picker near your cursor, or middle-click your cat. If the default shortcut is unavailable, ClipCat tries Ctrl+Shift+V. The tray menu shows the active shortcut.
3. Search by content, source app, or tags. Select the clip you need to paste it into your previous app, or choose plain-text paste when you do not want formatting.

KEEP USEFUL CLIPS CLOSE
Pin frequently used items and organize them with tags. Reuse text and images, preserve HTML/RTF formatting, or paste plain text for a clean result. Pause clipboard collection when needed.

A SMALL COMPANION FOR EVERYDAY WORK
Choose from 12 illustrated cats with their own copy reactions, expressions, and sleeping poses. Gently click to pet your cat, drag it to a comfortable spot, and adjust its size or sound from the tray menu. English and Korean interfaces are included.

LOCAL HISTORY, SIMPLE SETUP
No account, advertising, analytics, or cloud sync. Clipboard history stays on your device. ClipCat does not record typing or global mouse activity. Pause collection before copying sensitive information, and delete saved items when no longer needed.

Microsoft Store manages updates. For Windows 10 version 2004 or later and Windows 11 on x64 PCs.
```

### 기능 목록

1. Open clipboard history with a global shortcut or a middle-click on your desktop cat
2. Search copied text by content, source app, or tags; pin clips you reuse often
3. Reuse text and image history, preserve HTML/RTF formatting, or paste plain text
4. Choose from 12 illustrated desktop cats with copy reactions and sleeping poses
5. Keep history on your device; pause capture and delete saved items when needed
6. Use the app in English or Korean; Microsoft Store manages updates

### 검색어

1. `clipboard manager`
2. `clipboard history`
3. `quick paste`
4. `text snippets`
5. `clipboard picker`
6. `desktop cat`
7. `copy paste utility`

### 스크린샷 캡션

```text
ClipCat’s native Windows clipboard panel with a saved text item. Search history, pin useful clips, or pause collection while your desktop cat keeps you company.
```

## 한국어 — ko-KR

### 짧은 설명

```text
복사한 텍스트와 이미지를 다시 찾아 붙여넣으세요. 클립보드 기록 검색·태그·고정·서식 없는 붙여넣기와 함께, 조용한 데스크톱 고양이 12종이 작업을 함께합니다.
```

### 상세 설명

```text
복사한 내용을 다시 찾고, 하던 작업에 바로 붙여넣으세요. 조용한 데스크톱 고양이가 함께합니다.

ClipCat은 텍스트와 이미지 복사 기록을 내 PC에 저장하는 Windows 클립보드 관리 도구입니다. 자주 쓰는 답변, 코드 조각, 이미지를 원래 창으로 돌아가 다시 복사하는 수고를 줄여줍니다.

복사 → 검색 → 선택 → 붙여넣기
1. 평소처럼 텍스트나 이미지를 복사하세요. 고양이가 반응하며 클립보드 기록에 저장합니다.
2. Win+Shift+V로 커서 근처에 선택창을 열거나 고양이를 마우스 가운데 버튼으로 클릭하세요. 기본 단축키를 사용할 수 없으면 Ctrl+Shift+V 등록을 시도합니다. 현재 단축키는 트레이 메뉴에서 확인할 수 있습니다.
3. 내용·원본 앱·태그로 검색한 뒤 필요한 항목을 선택해 이전에 사용하던 프로그램에 붙여넣으세요. 서식이 필요 없을 때는 일반 텍스트로 붙여넣을 수 있습니다.

자주 쓰는 내용을 가까이에
유용한 항목은 고정하고 태그로 정리하세요. 텍스트·이미지 기록을 다시 사용하고 HTML/RTF 서식을 유지하거나 일반 텍스트만 붙여넣을 수 있습니다. 필요할 때 클립보드 수집을 일시정지할 수 있습니다.

작업을 함께하는 작은 고양이
복사 반응, 표정, 잠자는 모습이 다른 고양이 12종을 자유롭게 선택하세요. 고양이를 가볍게 클릭해 쓰다듬거나 원하는 위치로 드래그하고, 트레이 메뉴에서 크기와 소리를 조절할 수 있습니다. 한국어와 영어 UI를 지원합니다.

기록은 내 PC에
계정·광고·분석 수집·클라우드 동기화 없이 사용합니다. 클립보드 기록은 기기에 저장하며 키 입력이나 전역 마우스 활동을 기록하지 않습니다. 민감한 내용을 복사하기 전에는 수집을 일시정지하고, 필요 없는 기록은 삭제하세요.

앱 업데이트는 Microsoft Store에서 관리합니다. Windows 10 버전 2004 이상 및 Windows 11의 x64 PC를 지원합니다.
```

### 기능 목록

1. 전역 단축키 또는 고양이 가운데 클릭으로 클립보드 기록 열기
2. 내용·원본 앱·태그로 검색하고 자주 쓰는 항목 고정
3. 텍스트·이미지 기록 재사용, HTML/RTF 서식 유지 및 일반 텍스트 붙여넣기
4. 복사 반응과 잠자는 모습이 다른 데스크톱 고양이 12종
5. 내 PC에 기록 저장, 수집 일시정지 및 저장 항목 삭제
6. 한국어·영어 UI 지원 및 Microsoft Store를 통한 업데이트

### 검색어

1. `클립보드 관리`
2. `클립보드 기록`
3. `빠른 붙여넣기`
4. `복사 붙여넣기`
5. `텍스트 조각`
6. `데스크톱 고양이`
7. `clipboard manager`

### 스크린샷 캡션

```text
저장한 텍스트를 표시하는 ClipCat의 Windows 클립보드 패널입니다. 기록을 검색하고 항목을 고정하거나 수집을 일시정지할 수 있습니다.
```

## 저장과 재입력 시 확인

- 키워드: 언어별 7개, 항목당 40자 이내, 전체 공백 구분 단어 21개 이내.
- 기능: 언어별 6개, 항목당 200자 이내. 짧은 설명은 270자 이내로 작성했다.
- Windows 기본 단축키는 Win+Shift+V이며, 등록 실패 시 Ctrl+Shift+V를 시도한다.
  트레이에 표시되는 실제 등록 단축키와 다르게 안내하지 않는다.
- 키워드 입력 위젯은 입력한 칩이 보여도 저장 후 사라지거나 중복 표시된 사례가 있었다.
  최종 제출 전 CSV를 다시 내보내 두 언어의 SearchTerm1–7을 확인한다.
- CSV로 수정할 때 Field·ID·Type 열을 유지하고, 관련 없는 문구·이미지·트레일러를
  변경하지 않는다. 이번 작업의 한국어 검색어는 공식 CSV 가져오기로 저장했다.
- 전체 CSV에는 내부 자산 URL이 포함될 수 있다. 계정 화면·전체 CSV·내부 제출 ID는
  ignored `.local/`에 보관하고 공개 레포에는 상품 문구와 공개 메타데이터만 둔다.

## 상태와 남은 작업

2026-11-06 00:06 KST 관측 기준 기존 앱은 공개 중이며, 이 문구 업데이트는
심사 전처리 2/4단계다. 승인 후 자동 게시 설정이다. 이 기록은 현재 라이브 상태를
실시간으로 보장하지 않는다.

각 언어의 스크린샷은 기존 1장이다. 실제 Windows에서 검색·항목 선택·붙여넣기 결과·
서식 비교·설정 흐름을 추가 촬영하는 작업과 한국어 UI 촬영이 남아 있다.
설치 전환 효과는 데이터 부족으로 검증하지 않았다.

## 스크린샷 교체 등록 — 2026-11-06 01:25 KST

사용자가 검토한 메인 1장과 고양이 카드 2장을 영어·한국어 상품 페이지에 같은
순서로 등록하고 업데이트 심사에 제출했다. 기존 설명·검색어 최적화는 이 작업
시작 시 이미 Store에 공개된 것으로 Partner Center에서 확인했다.

| 순서 | 등록 파일 | 크기 | 내용 |
|---|---|---|---|
| 1 | 01-main.png | 1440×2168 | 첨부 클립보드·랙돌 화면의 비율을 유지한 4배 확대본 |
| 2 | 02-cats-1.png | 1536×1024 | 랙돌·버만·샴·봄베이·라가머핀·페르시안 카드 |
| 3 | 03-cats-2.png | 1536×1024 | 터키시 앙고라·아메리칸 컬·먼치킨·브리티시 쇼트헤어·러시안 블루·턱시도 무늬 카드 |

새 캡션·원본 파일 해시·검증 범위는
[이미지 등록 기록](releases/msstore-images-2026-11-06/receipt.json)이 기준이다.
위 언어별 본문의 캡션은 이전 설명 최적화 제출 당시의 기록이며, 이번 3장 등록으로
교체됐다. 서버 CSV를 대조해 두 언어 모두 3장과 새 캡션이 저장됐고 다른 상품
정보는 유지됐음을 확인했다.

관측 상태는 **Update in certification / 전처리 2/4단계**, 승인 후 자동 게시다.
새 이미지의 공개 Store 노출은 아직 확인하지 않았다. 원본·파생 이미지와 전체
CSV·계정 화면은 ignored `.local/`에 보관하며 공개 레포에는 포함하지 않는다.

## 상품 페이지 실험 초안 — 2026-11-06 01:40 KST

`Lead image - app vs cats - 20261106`을 Partner Center에 **In draft**로 저장하고
다시 열어 이름과 이미지 순서가 유지됨을 확인했다.

| 구분 | 이미지 순서 |
|---|---|
| A 기본 페이지: 게시 완료 후 기준 | 메인 → 고양이 카드 1 → 고양이 카드 2 |
| B 저장한 실험안 | 고양이 카드 1 → 메인 → 고양이 카드 2 |

첫 이미지의 기능 중심 소개와 고양이 중심 소개가 설치 전환에 미치는 영향을
비교한다. 로고 교체는 설정하지 않았다. 초안 UI에는 트래픽 배분 설정이 노출되지
않아 실제 배분을 확인했다고 기록하지 않는다.

기본 이미지 업데이트가 심사를 통과해 **Update in publishing / 게시 4/4단계**로
진행됐다. 새 기본 페이지 공개를 확인한 뒤 실험을 제출해야 하므로 현재 실험은
미제출·미실행 상태다. 결과나 전환 개선은 아직 없다. 실험 중에는 기본 이미지와
로고를 변경하지 않고 원본·실험안의 전환율과 조회·설치 수를 함께 평가한다.

[실험 기록](releases/msstore-experiment-2026-11-06/receipt.json)과
[Microsoft 공식 안내](https://learn.microsoft.com/en-us/windows/apps/publish/product-page-experiments)를
참조한다. 이미지 파일과 계정 화면은 ignored `.local/`에만 보관한다.

## 실험 심사 제출 완료 — 2026-11-06 01:50 KST

Partner Center에서 기본 이미지 업데이트의 게시 완료와 최신 상품의 Store 제공
확인을 확인한 뒤, 저장한 실험을 제출했다. 새로고침 후에도 동일한 실험명이
**In certification**으로 유지됐다. A/B 이미지 순서는 위 초안과 같다.

기본 이미지 업데이트는 게시 완료이며, 실험은 별도 심사 중이다. 실험의 실제
트래픽 노출과 성과는 아직 확인하지 않았다. 게시 확인은 Partner Center 기준이며,
공개 Store 웹페이지에서 각 이미지가 렌더링되는지는 별도로 확인하지 않았다.
최종 상태와 검증 범위는 [실험 기록](releases/msstore-experiment-2026-11-06/receipt.json)에
갱신했다.
