# Cottonball Hunter atlas

`cottonball-hunter.png` is original generated artwork created for this project
on 2026-10-30 from the user's selected ragdoll concept. It is a transparent
1536×1024 RGBA sheet with sixteen poses. It contains no clipboard content.

The first twelve poses form notice → crouch → reach → chew → recovery. The last
four are half-blink, blink, affection and sleep. `assets/cats/source-rects.json`
records the source cuts; `cat.rs` records current face anchors. The alpha gutters are not an
exact mathematical quarter-grid: do not cut this atlas into equal cells without
checking the feet and ears. Replacing the PNG requires reviewing all sixteen
frames, contours, baseline alignment and transitions together.

The original single-cat implementation retained sixteen 240×180 frames. The
current app uses the normalized twenty-frame `assets/cats/ragdoll.png`, retaining
3,456,000 bytes for the selected character (ADR-0018/0021). This larger original
sheet is not decoded per animation tick.

The byte-identical original is cataloged as `motion-copy-ragdoll` in the
[art source archive](../art/README.md). Source PNGs, available prompt records,
historical generation inputs and deterministic atlas reconstruction are linked
there; the original generation prompt for this sheet remains unrecovered.
