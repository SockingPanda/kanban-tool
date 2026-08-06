#!/usr/bin/env python3
"""Enforce the canonical single-host database dependency boundary.

The gate intentionally reads Cargo manifests instead of grepping source text.
That keeps comments, documentation, and unrelated package names from changing
the result while still covering normal, dev, build, and target-specific
dependencies.
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
# ``rusqlite`` remains forbidden for the retired backend, but the host-owned
# legacy import feature is an explicit, optional exception in
# ``kanban-service``.
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
    """Yield every package dependency table Cargo can use for a target."""

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
    """Resolve a Cargo dependency alias from parsed manifest metadata.

    Besides the explicit ``package =`` form, resolve local path dependencies
    and ``workspace = true`` aliases through their target Cargo.toml.  This
    prevents a forbidden package from hiding behind an innocuous dependency
    key such as ``store = { path = ... }`` or ``db = { workspace = true }``.
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
    """Allow only the optional rusqlite dependency owned by the store importer."""

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
    """Resolve ``workspace.members`` using Cargo's literal/glob path shape."""

    workspace = workspace_manifest.get("workspace")
    if not isinstance(workspace, dict):
        return [], ["workspace table is missing"]

    members = workspace.get("members")
    if not isinstance(members, list):
        return [], ["workspace.members is missing"]

    root_resolved = root.resolve()
    paths: list[Path] = []
    failures: list[str] = []
    seen: set[Path] = set()
    for member in members:
        if not isinstance(member, str):
            failures.append(f"workspace.members contains non-string entry {member!r}")
            continue

        has_glob = any(character in member for character in "*?[")
        candidates = sorted(root.glob(member)) if has_glob else [root / member]
        if not candidates:
            failures.append(f"workspace member pattern does not match a package: {member}")
            continue

        for candidate in candidates:
            manifest_path = (
                candidate if candidate.name == "Cargo.toml" else candidate / "Cargo.toml"
            )
            resolved = manifest_path.resolve()
            if not resolved.is_relative_to(root_resolved):
                failures.append(f"workspace member escapes repository root: {member}")
                continue
            if not manifest_path.is_file():
                failures.append(f"workspace member has no Cargo.toml: {member}")
                continue
            if resolved not in seen:
                paths.append(manifest_path)
                seen.add(resolved)

    return paths, failures


def _read_manifest(path: Path, failures: list[str]) -> dict[str, Any] | None:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        failures.append(f"{path}: cannot parse Cargo.toml: {error}")
        return None


def _is_test_support_manifest(path: Path, manifest: dict[str, Any]) -> bool:
    """Identify non-member test support packages without scanning source text."""

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
    # Keep discovery narrow and Cargo-manifest based.  This catches an
    # explicitly named fixture package moved under crates (or under tests)
    # without traversing vendored manifests or treating arbitrary source text
    # as a dependency.
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
    """Return deterministic gate failures for ``root`` without printing."""

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
            failures.append(f"{manifest_path}: package.name is missing")
            continue
        package_names.add(name)
        manifests.append((manifest_path, manifest, name))

        if name in FORBIDDEN_LEGACY:
            failures.append(
                f"{manifest_path.relative_to(root)}: legacy package {name} cannot be an active workspace member"
            )

    for manifest_path in _collect_test_support_paths(root, active_paths, failures):
        manifest = _read_manifest(manifest_path, failures)
        if manifest is None:
            continue
        name = package_name(manifest)
        if name is None:
            failures.append(f"{manifest_path}: package.name is missing")
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
                    f"Cargo.toml [workspace.dependencies]: legacy dependency {dependency} is retired"
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
                        f"{location}: active package {name} depends on legacy {dependency}"
                    )
                if dependency == "turso" and name != SERVICE_PACKAGE:
                    failures.append(
                        f"{location}: only {SERVICE_PACKAGE} may depend directly on turso"
                    )
                if dependency == SERVICE_PACKAGE and name != SERVER_PACKAGE:
                    failures.append(
                        f"{location}: only {SERVER_PACKAGE} may depend on {SERVICE_PACKAGE}"
                    )

    missing = sorted(REQUIRED_ACTIVE_PACKAGES - package_names)
    if missing:
        failures.append(f"required active packages are missing: {', '.join(missing)}")

    return failures


def main(root: Path = ROOT) -> int:
    failures = check_workspace(root)
    if failures:
        print("single-host dependency gate failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(
        "single-host dependency gate passed: only kanban-server reaches kanban-service, "
        "and only kanban-service reaches turso"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
