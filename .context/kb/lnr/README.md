# Lessons & Near-misses (LNR)

Short notes on trial-and-error: a symptom we hit, the real cause, the fix, and
the takeaway. These are the things that cost time and are easy to repeat — read
before touching the related area.

| # | Lesson | Area |
|--:|--------|------|
| [0001](0001-softbuffer-no-alpha.md) | softbuffer can't carry per-pixel alpha → black rectangle | portable render |
| [0002](0002-debug-vs-release-cpu.md) | Debug tiny-skia is ~10× slower; benchmark CPU on release | performance |
| [0003](0003-rdev-winit-threading.md) | rdev runs on its own thread; macOS/Wayland caveats | portable input |
| [0004](0004-cargo-cross-platform-deps.md) | "optional on Windows, required elsewhere" dependency layout | build/cargo |
| [0005](0005-macos-tis-eventtap-crash.md) | macOS 15 SIGTRAP: rdev translates keys (TIS) off the main thread | portable input (macOS) |
| [0006](0006-auto-paste-foreground-capture-order.md) | Auto-paste captured our own window as the target on the hotkey path | Windows backend (auto-paste) |
| [0007](0007-macos-paste-synthesis.md) | macOS auto-paste: Command flag missing on the V event + a main-thread sleep starved app activation | portable backend (auto-paste, macOS) |
