# ADR-0006: English/Korean i18n with an in-code vector Hangul font

- Status: Accepted
- Date: 2026-06-11
- Related: [ADR-0000](0000-zero-framework-rendering.md) (no asset pipeline)

## Context

v2.0 ships a bilingual (en/ko) UI, and clip previews must render arbitrary
user text — including Hangul — inside our own tiny-skia renderer. The
built-in font was a 5×7 uppercase-ASCII bitmap. Real Korean text rendering
normally means bundling a font (a Hangul-capable TTF is 1–10 MB, dwarfing
the ~700 KB binary) plus a rasterizer crate, or platform text APIs (which
would leak OS dependencies into the core renderer).

## Decision

- **Strings**: all user-visible text goes through `i18n.rs`
  (`Lang { En, Ko }` + a `Msg` enum + format helpers). The language is a
  persisted setting; first run detects it from the OS locale via a tiny
  per-OS leaf (`state::detect_lang`, same precedent as `today_string`).
- **Hangul glyphs are composed algorithmically** in `hangul.rs`: a syllable
  is decomposed by the U+AC00 formula into initial/vowel/final jamo; ~40
  hand-authored vector stroke shapes (polylines + ovals in a unit box) are
  placed by the standard layout classes (vertical / horizontal / mixed
  vowel, optional final, double/cluster splitting) and stroked at any size.
  Compatibility jamo render standalone. ASCII stays the 5×7 bitmap font,
  extended to the full printable range (incl. lowercase); anything else
  draws a hollow-box fallback.
- Verification is visual: `cargo run --release --example preview` renders
  Hangul samples to PNG for eyeballing, plus decomposition unit tests.

## Consequences

- ✅ Zero new dependencies, zero bundled assets; binary stays tiny.
- ✅ Hangul scales with the UI (vector strokes) and matches the hand-drawn
  aesthetic; legible from ~9 px cells (panel meta text) upward.
- ⚠️ It is a "cute" font, not typography: jamo shapes are simplified and
  spacing is blockier than a real typeface. Acceptable for pet UI + previews.
- ⚠️ Only Hangul + ASCII render; other scripts (kanji, emoji, …) show fallback
  boxes in previews. The stored clip text itself is always exact — only the
  preview rendering is limited.
- ⚠️ Adding a third language means adding strings to `i18n.rs`; adding a
  third *script* means another glyph module — revisit this ADR then.
