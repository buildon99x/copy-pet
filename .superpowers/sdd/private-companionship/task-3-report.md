# Task3 — Local affection, dedicated breed art and bounded motion

Date:2026-10-30. Base:0828094. Existing checkout/feature branch retained.
Status: implementation and implementer verification complete; controller owns actual
Linux interaction/screenshots/performance and final native CI/manual review.
No push, merge or release performed.

## Changes

- Shared core silhouette down/move/up gesture excludes transparent pixels, panels,
  unmatched releases and movement beyond3 logical window points. Cat scale does
  not change this threshold; portable density conversion remains at the boundary.
  Lock prevents window movement while still permitting affection and cancelling
  dragged clicks. No global mouse observation, counting, logging or sound added.
- Both backends initiate on first/double-click press and react on release only.
  Native Windows uses capture and cancels on capture loss; affection completes
  before ReleaseCapture, avoiding synchronous cancellation. Portable cancels on
  cursor leave, focus loss, density changes and drag initiation. Existing panel
  focus/paste/hotkey paths were preserved; no focus/activation calls added.
- Four dedicated poses per breed: eye-contact16, gesture17, contentment18, settle19.
  Authored durations in catalog order:0.96,1.02,0.78,0.86,1.08,1.16,0.90,0.98,
  0.82,1.12,1.04,0.92seconds. Eye-contact holds16–20% by breed, gesture to48%,
  contentment to78%, then settle. No copy-mouth tracks changed.
- At most one pending affection. Running copy finishes its cycle, then affection
  precedes queued decorative copy. Clipboard storage occurs immediately. Affection
  and copy wake; unrelated input/hover does not. Pause cancels existing effects,
  permits a new local response; hide and character selection clear pending effects.
  Stable frames stop repainting. Existing33ms platform/clipboard housekeeping stays;
  no affection-specific perpetual timer added.
- Selected20-frame detailed cache:3,456,000bytes (3.296MiB), below4MiB. Loader accepts
  complete20–24frames;24frames=4,147,200bytes<4,194,304. Gallery thumbnails decode
  sequentially and retain one source frame per temporary thumbnail operation;
  preview decode retains only original16source frames before reduction. Existing
  static thumbnail419,904bytes and compact preview691,200bytes remain. Temporary
  atlas/replacement decode peak is separate from steady selected-cache size.
- Offline Rust prepare_affection uses supplied measured cuts, largest connected
  silhouettes, common breed scale matched to idle, bounded crop/foot alignment.
  assets/cats/affection-source.json records source rectangles, scale, role and
  baseline175. Source PNGs remain outside repository. No creative image edits or
  regeneration were needed; Python was used only to inspect original/new RGBA,
  never redraw/edit art. Existing16visible frames and alpha are exact.
- Active English/Korean README, controls/privacy, companion/product specs, AGENTS,
  asset contract and Unreleased changelog migrated. Original older product spec
  retained in archive; legacy proposal entry points marked historical. ADR-0021
  and index capture contracts; ADR-0017/0018 carry supersession notes. Approved
  spec copied to stable docs/superpowers/specs/2026-10-30-private-companionship-design.md
  with implemented feature-branch status and plan link updated.

## RED/GREEN and verification

Environment: source /workspace/.clipcat-env/env.sh. All commands --locked.

RED before production changes: cargo test --release --lib local_affection failed
compilation as expected:21E0599/E0609errors for missing cat_hit/cat_press/cat_motion/
cat_release APIs and affection state. These were missing-feature compile failures,
not assertion failures; observed in tool transcript. Initial two meaningful tests
covered silhouette/press/release/logical drag/lock/panel exclusion and bounded
copy/affection/wake/pause/hide/settled state. GREEN:2passed,0failed,173filtered.
A third regression test covers immediate copy persistence during affection,
bounded repeated requests, deferred decoration and final no-repaint. Focused
cargo test --release --lib affection:3passed,0failed,173filtered.

Final complete commands/results:

- cargo test --release:175library passed,0failed,1ignored;30integration passed;
  3release-script passed; bins/doc-tests0failed. The ignored test is
  platform::registered_hotkey::native::tests::x11_conflict_rebind_and_cleanup
  (requires explicit X11 display; controller/Task2 has actual X11 coverage).
  Final suite was repeated after compact gallery decode refinement and passed.
