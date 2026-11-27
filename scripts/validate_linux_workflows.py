"""Exercise native Linux clipboard capture and panel in a private CI Xvfb session.

Never run against a developer's desktop or profile. All text is synthetic; the
caller owns the X server, so xclip selection owners disappear when it closes.
"""
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
            result.update(app_survived=True, captured_synthetic_clips=len(tokens),
                          panel_expanded=True, search_gestures_sent=True,
                          captured_sources=sorted({c.get("source") or "unknown" for c in saved}),
                          window_geometry=dimensions, screenshot="native-panel-ko.png")
        finally:
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
