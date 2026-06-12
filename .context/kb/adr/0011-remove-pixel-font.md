# ADR-0011: Remove the built-in pixel font and vector Hangul; system fonts everywhere

- Status: Accepted
- Date: 2026-06-12
- Supersedes: [ADR-0006](0006-i18n-vector-hangul.md) (vector Hangul);
  updates [ADR-0007](0007-system-font-ui-text.md) (the "tooltip keeps the
  pixel font" split no longer exists)

## Context

ADR-0007 split text rendering: panel/toast used the system font
(`sysfont.rs`), while the pet's decorations — the hover **stats bubble**,
fish badge letters, Zzz — kept the in-code 5×7 pixel font (`font.rs`) and
vector Hangul (`hangul.rs`), which also served as sysfont's per-character
fallback. The product owner asked for the stats bubble to use the normal
system font and for the custom font code to be **removed entirely** (~750
lines of glyph tables + jamo composition that existed only for those
surfaces and the fallback).

## Decision

- Delete `font.rs` and `hangul.rs`. Every drawn string goes through
  `sysfont` (bubble, toast, panel, fish badge letter, Zzz).
- sysfont's fallback for characters no loaded font covers — or for systems
  where no known font file exists at all — is a minimal hollow **tofu box**
  drawn from four rects (same 5×7 cell metrics, so layout/truncation stay
  sane). It is deliberately not a font.
- The fish badge letter used to rotate with the fish via the pixel font;
  sysfont rasterizes only under scale+translate transforms, so the letter
  now draws **upright** at the transformed chip center (the chip is a
  circle — an upright letter reads as intentional). Under a genuinely
  rotated transform sysfont draws tofu (nothing uses that path).
- The `px` unit (one pixel-font pixel, cell = 7*px) survives as sysfont's
  sizing unit so no call-site layout changed.

## Consequences

- ✅ One text pipeline; the stats bubble matches the panel typographically;
  ~750 lines and a whole glyph subsystem gone.
- ✅ Windows/macOS always have the candidate fonts; mainstream Linux has
  Noto/DejaVu. Korean now renders as real typography wherever a Korean
  font exists.
- ⚠️ On a fontless system (minimal containers) *all* text is tofu boxes —
  previously the pixel font kept text legible. Accepted trade-off per the
  product owner's "remove it completely"; `sysfont::available()` reports
  the situation and tests/previews tolerate it.
- ⚠️ On Linux without a Korean font, Hangul is tofu (it used to render via
  vector Hangul). The fix is installing any Korean font, which the
  candidate list picks up.
