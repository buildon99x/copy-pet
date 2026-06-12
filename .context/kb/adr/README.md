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
| [0005](0005-clipboard-manager.md) | Clipboard manager: native listener on Windows, arboard polling on portable, text-only | Accepted |
| [0006](0006-i18n-vector-hangul.md) | English/Korean i18n with an in-code vector Hangul font | Accepted |
| [0007](0007-system-font-ui-text.md) | System font for UI text via ab_glyph (tooltip keeps pixel font) | Accepted |
| [0008](0008-portable-global-hotkey.md) | Global panel hotkey on portable via rdev chord matching (Cmd+Shift+V on macOS) | Accepted |
| [0009](0009-auto-update.md) | Auto-update via GitHub releases: system curl probe, exe swap on Windows | Accepted |
