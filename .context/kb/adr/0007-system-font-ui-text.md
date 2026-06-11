# ADR-0007: System font for UI text via ab_glyph (tooltip keeps pixel font)

- Status: Accepted
- Date: 2026-06-11
- Related: [ADR-0000](0000-zero-framework-rendering.md) (no asset pipeline),
  [ADR-0006](0006-i18n-vector-hangul.md) (vector Hangul, now the fallback)

## Context

The clipboard panel shows real user content — clip previews, source apps,
search queries — and the 5×7 pixel font + simplified vector Hangul that suit
the pet's decorations read poorly at that information density. The product
ask: render everything except the pet's hover stats tooltip in the **user's
system font**, without bundling font files (a Hangul-capable TTF is 1–10 MB
against a ~700 KB binary) and without platform text APIs in the core
renderer (golden rule: the core never touches OS APIs).

## Decision

- Add one dependency, **`ab_glyph`** (pure-Rust TTF/OTF parsing +
  rasterizing; pulls only `ttf-parser`/`owned_ttf_parser` and
  `ab_glyph_rasterizer`). This is the smallest maintained crate that reads
  the OS's own font files; writing a TTF rasterizer ourselves is out of
  scope, and `font-kit`/`fontdb`-style discovery stacks are far heavier.
- New core module **`sysfont.rs`**: loads the primary UI font plus a
  Hangul-capable fallback from per-OS *candidate file paths* (Segoe UI +
  Malgun Gothic; SF/Helvetica + Apple SD Gothic Neo; Noto/DejaVu + Noto
  CJK/Nanum). Discovery is plain `std::fs` over `#[cfg]` path tables — data,
  not OS API calls, same precedent as `state::today_string`.
- `sysfont::{measure, truncate_to_width, draw}` mirror the `font.rs` API
  and take the same `px` unit, so the two fonts share one layout grid
  (`EM_PER_PX` maps pixel-font cells to an em size). Glyph coverage masks
  are cached per (font, glyph, device size); blending is a manual
  premultiplied source-over (the panel/toast transforms are always
  scale+translate).
- **Fallback is per character**: a char no loaded font covers (e.g. Hangul
  on a Linux box without Korean fonts) renders via vector Hangul / the
  pixel font; if no system font loads at all, every call degrades to
  `font.rs`. Text can never disappear.
- **Surface split**: panel + toast use `sysfont`; the hover stats tooltip,
  fish badge letter and Zzz particles intentionally keep the pixel look —
  they are pet art, not information surfaces.

## Consequences

- ✅ Panel/search/clip text is rendered by the user's own UI font — native
  legibility, full script coverage wherever the OS has fonts.
- ✅ Still no bundled assets, no network: fonts are read from the local OS
  font directories at startup, once.
- ✅ Headless CI and minimal containers keep working through the fallback
  chain (tests assert both paths).
- ⚠️ +3 transitive crates and ~100–200 KB of binary; accepted as the only
  new dependency since v1.
- ⚠️ The candidate path tables can miss exotic setups (then: pixel-font
  fallback). Extend the tables rather than adding a discovery crate.
- ⚠️ `sysfont::draw` only supports scale+translate transforms; rotated text
  (fish badge) must stay on `font.rs`.
