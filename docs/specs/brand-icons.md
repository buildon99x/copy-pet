# ClipCat app icon identity

Date: 2026-11-04. Scope: Windows EXE/tray/Store and macOS Finder/Dock.

The user rejected the first code-drawn cat-shaped clipboard and selected **C**
from the three-image comparison: a relaxed bicolor Ragdoll leaning out of a
terracotta document pocket. Two ivory cards communicate collecting copied notes;
blue eyes and a dusty-blue corner connect to the original cat/copy-event palette.
A draped front paw and curled tail express quiet companionship. No lettering,
badges, or copied content appears inside the icon.

The selected foreground is a preserved transparent ImageGen original. Its exact
request and comparison reference are recorded in art/catalog.json. Existing
Rust/tiny-skia code exports every size; no image library, network feature or font
bundle is added. Only a 256px derivative is embedded for startup/tray rendering;
the full original stays in art/ and is never bundled in the app.

Windows uses the transparent foreground silhouette. macOS places the same artwork
on a warm cream rounded tile with a restrained contact shadow. Export from the
full original at every resolution, with bicubic filtering; do not enlarge a small
PNG into the 1024px master. Fine fur detail naturally reduces to broad shapes at
16/24px. The cat and terracotta pocket remain the recognition cues at those sizes.

Deliverables: Windows multi-resolution ICO (16–256px), a 1024px Windows master,
macOS 16px–1024px iconset and ICNS, a 1024px macOS master, and a light/dark review
sheet. Store logos share this foreground and include scale/target-size variants.
The macOS packager copies ICNS into Resources and sets CFBundleIconFile before
signing. ICNS packaging preserves compatibility with macOS 11.

Validate exported alpha, dimensions, ICO/ICNS entries and macOS bundle wiring.
Review large and small renders separately. Existing Store certification is not
cancelled or replaced by this work; these assets ship in a future release.

References:
- https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-construction
- https://developer.apple.com/design/human-interface-guidelines/app-icons
- https://developer.apple.com/documentation/xcode/configuring-your-app-icon
