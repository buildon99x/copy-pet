# ClipCat — Product & Technical Spec

Status: implemented on the feature branch,2026-10-30; native OS verification
limits are recorded in task reports. Historical pre-companionship product details
are archived in[the previous spec](../archive/clipcat-spec-pre-private-companionship.md).

ClipCat is a local clipboard manager with twelve freely selectable cat companions.
It preserves clips, rich text, Windows images, tags, pins, search, suppression,
project trays, AI request assembly, pause/resume and optional paste-back. Settings
and selected cat migrate without resetting functional user data.

No global input listener, Input Monitoring permission, input counter, activity
minute, XP, level or statistics UI is used. Clipboard watchers observe copies;
optional paste synthesizes output only. OS registered shortcuts open the panel:
Win+Shift+V (Windows), Control+Shift+V (macOS), Super+Shift+V (Linux X11). Failed
registration gives an explanation and keeps middle-click, local C and menu access.
The old macOS default migrates once; other custom chords are preserved. Command
editing inside macOS panels remains standard.

A local silhouette press/release without dragging triggers quiet breed-specific
affection, even with position lock or capture pause. Panels and transparent pixels
are excluded. Motion thresholds use logical points at the display boundary.
Four dedicated poses run for0.78–1.16s with at most one pending response. Active
copy animation ends before affection; affection precedes queued copy decoration.
Copies save immediately. Copy/affection wake the cat; other input does not.
Hide clears effects. Pausing clears existing effects and permits new affection.

One selected20-frame240×180 art cache uses3,456,000 bytes, below4MiB. Up to24
frames are supported. Gallery decoding is sequential with reduced previews.
Stable idle/sleep states stop repainting; clipboard/platform bookkeeping continues.
No new runtime network or bundled font dependency is introduced. The existing
optional GitHub update request is the only network functionality.

Detailed contracts:[companions](cat-companions.md),[clipboard tags/images](clipboard-tags-images.md),
[former AI workbench (UI removed)](ai-request-workbench.md),[approved design](../superpowers/specs/2026-10-30-private-companionship-design.md),
[ADR-0021](../../.context/kb/adr/0021-local-affection-and-private-companionship.md).
