# QA Checklist

## Privacy
- [ ] No key characters are read or stored.
- [ ] Clipboard text stays local.
- [ ] Update check is optional and sends no telemetry.

## Clipboard
- [ ] Empty/whitespace ignored.
- [ ] >256KB ignored with warning toast.
- [ ] Re-copy bumps existing clip and preserves pin.
- [ ] Copy-back suppresses exactly one matching event.
- [ ] Pinned/unpinned capacity rules work.

## Panel
- [ ] Hotkey toggles panel.
- [ ] Fallback hotkey displayed if clash.
- [ ] Search supports Korean.
- [ ] IME composition does not trigger shortcuts incorrectly.
- [ ] Ctrl/Cmd+0..9 quick copies correct visible rows.
- [ ] Ctrl/Cmd+Z restores delete and clear.
- [ ] Esc layer order works.
- [ ] Auto-close off keeps panel open for all copy paths.

## Pet
- [ ] Idle/curious/sleep transitions match timing.
- [ ] Typing intensity changes paw speed.
- [ ] Fish animation plays on copy and never blocks storage.
- [ ] Queue max 3 and overflow merge works.
- [ ] Level up interrupts correctly and returns to stable state.

## Visual
- [ ] Dark premium surfaces match tokens.
- [ ] No text clipping in EN/KO.
- [ ] Cat anchor stable when panel opens/resizes.
- [ ] Rounded borders crisp at 1x/2x.
- [ ] Source badge colors consistent between fish and rows.

## Platform
- [ ] Windows RegisterHotKey fallback.
- [ ] macOS Accessibility missing state.
- [ ] Window clamp after monitor change.
- [ ] Graceful shutdown saves state/clips.
