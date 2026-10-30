# Twelve illustrated cats with quiet event motion

The user approved the breed expansion and appearance sheets and explicitly requested implementation. Preserve all existing data, freely select every cat, discard accessories, and remove keyboard/mouse-driven character feedback. Existing instructions authorize Sol Medium implementers, commit/push, real-app review and screenshot reporting. Work in this existing cloud checkout on `feat/cat-breeds-motion`.

## Task 1 — Catalog, selected-character art and motion

Create `src/cat.rs` with `CatKind::ALL`, stable `id()/from_id()`, localized `name/description/persona`, independent copy timing, frame selection and per-frame mouth positions. Create a selected-character `CatArt` cache in `src/hunter_art.rs`, without permanent caches for twelve cats. Use sixteen normalized 240×180 transparent frames in four-by-four 960×720 release atlases under `assets/cats/<id>.png`. The controller supplies original generated sheets; an offline Rust example prepares release atlases using alpha bounds, common scale per cat and foot baseline. Keep `hunter_art::draw/face_anchor` compatibility until previews are updated, with only a default ragdoll cache permitted if legacy callers need it; production Pet owns its single detailed cache. Test ID fallback, distinct timelines, frame/mouth bounds, alpha margins and cache budget (<4MiB).

Frame roles are common for asset interchange: 0 baseline, 1–3 notice/anticipation, 4–7 distinct capture action, 8–11 satisfaction/recovery, 12 drowsy, 13 blink, 14 satisfied, 15 sleep. These slots do not force identical poses or playback timing. Each cat uses its own sequence/durations and mouth anchors. Copy duration approximately 0.8–1.2s and immediate notice. The British tail revision is the reference. Ragdoll may reuse its already reviewed atlas.

## Task 2 — App integration and selector

Persist stable `cat_kind` with safe default/unknown fallback; ignore old accessory save data while preserving statistics/history/settings. Replace accessory menus/actions/rendering/unlocks with free cat choice, keep selected cat on reset and level-up. Integrate owned CatArt into Pet/Scene, use selected timing/anchors for fish and particles. Add a localized selection gallery reachable from Windows tray and right-click and portable shortcut, static breed choices with description/persona and explicit preview/apply. Preserve focus, drag, panels and hotkeys. Keep only one detailed active cache; release old detailed art on replacement. Synchronous decode of small normalized atlases is acceptable if measured fast; no background queue or stale selection race is needed if measured low latency.

Remove global input-triggered animation/tap sounds, input-based waking, body click bounce/affection and hover-triggered character feedback. Continue input counters for existing statistics and global hotkeys; explicit app controls still work. Clipboard events wake the cat, play its distinctive cycle, preserve cap-one visual queue and pause/hide cancellation. Blink and eventual sleep remain bounded; stable idle/sleep do not continuously repaint. Add failing regression tests first for input/click silence, migration, selection preservation, all choices reachable, copy/queue behavior, reset and level-up preservation. Update stale descriptions/help and docs.

## Task 3 — Asset production and review (controller)

Generate each breed's sixteen-pose transparent sheet from its approved appearance, without accessories. Review consistency, breed traits, motion arcs and clean silhouettes; prepare normalized release atlases via Task 1 tool, preserving source art externally. Check all frame bounds/margins/baseline and inspect at actual display size. Record measured anchors per cat, update catalog where needed.

## Task 4 — Verification and delivery

Run release build, Clippy, full tests, preview, Windows MSVC native/portable checks and release metadata verification. Independently review code against requirements. Use the actual Linux app under Xvfb: select all twelve, trigger synthetic copies, verify keyboard/mouse silence, pause/hide, rapid switching, persistence, sizes, panel and workbench. Capture real app screenshots and motion videos/GIFs; distinguish rendered previews and untested Windows desktop. Measure release idle CPU, RSS and switch cost. Fix discovered issues and repeat affected checks. Update ADR/index, product spec, changelog and AGENTS guidance, commit/push feature branch (do not publish or merge without new release request).

## Review focus

- Old `accessory` values and unknown cat IDs must not reset XP or other settings.
- No character animation or tap sound from keys, clicks, scrolling or hovering.
- Face alignment on black/small/long-bodied cats; British slim tail and Curl backward ears.
- Cat switching mid-copy and repeated selection must not retain twelve detailed caches or replay stale effects.
- Native Windows custom tray menu and portable/macOS common menu must both expose selection.
