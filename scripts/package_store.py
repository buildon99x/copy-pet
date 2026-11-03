"""Stage the Store-only Windows build and pack with the validating Windows SDK."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


def main():
    if sys.platform != "win32" or len(sys.argv) != 2:
        raise SystemExit("usage on Windows: python scripts/package_store.py MAKEAPPX_EXE")
    root = Path(__file__).resolve().parents[1]
    version = re.search(r'^version = "([\d.]+)"', (root / "Cargo.toml").read_text(), re.M).group(1)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    # A store release packages the published production commit, never main's
    # next development state. The workflow audits the public tag separately.
    production = subprocess.check_output(["git", "rev-parse", "origin/production"], cwd=root, text=True).strip()
    if commit != production:
        raise SystemExit("Store source must match origin/production exactly")
    out = root / "target/store"
    stage = out / "stage"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    subprocess.run(["cargo", "build", "--locked", "--release", "--features", "store", "--bin", "clipcat"], cwd=root, check=True)
    shutil.copy2(root / "target/release/clipcat.exe", stage / "clipcat.exe")
    subprocess.run(["cargo", "run", "--locked", "--release", "--example", "store_assets", "--", str(stage / "Assets")], cwd=root, check=True)
    manifest = f'''<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
 xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
 xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
 IgnorableNamespaces="uap rescap">
 <Identity Name="PIXO.ClipCat" Publisher="CN=62E0CEB7-8FB8-4E4C-BF40-5231FBBC83E0" Version="{version}.0" ProcessorArchitecture="x64"/>
 <Properties><DisplayName>ClipCat</DisplayName><PublisherDisplayName>PIXO</PublisherDisplayName><Logo>Assets\\StoreLogo.png</Logo></Properties>
 <Resources><Resource Language="en-US"/><Resource Language="ko-KR"/></Resources>
 <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0"/></Dependencies>
 <Applications><Application Id="ClipCat" Executable="clipcat.exe" EntryPoint="Windows.FullTrustApplication">
  <uap:VisualElements DisplayName="ClipCat" Description="A desktop cat and local clipboard manager" BackgroundColor="transparent"
   Square44x44Logo="Assets\\Square44x44Logo.png" Square150x150Logo="Assets\\Square150x150Logo.png"/>
 </Application></Applications>
 <Capabilities><rescap:Capability Name="runFullTrust"/></Capabilities>
</Package>
'''
    (stage / "AppxManifest.xml").write_text(manifest, encoding="utf-8")
    package = out / f"clipcat-store-x64-{version}.msix"
    subprocess.run([sys.argv[1], "pack", "/d", str(stage), "/p", str(package), "/o"], check=True)
    metadata = {
        "version": version, "source_tag": f"v{version}", "source_commit": commit,
        "package": package.name, "sha256": hashlib.sha256(package.read_bytes()).hexdigest(),
        "executable_sha256": hashlib.sha256((stage / "clipcat.exe").read_bytes()).hexdigest(),
        "cargo_features": ["store"], "identity": "PIXO.ClipCat",
        "publisher": "CN=62E0CEB7-8FB8-4E4C-BF40-5231FBBC83E0",
        "validation": "MakeAppx pack with default manifest validation; unsigned for Microsoft Store",
    }
    (out / "build-info.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
    print(json.dumps(metadata), flush=True)


if __name__ == "__main__":
    main()
