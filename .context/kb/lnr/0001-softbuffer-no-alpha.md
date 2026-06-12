# LNR-0001: softbuffer has no per-pixel alpha to the desktop

- Date: 2026-06-12 · Area: portable rendering

## Symptom

Plan for the portable backend was a transparent `winit` window fed by
`softbuffer`, matching the native layered-window look. But softbuffer's buffer
is `0RGB` — the top (alpha) byte is ignored — so a transparent window shows the
cat over a black rectangle, not the desktop.

## Cause

`softbuffer` presents opaque pixels (GDI BitBlt on Windows, equivalent
elsewhere). It is not a compositing surface; it cannot blend with what is behind
the window. Per-pixel desktop transparency requires either a platform layered
window (Win32-only) or a GPU compositing surface (`wgpu`), which is heavy and
platform-finicky.

## Fix

Render the pet on an **opaque rounded card** in the portable backend
(`render::render_card`), keep the transparent layered window only on native
Windows. Formalized as [ADR-0003](../adr/0003-portable-rendering-card.md).

## Takeaway

If you want a free-floating, click-through shaped window cross-platform, plan
for a GPU path from the start — softbuffer is for opaque/rectangular surfaces.
Don't assume `winit::with_transparent(true)` alone gives transparency; the
*presenter* has to support alpha too.

## Update — 2026-06-12

macOS now *does* get a transparent presenter — not via softbuffer but by
bypassing it: the pixmap is pushed to the window's `CALayer` as a `CGImage`
with alpha (`src/platform/mac_present.rs`, ADR-0003 update). The softbuffer
limitation here is unchanged; we just stopped using softbuffer on macOS. Linux
and Windows-portable still hit this wall and keep the opaque card.
