# ClipCat first Reddit promotion — 2026-11-06

## Published result

- [Posted comment](https://www.reddit.com/r/software/comments/1xpb2kw/comment/19s8b0ll8lz5w/)
- [Weekly Discovery Thread — November 06, 2026](https://www.reddit.com/r/software/comments/1xpb2kw/weekly_discovery_thread_november_06_2026/)
- Posted using the user's signed-in Reddit account. The user requested promotion execution and collaboration with the existing ChatGPT planning/writing session.
- Reopened the generated comment permalink and verified the author, complete text and both destination links. The screenshot shows the posted comment. This confirms persistence in the signed-in browser; logged-out visibility, moderation acceptance, reader responses and installation attribution were not independently verified.
- Account UI screenshot remains local at `.local/promotion-2026-11-06/reddit-comment.png`; it is not committed.

## Basis and checks

Read the existing launch copy and confirmed PR #93 was merged at `845cc6a`. Kept the previously prepared Store listing and experiment unchanged. The public Store page resolved to ClipCat by PIXO and displayed the localized clipboard/cat description.

The weekly thread explicitly allows relevant, useful self-promotion with disclosure and prohibits link spam and AI-generated content dumps. The sidebar self-promotion rule restricts non-open-source promotion; the production source's `LICENSE` is MIT. Chose one short, specific workflow comment in the weekly thread, with developer and AI writing-assistance disclosure. No separate promotional post or cross-post was made.

The existing ChatGPT session supplied a tailored English draft and three follow-up priorities. Reviewed its draft against the production source and Store documentation before posting. Corrected its GitHub-update statement: Store builds leave updates to Microsoft Store (`store` feature in `src/update.rs`). No performance, conversion or installation improvement was claimed.

## Exact posted copy

```text
Hi! I'm the developer of ClipCat, a free Windows clipboard manager with a little desktop cat.

When you copy text, the cat eats a fish. There are 12 cats to choose from, with searchable clipboard history, tags, pinned clips, and image history on Windows.

One workflow: copy a reusable reply, switch tasks, then press Win+Shift+V, search for the reply, and select it to paste into your previous app. If that shortcut cannot be registered, ClipCat tries Ctrl+Shift+V.

History stays on your PC (stored unencrypted), with no account or cloud sync. You can pause collection before copying sensitive content. The Store version's updates are managed by Microsoft Store.

Microsoft Store: https://apps.microsoft.com/detail/9NZBR8X5T677
Source (MIT): https://github.com/buildon99x/copy-pet

AI helped polish this post. I'm sharing my own project and would appreciate feedback: would you keep the cat visible while working, or hide it until needed?
```

## Follow-up plan, not scheduled work

| Window (KST) | Next action | Evidence to retain |
|---|---|---|
| 2026-11-06–2026-11-07 | Review responses to this comment and answer concrete product questions; avoid duplicate promotion | Permalinks, questions and verified answers |
| 2026-11-08–2026-11-10 | Capture a 20–30 second real Windows copy → cat reaction → search → paste demo using synthetic data; prepare AlternativeTo and Product Hunt materials | Build provenance and actual capture; those directories remain unsubmitted |
| 2026-11-11–2026-11-12 | Review available Partner Center visits/install data alongside qualitative Reddit feedback | Observation dates and counts; do not infer attribution from timing alone |

The published Store link has no campaign identifier. There is no verified Reddit-to-install attribution or measured campaign outcome. No recurring automation was created. Application source and release refs were not changed; Rust build/tests are unnecessary for this documentation-only record.
