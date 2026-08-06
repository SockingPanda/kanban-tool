#!/usr/bin/env python3
"""检查 kanban-server 的 Vector service boundary。

该 gate 只读取源码文本，专门守住 adapter 不得重新获得 canonical store handle
的静态约束；协议 fixture 和真实路由行为由 Rust tests 覆盖。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SERVER_FILES = (
    Path("crates/kanban-server/src/state.rs"),
    Path("crates/kanban-server/src/vector.rs"),
    Path("crates/kanban-server/src/dispatcher.rs"),
)
SERVICE_API = Path("crates/kanban-service/src/operations/vector.rs")
FORBIDDEN_SERVER_SYMBOLS = re.compile(
    r"\b(?:TursoApplicationStore|TursoStore|StoreError|VectorConfig|"
    r"VectorStatusRecord|VectorChunkHitRecord|VectorLabelAtomHitRecord|"
    r"ProjectionJobRecord)\b|\bvector_store\b"
)
REQUIRED_SERVICE_METHODS = (
    "vector_status",
    "configure_vector",
    "rebuild_vector",
    "sync_vector",
    "enqueue_vector_jobs",
    "query_vector_chunks",
    "query_vector_label_atoms",
    "vector_worker_tick",
)
REQUIRED_SERVER_METHODS = (
    "vector_status",
    "configure_vector",
    "rebuild_vector",
    "sync_vector",
    "query_vector_chunks",
    "query_vector_label_atoms",
    "vector_worker_tick",
)


def check_boundary(root: Path = ROOT) -> list[str]:
    """返回确定性 boundary failures，不打印结果。"""

    root = root.resolve()
    failures: list[str] = []
    service_path = root / SERVICE_API
    try:
        service_source = service_path.read_text(encoding="utf-8")
    except OSError as error:
        return [f"{SERVICE_API}: 无法读取 service API: {error}"]

    for method in REQUIRED_SERVICE_METHODS:
        if not re.search(rf"\bpub async fn {re.escape(method)}\b", service_source):
            failures.append(f"{SERVICE_API}: 缺少 public service method {method}")

    for relative in SERVER_FILES:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            failures.append(f"{relative}: 无法读取 server source: {error}")
            continue
        match = FORBIDDEN_SERVER_SYMBOLS.search(source)
        if match:
            failures.append(f"{relative}: server 不得引用 store/vector row symbol {match.group(0)}")

    state_path = root / SERVER_FILES[0]
    try:
        state_source = state_path.read_text(encoding="utf-8")
    except OSError:
        state_source = ""
    if "application: HostApplicationService" not in state_source:
        failures.append(f"{SERVER_FILES[0]}: AppState 必须只持有 HostApplicationService")

    vector_source = (root / SERVER_FILES[1]).read_text(encoding="utf-8")
    dispatcher_source = (root / SERVER_FILES[2]).read_text(encoding="utf-8")
    for method in REQUIRED_SERVER_METHODS[:-1]:
        if not re.search(rf"application\(\)\s*\.\s*{re.escape(method)}\b", vector_source):
            failures.append(f"{SERVER_FILES[1]}: vector route 未通过 application().{method}")
    if not re.search(r"application\(\)\s*\.\s*vector_worker_tick\b", dispatcher_source):
        failures.append(f"{SERVER_FILES[2]}: dispatcher 未通过 application().vector_worker_tick")
    return failures


def main(root: Path = ROOT) -> int:
    failures = check_boundary(root)
    if failures:
        print("vector service boundary gate 失败:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("vector service boundary gate 通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
