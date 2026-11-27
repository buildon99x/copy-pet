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
let profile = output.appendingPathComponent("paste-profile-\(ProcessInfo.processInfo.processIdentifier)")
try FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
let settings: [String: Any] = ["auto_update": false, "clip_capture": true, "onboarded": true, "sound_mode": 0]
try JSONSerialization.data(withJSONObject: settings).write(to: profile.appendingPathComponent("state.json"))
child.environment = environment.merging(["CLIPCAT_CONFIG_DIR": profile.path]) { _, new in new }
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
var trace: [[String: Any]] = []
var finalTypes: [String] = []
func history() -> [String] {
    guard let data = try? Data(contentsOf: profile.appendingPathComponent("clips.json")),
          let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let rows = json["clips"] as? [[String: Any]] else { return [] }
    return rows.compactMap { $0["text"] as? String }
}
func front() -> pid_t { NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0 }
func flyout() -> Int? {
    let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.first { row in
        guard row[kCGWindowOwnerPID as String] as? Int32 == child.processIdentifier,
              let bounds = row[kCGWindowBounds as String] as? [String: Any],
              let width = bounds["Width"] as? Double else { return false }
        return width > 260 // the pet is 240 points; the default panel is wider
    }?[kCGWindowNumber as String] as? Int
}
func record(_ label: String) {
    trace.append(["step": label, "target_active": front() == ProcessInfo.processInfo.processIdentifier,
                  "child_active": front() == child.processIdentifier, "flyout": flyout() ?? 0,
                  "change_count": NSPasteboard.general.changeCount, "target_text": text.string,
                  "clipboard_types": (NSPasteboard.general.types ?? []).map { $0.rawValue },
                  "persisted_history": history()])
}
func finish(_ error: String = "") -> Never {
    record("finish")
    child.terminate()
    child.waitUntilExit()
    let ok = error.isEmpty && checks.count == 3 && checks.allSatisfy { $0 }
    save(["paste_verified": ok, "return": checks.first ?? false,
          "reopen_keypad_enter": checks.count > 1 && checks[1],
          "command_return_plain_text": checks.count > 2 && checks[2],
          "final_text": text.string, "final_clipboard_types": finalTypes, "error": error, "trace": trace])
    exit(ok ? 0 : 1)
}
func awaitState(_ label: String, seconds: Double = 6,
                _ ready: @escaping () -> Bool, _ next: @escaping () -> Void) {
    let deadline = Date().addingTimeInterval(seconds)
    func poll() {
        if ready() { record(label); next(); return }
        if Date() > deadline { finish("Timed out waiting for " + label) }
        later(0.05, poll)
    }
    poll()
}
func modifier(_ code: CGKeyCode, _ down: Bool, _ flags: CGEventFlags = []) {
    let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
let sentinel = "ClipCat synthetic completion \(ProcessInfo.processInfo.processIdentifier)"
func phase(_ number: Int) {
    app.activate(ignoringOtherApps: true)
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(text)
    awaitState("target ready \(number)", { front() == ProcessInfo.processInfo.processIdentifier && window.firstResponder === text }) {
        modifier(59, true, .maskControl)
        modifier(56, true, [.maskControl, .maskShift])
        key(9, [.maskControl, .maskShift])
        modifier(56, false, .maskControl)
        modifier(59, false)
        awaitState("focused flyout \(number)", { front() == child.processIdentifier && flyout() != nil }) {
            key(125) // select original older row, so unexpected self-capture fails
            later(0.15) {
                if number == 2 { modifier(55, true, .maskCommand) }
                key(number == 1 ? 76 : 36, number == 2 ? .maskCommand : [])
                if number == 2 { later(0.1) { modifier(55, false) } }
                awaitState("editor received paste \(number)", { !text.string.isEmpty }) {
                    let board = NSPasteboard.general
                    finalTypes = (board.types ?? []).map { $0.rawValue }
                    let plain = board.string(forType: .html) == nil && board.data(forType: .rtf) == nil
                    checks.append(text.string == token && board.string(forType: .string) == token && (number != 2 || plain))
                    if number == 0 {
                        let capture = Process()
                        capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
                        capture.arguments = ["-x", "-l", String(window.windowNumber), output.appendingPathComponent("paste-target.png").path]
                        try? capture.run()
                        capture.waitUntilExit()
                    }
                    if number < 2 && checks.last == true { text.string = ""; phase(number + 1); return }
                    // Force a fresh autosave. A recaptured program write would
                    // move the selected token ahead of the external "other".
                    later(1) {
                        write(sentinel)
                        awaitState("final persisted order", seconds: 40, { history().first == sentinel }) {
                            guard history() == [sentinel, other, token] else { finish("self-copy reordered history") }
                            finish()
                        }
                    }
                }
            }
        }
    }
}
later(2) { write(token); NSPasteboard.general.setString("<b>\(token)</b>", forType: .html) }
later(3.5) {
    write(other)
    awaitState("captured rich and current rows", seconds: 40, { history() == [other, token] }) { phase(0) }
}
app.run()
