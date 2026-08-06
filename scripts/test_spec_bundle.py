#!/usr/bin/env python3
"""确定性 SPEC bundle 同步器的回归测试。"""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from stat import S_IMODE
from unittest.mock import patch

try:
    from scripts import spec_bundle
except ModuleNotFoundError:
    import spec_bundle


class SpecBundleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        for source in spec_bundle.SOURCE_PATHS:
            path = self.root / source
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"contents of {source}\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_render_uses_canonical_header_and_section_order(self) -> None:
        rendered = spec_bundle.render_bundle(self.root)

        previous = -1
        for source in spec_bundle.SOURCE_PATHS:
            self.assertIn(f"- {source}\n", rendered)
            offset = rendered.index(f"# 文件：{source}\n")
            self.assertGreater(offset, previous)
            previous = offset

        self.assertIn(
            "```rust\ncontents of crates/kanban-service/src/schema.rs\n```",
            rendered,
        )
        self.assertTrue(rendered.endswith("\n"))

    def test_render_rebases_relative_links_to_bundle_root(self) -> None:
        (self.root / "README.md").write_text(
            "[spec](docs/SPEC.md) [external](https://example.com) [section](#section)\n",
            encoding="utf-8",
        )
        (self.root / "docs/SPEC.md").write_text(
            "[state](STATE_MACHINE.md) "
            "[schema](../crates/kanban-service/src/schema.rs) "
            "[section](#section)\n",
            encoding="utf-8",
        )

        rendered = spec_bundle.render_bundle(self.root)

        self.assertIn("[spec](docs/SPEC.md)", rendered)
        self.assertIn("[external](https://example.com)", rendered)
        self.assertIn("[state](docs/STATE_MACHINE.md)", rendered)
        self.assertIn(
            "[schema](crates/kanban-service/src/schema.rs)", rendered
        )
        self.assertEqual(rendered.count("[section](#section)"), 2)

    def test_check_detects_source_drift_until_bundle_is_rewritten(self) -> None:
        spec_bundle.write_bundle(self.root)
        self.assertEqual(spec_bundle.check_bundle(self.root), None)
        if os.name != "nt":
            self.assertEqual(
                S_IMODE((self.root / spec_bundle.BUNDLE_PATH).stat().st_mode),
                0o644,
            )

        source = self.root / "docs/SPEC.md"
        source.write_text("changed source\n", encoding="utf-8")

        with self.assertRaisesRegex(spec_bundle.BundleDrift, "已过期"):
            spec_bundle.check_bundle(self.root)

        spec_bundle.write_bundle(self.root)
        self.assertEqual(spec_bundle.check_bundle(self.root), None)

    def test_missing_source_fails_closed(self) -> None:
        (self.root / "docs/API_SPEC.md").unlink()

        with self.assertRaisesRegex(spec_bundle.BundleError, "缺少 bundle source"):
            spec_bundle.render_bundle(self.root)

    def test_failed_atomic_replace_removes_temporary_file(self) -> None:
        with patch.object(spec_bundle.os, "replace", side_effect=OSError("replace failed")):
            with self.assertRaisesRegex(OSError, "replace failed"):
                spec_bundle.write_bundle(self.root)

        temporary_files = list(self.root.glob(f".{spec_bundle.BUNDLE_PATH}.*"))
        self.assertEqual(temporary_files, [])


if __name__ == "__main__":
    unittest.main()
