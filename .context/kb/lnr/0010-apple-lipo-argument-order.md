# LNR-0010: Apple lipo requires its input before -verify_arch

- Date: 2026-11-02 · Area: macOS self-update validation / release CI

The macOS install regression passed on the local toolchain but failed on the
GitHub macOS runner using Xcode 26.6. The log reported the executable path as an
unknown architecture specification: Apple lipo consumes every trailing argument
after `-verify_arch` as a CPU name. The old command was
`lipo -verify_arch arm64 /path/to/clipcat`.

Use `lipo /path/to/clipcat -verify_arch arm64`, which also works with LLVM lipo.
Keep the signed-bundle installation regression enabled: it exposed a real
installer failure on systems using the stricter parser. A local pass is not a
substitute for waiting for the macOS release job.
