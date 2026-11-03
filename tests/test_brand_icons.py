"""Shipping icon containers and bundle wiring, without launching the user's app."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class BrandIcons(unittest.TestCase):
    def test_windows_contains_native_shell_sizes(self):
        data = (ROOT / "assets/clipcat.ico").read_bytes()
        reserved, kind, count = struct.unpack_from("<HHH", data)
        self.assertEqual((reserved, kind), (0, 1))
        sizes = set()
        for i in range(count):
            w, h, _, _, planes, depth, length, offset = struct.unpack_from("<BBBBHHII", data, 6 + i * 16)
            self.assertEqual(w, h)
            self.assertEqual((planes, depth), (1, 32))
            self.assertLessEqual(offset + length, len(data))
            sizes.add(w or 256)
        self.assertTrue({16, 20, 24, 32, 40, 48, 64, 128, 256} <= sizes)

    def test_macos_contains_retina_master(self):
        path = ROOT / "assets/clipcat.icns"
        self.assertTrue(path.is_file(), "macOS needs a shipping ICNS resource")
        data = path.read_bytes()
        self.assertEqual(data[:4], b"icns")
        self.assertEqual(struct.unpack_from(">I", data, 4)[0], len(data))
        tags = set()
        offset = 8
        while offset < len(data):
            tag, length = struct.unpack_from(">4sI", data, offset)
            self.assertGreaterEqual(length, 8)
            tags.add(tag)
            offset += length
        self.assertEqual(offset, len(data))
        self.assertTrue({b"ic07", b"ic08", b"ic09", b"ic10"} <= tags)

    def test_macos_packager_copies_icon_before_signing(self):
        spec = importlib.util.spec_from_file_location("package_macos", ROOT / "scripts/package_macos_test.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertTrue(hasattr(module, "copy_bundle_icon"), "macOS packaging must install its icon resource")
        with tempfile.TemporaryDirectory() as tmp:
            contents = Path(tmp) / "ClipCat.app/Contents"
            name = module.copy_bundle_icon(contents)
            self.assertEqual(name, "ClipCat.icns")
            self.assertEqual((contents / "Resources" / name).read_bytes(), (ROOT / "assets/clipcat.icns").read_bytes())


if __name__ == "__main__":
    unittest.main()
