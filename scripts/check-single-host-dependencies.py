#!/usr/bin/env python3
"""Enforce the canonical single-host database dependency boundary."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path
from typing import Any, Iterator


ROOT = Path(__file__).resolve().parents[1]
STORE_PACKAGE = "kanban-store-turso"
SERVER_PACKAGE = "kanban-server"
FORBIDDEN_LEGACY = {"kanban-sqlite", "kanban-local", "rusqlite"}
DATABASE_PACKAGES = FORBIDDEN_LEGACY | {"turso", STORE_PACKAGE}


def dependency_tables(manifest: dict[str, Any]) -> Iterator[tuple[str, dict[str, Any]]]:
    for table_name in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = manifest.get(table_name)
        if isinstance(table, dict):
            yield table_name, table

    targets = manifest.get("target")
    if not isinstance(targets, dict):
        return
    for target_name, target in targets.items():
        if not isinstance(target, dict):
            continue
        for table_name in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = target.get(table_name)
            if isinstance(table, dict):
                yield f"target.{target_name}.{table_name}", table


def dependency_package(alias: str, value: Any) -> str:
    if isinstance(value, dict):
        package = value.get("package")
        if isinstance(package, str):
            return package
    return alias


def main() -> int:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    members = workspace.get("workspace", {}).get("members")
    if not isinstance(members, list):
        print("single-host dependency gate: workspace.members is missing", file=sys.stderr)
        return 1

    failures: list[str] = []
    package_names: set[str] = set()
    for member in members:
        manifest_path = ROOT / str(member) / "Cargo.toml"
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package_name = manifest.get("package", {}).get("name")
        if not isinstance(package_name, str):
            failures.append(f"{manifest_path}: package.name is missing")
            continue
        package_names.add(package_name)

        for table_name, table in dependency_tables(manifest):
            for alias, value in table.items():
                dependency = dependency_package(alias, value)
                location = f"{manifest_path.relative_to(ROOT)} [{table_name}]"
                if dependency in FORBIDDEN_LEGACY:
                    failures.append(
                        f"{location}: active package {package_name} depends on legacy {dependency}"
                    )
                if dependency == "turso" and package_name != STORE_PACKAGE:
                    failures.append(
                        f"{location}: only {STORE_PACKAGE} may depend directly on turso"
                    )
                if dependency == STORE_PACKAGE and package_name != SERVER_PACKAGE:
                    failures.append(
                        f"{location}: only {SERVER_PACKAGE} may depend on {STORE_PACKAGE}"
                    )

    required = {STORE_PACKAGE, SERVER_PACKAGE, "kanban-client", "kanban-cli", "kanban-mcp"}
    missing = sorted(required - package_names)
    if missing:
        failures.append(f"required active packages are missing: {', '.join(missing)}")

    adapter_roots = [
        ROOT / "crates/kanban-client",
        ROOT / "crates/kanban-cli",
        ROOT / "crates/kanban-mcp",
        ROOT / "apps/desktop",
    ]
    forbidden_source_tokens = (
        "kanban_store_turso",
        "kanban_sqlite",
        "rusqlite::",
        "turso::",
    )
    for adapter_root in adapter_roots:
        for path in adapter_root.rglob("*"):
            if not path.is_file() or path.suffix not in {".rs", ".toml", ".ts", ".tsx"}:
                continue
            text = path.read_text(encoding="utf-8")
            for token in forbidden_source_tokens:
                if token in text:
                    failures.append(
                        f"{path.relative_to(ROOT)}: adapter contains forbidden database token {token}"
                    )

    if failures:
        print("single-host dependency gate failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(
        "single-host dependency gate passed: only kanban-server reaches kanban-store-turso, "
        "and only kanban-store-turso reaches turso"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
