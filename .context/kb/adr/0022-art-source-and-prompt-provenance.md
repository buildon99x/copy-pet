# ADR-0022: Preserve image originals and prompt provenance outside runtime assets

- Status: Accepted
- Date: 2026-10-31

## Context

Character originals and generation prompts were spread across ephemeral task
artifacts. Some older conversation turns exceed the history API's single-turn
read limit. Runtime atlases were also overwritten by later frame additions,
making a path alone an ambiguous reference for the next image-generation call.

## Decision

Archive the 34 generated source PNGs byte-for-byte under `art/originals/`. Keep
historical code-render images at their existing versioned paths. A repository-
relative catalog connects originals, revisions, exact generation-input snapshots
and current atlases using IDs, PNG dimensions and SHA-256 hashes.

Preserve the three recovered tool-recorded `revisedPrompt` strings verbatim and
their known request arguments. Mark 31 unavailable older prompts as unrecovered;
do not invent originals, model versions, seeds or unknown reference parameters.
Implementation-task prompts are not relabeled as image-generation prompts.

Freeze the twelve exact sixteen-frame inputs from Git `0fffd4c`. Rebuilding adds
the archived affection sources through the existing Rust packager and compares
all twelve resulting PNG hashes and extraction metadata with the current assets.
The Python verifier/orchestrator uses only the standard library. It does not
generate new art or overwrite protected source/runtime asset directories.

For future generation, save submitted prompt and request parameters before the
tool call, then save original output bytes and returned metadata. Keep submitted
and revised prompt fields distinguishable whenever both are available.

## Consequences

The source checkout grows by the archived originals. No raw sources/prompts are
added to the app binary, rendering cache or release bundle. Missing prompt
coverage remains visible; `verify --require-prompts` intentionally fails until
it is resolved. Exact image-generation reproduction is not guaranteed without
provider/model/seed information; deterministic atlas packaging is separately
verified from the preserved byte sources.
