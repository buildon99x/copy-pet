# Cottonball Hunter atlas

`cottonball-hunter.png` is original generated artwork created for this project
on 2026-10-30 from the user's selected ragdoll concept. It is a transparent
1536×1024 RGBA sheet with sixteen poses. It contains no clipboard content.

The first twelve poses form notice → crouch → reach → chew → recovery. The last
four are half-blink, blink, affection and sleep. `hunter_art.rs` stores measured
cell separators, normalization and face anchors. The alpha gutters are not an
exact mathematical quarter-grid: do not cut this atlas into equal cells without
checking the feet and ears. Replacing the PNG requires reviewing all sixteen
frames, anchors and accessory previews together.

The runtime uses existing tiny-skia PNG decoding and retains only sixteen
240×180 premultiplied frames (2,764,800 bytes), dropping the source atlas and
temporary tiles after loading. No image processing step runs per animation tick.
See ADR-0017 for the narrowly scoped illustrated-art exception.
