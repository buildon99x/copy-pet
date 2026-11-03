# ClipCat Microsoft Store submission

2026-11-04 00:06 KST에 내장 브라우저로 v2.5.3.0을 심사 제출했다.
현재 확인한 상태는 **In certification / Pre-processing (2 of 4)** 이며,
Microsoft 심사 통과나 Store 공개 완료를 의미하지 않는다. 통과 후 자동 공개한다.

- [공개 릴리스와 MSIX](https://github.com/buildon99x/copy-pet/releases/tag/v2.5.3)
- [제출 기록](submission.json), [패키지 출처·해시](build-info.json)
- [실제 Windows 화면](windows-native-panel.png)
- [Windows 실행 검사 결과](windows-smoke.json)

계정 UI가 포함된 접수 화면과 내부 제출 ID는 공개 저장소에 올리지 않고
로컬 `.local/msstore-2026-11-04/`에 보관한다.

가격은 무료, 배포 지역은 전세계, 대상은 Windows Desktop x64이다.
영어·한국어 설명, 실제 실행 스크린샷, 개인정보처리방침, 연령 등급,
runFullTrust 사용 사유를 제출했다. 연령 등급 발급의 게시자/이메일 공유와
IARC 약관·성년 확인은 사용자에게 각각 승인받았다. 선택적인 만족도 설문은
보내지 않았다.

스크린샷은 공개 MSIX의 해시를 확인한 다음 그 안의 동일 exe를 CI의 Windows
데스크톱에서 실행해 촬영했다. 배경 터미널을 최소화했으며 합성 문구만 사용했다.
이미지에 Windows CI의 바탕화면과 테스트 모드 표기가 포함된다. 앱 화면을
생성 이미지로 바꾸거나 다른 OS 화면을 Windows 화면으로 표시하지 않았다.

Windows SDK 패키지 검증, Store 기능 빌드/Clippy/테스트, 실제 Win32 실행과
클립보드 수집·저장은 통과했다. MSIX 설치/삭제, Store 업데이트 및 WACK는
실행하지 않았다. 공개 여부·승인/반려는 Partner Center에서 후속 확인한다.