- cargo build --release:passed; target/release/clipcat is ready for controller.
- cargo clippy --release:passed with8existing warnings (6library,2gen_icon):
  clipboard question_mark/unnecessary_sort_by, portable chunks_exact/collapsible
  keyboard and middle-button match, render chunks_exact_mut, gen_icon chunks_exact/
  manual_checked_ops. No new affection/art warning. No lint gates weakened.
- cargo check --release --target x86_64-pc-windows-msvc:passed.
- same --features portable:passed. These are type/build checks, not native execution.
- cargo test --release --example prepare_cat_atlas:5passed,0failed. Tests cover
  fragments/connected low-alpha whiskers, baseline after interpolation, asymmetric
  cuts, empty input and shared normalization scale. Selected art tests in the full
  suite check all12×20frames, margins, baseline and<4MiB budget.
- cargo run --release --example prepare_affection -- SOURCE_DIRECTORY:passed;
  cargo run --release --example affection_preview -- ARTIFACT_DIRECTORY:passed.
- cargo run --release --example preview:passed; selected cache reports3,456,000bytes
  for each cat. Viewed actual PNGs for Korean panel, English paused panel and fish.
- bash scripts/release.sh verify:passed (crate/latest released section2.4.0).
- git diff --check:passed.

Durable command logs:
/workspace/artifacts/clipcat-private-companionship/task3-verification/
(task3-full-tests-final.log,task3-build.log,task3-clippy-final.log,
 task3-win-native-final.log,task3-win-portable-final.log,task3-atlas-tests.log,
 task3-art-preview.log,task3-preview.log,task3-release-verify.log).

## Image review evidence

Reviewed source affection-02.png and a1200×2160comparison grid at
/tmp/clipcat-affection-review.png. First compared Ragdoll/Siamese/Bombay/British,
then other8. Final actual-cell-size1200×720light/dark render previews:

/workspace/artifacts/clipcat-private-companionship/preview-affection-{1,2,3}-{light,dark}.png

Rows in group1:Ragdoll,Siamese,Bombay,British; group2:Birman,RagaMuffin,Persian,
TurkishAngora; group3:AmericanCurl,Munchkin,RussianBlue,Tuxedo. Columns:idle,16–19.
Viewed all3dark composites and the all12white composition. Contours/eyes/coats,
short legs, curled ears and slim British tail remain readable. Shared baseline
and idle scale seams are coherent. Initial low-alpha gutter traces were tightened
with opaque bounds plus2px padding in the offline extractor; final dark images
show no visible stray gutter lines. No copies/fish poses substitute for affection.

Original16verification JSON:
/workspace/artifacts/clipcat-private-companionship/original16-verification.json

Read-only Pillow comparison against HEAD confirms every pixel with alpha>0 and
all alpha values unchanged in the old960×720region for all12cats. Fully transparent
RGB garbage canonicalized tozero during Rust PNG roundtrip (78–514pixels/cat).
Raw straight-RGBA bytes therefore differ only where alpha=0; visible art and
premultiplied runtime frames are unchanged. This distinction was reported to root.

Additional renderer images viewed:
/tmp/clipcat-preview/4-panel-ko.png
/tmp/clipcat-preview/4-panel-en-paused.png
/tmp/clipcat-preview/2-fish.png

## Self-review and limits

Reviewed state arbitration/cleanup, gesture cancellation, logical-vs-cat scale,
selected cache ownership,20–24frame loader, compact gallery decode, existing mouth
tracks, no hooks/network/dependencies and Windows ReleaseCapture ordering. No
implementation blocker remains. Existing no-unnecessary-focus paths are retained;
macOS separate flyout/search focus and mixed-display native gestures still require
native validation. Windows native capture/drag/focus and macOS Retina input cannot
be certified from Linux crosschecks. Controller will record actual Linux GUI use,
all12clicks, rapid/locked/blank/panel/drag/concurrent-copy and performance separately;
this report does not claim those controller checks. No consolidated GUI automation
was duplicated by the implementer. Source/render previews are not OS screenshots.

## Independent review documentation follow-up

Independent Task3 review approved specification compliance and quality with no
blocking findings. Both Minor documentation findings were corrected: the stable
spec footer now distinguishes its approval-time pre-implementation statement from
the implemented feature branch and outstanding OS validation; the asset README
now states exact alpha and alpha>0 preservation while disclosing alpha0 RGB
canonicalization. Verified the scoped diff and git diff --check. Documentation
only; no runtime changes or repeated build/tests.
