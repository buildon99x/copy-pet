# ADR-0024: Selected illustrated app icon, code-owned packaging

Date: 2026-11-04. Status: Accepted (user selected C after rejecting the vector draft).

ClipCat's app icon was a small code-drawn cat face and fish. macOS packaging had
no dedicated ICNS. The user requested a Windows/macOS brand identity, rejected
the vector cat-clipboard draft, and explicitly chose C from an ImageGen comparison:
a Ragdoll in a terracotta document pocket.

Preserve this transparent original and its exact request/reference in art/.
Existing tiny-skia code exports ICO, ICNS inputs, PNG masters and Store logos;
Apple iconutil creates ICNS. Only one generated 256px derivative is embedded for
startup/tray rendering, decoded temporarily and dropped after use. Full artwork
is development-only, never bundled or cached by the running app. No new runtime
or generation dependency is introduced. This narrowly extends the code-only
icon art assumption in ADR-0000/0017; code still owns sizing and native formats.

Use the same foreground for Windows and macOS, adding only a macOS tile. Original
bytes, input prompt, request parameters and derivative hashes remain traceable.
Small icons retain the cat/pocket colors; fine fur is not expected to survive at
16px. Packaging copies ICNS and sets CFBundleIconFile before signing.
