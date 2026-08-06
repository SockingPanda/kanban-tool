#!/usr/bin/env python3
"""检查 host 是否只通过 ``KanbanService`` 访问 canonical service。

这是源码级 gate：server 不能重新导入 compatibility core、持久化 store 或
store error。真正的类型边界仍由 Rust 编译器和 service tests 覆盖。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SERVER_ROOT = Path("crates/kanban-server/src")
SERVICE_API = Path("crates/kanban-service/src/service.rs")
FORBIDDEN_SERVER_SYMBOLS = re.compile(
    r"\b(?:ApplicationService|TursoApplicationStore|TursoStore|StoreError|"
    r"HostApplicationService)\b"
)


def check_boundary(root: Path = ROOT) -> list[str]:
    root = root.resolve()
    failures: list[str] = []
    service_path = root / SERVICE_API
    try:
        service_source = service_path.read_text(encoding="utf-8")
    except OSError as error:
        return [f"{SERVICE_API}: 无法读取 service API: {error}"]
    if not re.search(r"\bpub struct KanbanService\b", service_source):
        failures.append(f"{SERVICE_API}: 缺少 host-facing KanbanService")

    server_root = root / SERVER_ROOT
    try:
        server_files = sorted(server_root.rglob("*.rs"))
    except OSError as error:
        return [f"{SERVER_ROOT}: 无法扫描 server source: {error}"]
    for path in server_files:
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            failures.append(f"{path.relative_to(root)}: 无法读取 server source: {error}")
            continue
        match = FORBIDDEN_SERVER_SYMBOLS.search(source)
        if match:
            failures.append(
                f"{path.relative_to(root)}: server 不得引用 service/store symbol {match.group(0)}"
            )

    state_path = server_root / "state.rs"
    try:
        state_source = state_path.read_text(encoding="utf-8")
    except OSError as error:
        failures.append(f"{state_path.relative_to(root)}: 无法读取 state source: {error}")
    else:
        if "application: KanbanService" not in state_source:
            failures.append(f"{state_path.relative_to(root)}: AppState 必须只持有 KanbanService")
    return failures


def main(root: Path = ROOT) -> int:
    failures = check_boundary(root)
    if failures:
        print("service boundary gate 失败:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("service boundary gate 通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
