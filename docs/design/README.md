# ClipCat Pet Motion Design Contract

This package is the design contract for the ClipCat desktop pet — the source of
truth for the pet appearance, state machine, motion clips and interactions. It
was imported into the repo as a living spec; implementation status is tracked in
[`../specs/pet-motion.md`](../specs/pet-motion.md) and the decision is recorded in
[ADR-0012](../../.context/kb/adr/0012-pet-motion-visual-system.md).

## Meaning

- Markdown = intent, QA, implementation instructions
- YAML = state machine, motion clips, interaction rules
- JSON = visual tokens, assets, runtime mock data
- JSON Schema = contract metadata

## Files

```txt
docs/design/
  IMPLEMENTATION_BRIEF.md
  pet_motion_intent.md
  schema_guide.md
  codex_prompt.md
  visual_qa.md
  pet_state_machine.yaml
  motion_spec.yaml
  interaction_behavior.yaml
  pet_tokens.json
  asset_manifest.json
  runtime_mock.json
  schemas/
    pet_tokens.schema.json
    asset_manifest.schema.json
    pet_state_machine.schema.json
    motion_spec.schema.json
    interaction_behavior.schema.json
    runtime_mock.schema.json
assets/reference/
  clipcat_pet_motion_reference.png   # the concept board (single copy; YAML refs use ../assets/reference/)
```

