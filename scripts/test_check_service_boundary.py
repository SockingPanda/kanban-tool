#!/usr/bin/env python3
"""KanbanService host boundary gate 的最小回归测试。"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check-service-boundary.py")
SPEC = importlib.util.spec_from_file_location("service_boundary", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


class ServiceBoundaryGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        (self.root / GATE.SERVER_ROOT).mkdir(parents=True)
        (self.root / GATE.SERVICE_API).parent.mkdir(parents=True, exist_ok=True)
        (self.root / GATE.SERVICE_ROOT).parent.mkdir(parents=True, exist_ok=True)
        (self.root / GATE.SERVICE_API).write_text(
            "pub struct KanbanService;\n", encoding="utf-8"
        )
        (self.root / GATE.SERVICE_ROOT).write_text(
            "pub struct KanbanService;\n", encoding="utf-8"
        )
        (self.root / GATE.SERVER_ROOT / "state.rs").write_text(
            "pub struct AppState { application: KanbanService }\n", encoding="utf-8"
        )

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_clean_service_boundary_passes(self) -> None:
        self.assertEqual(GATE.check_boundary(self.root), [])

    def test_compatibility_core_is_rejected(self) -> None:
        (self.root / GATE.SERVER_ROOT / "state.rs").write_text(
            "use kanban_service::ApplicationService;\n", encoding="utf-8"
        )
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("ApplicationService" in failure for failure in failures))

    def test_missing_service_is_rejected(self) -> None:
        (self.root / GATE.SERVICE_API).write_text("", encoding="utf-8")
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("KanbanService" in failure for failure in failures))

    def test_public_store_reexport_is_rejected(self) -> None:
        (self.root / GATE.SERVICE_ROOT).write_text(
            "pub struct KanbanService;\npub use db::TursoStore;\n",
            encoding="utf-8",
        )
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("root 不得 public re-export" in failure for failure in failures))

    def test_public_vector_row_reexport_is_rejected(self) -> None:
        (self.root / GATE.SERVICE_ROOT).write_text(
            "pub struct KanbanService;\npub use vector::VectorStatusRecord;\n",
            encoding="utf-8",
        )
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("root 不得 public re-export" in failure for failure in failures))

    def test_public_vector_module_is_rejected(self) -> None:
        (self.root / GATE.SERVICE_ROOT).write_text(
            "pub struct KanbanService;\npub mod vector;\n",
            encoding="utf-8",
        )
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("root 不得 public re-export" in failure for failure in failures))

    def test_crate_private_store_reexport_is_allowed(self) -> None:
        (self.root / GATE.SERVICE_ROOT).write_text(
            "pub struct KanbanService;\npub(crate) use db::TursoStore;\n",
            encoding="utf-8",
        )
        self.assertEqual(GATE.check_boundary(self.root), [])


if __name__ == "__main__":
    unittest.main()
