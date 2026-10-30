# Illustrated cat release atlases

Twelve original character atlases, authored from the approved individual breed
appearance sheets. British Shorthair uses the user's revised slimmer tail.
No accessories are baked into the characters. Tuxedo is a coat pattern.

Each PNG is 960×900 RGBA: four columns/five rows of 240×180 cells. Frame roles:
0 idle; 1–3 notice/anticipation; 4–7 reach/capture; 8–11 satisfaction/recovery;
12 drowsy; 13 blink; 14 contented; 15 sleep;16 eye-contact;17 affection gesture;18 contentment;19 settle. Roles are shared interchange slots,
not a requirement to reuse poses or timing. `src/cat.rs` contains independent
copy timelines and measured per-frame mouths (canvas y includes a 56px offset).
Birman and Munchkin capture on open frame 7; others on frame 6.

Original larger art stays in the task's source-art artifacts, rather than being
decoded in the app. Ragdoll derives from the previously reviewed
`../cottonball-hunter.png`; other cats have independently authored sheets.
`source-rects.json` records measured extraction rectangles and source filenames.
Given the corresponding original PNG, prepare a release atlas with:

```bash
cargo run --locked --release --example prepare_cat_atlas -- SOURCE.png DEST.png CUTS.json
```

`CUTS.json` accepts either `{"cells":[[x,y,w,h],...16]}` (used for irregular
drawn sheets) or `{"xs":[...5],"ys":[...5]}`; omit it for a regular grid.
The largest connected nonzero-alpha silhouette excludes neighboring fragments;
its bounds preserve connected AA edges and whiskers. One scale per cat
preserves relative pose size; feet are aligned after resampling at local row175.
Do not resize each pose independently. Review every frame at actual display
sizes, on bright and dark backgrounds, and along the fish path.

Runtime only decodes the selected normalized atlas. Its twenty premultiplied
frames total 3,456,000 bytes; replacement and source decode peaks are separate.
Tests guard complete frames, clear side/top margins, baseline and cache budget.
See ADR-0018 for cache ownership and gallery memory.

Dedicated original affection sources are separately authored, with measured cuts,
common per-breed scale matched to idle height, and post-resampling foot alignment.
`affection-source.json` records source names, rectangles, scale, roles and baseline.
Reproduce from the source-art artifact directory (manifest +3 PNGs + cuts):

```bash
cargo run --locked --release --example prepare_affection -- SOURCE_DIRECTORY
```

This preserves the original16 frames pixel-for-pixel and appends4 poses. The
runtime accepts20–24 complete frames, bounded below4MiB; gallery previews retain
only their existing16 reduced frames and decode thumbnails sequentially.
