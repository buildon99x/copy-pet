# 0025 — On-demand Windows OCR in a bounded worker process

- Status: Accepted (2026-11-27)
- Extends [0015](0015-clipboard-tags-and-windows-images.md).

Native Windows starts one child copy of ClipCat in OCR mode. The child uses
Windows.Media.Ocr and installed user-profile languages. Typed Windows-only
Rust projections provide WinRT activation and object lifetimes. No OCR model
ships with ClipCat. Input and output use anonymous pipes with bounded sizes.
The parent supervises the job off the UI thread, kills it on timeout or exit,
and reaps it. The worker also enforces an independent lifetime limit.

A clip stores optional recognized text alongside its image. Image deduplication
and normal clipboard output continue using the original bytes. A result
requires matching clip ID, capture generation, image, and privacy state.
Manual requests keep image recognition under direct user control.

Microsoft documents package identity for the OCR API. Availability and language
support are checked at runtime. The standalone Windows integration test uses
synthetic system-font text and requires an actual recognition result.

[Microsoft Windows.Media.Ocr documentation](https://learn.microsoft.com/en-us/uwp/api/windows.media.ocr)
