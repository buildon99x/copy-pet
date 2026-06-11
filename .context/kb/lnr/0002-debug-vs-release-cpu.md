# LNR-0002: Debug tiny-skia is ~10× slower — benchmark CPU on release

- Date: 2026-06-12 · Area: performance

## Symptom

The portable backend appeared to burn ~50% of a core (21.8 s CPU over 44 s).
First read: the winit event loop is busy-spinning or softbuffer present is
expensive. Nearly panicked into "optimizing" the event loop.

## Cause

It was a **debug build**. tiny-skia's anti-aliased vector rasterization is
heavily optimization-dependent; unoptimized it is roughly an order of magnitude
slower. The native backend showed the same effect: ~34% CPU in debug vs ~3% in
release. The portable release build measured **0.47 s over 12 s (~4%)** — fine.

## Fix

Always measure CPU/perf on `cargo build --release`. Debug numbers for this app
are meaningless for performance decisions.

## Takeaway

Before "fixing" a performance problem in a tiny-skia (or any rasterizer-heavy)
app, confirm you're on a release build. Keep the redraw-skip optimizations
(skip frames when fully asleep & idle, only present on the redraw hint) — they
help, but they are not what caused the scary debug number.
