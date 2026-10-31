// Native clipboard regression test. Only run in an ephemeral CI profile.
// No AX element contents, global input listener or production test hook is used.
import AppKit
import ApplicationServices

let environment = ProcessInfo.processInfo.environment
guard environment["GITHUB_ACTIONS"] == "true", CommandLine.arguments.count == 2,
      let temp = environment["RUNNER_TEMP"] else {
    fatalError("Run only with an isolated macOS CI profile: swift validate_macos_paste.swift BINARY")
}
let output = URL(fileURLWithPath: temp).appendingPathComponent("clipcat-macos-validation")
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
func save(_ result: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
    try! data.write(to: output.appendingPathComponent("paste-result.json"))
    print(String(data: data, encoding: .utf8)!)
}
guard AXIsProcessTrusted() else {
    save(["paste_verified": false, "limitation": "Accessibility permission unavailable; native paste test skipped"])
    exit(0)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let mainMenu = NSMenu()
let edit = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
let editMenu = NSMenu(title: "Edit")
editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
edit.submenu = editMenu
mainMenu.addItem(edit)
app.mainMenu = mainMenu
let window = NSWindow(contentRect: NSRect(x: 250, y: 250, width: 500, height: 300),
                      styleMask: [.titled], backing: .buffered, defer: false)
window.title = "ClipCat synthetic paste verification"
let text = NSTextView(frame: window.contentView!.bounds)
window.contentView!.addSubview(text)
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(text)
let child = Process()
child.executableURL = URL(fileURLWithPath: CommandLine.arguments[1]).standardizedFileURL
try child.run()
let token = "ClipCat synthetic selected \(ProcessInfo.processInfo.processIdentifier)"
let other = "ClipCat synthetic current \(ProcessInfo.processInfo.processIdentifier)"
var checks: [Bool] = []
func later(_ seconds: Double, _ action: @escaping () -> Void) {
    DispatchQueue.main.asyncAfter(deadline: .now() + seconds, execute: action)
}
func key(_ code: CGKeyCode, _ flags: CGEventFlags = []) {
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        event.flags = flags
        event.post(tap: .cghidEventTap)
    }
}
func write(_ value: String) {
    NSPasteboard.general.clearContents()
    NSPasteboard.general.setString(value, forType: .string)
}
func open(_ offset: Double) {
    later(offset) {
        app.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(text)
    }
    later(offset + 0.5) {
        CGEvent(keyboardEventSource: nil, virtualKey: 59, keyDown: true)!.post(tap: .cghidEventTap)
        CGEvent(keyboardEventSource: nil, virtualKey: 56, keyDown: true)!.post(tap: .cghidEventTap)
    }
    later(offset + 0.7) { key(9, [.maskControl, .maskShift]) }
    later(offset + 0.9) {
        CGEvent(keyboardEventSource: nil, virtualKey: 56, keyDown: false)!.post(tap: .cghidEventTap)
        CGEvent(keyboardEventSource: nil, virtualKey: 59, keyDown: false)!.post(tap: .cghidEventTap)
    }
}
later(2) { write(token); NSPasteboard.general.setString("<b>\(token)</b>", forType: .html) }
later(3) { write(other) }
open(4)
later(5.5) { key(125) } // select the previous clip, not the current clipboard
later(5.8) { key(36) } // Return
later(7) {
    checks.append(text.string == token && NSPasteboard.general.string(forType: .string) == token)
    // Only the synthetic target window is captured, never the user's desktop.
    let capture = Process()
    capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    capture.arguments = ["-x", "-l", String(window.windowNumber), output.appendingPathComponent("paste-target.png").path]
    try? capture.run()
    capture.waitUntilExit()
    text.string = ""
}
open(8)
later(9.5) { key(125) }
later(9.8) { key(76) } // keypad Enter, reused flyout window
later(11) { checks.append(text.string == token); text.string = "" }
open(12)
later(13.5) { key(125) }
later(13.8) { key(36, .maskCommand) } // Cmd+Return, plain-text paste
later(15) {
    checks.append(text.string == token && NSPasteboard.general.string(forType: .string) == token
                  && NSPasteboard.general.string(forType: .html) == nil
                  && NSPasteboard.general.data(forType: .rtf) == nil)
    child.terminate()
    child.waitUntilExit()
    let passed = checks.count == 3 && checks.allSatisfy { $0 }
    save(["paste_verified": passed, "return": checks[0], "reopen_keypad_enter": checks[1],
          "command_return_plain_text": checks[2], "final_text": text.string,
          "final_clipboard_types": (NSPasteboard.general.types ?? []).map { $0.rawValue }])
    exit(passed ? 0 : 1)
}
app.run()
