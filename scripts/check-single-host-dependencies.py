#!/usr/bin/env python3
"""强制执行 canonical single-host 数据库依赖边界。

该 gate 有意读取 Cargo manifest，而不是 grep 源码文本。这样注释、文档和
无关 package 名称不会改变结果，同时仍能覆盖 normal、dev、build 以及
target-specific dependency。
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path
from typing import Any, Iterator


ROOT = Path(__file__).resolve().parents[1]
SERVICE_PACKAGE = "kanban-service"
SERVER_PACKAGE = "kanban-server"
LEGACY_PACKAGES = {"kanban-sqlite", "kanban-local"}
# ``rusqlite`` 对 retired backend 仍然禁止，但 host-owned legacy import
# feature 是 ``kanban-service`` 中显式的 optional exception。
FORBIDDEN_LEGACY = LEGACY_PACKAGES | {"rusqlite"}
LEGACY_IMPORT_FEATURE = "legacy-sqlite-import"

REQUIRED_ACTIVE_PACKAGES = {
    SERVICE_PACKAGE,
    SERVER_PACKAGE,
    "kanban-client",
    "kanban-cli",
    "kanban-mcp",
}
TEST_SUPPORT_MANIFESTS = (Path("crates/kanban-test-support/Cargo.toml"),)


def dependency_tables(manifest: dict[str, Any]) -> Iterator[tuple[str, dict[str, Any]]]:
    """枚举 Cargo 可为 target 使用的全部 package dependency table。"""

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


def dependency_package(
    alias: str,
    value: Any,
    *,
    root: Path | None = None,
    manifest_path: Path | None = None,
    workspace_dependencies: dict[str, str] | None = None,
) -> str:
    """从已解析的 manifest metadata 中解析 Cargo dependency alias。

    除显式 ``package =`` 形式外，还要通过目标 Cargo.toml 解析 local path
    dependency 和 ``workspace = true`` alias。这样可避免被禁止的 package
    藏在看似无害的 dependency key 后面，例如 ``store = { path = ... }`` 或
    ``db = { workspace = true }``。
    """

    if not isinstance(value, dict):
        return alias

    package = value.get("package")
    if isinstance(package, str):
        return package

    if value.get("workspace") is True and workspace_dependencies is not None:
        workspace_package = workspace_dependencies.get(alias)
        if workspace_package is not None:
            return workspace_package

    path_value = value.get("path")
    if isinstance(path_value, str) and manifest_path is not None:
        is_root_manifest = root is not None and manifest_path.resolve() == root.resolve() / "Cargo.toml"
        base = root if is_root_manifest and root is not None else manifest_path.parent
        target = (base / path_value).resolve()
        target_manifest = target if target.name == "Cargo.toml" else target / "Cargo.toml"
        try:
            target_data = tomllib.loads(target_manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            return alias
        target_name = package_name(target_data)
        if target_name is not None:
            return target_name

    return alias


def package_name(manifest: dict[str, Any]) -> str | None:
    package = manifest.get("package")
    if not isinstance(package, dict):
        return None
    name = package.get("name")
    return name if isinstance(name, str) else None


def allows_store_legacy_import(
    package: str,
    manifest: dict[str, Any],
    table_name: str,
    alias: str,
    value: Any,
) -> bool:
    """只允许 store importer 所有的 optional rusqlite dependency。"""

    if package != SERVICE_PACKAGE or alias != "rusqlite" or table_name != "dependencies":
        return False
    if not isinstance(value, dict) or value.get("optional") is not True:
        return False
    features = manifest.get("features")
    enabled = features.get(LEGACY_IMPORT_FEATURE) if isinstance(features, dict) else None
    return isinstance(enabled, list) and "dep:rusqlite" in enabled


def workspace_member_paths(
    root: Path, workspace_manifest: dict[str, Any]
) -> tuple[list[Path], list[str]]:
    """按 Cargo 的 literal/glob path 形状解析 ``workspace.members``。"""

    workspace = workspace_manifest.get("workspace")
    if not isinstance(workspace, dict):
        return [], ["缺少 workspace table"]

    members = workspace.get("members")
    if not isinstance(members, list):
        return [], ["缺少 workspace.members"]

    root_resolved = root.resolve()
    paths: list[Path] = []
    failures: list[str] = []
    seen: set[Path] = set()
    for member in members:
        if not isinstance(member, str):
            failures.append(f"workspace.members 包含非字符串项 {member!r}")
            continue

        has_glob = any(character in member for character in "*?[")
        candidates = sorted(root.glob(member)) if has_glob else [root / member]
        if not candidates:
            failures.append(f"workspace member pattern 未匹配 package: {member}")
            continue

        for candidate in candidates:
            manifest_path = (
                candidate if candidate.name == "Cargo.toml" else candidate / "Cargo.toml"
            )
            resolved = manifest_path.resolve()
            if not resolved.is_relative_to(root_resolved):
                failures.append(f"workspace member 越出 repository root: {member}")
                continue
            if not manifest_path.is_file():
                failures.append(f"workspace member 缺少 Cargo.toml: {member}")
                continue
            if resolved not in seen:
                paths.append(manifest_path)
                seen.add(resolved)

    return paths, failures


def _read_manifest(path: Path, failures: list[str]) -> dict[str, Any] | None:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        failures.append(f"{path}: 无法解析 Cargo.toml: {error}")
        return None


def _is_test_support_manifest(path: Path, manifest: dict[str, Any]) -> bool:
    """不扫描源码文本，识别非 member 的 test support package。"""

    name = package_name(manifest)
    if name == "kanban-test-support" or (name and name.endswith("-test-support")):
        return True
    relative_parts = set(path.parts)
    return bool(relative_parts & {"test-fixtures", "test-support", "fixtures"})


def _collect_test_support_paths(
    root: Path,
    active_paths: set[Path],
    failures: list[str],
) -> list[Path]:
    paths: list[Path] = []
    seen = set(active_paths)

    candidates = [root / relative for relative in TEST_SUPPORT_MANIFESTS]
    # 保持发现范围窄且以 Cargo manifest 为依据。这样可捕获移动到 crates
    #（或 tests）下的显式命名 fixture package，而不会遍历 vendored manifest
    # 或把任意源码文本当作 dependency。
    candidates.extend(
        path
        for path in sorted(root.rglob("Cargo.toml"))
        if path not in candidates
        and not {".git", "node_modules", "target", "vendor"}.intersection(path.parts)
    )

    for path in candidates:
        if not path.is_file() or path.resolve() in seen:
            continue
        manifest = _read_manifest(path, failures)
        if manifest is None or not _is_test_support_manifest(path, manifest):
            continue
        paths.append(path)
        seen.add(path.resolve())
    return paths


def check_workspace(root: Path = ROOT) -> list[str]:
    """返回 ``root`` 的确定性 gate failures，但不打印结果。"""

    root = root.resolve()
    failures: list[str] = []
    workspace_path = root / "Cargo.toml"
    workspace_manifest = _read_manifest(workspace_path, failures)
    if workspace_manifest is None:
        return failures

    member_paths, member_failures = workspace_member_paths(root, workspace_manifest)
    failures.extend(f"{workspace_path}: {failure}" for failure in member_failures)

    active_paths = {path.resolve() for path in member_paths}
    package_names: set[str] = set()
    manifests: list[tuple[Path, dict[str, Any], str]] = []

    for manifest_path in member_paths:
        manifest = _read_manifest(manifest_path, failures)
        if manifest is None:
            continue
        name = package_name(manifest)
        if name is None:
            failures.append(f"{manifest_path}: 缺少 package.name")
            continue
        package_names.add(name)
        manifests.append((manifest_path, manifest, name))

        if name in FORBIDDEN_LEGACY:
            failures.append(
                f"{manifest_path.relative_to(root)}: legacy package {name} 不能作为 active workspace member"
            )

    for manifest_path in _collect_test_support_paths(root, active_paths, failures):
        manifest = _read_manifest(manifest_path, failures)
        if manifest is None:
            continue
        name = package_name(manifest)
        if name is None:
            failures.append(f"{manifest_path}: 缺少 package.name")
            continue
        manifests.append((manifest_path, manifest, name))

    workspace = workspace_manifest.get("workspace")
    workspace_dependencies = workspace.get("dependencies") if isinstance(workspace, dict) else None
    workspace_dependency_packages: dict[str, str] = {}
    if isinstance(workspace_dependencies, dict):
        for alias, value in workspace_dependencies.items():
            dependency = dependency_package(
                alias,
                value,
                root=root,
                manifest_path=workspace_path,
            )
            workspace_dependency_packages[alias] = dependency
            if dependency in FORBIDDEN_LEGACY and dependency != "rusqlite":
                failures.append(
                    f"Cargo.toml [workspace.dependencies]: legacy dependency {dependency} 已 retired"
                )

    for manifest_path, manifest, name in manifests:
        for table_name, table in dependency_tables(manifest):
            for alias, value in table.items():
                dependency = dependency_package(
                    alias,
                    value,
                    root=root,
                    manifest_path=manifest_path,
                    workspace_dependencies=workspace_dependency_packages,
                )
                location = f"{manifest_path.relative_to(root)} [{table_name}]"
                if dependency in FORBIDDEN_LEGACY and not allows_store_legacy_import(
                    name, manifest, table_name, alias, value
                ):
                    failures.append(
                        f"{location}: active package {name} 依赖 legacy {dependency}"
                    )
                if dependency == "turso" and name != SERVICE_PACKAGE:
                    failures.append(
                        f"{location}: 只有 {SERVICE_PACKAGE} 可以直接依赖 turso"
                    )
                if dependency == SERVICE_PACKAGE and name != SERVER_PACKAGE:
                    failures.append(
                        f"{location}: 只有 {SERVER_PACKAGE} 可以依赖 {SERVICE_PACKAGE}"
                    )

    missing = sorted(REQUIRED_ACTIVE_PACKAGES - package_names)
    if missing:
        failures.append(f"缺少 required active packages: {', '.join(missing)}")

    return failures


def main(root: Path = ROOT) -> int:
    failures = check_workspace(root)
    if failures:
        print("single-host dependency gate 失败:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(
        "single-host dependency gate 通过：只有 kanban-server 可达 kanban-service，"
        "且只有 kanban-service 可达 turso"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
