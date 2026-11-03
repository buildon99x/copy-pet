# ClipCat C icon Store submission — 2026-11-04

The user authorized continuing the selected C icon through CI and Microsoft
Store release. v2.5.4 was published through the normal release workflow;
production and the public tag both resolve to
`10aa9df8344c07b47d92600b094efe5aa4da7bfc`.

The exact production MSIX passed package CI and SHA-256 verification. Partner
Center validated v2.5.4.0, removed the superseded v2.5.3 draft package on save,
and accepted the resubmission. Both English and Korean listings have the
selected C 300×300 app tile. Existing descriptions, screenshot, age ratings,
free pricing and automatic publishing were retained.

At 2026-11-04 01:24 KST the observed status was **In certification**, with
**Pre-processing in progress (step 2/4)**. Microsoft approval and public Store
availability were not verified. The product is configured to publish after
certification passes.

- [Release CI](https://github.com/buildon99x/copy-pet/actions/runs/41666431372)
- [Store package CI](https://github.com/buildon99x/copy-pet/actions/runs/41671442148)
- [Published GitHub release](https://github.com/buildon99x/copy-pet/releases/tag/v2.5.4)
- [Store product URL](https://apps.microsoft.com/detail/9NZBR8X5T677) (availability pending)

The first tag attempt failed two native macOS paste checks. The same commit's
main CI passed all three; a retry of the failed tag jobs also passed all three.
Checks were not weakened. Another workflow-only main run failed Command+Return.
This is evidence of intermittent results, not a proven diagnosis or a claim
that the paste probe's reliability has been fixed.

The archived Windows screenshot uses synthetic clipboard data. The extracted
32px associated icon came from Windows decoding the packaged executable's
actual embedded icon. This verifies native icon decoding, not manual review
of every shell view. No MSIX installation/uninstallation, Store update, or WACK
execution was performed. The macOS package uses ad-hoc signing and is not
Developer ID signed or notarized.

Account screenshots and the internal submission identifier are kept only in
ignored local evidence. This public receipt contains public product/release
metadata and synthetic app validation evidence.
