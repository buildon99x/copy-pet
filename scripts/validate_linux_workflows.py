"""Exercise native Linux clipboard capture and panel in a private CI Xvfb session.

Never run against a developer's desktop or profile. All text is synthetic; the
caller owns the X server, so xclip selection owners disappear when it closes.
"""
import ctypes
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main():
    if sys.platform != "linux" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Run only on an ephemeral Linux CI runner under Xvfb.")
    output = Path(os.environ["RUNNER_TEMP"]) / "clipcat-linux-validation"
    profile = output / "profile"
    profile.mkdir(parents=True, exist_ok=True)
    (profile / "state.json").write_text(json.dumps({
        "lang": "ko", "auto_update": False, "sound_mode": 0,
        "onboarded": True, "clip_capture": True, "hotkey": "ctrl+alt+9",
        "hotkey_version": 1,
    }))
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/clipcat").resolve()
    tokens = [f"ClipCat Linux synthetic {os.getpid()} {n}" for n in range(3)]
    result = {"binary": str(binary), "profile": str(profile)}
    env = dict(os.environ, CLIPCAT_CONFIG_DIR=str(profile))
    target = None
    window = None
    with (output / "app.log").open("w") as log:
        child = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
        try:
            time.sleep(2)
            if child.poll() is not None:
                raise RuntimeError(f"App exited during startup: {child.returncode}")
            for token in tokens:
                subprocess.run(["xclip", "-selection", "clipboard"], input=token.encode(),
                               stdout=subprocess.DEVNULL, stderr=log, check=True, timeout=5)
                time.sleep(0.8)
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline:
                if child.poll() is not None:
                    raise RuntimeError(f"App exited during capture: {child.returncode}")
                file = profile / "clips.json"
                saved = json.loads(file.read_text()).get("clips", []) if file.exists() else []
                if set(tokens).issubset({c["text"] for c in saved}):
                    break
                time.sleep(0.5)
            else:
                raise RuntimeError("Synthetic clipboard entries were not saved before timeout.")
            windows = subprocess.check_output(["xdotool", "search", "--onlyvisible", "--pid", str(child.pid)],
                                              text=True, timeout=5).splitlines()
            if not windows:
                raise RuntimeError("No native ClipCat window was visible.")
            window = windows[0]
            subprocess.run(["xdotool", "mousemove", "--window", window, "110", "150", "click", "2"],
                           check=True, timeout=5)
            time.sleep(0.5)
            geometry = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window],
                                               text=True, timeout=5)
            dimensions = dict(line.split("=", 1) for line in geometry.splitlines() if "=" in line)
            if int(dimensions.get("HEIGHT", 0)) <= 256 and int(dimensions.get("WIDTH", 0)) <= 240:
                raise RuntimeError(f"Middle-click did not expand the native panel: {geometry}")
            subprocess.run(["xdotool", "windowfocus", "--sync", window, "type", "--clearmodifiers", "--window", window, "Linux synthetic"],
                           check=True, timeout=5)
            time.sleep(0.5)
            subprocess.run(["import", "-window", window, str(output / "native-panel-ko.png")],
                           check=True, timeout=10)

            def key(chord):
                subprocess.run(["xdotool", "key", "--clearmodifiers", chord], check=True, timeout=5)
                time.sleep(0.25)

            # F3 adds snapshots in visible newest-first order; F2 shows the queue.
            key("F3")
            key("Down")
            key("F3")
            key("F2")
            subprocess.run(["import", "-window", window, str(output / "native-queue-ko.png")],
                           check=True, timeout=10)
            target_file = output / "paste-target.json"
            target_code = r'''
import json, sys, tkinter as tk
from pathlib import Path
root = tk.Tk()
root.title("ClipCat synthetic paste target")
text = tk.Text(root, width=55, height=8)
text.pack()
text.focus_set()
root.update()
file = Path(sys.argv[1])
command = file.with_suffix(".command")
stats = {"control_v": 0, "paste": 0}
def record(name):
    stats[name] += 1
text.bind("<Control-v>", lambda event: record("control_v"), add="+")
text.bind("<<Paste>>", lambda event: record("paste"), add="+")
def save():
    if command.exists():
        action = command.read_text()
        command.unlink()
        if action == "clear":
            text.delete("1.0", "end")
        elif action == "focus":
            text.focus_force()
    tmp = file.with_suffix(".tmp")
    tmp.write_text(json.dumps({"text": text.get("1.0", "end-1c"),
                               "window": str(text.winfo_id()),
                               "frame_window": str(root.tk.call("wm", "frame", root)),
                               "focus_widget": str(root.focus_get()),
                               "has_focus": root.focus_displayof() == text,
                               "events": stats}))
    tmp.replace(file)
    root.after(50, save)
save()
root.mainloop()
'''
            target = subprocess.Popen([sys.executable, "-c", target_code, str(target_file)],
                                      stdout=log, stderr=log)
            ready_until = time.monotonic() + 15
            target_windows = []
            while time.monotonic() < ready_until:
                if target.poll() is not None:
                    raise RuntimeError(f"Synthetic paste target exited: {target.returncode}")
                if target_file.exists():
                    ready = json.loads(target_file.read_text())
                    if ready.get("window"):
                        target_windows = [ready["window"]]
                        break
                time.sleep(0.1)
            if not target_windows:
                raise RuntimeError("Synthetic paste target did not report its native window before timeout.")

            def focus(win):
                subprocess.run(["xdotool", "windowfocus", "--sync", win], check=True, timeout=5)
                time.sleep(0.2)

            def expect_text(expected):
                until = time.monotonic() + 8
                actual = ""
                while time.monotonic() < until:
                    if target_file.exists():
                        actual = json.loads(target_file.read_text())["text"]
                        if actual == expected:
                            return
                    time.sleep(0.1)
                raise RuntimeError(f"Native queue paste mismatch: expected {expected!r}, got {actual!r}")

            def focus_target():
                focus(target_windows[0])
                target_file.with_suffix(".command").write_text("focus")
                until = time.monotonic() + 5
                while time.monotonic() < until:
                    if json.loads(target_file.read_text()).get("has_focus"):
                        return
                    time.sleep(0.1)
                raise RuntimeError("Synthetic editable control did not gain keyboard focus.")

            # Establish the editable target and normal clipboard paste first.
            focus_target()
            key("ctrl+v")
            expect_text(tokens[2])
            target_file.with_suffix(".command").write_text("clear")
            expect_text("")
            result["target_normal_paste_verified"] = True

            # PointerRoot over empty desktop is not an external input target.
            # The following real shortcut must leave the first queue item ready.
            subprocess.run(["xdotool", "mousemove", "1", "1"], check=True, timeout=5)
            x11 = ctypes.CDLL("libX11.so.6")
            x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
            x11.XOpenDisplay.restype = ctypes.c_void_p
            x11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
            x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
            x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
            display = x11.XOpenDisplay(None)
            if not display:
                raise RuntimeError("Cannot open private Xvfb display for focus regression.")
            try:
                x11.XSetInputFocus(display, 1, 0, 0)
                x11.XSync(display, 0)
            finally:
                x11.XCloseDisplay(display)
            time.sleep(0.3)
            key("ctrl+alt+j")
            expect_text("")

            focus_target()
            subprocess.run(["xset", "r", "on"], check=True, timeout=5)
            subprocess.run(["xset", "r", "rate", "200", "30"], check=True, timeout=5)
            subprocess.run(["xdotool", "keydown", "ctrl+alt+j"], check=True, timeout=5)
            time.sleep(0.95)
            subprocess.run(["xdotool", "keyup", "ctrl+alt+j"], check=True, timeout=5)
            expect_text(tokens[2])
            key("ctrl+alt+j")
            expect_text(tokens[2] + tokens[1])
            focus(window)
            key("F5")
            focus_target()
            key("ctrl+alt+j")
            replay = tokens[2] + tokens[1] + tokens[1]
            expect_text(replay)
            focus(window)
            key("F6")
            focus_target()
            key("ctrl+alt+j")
            combined = replay + tokens[2] + "\n" + tokens[1]
            expect_text(combined)
            subprocess.run(["import", "-window", target_windows[0], str(output / "native-paste-target.png")],
                           check=True, timeout=10)
            result.update(app_survived=True, captured_synthetic_clips=len(tokens),
                          panel_expanded=True, search_gestures_sent=True,
                          queue_next_order_verified=True, queue_replay_verified=True,
                          queue_combine_verified=True, queue_no_target_retains_item=True,
                          queue_held_chord_single_item=True, pasted_synthetic_text=combined,
                          captured_sources=sorted({c.get("source") or "unknown" for c in saved}),
                          window_geometry=dimensions, screenshot="native-panel-ko.png")
        finally:
            if target is not None:
                target_state = output / "paste-target.json"
                if target_state.exists():
                    result["target_diagnostic"] = json.loads(target_state.read_text())
            if window is not None and child.poll() is None:
                try:
                    subprocess.run(["import", "-window", window, str(output / "native-queue-final.png")],
                                   stdout=subprocess.DEVNULL, stderr=log, timeout=10)
                except (OSError, subprocess.TimeoutExpired) as error:
                    result["final_screenshot_error"] = str(error)
            if target is not None and target.poll() is None:
                target.terminate()
                try:
                    target.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    target.kill()
                    target.wait()
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
            result["test_app_stopped"] = child.poll() is not None
            (output / "result.json").write_text(json.dumps(result, indent=2))
            print(json.dumps(result, indent=2), flush=True)


if __name__ == "__main__":
    main()
