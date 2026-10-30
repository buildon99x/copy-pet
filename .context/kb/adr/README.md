# Architecture Decision Records

Each ADR captures one significant, hard-to-reverse decision: its context, the
choice made, and the consequences. Newest decisions get the next number; supersede
rather than rewrite.

| # | Decision | Status |
|--:|----------|--------|
| [0000](0000-zero-framework-rendering.md) | Zero-framework rendering: raw platform APIs + tiny-skia | Accepted |
| [0001](0001-cross-platform-architecture.md) | Native Windows backend + portable backend, shared core | Accepted |
| [0002](0002-audio-strategy.md) | winmm audio on Windows, silent no-op elsewhere (v1) | Accepted |
| [0003](0003-portable-rendering-card.md) | Opaque "card" on the portable backend (no per-pixel alpha) | Accepted |
| [0004](0004-portable-settings-keyboard.md) | Keyboard shortcuts instead of a tray menu on portable | Accepted |
| [0005](0005-clipboard-manager.md) | Clipboard manager: native listener on Windows, arboard polling on portable, text-only | Superseded by 0014 |
| [0006](0006-i18n-vector-hangul.md) | English/Korean i18n with an in-code vector Hangul font | Superseded by 0011 |
| [0007](0007-system-font-ui-text.md) | System font for UI text via ab_glyph (tooltip keeps pixel font) | Accepted |
| [0008](0008-portable-global-hotkey.md) | Global panel hotkey on portable via rdev chord matching (Cmd+Shift+V on macOS) | Accepted |
| [0009](0009-auto-update.md) | Auto-update via GitHub releases: system curl probe, exe swap on Windows | Accepted |
| [0010](0010-movable-resizable-panel.md) | Movable, resizable clipboard panel on a dynamic union canvas | Accepted |
| [0011](0011-remove-pixel-font.md) | Remove the built-in pixel font and vector Hangul; system fonts everywhere | Accepted |
| [0012](0012-auto-paste.md) | Opt-in auto-paste on clip select via synthesized Ctrl/Cmd+V (no new dependency) | Accepted |
| [0013](0013-flyout-panel-caret-anchor.md) | Caret-anchored clipboard flyout in a separate OS window (Windows + macOS) | Accepted |
| [0014](0014-rich-clipboard-formats.md) | Rich clipboard formats (HTML/RTF) preserved by default + per-row "paste as text" | Accepted |
| [0015](0015-clipboard-tags-and-windows-images.md) | Tags and bounded PNG/DIB image history on Windows | Accepted |
| [0016](0016-local-ai-request-workbench.md) | Explicit local AI request workbench, project trays and confirmed safe copies | Accepted |
| [0017](0017-cached-illustrated-pet.md) | Cached illustrated ragdoll frames and visible-state redraws | Accepted |
| [0018](0018-selected-cat-atlases.md) | Twelve cats, bounded selected art, free gallery and quiet input | Accepted |
| [0019](0019-logical-ui-and-system-font-faces.md) | Logical macOS UI geometry, native-density text and explicit Regular faces | Accepted |
