"""Package an already-built native/cross pair as a portable macOS test app."""
import hashlib
import json
from pathlib import Path
import plistlib
import re
import subprocess
import sys


def main():
    if sys.platform != "darwin" or len(sys.argv) != 3:
        raise SystemExit("usage on macOS: package_macos_test.py NATIVE_BINARY CROSS_BINARY")
    output = Path("dist/macos-test")
    app = output / "ClipCat.app"
    contents = app / "Contents"
    executable = contents / "MacOS/clipcat"
    executable.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["lipo", "-create", sys.argv[1], sys.argv[2], "-output", str(executable)], check=True)
    architectures = subprocess.check_output(["lipo", "-archs", str(executable)], text=True).split()
    if set(architectures) != {"arm64", "x86_64"}:
        raise RuntimeError(f"Unexpected application architectures: {architectures}")
    executable.chmod(0o755)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    version = re.search(r'^version\s*=\s*"([^"]+)"', Path("Cargo.toml").read_text(), re.M).group(1)
    with (contents / "Info.plist").open("wb") as f:
        plistlib.dump({
            "CFBundleIdentifier": "dev.clipcat.preview",
            "CFBundleName": "ClipCat",
            "CFBundleDisplayName": "ClipCat Preview",
            "CFBundleExecutable": "clipcat",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version,
            "CFBundleVersion": version,
            "CFBundleGetInfoString": f"Cat companions test build {commit[:7]}",
            "LSMinimumSystemVersion": "11.0",
            "NSHighResolutionCapable": True,
        }, f)
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)], check=True)
    archive = output / "ClipCat-macos-universal-test.zip"
    subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(archive)], check=True)
    metadata = {"commit": commit, "architectures": architectures, "minimum_macos": "11.0",
                "archive": archive.name, "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                "signing": "ad-hoc; not Developer ID signed or notarized"}
    (output / "build-info.json").write_text(json.dumps(metadata, indent=2))
    (output / "README.txt").write_text(
        "ClipCat macOS test build / 고양이 12종 테스트 빌드\n\n"
        "Apple Silicon + Intel / macOS 11 이상\n"
        f"Commit: {commit}\n\n"
        "1. ClipCat-macos-universal-test.zip 압축을 풀어 주세요.\n"
        "2. 기존 ClipCat을 종료한 뒤 ClipCat.app을 실행하세요.\n"
        "3. 우클릭 메뉴의 고양이 선택에서 12종과 복사 반응을 확인하세요.\n\n"
        "최초 실행이 차단되면 시스템 설정 > 개인정보 보호 및 보안에서\n"
        "이 앱의 열기 허용(확인 없이 열기)을 선택하세요.\n"
        "전역 단축키는 손쉬운 사용(접근성) 권한이 필요할 수 있습니다.\n"
        "테스트 앱은 기존 ClipCat의 로컬 기록과 설정을 사용합니다.\n\n"
        "Unzip the inner archive and open ClipCat.app after quitting an existing copy.\n"
        "This preview is ad-hoc signed and has not been notarized. If macOS blocks it,\n"
        "use System Settings > Privacy & Security > Open Anyway for this app.\n"
        "Global shortcuts may require Accessibility permission.\n"
    )
    print(json.dumps(metadata), flush=True)


if __name__ == "__main__":
    main()
