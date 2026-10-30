# ADR-0017: Cached illustrated Cottonball Hunter frames

- Status: Accepted
- Date: 2026-10-30

## Context

The user explicitly selected an illustrated theatrical 2D ragdoll and asked to
implement additional animation frames. Procedural primitive art cannot preserve
the selected character design. Always-on rendering must remain bounded.

## Decision

Allow one embedded transparent PNG sprite atlas for the pet. Decode it once with
existing tiny-skia support, normalize and cache sixteen small premultiplied
frames below 4 MiB, and select frames at event time. No runtime asset downloads,
new image dependencies or general-purpose asset build system are added. Icons,
UI and sounds keep their existing generation paths.

Stable pet states do not continuously redraw. Existing clipboard storage,
input counters, source identity and platform behavior remain independent of
visual playback. This narrowly supersedes ADR-0000's all-vector pet decision.

## Consequences

The executable contains the illustrated atlas and grows by its compressed size.
The renderer retains a bounded frame cache; decoding the source briefly costs
additional memory at startup. Art revisions require checking extraction, alpha,
baseline alignment and transitions rather than trusting the generated sheet.
Additional asset types or pipelines require their own decision.
