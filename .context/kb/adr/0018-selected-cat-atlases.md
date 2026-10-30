# ADR-0018: Twelve cats, one detailed animation cache

> Input, growth, body-click behavior and frame/cache counts are superseded by
> [ADR-0021](0021-local-affection-and-private-companionship.md).

- Status: Accepted
- Date: 2026-10-30
- Supersedes the single-cat restriction in ADR-0017.

## Context

The user replaced accessories with twelve freely selectable characters and
approved individual appearance sheets. Breeds must differ in anatomy, pose,
expression and copy action. Frequent keyboard/mouse reactions are unwanted.

## Decision

Embed a normalized 960×720 RGBA PNG per character, with sixteen 240×180 frames.
The offline Rust `prepare_cat_atlas` example accepts measured source rectangles,
uses one scale per character, and aligns visible feet after resampling. Original
large artwork is retained outside the executable. No new dependency, runtime
network access, GPU, or asset service is introduced.

`CatKind` stores stable IDs and authored timing/mouth tracks. A `Pet` owns one
replaceable `CatArt` (2,764,800 bytes); there are no per-breed global caches.
The picker retains twelve 108×81 thumbnails (419,904 bytes) and only one reduced
sixteen-frame 120×90 preview (691,200 bytes). Temporary decoding is sequential.
Synchronous decode avoids queues and stale async selections; measure release
latency and reconsider only if it becomes perceptible. A replacement is decoded
before dropping the previous art, so transition peak memory is separate from
steady-state cache size.

Input hooks continue statistics and hotkeys. Input activity, body clicks and
hover do not drive character reactions, sound or waking. A captured copy wakes
the selected cat and plays its short authored cycle. Stable idle/sleep frames do
not continuously repaint. Pause/hide clear the entire copy effect state.

## Compatibility

`cat_kind` is a stable string with Ragdoll fallback for missing, unknown or
malformed values. Old accessory data is ignored. Cat changes, level progression
and statistics resets preserve the selected character; history and AI projects
remain independent. Tuxedo is described as a coat pattern, not a breed.

## Consequences

Compressed distribution size grows with twelve embedded atlases; retained
detailed art does not grow when switching cats. Each art revision requires
checking contours, baseline, anatomy, mouth alignment, copy timing, small sizes
and bright/dark backgrounds. Native Windows behavior requires OS-level checking
in addition to cross-target builds and Linux desktop smoke tests.
