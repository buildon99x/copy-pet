# 06. Rust Implementation Architecture

## Suggested module map
```text
src/
  app.rs                  // main app state orchestration
  pet/
    mod.rs
    state_machine.rs
    motion.rs
    xp.rs
    accessories.rs
  panel/
    mod.rs
    layout.rs
    model.rs
    keyboard.rs
    search.rs
    undo.rs
  render/
    mod.rs
    primitives.rs         // rounded rect, text, icons, shadows
    tokens.rs             // generated from tokens
    pet_draw.rs
    panel_draw.rs
    fx_draw.rs
  clipboard/
    store.rs
    watcher.rs
  platform/
    windows.rs
    macos.rs
  i18n.rs
  state.rs
```

## Rendering approach
- Keep immediate-mode software rendering.
- Use logical coordinate system, then scale to backing pixels.
- Draw order: shadow -> pet body -> accessories -> transient fx -> panel if open -> toasts -> cursor/resize affordances.
- Prefer code-generated vector primitives. SVG files are reference only; if embedding SVG strings, gate behind feature flag and keep small.

## Core state structs
```rust
pub struct UiState {
    pub pet: PetState,
    pub panel: PanelState,
    pub toasts: ToastQueue,
    pub fx: FxQueue,
    pub theme: ThemeTokens,
}

pub struct PanelState {
    pub open: bool,
    pub query: String,
    pub ime_composing: bool,
    pub selected_visible_index: usize,
    pub source_filter: SourceFilter,
    pub scroll_y: f32,
    pub clear_armed: bool,
    pub auto_close: bool,
    pub layout: PanelLayout,
}
```

## Event flow
1. Platform input/clipboard watchers produce sanitized events.
2. App tick consumes events and updates stores/state machines.
3. Layout computes union bounds: cat canvas + panel card.
4. Platform window resize/move applies `take_window_shift` so cat anchor remains fixed.
5. Renderer draws frame.
6. Dirty state persistence throttled to 30s plus explicit mutation saves.

## Dependency policy
Allowed if already present: winit, softbuffer, rdev, arboard, ab_glyph, serde, serde_json.
Before adding: resvg/usvg/tiny-skia only if code-generated drawing is not enough.
Avoid: heavy GUI frameworks, webview, telemetry, bundled font/icon packs.
