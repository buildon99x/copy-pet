"""Exercise the actual macOS app on an ephemeral GitHub-hosted runner.

The CI-only guard prevents replacing a developer's real ClipCat preferences.
Screen capture is optional: macOS screen-recording permission may be unavailable.
App survival, clipboard capture and saved state are required independently.
"""
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


def main():
    if sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Run this smoke test only on an ephemeral macOS CI runner.")
    output = Path(os.environ["RUNNER_TEMP"]) / "clipcat-macos-validation"
    output.mkdir(parents=True, exist_ok=True)
    profile = Path.home() / "Library/Application Support/ClipCat"
    profile.mkdir(parents=True, exist_ok=True)
    state_path = profile / "state.json"
    state_path.write_text(json.dumps({
        "cat_kind": "british-shorthair", "lang": "ko", "auto_update": False,
        "sound_mode": 0, "onboarded": True, "clip_capture": True,
        "total_xp": 12345, "total_copies": 0,
    }))
    tokens = [f"ClipCat macOS CI synthetic capture {os.getpid()} {n}" for n in range(3)]
    result = {"platform": platform.platform(), "architecture": platform.machine()}
    with (output / "app.log").open("w") as log:
        app = subprocess.Popen(["target/release/clipcat"], stdout=log, stderr=log)
        try:
            time.sleep(3)
            if app.poll() is not None:
                raise RuntimeError(f"macOS app exited during startup: {app.returncode}")
            for token in tokens:
                subprocess.run(["pbcopy"], input=token.encode(), check=True)
                time.sleep(1.3)
                if app.poll() is not None:
                    raise RuntimeError(f"macOS app exited while collecting: {app.returncode}")
            # Normal autosave runs every 30 seconds; do not add a test-only
            # production hook or pretend SIGTERM performs a graceful save.
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    raise RuntimeError(f"macOS app exited before autosave: {app.returncode}")
                state = json.loads(state_path.read_text())
                clips_path = profile / "clips.json"
                clips = clips_path.read_text() if clips_path.exists() else ""
                if state.get("total_copies", 0) >= 3 and all(t in clips for t in tokens):
                    break
                time.sleep(1)
            else:
                raise RuntimeError("Three native clipboard captures were not saved before timeout.")
            assert state["cat_kind"] == "british-shorthair", state
            assert state["total_xp"] >= 12360, state
            result.update(app_survived=True, captured_synthetic_clips=3,
                          saved_cat=state["cat_kind"], saved_total_xp=state["total_xp"])
            capture = subprocess.run(["screencapture", "-x", str(output / "desktop.png")],
                                     capture_output=True, text=True)
            result["screenshot_available"] = capture.returncode == 0
            if capture.returncode:
                result["screenshot_limitation"] = capture.stderr.strip()
            (output / "smoke-result.json").write_text(json.dumps(result, indent=2))
            print(json.dumps(result), flush=True)
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait()


if __name__ == "__main__":
    main()
