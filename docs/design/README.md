# ClipCat Dark Premium Desktop App UI — Complete Implementation Design Package

목표: 이 패키지만 보고 Codex 또는 Claude Code가 Rust 기반 macOS/Windows ClipCat 앱을 동일한 수준으로 구현할 수 있도록 만든다.

포함 범위:
- 다크 프리미엄 데스크탑 앱 UI 디자인 시스템
- 데스크탑 펫 UI/동작 상세 정의
- 클립보드 패널 UI 상세 동작 및 예외 처리
- Rust 렌더링/상태/입력/저장 구조 제안
- SVG 레퍼런스 에셋 + PNG 미리보기
- Codex/Claude Code 작업 프롬프트
- QA 체크리스트와 시각 회귀 검수 기준

우선순위:
1. 기존 ClipCat 철학 유지: tiny, frameless, dependency-light, local-only, no telemetry.
2. 펫이 중심이고 패널은 펫이 열어주는 보조 UI.
3. 사용자 경험: 삭제/클리어/핫키 충돌/권한 부족/캡처 중지/긴 텍스트/다국어 입력까지 예외 처리.
4. 구현 가능성: Rust immediate-mode rendering 기준. SVG는 최종 런타임 의존물이 아니라 구현 레퍼런스다.

핵심 참조 이미지:
- assets/png_preview/a_clean_dark_themed_desktop_application_ui_screens.png
- assets/png_preview/a_clean_ui_asset_sheet_reference_image_with_a_da.png
- assets/png_preview/a_clean_ui_concept_promo_infographic_scene_on_a_da.png
