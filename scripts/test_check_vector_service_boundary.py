#!/usr/bin/env python3
"""Vector service boundary gate 的最小静态回归测试。"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check-vector-service-boundary.py")
SPEC = importlib.util.spec_from_file_location("vector_service_boundary", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


SERVICE_METHODS = GATE.REQUIRED_SERVICE_METHODS


class VectorServiceBoundaryGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        for relative in GATE.SERVER_FILES:
            (self.root / relative).parent.mkdir(parents=True, exist_ok=True)
        (self.root / GATE.SERVICE_API).parent.mkdir(parents=True, exist_ok=True)
        service_methods = "\n".join(f"pub async fn {method}(&self) {{}}" for method in SERVICE_METHODS)
        self._write(GATE.SERVICE_API, service_methods)
        self._write(
            GATE.SERVER_FILES[0],
            "pub struct AppState { application: HostApplicationService }",
        )
        self._write(
            GATE.SERVER_FILES[1],
            "\n".join(f"state.application().{method}();" for method in SERVICE_METHODS[:-1]),
        )
        self._write(
            GATE.SERVER_FILES[2],
            "state.application().vector_worker_tick();",
        )

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def _write(self, relative: Path, content: str) -> None:
        path = self.root / relative
        path.write_text(content, encoding="utf-8")

    def test_clean_service_api_and_server_sources_pass(self) -> None:
        self.assertEqual(GATE.check_boundary(self.root), [])

    def test_forbidden_store_symbol_in_server_is_rejected(self) -> None:
        self._write(GATE.SERVER_FILES[1], "use kanban_service::TursoStore;")
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("TursoStore" in failure for failure in failures))

    def test_missing_service_method_is_rejected(self) -> None:
        self._write(GATE.SERVICE_API, "pub async fn vector_status(&self) {}")
        failures = GATE.check_boundary(self.root)
        self.assertTrue(any("缺少 public service method" in failure for failure in failures))


if __name__ == "__main__":
    unittest.main()
