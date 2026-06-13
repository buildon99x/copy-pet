# 03. Desktop Pet Behavior Spec

## State machine
PetMood:
- Idle
- Curious
- TypingSlow
- TypingFast
- TypingExtreme
- CopyIncoming
- MouthOpen
- Nom
- Happy
- Petting
- Boop
- LevelUp
- Sleeping
- PanelOpen

## Input classification
- Count key presses, clicks, wheel events globally.
- Held key counts once; OS auto-repeat is ignored.
- Never persist keycodes, characters, window titles, or raw timing streams.
- Maintain short rolling counters only for animation intensity: 1s window for kps/cps.

## Mood transitions
- Idle -> TypingSlow when keys_per_sec 1–4.
- TypingSlow -> TypingFast at 5–9 keys/sec.
- TypingFast -> TypingExtreme at 10+ keys/sec.
- Any typing -> Idle after 700ms without key/click activity.
- Idle -> Curious after 30s; random yawn/look-around/ear-twitch every 12–18s.
- Idle/Curious -> Sleeping after 75s no input.
- Sleeping -> Idle immediately on any input.
- Any non-critical state -> CopyIncoming on new clip event.
- CopyIncoming -> MouthOpen at fish progress 72%.
- MouthOpen -> Nom on fish arrival.
- Nom -> Happy for 600ms -> previous stable mood.
- Any state -> LevelUp when threshold crossed; levelup has priority except app shutdown.

## Copy fish sequence
1. Spawn fish at top-right of pet/card union bounds.
2. Attach source badge: Windows real icon if available; portable fallback app initial/dot.
3. Flight duration 900ms, cubic ease-in-out.
4. Max queued fish: 3. New copies after queue full merge into latest fish with +N badge.
5. Mouth opens at 72% progress.
6. Arrival triggers nom sound if sound enabled, sparkles, one heart, +5 XP popup.
7. Clip is already stored when animation starts; animation failure must not lose clip.

## Petting/boop
- Single click: squash bounce, +1 XP, tiny sparkle. Debounce 250ms.
- Double click: heart burst, purr/toast, +10 XP. Double-click consumes single-click XP only once: final award should be +10, not +12.
- Right click native: context menu. Portable: no accidental pet/boop on context open.

## Dragging
- Drag pet moves anchor unless position locked.
- If panel is open, dragging pet moves both cat and panel union.
- Drag begins after movement > 4px to avoid accidental click.
- On multi-monitor disconnect, clamp pet back into visible work area.

## Sleeping
- Sleeping cat breathes scale 1.0 -> 1.025 -> 1.0 over 2600ms.
- Zzz appears every 1400ms and floats upward/fades.
- Copy event wakes cat and proceeds with fish animation.
