"""Offline packaging regressions using real archive checksums and verbatim legal bytes."""

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "package-third-party-notices.py"
SPEC = importlib.util.spec_from_file_location("third_party_notices", SCRIPT)
notices = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(notices)


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.package = {"name": "example", "version": "1.0.0", "source":
                        "registry+https://github.com/rust-lang/crates.io-index"}

    def archive(self, materials, extra=None, license_expression="MIT"):
        archive = self.root / "example.crate"
        with tarfile.open(archive, "w:gz") as stream:
            manifest = (f'[package]\nname="{self.package["name"]}"\n'
                        f'version="{self.package["version"]}"\n'
                        f'license="{license_expression}"\n').encode()
            for path, data in [("Cargo.toml", manifest), *materials]:
                item = tarfile.TarInfo(
                    f'{self.package["name"]}-{self.package["version"]}/{path}')
                item.size = len(data)
                stream.addfile(item, io.BytesIO(data))
            if extra is not None:
                stream.addfile(extra)
        self.package["checksum"] = hashlib.sha256(archive.read_bytes()).hexdigest()
        return archive

    def rust(self):
        root = self.root / "toolchain/share/doc/rust"
        (root / "licenses").mkdir(parents=True)
        for name, data in [("COPYRIGHT-library.html", b"<p>actual attribution</p>\r\n"),
                           ("licenses/MIT.txt", b"complete MIT fixture\n"),
                           ("licenses/Apache-2.0.txt", b"complete Apache fixture\n")]:
            (root / name).write_bytes(data)
        return self.root / "toolchain"

    def test_verbatim_licenses_notices_and_copyright_are_preserved(self):
        legal = [("LICENSE-MIT", "Copyright Example\r\nLegal terms: \u2014\n".encode()),
                 ("vendor/NOTICE", b"Mandatory vendor NOTICE\n"),
                 ("COPYRIGHT", b"Upstream copyright holder\n")]
        expression, actual, authors = notices.crate_materials(self.archive(legal), self.package)
        self.assertEqual(expression, "MIT")
        self.assertEqual(actual, sorted(legal))
        self.assertEqual(authors, [])

    def test_checksum_duplicate_escaping_and_indirect_legal_members_fail(self):
        archive = self.archive([("LICENSE", b"legal")])
        self.package["checksum"] = "0" * 64
        with self.assertRaises(ValueError):
            notices.crate_materials(archive, self.package)
        for path, data in [("LICENSE", b"duplicate"), ("../LICENSE", b"escape")]:
            archive = self.archive([("LICENSE", b"legal"), (path, data)])
            with self.assertRaises(ValueError):
                notices.crate_materials(archive, self.package)
        link = tarfile.TarInfo("example-1.0.0/NOTICE")
        link.type = tarfile.SYMTYPE
        link.linkname = "LICENSE"
        with self.assertRaises(ValueError):
            notices.crate_materials(self.archive([("LICENSE", b"legal")], link), self.package)

    def test_font_archive_preserves_all_four_named_notices_without_font_assets(self):
        self.package.update(name="epaint_default_fonts", version="0.36.2")
        legal = [("fonts/OFL.txt", b"SIL Open Font License and original holders\r\n"),
                 ("fonts/UFL.txt", b"Ubuntu Font License and original holders\n"),
                 ("fonts/Hack-Regular.txt", b"MIT and Bitstream Vera terms\n"),
                 ("fonts/emoji-icon-font-mit-license.txt", b"Icon font MIT notice\n")]
        assets = [("fonts/Hack-Regular.ttf", b"binary\0font"),
                  ("fonts/unrelated.txt", b"\xffnot legal text"),
                  ("src/generated.rs", b"\xffsource")]
        expression = "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0"
        actual_expression, actual, _ = notices.crate_materials(
            self.archive(legal + assets, license_expression=expression), self.package)
        self.assertEqual(actual_expression, expression)
        self.assertEqual(actual, sorted(legal))
        self.package["name"] = "example"
        _, actual, _ = notices.crate_materials(self.archive(legal + assets), self.package)
        self.assertEqual(actual, sorted(legal[:2]))

    def test_accessibility_authors_and_derived_license_survive_complete_document(self):
        self.package.update(name="accesskit", version="0.24.1")
        legal = [("LICENSE-MIT", b"MIT terms\r\n"),
                 ("LICENSE-APACHE", b"Apache terms\n"),
                 ("LICENSE.chromium", b"Chromium BSD terms and copyrights\n"),
                 ("AUTHORS", b"Original copyright authors\r\n")]
        archive = self.archive(legal + [("AUTHORS.json", b"\xffunrelated asset"),
                                       ("src/data.bin", b"binary\0")])
        _, actual, _ = notices.crate_materials(archive, self.package)
        self.assertEqual(actual, sorted(legal))
        cache = self.root / "cargo/registry/cache/index"
        cache.mkdir(parents=True)
        (cache / "accesskit-0.24.1.crate").write_bytes(archive.read_bytes())
        body = notices.document(notices.TARGETS[0], [self.package], self.root / "cargo",
                                self.root, self.rust())
        for path, data in legal:
            self.assertIn(f"--- {path} ---\n".encode() + data, body)

    def test_new_legal_names_are_bounded_regular_utf8_materials(self):
        self.package.update(name="epaint_default_fonts", version="0.36.2")
        for path in ["AUTHORS", "fonts/OFL.txt", "fonts/UFL.txt", "fonts/Hack-Regular.txt",
                     "fonts/emoji-icon-font-mit-license.txt"]:
            for body in [b"", b"nul\0", b"\xff", b"a" * (notices.MAX_LEGAL_BYTES + 1)]:
                with self.assertRaises((ValueError, UnicodeDecodeError)):
                    notices.crate_materials(self.archive([(path, body)]), self.package)
            link = tarfile.TarInfo(f"epaint_default_fonts-0.36.2/{path}")
            link.type = tarfile.SYMTYPE
            link.linkname = "unrelated.bin"
            with self.assertRaises(ValueError):
                notices.crate_materials(self.archive([], link), self.package)

    def test_fallback_requires_exact_package_archive_and_legal_checksum(self):
        self.archive([])
        root = self.root / "support/third-party-licenses"
        root.mkdir(parents=True)
        body = b"Retained upstream license and copyright\n"
        (root / "legal.txt").write_bytes(body)
        entry = {"archive_sha256": self.package["checksum"], "file": "legal.txt",
                 "sha256": notices.digest(body), "url": "https://example.invalid/pinned/LICENSE",
                 "declared_spdx_expression": "MIT", "license_terms": ["MIT"]}
        manifest = {"packages": {"example@1.0.0": entry}, "canonical_terms": {
            "MIT": {"file": "legal.txt", "sha256": notices.digest(body), "url": entry["url"]}}}
        (root / "manifest.json").write_text(json.dumps(manifest))
        self.assertEqual(notices.retained_fallback(self.root, self.package, "MIT")[0][1], body)
        (root / "legal.txt").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            notices.retained_fallback(self.root, self.package, "MIT")
        self.package["checksum"] = "0" * 64
        with self.assertRaises(ValueError):
            notices.retained_fallback(self.root, self.package, "MIT")

    def test_shipping_resolver_is_locked_offline_normal_build_and_includes_linux_helper(self):
        (self.root / "Cargo.lock").write_text(
            '[[package]]\nname="retonr-cli"\nversion="0.1.0"\n'
            '[[package]]\nname="rewrite-runtime-isolation"\nversion="0.1.0"\n'
            '[[package]]\nname="example"\nversion="1.0.0"\nsource="registry"\nchecksum="abc"\n'
        )
        with patch.object(notices.subprocess, "check_output", return_value=
                          "retonr-cli v0.1.0 (/workspace)\nexample v1.0.0\n") as command:
            packages = notices.selected_packages(self.root, notices.TARGETS[0])
        self.assertEqual([p["name"] for p in packages], ["example"])
        self.assertEqual(command.call_count, 2)
        for args, _ in command.call_args_list:
            invocation = args[0]
            self.assertIn("--locked", invocation)
            self.assertIn("--offline", invocation)
            self.assertIn("normal,build", invocation)
            self.assertNotIn("--all-features", invocation)

    def test_complete_document_is_deterministic_and_rust_material_is_verbatim(self):
        archive = self.archive([("LICENSE", b"Actual upstream terms\r\n")])
        cache = self.root / "cargo/registry/cache/index"
        cache.mkdir(parents=True)
        (cache / "example-1.0.0.crate").write_bytes(archive.read_bytes())
        sysroot = self.rust()
        first = notices.document(notices.TARGETS[0], [self.package], self.root / "cargo", self.root, sysroot)
        second = notices.document(notices.TARGETS[0], [self.package], self.root / "cargo", self.root, sysroot)
        self.assertEqual(first, second)
        self.assertIn(b"Actual upstream terms\r\n", first)
        self.assertIn(b"<p>actual attribution</p>\r\n", first)
        (sysroot / "share/doc/rust/licenses/MIT.txt").unlink()
        with self.assertRaises(ValueError):
            notices.rust_materials(sysroot)

    def test_missing_archive_and_invalid_legal_bytes_fail(self):
        self.package["checksum"] = "0" * 64
        with self.assertRaises(ValueError):
            notices.cached_archive(self.root, self.package)
        for body in [b"", b"nul\0", b"\xff", b"a" * (notices.MAX_LEGAL_BYTES + 1)]:
            with self.assertRaises((ValueError, UnicodeDecodeError)):
                notices.legal_bytes(body)


if __name__ == "__main__":
    unittest.main()
