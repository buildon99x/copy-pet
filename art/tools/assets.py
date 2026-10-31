#!/usr/bin/env python3
"""Verify archived artwork and rebuild release atlases without editing originals.

Uses only Python's standard library. PNG decoding/packaging stays in the existing
Rust example; this script never calls an image generator or changes app assets.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "art/catalog.json"


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(65536), b""):
            h.update(block)
    return h.hexdigest()


def repo_path(value):
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        raise ValueError("asset paths must be nonempty repository-relative strings")
    path = (ROOT / value).resolve()
    if ROOT not in path.parents:
        raise ValueError("asset path escapes repository: " + value)
    return path


def verify(catalog, require_prompts=False):
    data = json.loads(catalog.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise ValueError("unsupported catalog schema")
    records = data["assets"]
    by_id = {item["id"]: item for item in records}
    if len(by_id) != len(records):
        raise ValueError("duplicate asset IDs")
    for file in data["metadata_files"]:
        path = repo_path(file["path"])
        if path.stat().st_size != file["bytes"] or digest(path) != file["sha256"]:
            raise ValueError("extraction metadata checksum mismatch: " + file["path"])
    missing = []
    recorded = 0
    for item in records:
        if "cat_id" in item and not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*", item["cat_id"]):
            raise ValueError("unsafe cat identifier: " + item["id"])
        file = item["file"]
        path = repo_path(file["path"])
        if path.stat().st_size != file["bytes"] or digest(path) != file["sha256"]:
            raise ValueError("image checksum/size mismatch: " + item["id"])
        for alias in item.get("repository_aliases", []):
            if digest(repo_path(alias)) != file["sha256"]:
                raise ValueError("repository alias differs from original: " + item["id"])
        with path.open("rb") as source:
            header = source.read(33)
        if header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
            raise ValueError("not a PNG: " + item["id"])
        width, height = struct.unpack(">II", header[16:24])
        if (width, height, header[25]) != (file["width"], file["height"], file["color_type"]):
            raise ValueError("PNG metadata mismatch: " + item["id"])
        for key in ["source_asset_ids", "design_reference_ids"]:
            for reference in item.get(key, []):
                if reference not in by_id:
                    raise ValueError("unknown asset reference: " + reference)
        if item.get("supersedes") and item["supersedes"] not in by_id:
            raise ValueError("unknown revision parent: " + item["id"])
        prompt = item["prompt"]
        if prompt["status"] == "recorded":
            recorded += 1
            text_path = repo_path(prompt["prompt_path"])
            if not text_path.read_text(encoding="utf-8").strip() or digest(text_path) != prompt["sha256"]:
                raise ValueError("prompt checksum/empty text: " + item["id"])
            request_path = repo_path(prompt["request_path"])
            if digest(request_path) != prompt["request_sha256"]:
                raise ValueError("request parameters checksum mismatch: " + item["id"])
            request = json.loads(request_path.read_text(encoding="utf-8"))
            if request["asset_id"] != item["id"] or request["prompt_path"] != prompt["prompt_path"]:
                raise ValueError("request does not match its prompt/asset: " + item["id"])
            if request["reference_asset_ids"] != item["source_asset_ids"]:
                raise ValueError("request references disagree: " + item["id"])
        elif prompt["status"] == "unrecovered":
            if prompt["prompt_path"] is not None or not prompt.get("reason_code"):
                raise ValueError("unrecovered prompt must have a reason and no invented text")
            missing.append(item["id"])
        elif prompt["status"] not in ["not_applicable", "derived"]:
            raise ValueError("unknown prompt status: " + item["id"])
        if item.get("extraction_metadata"):
            repo_path(item["extraction_metadata"]).read_bytes()
    # All input assets are a directed acyclic graph, including historical snapshots.
    visited, visiting = set(), set()

    def visit(asset_id):
        if asset_id in visiting:
            raise ValueError("cyclic source references: " + asset_id)
        if asset_id in visited:
            return
        visiting.add(asset_id)
        for parent in by_id[asset_id].get("source_asset_ids", []):
            visit(parent)
        visiting.remove(asset_id)
        visited.add(asset_id)

    for asset_id in by_id:
        visit(asset_id)
    references = [item["cat_id"] for item in records if item["kind"] == "reference-snapshot"]
    runtime = [item["cat_id"] for item in records if item["kind"] == "runtime-atlas"]
    if len(set(references)) != len(references) or len(set(runtime)) != len(runtime) or set(references) != set(runtime):
        raise ValueError("reference/runtime character sets must agree and be unique")
    manifest = json.loads(repo_path(data["rebuild"]["affection_manifest"]).read_text(encoding="utf-8"))
    rows = []
    for source in manifest["sources"]:
        if not re.fullmatch(r"[A-Za-z0-9_-]+\.png", source["file"]):
            raise ValueError("source sheet must be a simple PNG filename")
        rows.extend(source["rows"])
    if len(set(rows)) != len(rows) or set(rows) != set(runtime):
        raise ValueError("affection source rows must match the character set")
    coverage = data["coverage"]
    if recorded != coverage["recorded_generation_prompts"] or len(missing) != coverage["unrecovered_generation_prompts"]:
        raise ValueError("prompt coverage summary is stale")
    for kind, field in [("generated-original", "generated_originals"), ("code-render-preview", "historical_code_previews"), ("reference-snapshot", "reference_snapshots"), ("runtime-atlas", "runtime_atlases")]:
        if sum(item["kind"] == kind for item in records) != coverage[field]:
            raise ValueError("asset coverage summary is stale: " + field)
    print(f"Verified {len(records)} PNG assets, {recorded} verbatim prompt records; {len(missing)} generation prompts explicitly unrecovered.")
    if require_prompts and missing:
        raise ValueError("original prompt coverage incomplete: " + ", ".join(missing))
    return data


def rebuild(data, output):
    output = output.resolve()
    if output == ROOT or any(output == ROOT / name or ROOT / name in output.parents for name in ["art", "assets", "src", "docs"]):
        raise ValueError("rebuild output must not overwrite repository source/assets")
    if output.exists() and any(output.iterdir()):
        raise ValueError("rebuild output must be a new or empty directory")
    output.mkdir(parents=True, exist_ok=True)
    for item in data["assets"]:
        if item["kind"] == "reference-snapshot":
            shutil.copyfile(repo_path(item["file"]["path"]), output / (item["cat_id"] + ".png"))
    source_dir = repo_path(data["rebuild"]["affection_manifest"]).parent
    subprocess.run(["cargo", "run", "--quiet", "--locked", "--release", "--example", "prepare_affection", "--", str(source_dir), str(output)], cwd=ROOT, check=True)
    for item in data["assets"]:
        if item["kind"] == "runtime-atlas":
            if digest(output / (item["cat_id"] + ".png")) != item["file"]["sha256"]:
                raise ValueError("rebuilt atlas differs from release: " + item["cat_id"])
    original_meta = json.loads(repo_path(data["rebuild"]["affection_metadata"]).read_text(encoding="utf-8"))
    generated_meta = json.loads((output / "affection-source.json").read_text(encoding="utf-8"))
    if generated_meta != original_meta:
        raise ValueError("rebuilt extraction metadata differs from release")
    print("Rebuilt all 12 runtime atlases: every PNG SHA-256 and extraction metadata match.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["verify", "rebuild"])
    parser.add_argument("--catalog", type=Path, default=CATALOG)
    parser.add_argument("--require-prompts", action="store_true", help="fail on explicitly unrecovered original prompts")
    parser.add_argument("--output", type=Path, help="keep rebuilt atlases in this new/empty directory (default: temporary verification)")
    args = parser.parse_args()
    data = verify(args.catalog, args.require_prompts)
    if args.command == "rebuild":
        if args.output:
            rebuild(data, args.output)
        else:
            with tempfile.TemporaryDirectory(prefix="clipcat-art-check-") as temp:
                rebuild(data, Path(temp))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print("art-assets: " + str(error), file=sys.stderr)
        sys.exit(1)
