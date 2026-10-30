# Cottonball Hunter / 솜뭉치 사냥꾼

The current implementation extends this original character to twelve companions.
See [Cat companions](cat-companions.md) for current selection and quiet-input
behavior; the original visual identity below remains the Ragdoll reference.

ClipCat's illustrated ragdoll is a quiet collecting companion. Bright blue eyes,
a seal bicolor mask with an inverted white V, broad white paws and a rounded
loaf silhouette identify the character. Fish stay badged with the source app;
the decorative art never shows copied text or image contents.

An accepted clipboard capture starts notice → crouch → paw reach, followed by
chewing, satisfaction and a return to rest. Twelve frames form this reaction;
four more provide half-blink, blink, affection and sleep. The complete copy
cycle now follows the Ragdoll's authored 1.06-second timeline; the fish
reaches its measured mouth during the open-mouth frame before satisfaction.

Body clicks and ordinary input no longer create reactions. Sleep begins after
75 seconds without a captured copy. The capture-pause badge is a separate persistent
state: sleeping does not mean capture has stopped. Pausing collection cancels
running and pending copy reactions, without deleting clips already collected.
Resume keeps the platform's existing clipboard synchronization behavior.

During a copy burst, every accepted clip updates history and counters as before,
but only one pending decorative reaction is retained. Hiding the pet clears
copy feedback; captures while hidden still enter history without queued art.
Panels, shortcuts, rich formats, images and self-copy suppression keep their
existing behavior.

## Performance and limits

The embedded PNG is decoded once with existing tiny-skia support. Sixteen
240×180 premultiplied frames occupy 2,764,800 bytes (about 2.64 MiB); source
decoding temporarily uses additional memory. No PNG decoding, file reads or
new allocations for the art occur per animation frame.

Visible-state changes trigger redraws. Static idle/sleep art stops repainting;
short fish/particle/affection transitions continue updating as needed. Panels
redraw on state changes and caret blinking. The platform's 33 ms bookkeeping
tick remains, so this release reduces rendering work rather than promising zero
timer wakeups. Clipboard polling on portable remains independent.

Generated art is a finite pose sequence rather than a continuously deformable
skeleton. Check all scales and measured mouth tracks when replacing the atlas.
Accessories are removed. See ADR-0018 and `assets/cats/README.md` for the current
source and integration contract.

## 한국어

평상시에는 느긋한 식빵 자세를 유지하고, 클립이 들어오면 발견·사냥·만족의
짧은 반응을 보여줍니다. 연속 복사도 항목은 모두 저장하며 연출 대기만 하나로
제한합니다. 수집 정지는 별도의 Ⅱ 표시로 구분하며, 정지하거나 고양이를 숨기면
남은 복사 연출을 취소합니다. 잠들어도 수집 기능은 설정대로 유지됩니다.

PNG는 한 번만 읽어 재사용합니다. 안정된 대기·수면 상태에서는 계속 다시
그리지 않으며, 클립보드 감지와 단축키 처리는 화면 갱신과 별도로 유지합니다.
