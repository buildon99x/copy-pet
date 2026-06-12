# ADR-0003: Opaque "card" on the portable backend (no per-pixel alpha)

- Status: Accepted
- Date: 2026-06-12

## Context

The native Windows backend uses a layered window (`UpdateLayeredWindow`) for
true per-pixel transparency and click-through — the pet appears to float on the
desktop. The portable backend presents its CPU pixel buffer with `softbuffer`,
whose buffer format is `0RGB`: the high byte is ignored, so it **cannot deliver
per-pixel alpha to the desktop compositor**. A transparent `winit` window fed by
softbuffer renders the cat over a black rectangle (see
[LNR-0001](../lnr/0001-softbuffer-no-alpha.md)). Pursuing true transparency
portably would mean a GPU path (`wgpu`) with finicky, per-platform,
hard-to-test compositing behavior.

## Decision

On the portable backend, draw the pet on an **opaque rounded "card"**: a soft
solid background with a subtle inset frame (`render::render_card` +
`render::CARD_BG`). The window is a small, frameless, always-on-top rectangle —
it reads as an intentional desktop widget, like a sticky-note pet. The native
Windows backend keeps the fully transparent, click-through presentation.

## Consequences

- ✅ Works identically and reliably on Windows/macOS/Linux via softbuffer.
- ✅ No GPU dependency; stays in the lightweight CPU-rasterization model of
  [ADR-0000](0000-zero-framework-rendering.md).
- ⚠️ Visual divergence between backends: portable is a card, not a free-floating
  silhouette; the portable window is click-opaque over its whole rectangle.
- 🔜 A `wgpu` transparent-surface backend could restore the floating look later;
  deferred until it can be validated on macOS/Linux.

## Update — 2026-06-12: macOS gets a native transparent presenter

macOS no longer uses the opaque card. Instead of softbuffer it drives the
window's `CALayer` directly: each frame the tiny-skia pixmap (which already
carries premultiplied alpha) is wrapped in a `CGImage` and set as
`layer.contents` over a non-opaque, shadowless `NSWindow`
([`src/platform/mac_present.rs`](../../../src/platform/mac_present.rs)). This
restores the free-floating, background-transparent look of the native Windows
layered window without a GPU stack — softbuffer's `0RGB` limitation is sidestepped
by not going through softbuffer at all. The window stays interactive over its
whole rectangle (visually transparent, not click-through). It reuses crates
already in the tree (`core-graphics`, `objc`, `foreign-types` — no new graph
nodes).

**Linux and Windows `--features portable` keep the opaque card** — they have no
equivalent cheap alpha-capable presentation, and the card remains the reliable
softbuffer path there. So the portable backend is now split: transparent layer
on macOS, opaque card elsewhere.
