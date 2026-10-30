# 고양이 동료 / Cat companions

All twelve characters are freely selectable from the first launch. Open
**고양이 선택 / Choose a cat** in the Windows tray/right-click menu or macOS
context menu. On portable, focus the cat and press `A`. Browse static cards,
explicitly preview Idle/Copy/Sleep (keys `1`/`2`/`3`), then choose **함께하기 /
Apply** or Enter. Arrow keys browse, Escape closes without applying.

| Character | Distinguishing shape and acting |
| --- | --- |
| 랙돌 / Ragdoll | Blue eyes, white inverted V, fluffy low hunter; crouch and gather |
| 버만 / Birman | Seal mask, four white gloves; neat observation and paired paws |
| 샴 / Siamese | Wedge face, large ears, slim long legs; ears and neck lead the reach |
| 봄베이 / Bombay | Black short coat, copper eyes, muscular body; low precise capture |
| 라가머핀 / RagaMuffin | Broad tabby-and-white fluff, hazel eyes; scoop toward the chest |
| 페르시안 / Persian | Cream cobby longhair, short muzzle, small ears; patient inspection |
| 터키시 앙고라 / Turkish Angora | Slender white silky coat, green eyes; lifted graceful paw |
| 아메리칸 컬 / American Curl | Backward-curled ears, tabby coat; curious tilt and paw check |
| 먼치킨 / Munchkin | Adult-length ginger body with short legs; eager short-paw taps |
| 브리티시 쇼트헤어 / British Shorthair | Round cheeks, dense blue coat, copper eyes; composed single paw and revised slim tail |
| 러시안 블루 / Russian Blue | Slender silver blue, green eyes and large ears; deliberate fine-paw pull |
| 턱시도 무늬 / Tuxedo pattern | Black-and-white bib/socks; lifted white paw and sly expression |

Tuxedo is a coat pattern, not a breed. Appearance descriptions explain the
chosen representative design; the persona is fictional and does not claim that
all real cats of a breed behave alike.

## Quiet events

Typing, scrolling, other applications' mouse activity and hovering do not
change the cat or wake it. No input counts, activity statistics, XP or levels
are collected. Local down/up on the visible silhouette, with at most3 logical
points movement, plays four dedicated eye-contact/gesture/contentment/settle
poses. Panel/blank-area presses, drags and unpaired releases are excluded.
Position lock still allows affection; collection pause allows a new local response.
Each breed's quiet response lasts0.78–1.16 seconds, with at most one pending
response. An active copy finishes before affection; affection precedes queued
copy decoration. Clips save immediately regardless of decorative priority.

A captured clipboard item wakes the selected cat immediately. Its authored
notice/capture/recovery sequence lasts roughly 0.83–1.19 seconds; individual
timelines and measured mouth tracks keep the fish aligned with that character.
Blinking is sparse; after 75 seconds without a capture or local affection the cat becomes drowsy
and sleeps, even if typing continues. Sleep does not disable collection.

Every captured clip is stored while decorative playback retains at most one
pending reaction. Pause or hide cancels the fish, queue, particles and
compression immediately. Hidden collection does not replay a backlog on reveal.
The pause badge is a separate functional state, independent of sleeping.

## Persistence and performance

The stable string `cat_kind` defaults to `ragdoll` when missing, unknown or
malformed. Old accessory values are ignored. Selecting another character preserves clips,
tags and AI projects retain their separate stores. Legacy sound mode 2 becomes
event sounds only. Accessories and level unlocks are removed.

Only one detailed twenty-frame 240×180 cache is retained (3,456,000 bytes /
3.30MiB). The gallery temporarily adds static thumbnails (419,904 bytes) and one
reduced preview (691,200 bytes). Atlas decoding happens during selection, never
per animation frame; a ready replacement swaps with the old cache. Source PNG
and replacement decoding temporarily need additional memory. There are no
per-breed global caches or runtime asset downloads.

Stable idle/sleep frames stop repainting. Platform bookkeeping still runs at
33ms and portable clipboard polling remains independent. See
[ADR-0018](../../.context/kb/adr/0018-selected-cat-atlases.md) and
[the asset contract](../../assets/cats/README.md).

## 한국어 사용 요약

메뉴의 **고양이 선택**에서12종을 비교하고 **함께하기**로 적용합니다.
통계·XP·레벨·전역 입력 관찰은 사용하지 않습니다. 고양이 몸을 누른 뒤
드래그 없이 놓으면 짧게 반응하고 잠에서 깹니다. 위치 잠금·수집 일시정지
상태에서도 쓰다듬을 수 있습니다. 패널·투명 영역·다른 앱 입력은 제외하며,
숨기면 남은 반응을 지웁니다. 기록·태그·프로젝트는 유지합니다.
