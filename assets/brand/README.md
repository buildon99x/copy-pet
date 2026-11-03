# ClipCat C icon assets

Selected by the user on 2026-11-04: a Ragdoll leaning out of a terracotta document
pocket. [Design](../../docs/specs/brand-icons.md) · [Original/request catalog](../../art/catalog.json).

- `windows-1024.png`: transparent Windows master.
- `macos-1024.png`: the same foreground on a warm macOS tile.
- `windows/icon-*.png`: native shell size previews.
- `ClipCat.iconset/`: standard macOS iconset, 16px through 1024px Retina.
- `selected-c-256.png`: the only brand raster embedded in the executable.
- `icon-review.png`: light/dark size comparison, a render preview.
- `../clipcat.ico`, `../clipcat.icns`: native packaged icons.

Regenerate with `cargo run --locked --release --bin gen_icon` from the repo root.
ICNS creation uses Apple's iconutil on macOS. Windows/Linux can regenerate the
ICO/PNGs/iconset; use macOS to update ICNS. Code exports from the preserved full
master, not from the runtime 256px derivative. No font is bundled.

ImageGen created the selected foreground; Rust exports native assets and the
macOS tile. Exact prompts are in `art/prompts/icon-brand-2026-11-04/`. The original
is unchanged. No new public release or replacement Store submission is implied.
