#!/usr/bin/env python3
"""Focused regression tests for the single-host dependency gate."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check-single-host-dependencies.py")
SPEC = importlib.util.spec_from_file_location("single_host_gate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


REQUIRED = (
    "kanban-store-turso",
    "kanban-server",
    "kanban-client",
    "kanban-cli",
    "kanban-mcp",
)


class SingleHostDependencyGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        self._write_workspace()

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def _write_workspace(self, members: tuple[str, ...] = REQUIRED) -> None:
        member_lines = ",\n".join(f'  "crates/{name}"' for name in members)
        self._write(
            "Cargo.toml",
            f"""[workspace]
members = [
{member_lines}
]

[workspace.dependencies]
turso = {{ version = \"=0.7.2\", default-features = false }}
kanban-store-turso = {{ path = \"crates/kanban-store-turso\" }}
""",
        )
        for name in REQUIRED:
            dependencies = ""
            if name == "kanban-store-turso":
                dependencies = "\n[dependencies]\nturso.workspace = true\n"
            elif name == "kanban-server":
                dependencies = "\n[dependencies]\nkanban-store-turso.workspace = true\n"
            self._write(
                f"crates/{name}/Cargo.toml",
                f"[package]\nname = \"{name}\"\n{dependencies}",
            )

    def _write(self, relative: str, content: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def test_clean_manifest_graph_passes_and_source_text_is_irrelevant(self) -> None:
        self._write("crates/kanban-cli/src/lib.rs", '// kanban_store_turso is not a dependency\n')

        self.assertEqual(GATE.check_workspace(self.root), [])

    def test_renamed_store_dependency_is_rejected_from_cli(self) -> None:
        self._write(
            "crates/kanban-cli/Cargo.toml",
            """[package]
name = "kanban-cli"

[dependencies]
database = { package = "kanban-store-turso", path = "../kanban-store-turso" }
""",
        )

        failures = GATE.check_workspace(self.root)

        self.assertTrue(any("only kanban-server may depend on kanban-store-turso" in failure for failure in failures))

    def test_path_store_dependency_is_resolved_from_target_manifest(self) -> None:
        self._write(
            "crates/kanban-cli/Cargo.toml",
            """[package]
name = "kanban-cli"

[dependencies]
database = { path = "../kanban-store-turso" }
""",
        )

        failures = GATE.check_workspace(self.root)

        self.assertTrue(any("only kanban-server may depend on kanban-store-turso" in failure for failure in failures))

    def test_target_specific_dev_dependency_is_checked(self) -> None:
        self._write(
            "crates/kanban-cli/Cargo.toml",
            """[package]
name = "kanban-cli"

[target.'cfg(unix)'.dev-dependencies]
store = { package = "kanban-store-turso", path = "../kanban-store-turso" }
""",
        )

        failures = GATE.check_workspace(self.root)

        self.assertTrue(any("target.cfg(unix).dev-dependencies" in failure for failure in failures))

    def test_test_support_manifest_is_checked_even_when_not_a_member(self) -> None:
        self._write(
            "crates/kanban-test-support/Cargo.toml",
            """[package]
name = "kanban-test-support"

[dev-dependencies]
sqlite = { package = "kanban-sqlite", path = "../kanban-sqlite" }
""",
        )

        failures = GATE.check_workspace(self.root)

        self.assertTrue(any("kanban-test-support" in failure and "kanban-sqlite" in failure for failure in failures))

    def test_projection_helper_workspace_member_is_rejected_by_package_name(self) -> None:
        self._write_workspace(REQUIRED + ("kanban-vector-lancedb",))
        self._write(
            "crates/kanban-vector-lancedb/Cargo.toml",
            "[package]\nname = \"kanban-vector-lancedb\"\n",
        )

        failures = GATE.check_workspace(self.root)

        self.assertTrue(any("projection helper kanban-vector-lancedb" in failure for failure in failures))


if __name__ == "__main__":
    unittest.main()
