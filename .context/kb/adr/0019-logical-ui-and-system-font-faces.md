# ADR-0019: Logical macOS UI geometry and explicit readable font faces

- Status: Accepted
- Date: 2026-10-31

## Problem

macOS clipboard geometry was used as physical pixels while CALayer applied the
window backing scale. A 352-unit panel was only 176 points wide on a 2x Retina
screen; glyphs also occupied half their intended point size. TTC face zero was
assumed Regular without inspecting style metadata. Secondary panel text and
opacity-faded instructions had insufficient contrast.

## Decision

Keep core canvas, panel, picker and saved panel geometry in logical units.
Portable macOS converts sizes/shifts to physical pixels only at the window
boundary and converts physical input and drag deltas back exactly once.
Mac windows begin with logical sizes; display-density changes rebuild buffers,
cancel stale drags and refit panels. Native-density rendering transforms the
whole scene before font rasterization rather than enlarging a low-resolution
bitmap. CALayer contentsScale matches the image backing density. Linux and
Windows portable retain their existing 1x boundary; native Win32 is unchanged.

System-font discovery reads bounded SFNT/TTC style metadata, chooses an upright
350–500 weight face closest to Regular400, and validates script coverage through
the existing ab_glyph parser. Fonts remain local and unbundled. No new dependency
is introduced. A diagnostic example records actual selected faces and generates
mixed English/Korean samples on the target OS.

Secondary text uses an opaque darker color, maintaining at least4.5:1 contrast
on panel backgrounds; decorative icon/animation fades remain separate. Existing
logical typography sizes and row geometry remain unchanged.

## Verification and limits

Tests cover2x panel coverage/hit regions/resize, native rasterization, physical
fit correction and real-font TTC Regular selection plus mixed-script clipping.
The macOS CI runs Apple font coverage tests, native1x/2x renderer samples, actual
app/clipboard smoke and CoreGraphics window-point geometry checks. Physical
Retina↔external-display dragging still needs interactive device verification.
The existing mac_caret use of main-screen scale for mixed-monitor AX anchors is
unchanged and approximate; this is separate from UI size/readability.

High-density pixmaps and glyph masks use additional memory. Sprite atlas caches
remain one selected cat, and stable idle/sleep invalidation remains intact.
