# Schema Guide for Codex and Claude Code

This design contract uses:

- Markdown for product intent, acceptance, and implementation instructions.
- YAML for state machines, animation rules, and interaction behavior.
- JSON for design tokens, asset manifest, and mock runtime data.
- JSON Schema for metadata and validation structure.

## File Meanings

### Markdown

Markdown files explain why the system exists, what must be implemented, and how to validate it.

Use Markdown files to understand intent before writing code.

### YAML

YAML files define executable design logic:

- state transitions
- animation durations
- event priority
- UI behavior
- failure handling

Codex should translate YAML into Rust enums, structs, constants, and tests.

### JSON

JSON files define stable data contracts:

- tokens
- assets
- mock runtime state
- source badges

Codex should translate JSON into typed Rust structs or compile-time constants.

### JSON Schema

Schema files describe the expected structure of JSON/YAML-like documents. They are not runtime code by default, but can be used to validate the package.

## Recommended Rust Types

```rust
enum PetState {
    Idle,
    TypingSlow,
    TypingFast,
    TypingExtreme,
    Sleep,
    Yawn,
    LookAround,
    CopyAnticipate,
    MouthOpen,
    Nom,
    Happy,
    Petting,
    Boop,
    LevelUp,
    NewItem,
    PanelOpen,
}

struct MotionClip {
    id: &'static str,
    duration_ms: u32,
    easing: Easing,
    tracks: Vec<MotionTrack>,
}

struct PetAsset {
    id: &'static str,
    kind: AssetKind,
    path: &'static str,
    anchor: Point,
    default_scale: f32,
}
```
