# Cottonball Hunter design

The user selected the quiet collecting companion, then the ragdoll Cottonball
Hunter, and authorized implementation after discussing additional animation
frames. This implements the proposed first cycle: notice → hunt → satisfaction
→ rest, alongside blink, sleep and affectionate reactions.

## Intent and choices

Use the selected cream/seal bicolor ragdoll with blue eyes, white inverted V,
large paws and rounded loaf silhouette. Maintain a theatrical hand-drawn 2D
look with simple shading. Compared approaches were a quiet companion, an
expressive emoticon actor and a mostly static mascot; the user chose the quiet
companion. Compared procedural primitive art and cached illustrated frames;
the user requested the illustrated character and additional frames.

## Behavior

- Decode a transparent 16-frame atlas once, normalize frames to a fixed canvas
  and baseline, and reuse premultiplied pixmaps. Twelve frames form the copy
  reaction; four supply blink, affection and sleep states.
- Copy collection, storage, counters and XP retain existing semantics. Visual
  bursts coalesce to bounded pending feedback and never replay an old backlog.
- Pause cancels copy feedback without removing stored clips; resume retains
  platform synchronization semantics. Sleeping and paused remain distinct.
- Keep existing source-badged fish, accessories, panel interactions, sounds,
  localization and transparency/click-through contracts. Adapt their placement
  to the new character and keep notification text above the face.
- Rendering is event-driven: stable idle and sleep frames do not continuously
  redraw, short transitions animate, panel caret and notices remain responsive.
  Do not add continuous sleep particles. Rendering throttling must not throttle
  clipboard collection, hotkeys or counters.

## Constraints and limits

No new runtime dependency or network feature. Use Rust and existing tiny-skia
PNG support. Bound cached frames below 4 MiB. No clipboard content in decorative
art. A single new art pipeline exception is documented in ADR-0017; app and tray
icons remain generated. Initial integration does not claim Windows execution
from Linux checks, or a CPU budget before release measurement.

## Verification

Tests cover timing, return to idle, burst collection vs feedback, pause
cancellation, cache size/alpha, idle redraw suppression, panels and three pet
scales. Generate previews, use the actual Linux release app with synthetic
clipboard input, capture screens/animation, compare release CPU and memory with
the baseline, and cross-check Windows native and portable targets.
