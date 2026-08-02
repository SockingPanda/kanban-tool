#!/usr/bin/env python3
"""schema dependency isolation shell gate 的 fake-cargo 回归测试。"""

from __future__ import annotations

import copy
import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import textwrap
import tomllib
import unittest
from unittest import mock
from pathlib import Path

import schema_dependency_policy as dependency_policy


ROOT = Path(__file__).resolve().parents[1]
GATE = ROOT / "scripts/test-schema-cargo-tree.sh"
PRODUCTS = (
    "kanban-cli",
    "kanban-server",
    "kanban-sqlite",
    "kanban-desktop",
    "kanban-vector-lancedb",
    "kanban-graph-oxigraph",
)
TOOL_PACKAGE = "kanban-schema-tool"
CONTRACT_PACKAGE = "kanban-contract"
TOOL_MEMBER = "crates/kanban-schema-tool"


CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
CONTRACT_PATH = str(ROOT / "crates/kanban-contract")
CONTRACT_MANIFEST_PATH = str(ROOT / "crates/kanban-contract/Cargo.toml")
TOOL_MANIFEST_PATH = str(ROOT / TOOL_MEMBER / "Cargo.toml")
CONTRACT_ID = f"path+file:///workspace/{CONTRACT_PACKAGE}#2.1.3"
TOOL_ID = f"path+file:///workspace/{TOOL_PACKAGE}#2.1.3"
INTERNAL_PACKAGE = "kanban-context"
REGISTRY_VERSIONS = {
    "jsonschema": "0.47.0",
    "schemars": "1.2.1",
    "serde": "1.0.228",
    "serde_json": "1.0.150",
    "sha2": "0.10.9",
}
TOOL_EDGE_SIGNATURES = {
    "jsonschema": {
        "source": CRATES_IO_SOURCE,
        "req": "^0.47.0",
        "uses_default_features": False,
        "features": [],
    },
    "kanban-contract": {
        "source": None,
        "req": "*",
        "uses_default_features": False,
        "features": ["schema"],
        "path": CONTRACT_PATH,
    },
    "serde": {
        "source": CRATES_IO_SOURCE,
        "req": "^1.0",
        "uses_default_features": True,
        "features": ["derive"],
    },
    "serde_json": {
        "source": CRATES_IO_SOURCE,
        "req": "^1.0",
        "uses_default_features": True,
        "features": [],
    },
    "sha2": {
        "source": CRATES_IO_SOURCE,
        "req": "^0.10",
        "uses_default_features": True,
        "features": [],
    },
}
CONTRACT_EDGE_SIGNATURES = {
    "schemars": {
        "source": CRATES_IO_SOURCE,
        "req": "^1.2.1",
        "optional": True,
        "uses_default_features": False,
        "features": ["std", "derive"],
    },
    "serde": {
        "source": CRATES_IO_SOURCE,
        "req": "^1.0",
        "uses_default_features": True,
        "features": ["derive"],
    },
    "serde_json": {
        "source": CRATES_IO_SOURCE,
        "req": "^1.0",
        "uses_default_features": True,
        "features": [],
    },
}

TOOL_MANIFEST_DEPENDENCIES = {
    "jsonschema": {"workspace": True},
    "kanban-contract": {
        "workspace": True,
        "default-features": False,
        "features": ["schema"],
    },
    "serde": {"workspace": True},
    "serde_json": {"workspace": True},
    "sha2": {"workspace": True},
}
CONTRACT_MANIFEST_FEATURES = {
    "default": [],
    "schema": ["dep:schemars"],
}
CONTRACT_MANIFEST_DEPENDENCIES = {
    "schemars": {"workspace": True, "optional": True},
    "serde": {"workspace": True},
    "serde_json": {"workspace": True},
}
WORKSPACE_CANONICAL_DEPENDENCIES = {
    "jsonschema": {"version": "0.47.0", "default-features": False},
    "kanban-contract": {
        "path": "crates/kanban-contract",
        "default-features": False,
    },
    "serde": {"version": "1.0", "features": ["derive"]},
    "schemars": {
        "version": "1.2.1",
        "default-features": False,
        "features": ["std", "derive"],
    },
    "serde_json": "1.0",
    "sha2": "0.10",
}


def dependency(
    name: str,
    *,
    kind: str | None = None,
    rename: str | None = None,
    optional: bool = False,
    target: str | None = None,
    source: str | None = None,
    path: str | None = None,
    uses_default_features: bool = True,
    features: list[str] | None = None,
    req: str = "*",
) -> dict[str, object]:
    record: dict[str, object] = {
        "name": name,
        "source": source,
        "req": req,
        "kind": kind,
        "rename": rename,
        "optional": optional,
        "uses_default_features": uses_default_features,
        "features": list(features or []),
        "target": target,
        "registry": None,
    }
    if path is not None:
        record["path"] = path
    return record


def tool_dependency(name: str, **overrides: object) -> dict[str, object]:
    signature = dict(TOOL_EDGE_SIGNATURES[name])
    signature["features"] = list(signature["features"])
    signature.update(overrides)
    return dependency(name, **signature)


def contract_dependency(name: str, **overrides: object) -> dict[str, object]:
    signature = dict(CONTRACT_EDGE_SIGNATURES[name])
    signature["features"] = list(signature["features"])
    signature.update(overrides)
    return dependency(name, **signature)


def registry_id(name: str) -> str:
    return f"{CRATES_IO_SOURCE}#{name}@{REGISTRY_VERSIONS[name]}"


def registry_package(name: str) -> dict[str, object]:
    version = REGISTRY_VERSIONS[name]
    return package(
        name,
        package_id=registry_id(name),
        source=CRATES_IO_SOURCE,
        manifest_path=f"/cargo/registry/{name}-{version}/Cargo.toml",
        version=version,
    )


def resolved_dependency(name: str, package_id: str) -> dict[str, object]:
    return {
        "name": name.replace("-", "_"),
        "pkg": package_id,
        "dep_kinds": [{"kind": None, "target": None}],
    }


def cargo_target(name: str, kind: str, src_path: str) -> dict[str, object]:
    return {
        "name": name,
        "kind": [kind],
        "src_path": src_path,
    }


def canonical_targets(name: str) -> list[dict[str, object]]:
    if name == TOOL_PACKAGE:
        return [
            cargo_target(
                "kanban_schema_tool",
                "lib",
                str(ROOT / TOOL_MEMBER / "src/lib.rs"),
            ),
            cargo_target(
                "kanban-schema",
                "bin",
                str(ROOT / TOOL_MEMBER / "src/bin/kanban-schema.rs"),
            ),
            cargo_target(
                "tooling",
                "test",
                str(ROOT / TOOL_MEMBER / "tests/tooling.rs"),
            ),
        ]
    if name == CONTRACT_PACKAGE:
        return [
            cargo_target(
                "kanban_contract",
                "lib",
                str(ROOT / "crates/kanban-contract/src/lib.rs"),
            ),
            cargo_target(
                "foundation",
                "test",
                str(ROOT / "crates/kanban-contract/tests/foundation.rs"),
            ),
            cargo_target(
                "g0_metadata",
                "test",
                str(ROOT / "crates/kanban-contract/tests/g0_metadata.rs"),
            ),
        ]
    return []


def resolve_node(
    package_id: str,
    dependencies: list[dict[str, object]] | None = None,
    *,
    features: list[str] | None = None,
) -> dict[str, object]:
    resolved = dependencies or []
    return {
        "id": package_id,
        "dependencies": [dependency["pkg"] for dependency in resolved],
        "deps": resolved,
        "features": features or [],
    }


def package(
    name: str,
    dependencies: list[dict[str, object]] | None = None,
    *,
    package_id: str | None = None,
    source: str | None = None,
    manifest_path: str | None = None,
    version: str = "2.1.3",
    targets: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    if manifest_path is None:
        if name == TOOL_PACKAGE:
            manifest_path = TOOL_MANIFEST_PATH
        elif name == CONTRACT_PACKAGE:
            manifest_path = CONTRACT_MANIFEST_PATH
        else:
            manifest_path = f"/workspace/{name}/Cargo.toml"
    return {
        "id": package_id or f"path+file:///workspace/{name}#2.1.3",
        "name": name,
        "version": version,
        "source": source,
        "manifest_path": manifest_path,
        "dependencies": dependencies or [],
        "targets": canonical_targets(name) if targets is None else targets,
    }


def valid_phase_one_metadata() -> dict[str, object]:
    contract = package(
        CONTRACT_PACKAGE,
        [contract_dependency(name) for name in CONTRACT_EDGE_SIGNATURES],
    )
    tool = package(
        TOOL_PACKAGE,
        [tool_dependency(name) for name in TOOL_EDGE_SIGNATURES],
    )
    workspace = [contract, tool]
    workspace.extend(package(name) for name in (*PRODUCTS, INTERNAL_PACKAGE))
    registry = [registry_package(name) for name in REGISTRY_VERSIONS]
    nodes = [resolve_node(record["id"]) for record in (*workspace, *registry)]
    tool_dependencies = [
        resolved_dependency(
            name,
            CONTRACT_ID if name == CONTRACT_PACKAGE else registry_id(name),
        )
        for name in TOOL_EDGE_SIGNATURES
    ]
    contract_dependencies = [
        resolved_dependency(name, registry_id(name))
        for name in CONTRACT_EDGE_SIGNATURES
    ]
    node = next(record for record in nodes if record["id"] == TOOL_ID)
    node.update(resolve_node(TOOL_ID, tool_dependencies))
    node = next(record for record in nodes if record["id"] == CONTRACT_ID)
    node.update(
        resolve_node(
            CONTRACT_ID,
            contract_dependencies,
            features=["schema"],
        )
    )
    resolved_node_by_id = {record["id"]: record for record in nodes}
    resolved_node_by_id[registry_id("jsonschema")]["features"] = []
    resolved_node_by_id[registry_id("schemars")]["features"] = [
        "derive", "schemars_derive", "std"
    ]
    return {
        "workspace_members": [record["id"] for record in workspace],
        "packages": [*workspace, *registry],
        "resolve": {"nodes": nodes, "root": TOOL_ID},
    }


def metadata_closure_ids(metadata: dict[str, object]) -> set[str]:
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [metadata["resolve"]["root"]]
    visited: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        pending.extend(nodes[package_id]["dependencies"])
    return visited


def lock_checksum(name: str, version: str, source: str) -> str:
    return hashlib.sha256(f"{name}@{version}@{source}".encode()).hexdigest()


def valid_lockfile(metadata: dict[str, object]) -> dict[str, object]:
    packages = []
    for record in metadata["packages"]:
        package_record = {
            "name": record["name"],
            "version": record["version"],
        }
        source = record.get("source")
        if source is not None:
            package_record.update(
                {
                    "source": source,
                    "checksum": lock_checksum(
                        record["name"], record["version"], source
                    ),
                }
            )
        packages.append(package_record)
    return {"version": 4, "package": packages}


def valid_approved_snapshot(
    metadata: dict[str, object], lockfile: dict[str, object]
) -> dict[str, object]:
    closure = metadata_closure_ids(metadata)
    packages_by_id = {record["id"]: record for record in metadata["packages"]}
    lock_by_identity = {
        (record["name"], record["version"], record.get("source")): record
        for record in lockfile["package"]
    }
    approved = []
    for package_id in closure:
        record = packages_by_id[package_id]
        source = record.get("source")
        if source is None:
            continue
        lock_record = lock_by_identity[(record["name"], record["version"], source)]
        approved.append(
            {
                "name": record["name"],
                "version": record["version"],
                "source": source,
                "checksum": lock_record["checksum"],
            }
        )
    approved.sort(
        key=lambda record: (record["name"], record["version"], record["source"])
    )
    return {
        "format_version": 1,
        "lockfile_version": 4,
        "root_package": TOOL_PACKAGE,
        "packages": approved,
    }


def add_getrandom_versions(metadata: dict[str, object]) -> None:
    parents = (registry_id("jsonschema"), registry_id("sha2"))
    for version, parent_id in zip(("0.2.17", "0.3.4"), parents, strict=True):
        package_id = f"{CRATES_IO_SOURCE}#getrandom@{version}"
        metadata["packages"].append(
            package(
                "getrandom",
                package_id=package_id,
                source=CRATES_IO_SOURCE,
                manifest_path=f"/cargo/registry/getrandom-{version}/Cargo.toml",
                version=version,
                targets=[],
            )
        )
        metadata["resolve"]["nodes"].append(resolve_node(package_id))
        parent = resolved_node(metadata, parent_id)
        edge = resolved_dependency("getrandom", package_id)
        parent["deps"].append(edge)
        parent["dependencies"].append(package_id)


def workspace_package(metadata: dict[str, object], name: str) -> dict[str, object]:
    return next(record for record in metadata["packages"] if record["name"] == name)


def package_record(metadata: dict[str, object], package_id: str) -> dict[str, object]:
    return next(record for record in metadata["packages"] if record["id"] == package_id)


def resolved_node(metadata: dict[str, object], package_id: str) -> dict[str, object]:
    return next(node for node in metadata["resolve"]["nodes"] if node["id"] == package_id)


def resolved_edge(
    metadata: dict[str, object], parent_id: str, dependency_name: str
) -> dict[str, object]:
    crate_name = dependency_name.replace("-", "_")
    return next(
        edge
        for edge in resolved_node(metadata, parent_id)["deps"]
        if edge["name"] == crate_name
    )


def replace_resolved_registry_package(
    metadata: dict[str, object], name: str, source: str | None
) -> str:
    old_id = registry_id(name)
    if source is None:
        new_id = f"path+file:///override/{name}#{REGISTRY_VERSIONS[name]}"
        manifest_path = f"/override/{name}/Cargo.toml"
    else:
        new_id = f"{source}#{name}@{REGISTRY_VERSIONS[name]}"
        manifest_path = f"/git/checkouts/{name}/Cargo.toml"
    record = package_record(metadata, old_id)
    record.update(
        {
            "id": new_id,
            "source": source,
            "manifest_path": manifest_path,
        }
    )
    resolved_node(metadata, old_id)["id"] = new_id
    for node in metadata["resolve"]["nodes"]:
        node["dependencies"] = [
            new_id if package_id == old_id else package_id
            for package_id in node["dependencies"]
        ]
        for edge in node["deps"]:
            if edge["pkg"] == old_id:
                edge["pkg"] = new_id
    return new_id


def add_transitive_override(
    metadata: dict[str, object], source: str | None
) -> str:
    name = "transport-override"
    if source is None:
        package_id = "path+file:///override/transport-override#1.0.0"
        manifest_path = "/override/transport-override/Cargo.toml"
    else:
        package_id = f"{source}#transport-override@1.0.0"
        manifest_path = "/git/checkouts/transport-override/Cargo.toml"
    metadata["packages"].append(
        package(
            name,
            package_id=package_id,
            source=source,
            manifest_path=manifest_path,
            version="1.0.0",
        )
    )
    metadata["resolve"]["nodes"].append(resolve_node(package_id))
    parent_node = resolved_node(metadata, registry_id("jsonschema"))
    edge = resolved_dependency(name, package_id)
    parent_node["deps"].append(edge)
    parent_node["dependencies"].append(package_id)
    package_record(metadata, registry_id("jsonschema"))["dependencies"].append(
        dependency(name, source=source, path=manifest_path.removesuffix("/Cargo.toml"))
    )
    return package_id


class DependencyIsolationGateTests(unittest.TestCase):
    def run_gate(
        self, leak: str | None = None, tool_leak_package: str | None = None
    ) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            cargo = temp / "cargo"
            log = temp / "cargo.jsonl"
            cargo.write_text(
                textwrap.dedent(
                    """\
                    #!/usr/bin/env python3
                    import json
                    import os
                    import sys

                    args = sys.argv[1:]
                    with open(os.environ["FAKE_CARGO_LOG"], "a", encoding="utf-8") as handle:
                        handle.write(json.dumps(args) + "\\n")
                    package = args[args.index("-p") + 1]
                    leak = os.environ.get("FAKE_CARGO_LEAK")
                    tool_leak_package = os.environ.get("FAKE_CARGO_TOOL_LEAK_PACKAGE")
                    if package == tool_leak_package:
                        print(f"{package}\\n└── kanban-schema-tool v2.1.3")
                    elif leak and package not in ("kanban-contract", "kanban-schema-tool"):
                        print(f"{package}\\n└── kanban-contract feature \\\"{leak}\\\"")
                    elif package == "kanban-schema-tool":
                        print(
                            "kanban-schema-tool v2.1.3 (/workspace/crates/kanban-schema-tool)\\n"
                            "├── kanban-contract feature \\\"schema\\\"\\n"
                            "├── schemars v1.2.1\\n"
                            "├── jsonschema v0.47.0\\n"
                            "└── sha2 v0.10.9"
                        )
                    else:
                        print(f"{package}\\n└── kanban-contract v2.1.3 (/workspace/crates/kanban-contract)")
                    """
                ),
                encoding="utf-8",
            )
            cargo.chmod(0o755)
            env = os.environ | {
                "PATH": f"{temp}:{os.environ['PATH']}",
                "FAKE_CARGO_LOG": str(log),
            }
            if leak is not None:
                env["FAKE_CARGO_LEAK"] = leak
            if tool_leak_package is not None:
                env["FAKE_CARGO_TOOL_LEAK_PACKAGE"] = tool_leak_package
            lock_fd_raw = env.get("KANBAN_CARGO_BUILD_LOCK_FD", "")
            pass_fds: tuple[int, ...] = ()
            if (
                env.get("KANBAN_CARGO_BUILD_LOCK_HELD") == "1"
                and lock_fd_raw.isascii()
                and lock_fd_raw.isdecimal()
                and lock_fd_raw[0] in "3456789"
            ):
                try:
                    lock_fd = int(lock_fd_raw)
                    os.fstat(lock_fd)
                except (OSError, OverflowError, ValueError):
                    pass
                else:
                    # The gate re-enters cargo-build-lock.sh. Keep only its
                    # already-proven lock descriptor so the fake cargo command
                    # is reached under the release cohort's inherited lock.
                    pass_fds = (lock_fd,)
            completed = subprocess.run(
                ["bash", str(GATE)],
                cwd=ROOT,
                env=env,
                close_fds=True,
                pass_fds=pass_fds,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            records = (
                [json.loads(line) for line in log.read_text().splitlines()]
                if log.exists()
                else []
            )
            return completed, records

    def test_fake_cargo_gate_preserves_inherited_build_lock_descriptor(self) -> None:
        target_dir = Path(
            subprocess.check_output(
                [str(ROOT / "scripts/cargo-build-lock.sh"), "--print-target-dir"],
                text=True,
            ).strip()
        )
        lock_path = target_dir / ".build.lock"

        if os.environ.get("KANBAN_CARGO_BUILD_LOCK_HELD") == "1":
            completed, records = self.run_gate()
        else:
            lock_fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o666)
            try:
                fcntl.flock(lock_fd, fcntl.LOCK_EX)
                with mock.patch.dict(
                    os.environ,
                    {
                        "CARGO_TARGET_DIR": str(target_dir),
                        "KANBAN_CARGO_BUILD_LOCK_FD": str(lock_fd),
                        "KANBAN_CARGO_BUILD_LOCK_HELD": "1",
                        "KANBAN_CARGO_BUILD_LOCK_PATH": str(lock_path),
                    },
                ):
                    completed, records = self.run_gate()
            finally:
                fcntl.flock(lock_fd, fcntl.LOCK_UN)
                os.close(lock_fd)

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(len(records), len(PRODUCTS) + 2)

    def assert_inherited_lock_descriptors_are_rejected(
        self, *lock_fd_values: str
    ) -> None:
        target_dir = Path(
            subprocess.check_output(
                [str(ROOT / "scripts/cargo-build-lock.sh"), "--print-target-dir"],
                text=True,
            ).strip()
        )
        lock_path = target_dir / ".build.lock"
        lock_fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o666)
        os.close(lock_fd)

        for lock_fd_raw in lock_fd_values:
            with self.subTest(lock_fd=lock_fd_raw), mock.patch.dict(
                os.environ,
                {
                    "CARGO_TARGET_DIR": str(target_dir),
                    "KANBAN_CARGO_BUILD_LOCK_FD": lock_fd_raw,
                    "KANBAN_CARGO_BUILD_LOCK_HELD": "1",
                    "KANBAN_CARGO_BUILD_LOCK_PATH": str(lock_path),
                },
            ):
                completed, records = self.run_gate()
                self.assertNotEqual(completed.returncode, 0)
                self.assertEqual(records, [])
                self.assertIn(
                    "KANBAN_CARGO_BUILD_LOCK_HELD requires an inherited lock proof",
                    completed.stderr,
                )

    def test_fake_cargo_gate_rejects_invalid_inherited_lock_descriptors(self) -> None:
        closed_fd = fcntl.fcntl(0, fcntl.F_DUPFD, 100)
        os.close(closed_fd)
        self.assert_inherited_lock_descriptors_are_rejected(
            "not-a-fd", "999999", str(closed_fd)
        )

    def test_fake_cargo_gate_rejects_unrepresentable_inherited_lock_descriptors(
        self,
    ) -> None:
        self.assert_inherited_lock_descriptors_are_rejected("9" * 5000, str(2**63))

    def test_command_shapes_cover_default_product_and_tooling_graphs(self) -> None:
        completed, records = self.run_gate()
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertNotIn("--all-features", records[0])
        self.assertEqual(
            records[0],
            ["tree", "-p", "kanban-contract", "--target", "all", "--edges", "normal,features", "--locked"],
        )
        for package, args in zip(PRODUCTS, records[1:-1], strict=True):
            self.assertEqual(
                args,
                ["tree", "-p", package, "--all-features", "--target", "all", "--edges", "normal,features", "--locked"],
            )
        self.assertEqual(
            records[-1],
            ["tree", "-p", TOOL_PACKAGE, "--all-features", "--target", "all", "--edges", "normal,features", "--locked"],
        )

    def test_workspace_declares_leaf_tool_outside_product_defaults(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)["workspace"]

        self.assertIn(TOOL_MEMBER, workspace["members"])
        self.assertNotIn(TOOL_MEMBER, workspace["default-members"])

    def test_contract_manifest_has_only_model_schema_feature(self) -> None:
        with (ROOT / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            contract = tomllib.load(handle)

        self.assertEqual(set(contract["features"]), {"default", "schema"})
        self.assertEqual(
            {name: contract["package"].get(name) for name in dependency_policy.AUTO_TARGET_FLAGS},
            dependency_policy.AUTO_TARGET_FLAGS,
        )
        self.assertEqual(contract["lib"], dependency_policy.CONTRACT_MANIFEST_LIB)
        self.assertEqual(contract["test"], dependency_policy.CONTRACT_MANIFEST_TESTS)
        self.assertNotIn("jsonschema", contract["dependencies"])
        self.assertNotIn("sha2", contract["dependencies"])
        self.assertNotIn("bin", contract)

    def test_leaf_tool_owns_binary_and_tooling_dependencies(self) -> None:
        manifest = ROOT / TOOL_MEMBER / "Cargo.toml"
        self.assertTrue(manifest.is_file(), manifest)
        with manifest.open("rb") as handle:
            tool = tomllib.load(handle)

        package_metadata = tool["package"]
        self.assertEqual(package_metadata["name"], TOOL_PACKAGE)
        self.assertIs(package_metadata.get("publish"), False)
        self.assertEqual(
            {name: package_metadata.get(name) for name in dependency_policy.AUTO_TARGET_FLAGS},
            dependency_policy.AUTO_TARGET_FLAGS,
        )
        self.assertNotIn("features", tool)
        self.assertEqual(tool["lib"], dependency_policy.TOOL_MANIFEST_LIB)
        self.assertEqual(tool["bin"], dependency_policy.TOOL_MANIFEST_BINS)
        self.assertEqual(tool["test"], dependency_policy.TOOL_MANIFEST_TESTS)
        self.assertEqual(tool["dependencies"], TOOL_MANIFEST_DEPENDENCIES)
        for forbidden_section in ("dev-dependencies", "build-dependencies", "target", "example", "bench"):
            self.assertNotIn(forbidden_section, tool)

        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)["workspace"]
        for name, expected in WORKSPACE_CANONICAL_DEPENDENCIES.items():
            self.assertEqual(workspace["dependencies"][name], expected)

    def test_manifest_policy_accepts_current_phase_one_shape(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            tool = tomllib.load(handle)

        dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_manifest_policy_rejects_auto_or_explicit_target_drift(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)

        mutations = []
        for flag in dependency_policy.AUTO_TARGET_FLAGS:
            enabled = copy.deepcopy(baseline)
            enabled["package"][flag] = True
            mutations.append(enabled)
        wrong_lib = copy.deepcopy(baseline)
        wrong_lib["lib"]["path"] = "src/other.rs"
        mutations.append(wrong_lib)
        extra_bin = copy.deepcopy(baseline)
        extra_bin["bin"].append({"name": "extra", "path": "src/bin/extra.rs"})
        mutations.append(extra_bin)
        extra_test = copy.deepcopy(baseline)
        extra_test["test"].append({"name": "extra", "path": "tests/extra.rs"})
        mutations.append(extra_test)
        extra_example = copy.deepcopy(baseline)
        extra_example["example"] = [
            {"name": "extra", "path": "examples/extra.rs"}
        ]
        mutations.append(extra_example)
        extra_bench = copy.deepcopy(baseline)
        extra_bench["bench"] = [{"name": "extra", "path": "benches/extra.rs"}]
        mutations.append(extra_bench)

        for tool in mutations:
            with self.subTest(package=tool["package"]):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_manifest_policy_rejects_leaf_in_product_defaults(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            tool = tomllib.load(handle)
        workspace["workspace"]["default-members"].append(TOOL_MEMBER)

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_manifest_policy_rejects_alias_or_non_workspace_dependency(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)

        mutations = []
        aliased = copy.deepcopy(baseline)
        aliased["dependencies"].pop("serde")
        aliased["dependencies"]["serde-shadow"] = {
            "workspace": True,
            "package": "serde",
        }
        mutations.append(aliased)
        non_workspace = copy.deepcopy(baseline)
        non_workspace["dependencies"]["sha2"] = {"version": "0.10"}
        mutations.append(non_workspace)

        for tool in mutations:
            with self.subTest(dependencies=tool["dependencies"]):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_manifest_policy_rejects_dev_build_or_target_dependency_sections(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)

        sections = {
            "dev-dependencies": {"serde": {"workspace": True}},
            "build-dependencies": {"serde": {"workspace": True}},
            "target": {
                "cfg(unix)": {"dependencies": {"serde": {"workspace": True}}}
            },
        }
        for section, value in sections.items():
            with self.subTest(section=section):
                tool = copy.deepcopy(baseline)
                tool[section] = value
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_manifest_policy_rejects_workspace_canonical_dependency_drift(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            tool = tomllib.load(handle)
        workspace["workspace"]["dependencies"]["jsonschema"][
            "default-features"
        ] = True

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_root_patch_and_replace_sections_are_rejected(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)
        with (ROOT / TOOL_MEMBER / "Cargo.toml").open("rb") as handle:
            tool = tomllib.load(handle)

        for section in ("patch", "replace"):
            with self.subTest(section=section):
                workspace = copy.deepcopy(baseline)
                workspace[section] = {
                    "crates-io": {"jsonschema": {"path": "/override/jsonschema"}}
                }
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_manifest_data(workspace, tool, ROOT)

    def test_contract_manifest_policy_accepts_current_phase_one_shape(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            contract = tomllib.load(handle)

        audit = getattr(
            dependency_policy, "audit_contract_manifest_data", lambda *_: None
        )
        audit(workspace, contract, ROOT)

    def test_contract_manifest_rejects_feature_dependency_and_section_drift(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)
        audit = getattr(
            dependency_policy, "audit_contract_manifest_data", lambda *_: None
        )

        mutations = []
        feature = copy.deepcopy(baseline)
        feature["features"]["schema"].append("dep:reqwest")
        mutations.append(feature)
        dependency_drift = copy.deepcopy(baseline)
        dependency_drift["dependencies"]["reqwest"] = {
            "workspace": True,
            "optional": True,
        }
        mutations.append(dependency_drift)
        schemars = copy.deepcopy(baseline)
        schemars["dependencies"]["schemars"] = {
            "workspace": True,
            "optional": False,
        }
        mutations.append(schemars)
        for section in ("dev-dependencies", "build-dependencies", "target"):
            contract = copy.deepcopy(baseline)
            contract[section] = {"serde": {"workspace": True}}
            mutations.append(contract)

        for contract in mutations:
            with self.subTest(contract=contract):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    audit(workspace, contract, ROOT)

    def test_contract_manifest_rejects_auto_or_explicit_target_drift(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)

        mutations = []
        for flag in dependency_policy.AUTO_TARGET_FLAGS:
            enabled = copy.deepcopy(baseline)
            enabled["package"][flag] = True
            mutations.append(enabled)
        wrong_lib = copy.deepcopy(baseline)
        wrong_lib["lib"]["name"] = "wrong_contract"
        mutations.append(wrong_lib)
        extra_test = copy.deepcopy(baseline)
        extra_test["test"].append({"name": "extra", "path": "tests/extra.rs"})
        mutations.append(extra_test)
        extra_bin = copy.deepcopy(baseline)
        extra_bin["bin"] = [{"name": "extra", "path": "src/bin/extra.rs"}]
        mutations.append(extra_bin)
        extra_example = copy.deepcopy(baseline)
        extra_example["example"] = [
            {"name": "extra", "path": "examples/extra.rs"}
        ]
        mutations.append(extra_example)
        extra_bench = copy.deepcopy(baseline)
        extra_bench["bench"] = [{"name": "extra", "path": "benches/extra.rs"}]
        mutations.append(extra_bench)

        for contract in mutations:
            with self.subTest(package=contract["package"]):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_contract_manifest_data(
                        workspace, contract, ROOT
                    )

    def test_target_discovery_files_reject_extra_or_symlink_target(self) -> None:
        extras = (
            ("crates/kanban-schema-tool/src/bin/extra.rs", False),
            ("crates/kanban-contract/build.rs", False),
            ("crates/kanban-schema-tool/tests/tooling.rs", True),
        )
        for extra, symlink in extras:
            with self.subTest(extra=extra, symlink=symlink):
                with tempfile.TemporaryDirectory() as temp_dir:
                    root = Path(temp_dir)
                    approved = (
                        "crates/kanban-schema-tool/src/lib.rs",
                        "crates/kanban-schema-tool/src/bin/kanban-schema.rs",
                        "crates/kanban-schema-tool/tests/tooling.rs",
                        "crates/kanban-contract/src/lib.rs",
                        "crates/kanban-contract/tests/foundation.rs",
                        "crates/kanban-contract/tests/g0_metadata.rs",
                    )
                    for relative in approved:
                        target = root / relative
                        target.parent.mkdir(parents=True, exist_ok=True)
                        target.touch()
                    dependency_policy.audit_target_files(root)
                    escaped = root / extra
                    if symlink:
                        escaped.unlink()
                        escaped.symlink_to(root / "crates/kanban-schema-tool/src/lib.rs")
                    else:
                        escaped.parent.mkdir(parents=True, exist_ok=True)
                        escaped.touch()
                    with self.assertRaises(dependency_policy.DependencyPolicyError):
                        dependency_policy.audit_target_files(root)

    def test_target_discovery_rejects_symlinked_parent_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir) / "repo"
            outside = Path(temp_dir) / "outside"
            approved = (
                "crates/kanban-schema-tool/src/lib.rs",
                "crates/kanban-schema-tool/src/bin/kanban-schema.rs",
                "crates/kanban-schema-tool/tests/tooling.rs",
                "crates/kanban-contract/src/lib.rs",
                "crates/kanban-contract/tests/foundation.rs",
                "crates/kanban-contract/tests/g0_metadata.rs",
            )
            for relative in approved:
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.touch()
            dependency_policy.audit_target_files(root)

            source_dir = root / "crates/kanban-schema-tool/src"
            outside_source = outside / "src"
            outside_source.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(source_dir), str(outside_source))
            source_dir.symlink_to(outside_source, target_is_directory=True)

            with self.assertRaises(dependency_policy.DependencyPolicyError):
                dependency_policy.audit_target_files(root)

    def test_contract_manifest_rejects_root_schemars_canonical_drift(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)
        with (ROOT / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            contract = tomllib.load(handle)
        audit = getattr(
            dependency_policy, "audit_contract_manifest_data", lambda *_: None
        )

        mutations = []
        for replacement in (
            {"version": "1.2.1", "default-features": True, "features": ["std", "derive"]},
            {"version": "1.2.1", "default-features": False, "features": ["std", "derive", "preserve_order"]},
            {"path": "/override/schemars"},
            {"git": "https://example.invalid/schemars", "rev": "deadbeef"},
        ):
            workspace = copy.deepcopy(baseline)
            workspace["workspace"]["dependencies"]["schemars"] = replacement
            mutations.append(workspace)
        missing = copy.deepcopy(baseline)
        missing["workspace"]["dependencies"].pop("schemars")
        mutations.append(missing)

        for workspace in mutations:
            with self.subTest(schemars=workspace["workspace"]["dependencies"].get("schemars")):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    audit(workspace, contract, ROOT)

    def test_phase_one_metadata_topology_is_valid(self) -> None:
        dependency_policy.audit_metadata(valid_phase_one_metadata())

    def _audit_registry_snapshot(
        self,
        metadata: dict[str, object],
        lockfile: dict[str, object] | None = None,
        approved: dict[str, object] | None = None,
    ) -> None:
        actual_lockfile = lockfile or valid_lockfile(metadata)
        actual_approved = approved or valid_approved_snapshot(
            metadata, actual_lockfile
        )
        closure_ids = dependency_policy.audit_metadata(metadata)
        dependency_policy.audit_registry_closure_snapshot(
            metadata, closure_ids, actual_lockfile, actual_approved
        )

    def test_registry_closure_snapshot_baseline_is_valid(self) -> None:
        metadata = valid_phase_one_metadata()
        self.assertEqual(
            dependency_policy.audit_metadata(metadata),
            metadata_closure_ids(metadata),
        )
        self._audit_registry_snapshot(metadata)

    def test_registry_closure_rejects_valid_but_unapproved_checksum_drift(self) -> None:
        metadata = valid_phase_one_metadata()
        baseline_lock = valid_lockfile(metadata)
        approved = valid_approved_snapshot(metadata, baseline_lock)
        for name in ("jsonschema", "serde"):
            with self.subTest(name=name):
                lockfile = copy.deepcopy(baseline_lock)
                record = next(
                    item
                    for item in lockfile["package"]
                    if item["name"] == name
                    and item.get("source") == CRATES_IO_SOURCE
                )
                record["checksum"] = "0" * 64
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    self._audit_registry_snapshot(metadata, lockfile, approved)

    def test_registry_closure_rejects_malformed_or_missing_checksum(self) -> None:
        metadata = valid_phase_one_metadata()
        baseline_lock = valid_lockfile(metadata)
        approved = valid_approved_snapshot(metadata, baseline_lock)
        mutations = (None, "a" * 63, "g" * 64, "A" * 64)
        for checksum in mutations:
            with self.subTest(checksum=checksum):
                lockfile = copy.deepcopy(baseline_lock)
                record = next(
                    item
                    for item in lockfile["package"]
                    if item["name"] == "jsonschema"
                )
                if checksum is None:
                    record.pop("checksum")
                else:
                    record["checksum"] = checksum
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    self._audit_registry_snapshot(metadata, lockfile, approved)

    def test_registry_closure_distinguishes_same_name_multiple_versions(self) -> None:
        metadata = valid_phase_one_metadata()
        add_getrandom_versions(metadata)
        baseline_lock = valid_lockfile(metadata)
        approved = valid_approved_snapshot(metadata, baseline_lock)
        self._audit_registry_snapshot(metadata, baseline_lock, approved)

        for version in ("0.2.17", "0.3.4"):
            with self.subTest(version=version):
                lockfile = copy.deepcopy(baseline_lock)
                record = next(
                    item
                    for item in lockfile["package"]
                    if item["name"] == "getrandom" and item["version"] == version
                )
                record["checksum"] = "1" * 64
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    self._audit_registry_snapshot(metadata, lockfile, approved)

        swapped = copy.deepcopy(baseline_lock)
        records = [
            item for item in swapped["package"] if item["name"] == "getrandom"
        ]
        records[0]["checksum"], records[1]["checksum"] = (
            records[1]["checksum"],
            records[0]["checksum"],
        )
        with self.assertRaises(dependency_policy.DependencyPolicyError):
            self._audit_registry_snapshot(metadata, swapped, approved)

    def test_approved_snapshot_shape_and_exact_set_are_fail_closed(self) -> None:
        metadata = valid_phase_one_metadata()
        lockfile = valid_lockfile(metadata)
        baseline = valid_approved_snapshot(metadata, lockfile)
        mutations = []

        missing = copy.deepcopy(baseline)
        missing["packages"].pop()
        mutations.append(missing)
        extra = copy.deepcopy(baseline)
        extra["packages"].append(
            {
                "name": "zz-extra",
                "version": "1.0.0",
                "source": CRATES_IO_SOURCE,
                "checksum": "f" * 64,
            }
        )
        mutations.append(extra)
        duplicate = copy.deepcopy(baseline)
        duplicate["packages"].append(copy.deepcopy(duplicate["packages"][-1]))
        mutations.append(duplicate)
        for field, value in (
            ("source", "registry+https://example.invalid/index"),
            ("version", "999.0.0"),
            ("checksum", "0" * 64),
        ):
            drift = copy.deepcopy(baseline)
            drift["packages"][0][field] = value
            mutations.append(drift)
        noncanonical = copy.deepcopy(baseline)
        noncanonical["packages"].reverse()
        mutations.append(noncanonical)
        unknown_top = copy.deepcopy(baseline)
        unknown_top["generated_by"] = "test"
        mutations.append(unknown_top)
        unknown_record = copy.deepcopy(baseline)
        unknown_record["packages"][0]["id"] = "forbidden"
        mutations.append(unknown_record)

        for approved in mutations:
            with self.subTest(approved=approved):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    self._audit_registry_snapshot(metadata, lockfile, approved)

    def test_lockfile_version_and_duplicate_identity_are_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        baseline_lock = valid_lockfile(metadata)
        approved = valid_approved_snapshot(metadata, baseline_lock)

        wrong_version = copy.deepcopy(baseline_lock)
        wrong_version["version"] = 3
        duplicate = copy.deepcopy(baseline_lock)
        duplicate["package"].append(copy.deepcopy(duplicate["package"][0]))
        for lockfile in (wrong_version, duplicate):
            with self.subTest(lockfile=lockfile):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    self._audit_registry_snapshot(metadata, lockfile, approved)

    def test_reachable_noncanonical_sources_remain_rejected(self) -> None:
        sources = (
            None,
            "git+https://example.invalid/jsonschema#deadbeef",
            "registry+https://rsproxy.cn/crates.io-index",
        )
        for source in sources:
            with self.subTest(source=source):
                metadata = valid_phase_one_metadata()
                replace_resolved_registry_package(metadata, "jsonschema", source)
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_nonclosure_registry_checksum_is_outside_snapshot_scope(self) -> None:
        metadata = valid_phase_one_metadata()
        package_id = f"{CRATES_IO_SOURCE}#unrelated@9.9.9"
        metadata["packages"].append(
            package(
                "unrelated",
                package_id=package_id,
                source=CRATES_IO_SOURCE,
                manifest_path="/cargo/registry/unrelated-9.9.9/Cargo.toml",
                version="9.9.9",
                targets=[],
            )
        )
        lockfile = valid_lockfile(metadata)
        approved = valid_approved_snapshot(metadata, lockfile)
        unrelated = next(
            item for item in lockfile["package"] if item["name"] == "unrelated"
        )
        unrelated["checksum"] = "f" * 64
        self._audit_registry_snapshot(metadata, lockfile, approved)

    def test_metadata_checksum_field_is_ignored_but_lock_drift_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        for record in metadata["packages"]:
            if record.get("source") is not None:
                record["checksum"] = "metadata-is-not-authoritative"
        baseline_lock = valid_lockfile(metadata)
        approved = valid_approved_snapshot(metadata, baseline_lock)
        self._audit_registry_snapshot(metadata, baseline_lock, approved)

        drift = copy.deepcopy(baseline_lock)
        record = next(
            item for item in drift["package"] if item["name"] == "jsonschema"
        )
        record["checksum"] = "0" * 64
        with self.assertRaises(dependency_policy.DependencyPolicyError):
            self._audit_registry_snapshot(metadata, drift, approved)

    def test_metadata_target_surface_is_exact(self) -> None:
        baseline = valid_phase_one_metadata()
        dependency_policy.audit_metadata(baseline)

        mutations = []
        extra = valid_phase_one_metadata()
        workspace_package(extra, TOOL_PACKAGE)["targets"].append(
            cargo_target(
                "extra",
                "bin",
                str(ROOT / TOOL_MEMBER / "src/bin/extra.rs"),
            )
        )
        mutations.append(extra)
        custom_build = valid_phase_one_metadata()
        workspace_package(custom_build, CONTRACT_PACKAGE)["targets"].append(
            cargo_target(
                "build-script-build",
                "custom-build",
                str(ROOT / "crates/kanban-contract/build.rs"),
            )
        )
        mutations.append(custom_build)
        wrong_kind = valid_phase_one_metadata()
        workspace_package(wrong_kind, TOOL_PACKAGE)["targets"][0]["kind"] = ["bin"]
        mutations.append(wrong_kind)
        wrong_path = valid_phase_one_metadata()
        workspace_package(wrong_path, CONTRACT_PACKAGE)["targets"][0][
            "src_path"
        ] = str(ROOT / "crates/kanban-contract/src/other.rs")
        mutations.append(wrong_path)

        for metadata in mutations:
            with self.subTest(targets=workspace_package(metadata, TOOL_PACKAGE)["targets"]):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_metadata_target_src_path_does_not_follow_symlink_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir) / "repo"
            targets = []
            for (name, kinds), relative in dependency_policy.TOOL_TARGETS.items():
                source = root / relative
                source.parent.mkdir(parents=True, exist_ok=True)
                source.touch()
                targets.append(cargo_target(name, kinds[0], str(source)))
            package_record = {"targets": targets}
            dependency_policy._audit_package_targets(
                package_record,
                TOOL_PACKAGE,
                dependency_policy.TOOL_TARGETS,
                root,
            )

            alias = root / TOOL_MEMBER / "src/alias.rs"
            alias.symlink_to(root / TOOL_MEMBER / "src/lib.rs")
            targets[0]["src_path"] = str(alias)
            with self.assertRaises(dependency_policy.DependencyPolicyError):
                dependency_policy._audit_package_targets(
                    package_record,
                    TOOL_PACKAGE,
                    dependency_policy.TOOL_TARGETS,
                    root,
                )

    def test_tool_root_effective_feature_unions_are_exact(self) -> None:
        mutations = []
        for feature in (
            "default",
            "resolve-http",
            "file",
            "reqwest",
            "native-tls",
            "rustls-tls",
        ):
            metadata = valid_phase_one_metadata()
            resolved_node(metadata, registry_id("jsonschema"))["features"] = [feature]
            mutations.append(metadata)
        for features in (
            ["derive", "schemars_derive", "std", "preserve_order"],
            ["derive", "std"],
            ["default", "derive", "schemars_derive", "std"],
        ):
            metadata = valid_phase_one_metadata()
            resolved_node(metadata, registry_id("schemars"))["features"] = features
            mutations.append(metadata)

        for metadata in mutations:
            with self.subTest(
                jsonschema=resolved_node(
                    metadata, registry_id("jsonschema")
                )["features"],
                schemars=resolved_node(metadata, registry_id("schemars"))["features"],
            ):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_unreachable_older_schemars_does_not_spoof_tool_root_identity(self) -> None:
        metadata = valid_phase_one_metadata()
        old_id = f"{CRATES_IO_SOURCE}#schemars@0.8.22"
        metadata["packages"].append(
            package(
                "schemars",
                package_id=old_id,
                source=CRATES_IO_SOURCE,
                manifest_path="/cargo/registry/schemars-0.8.22/Cargo.toml",
                version="0.8.22",
                targets=[],
            )
        )
        metadata["resolve"]["nodes"].append(
            resolve_node(old_id, features=["derive", "preserve_order"])
        )

        dependency_policy.audit_metadata(metadata)

    def test_product_direct_tool_normal_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, "kanban-cli")["dependencies"].append(
            dependency(TOOL_PACKAGE)
        )

        with self.assertRaisesRegex(
            dependency_policy.DependencyPolicyError, "kanban-cli.*normal dependency"
        ):
            dependency_policy.audit_metadata(metadata)

    def test_product_aliased_tool_normal_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, "kanban-server")["dependencies"].append(
            dependency(TOOL_PACKAGE, rename="schema-auditor")
        )

        with self.assertRaisesRegex(
            dependency_policy.DependencyPolicyError, "schema-auditor"
        ):
            dependency_policy.audit_metadata(metadata)

    def test_product_dev_alias_tool_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, "kanban-cli")["dependencies"].append(
            dependency(TOOL_PACKAGE, kind="dev", rename="schema-dev-tool")
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_product_build_alias_tool_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, "kanban-server")["dependencies"].append(
            dependency(TOOL_PACKAGE, kind="build", rename="schema-build-tool")
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_unlisted_internal_crate_tool_dependency_is_rejected_for_all_kinds(self) -> None:
        for kind in (None, "dev", "build"):
            with self.subTest(kind=kind):
                metadata = valid_phase_one_metadata()
                workspace_package(metadata, INTERNAL_PACKAGE)["dependencies"].append(
                    dependency(TOOL_PACKAGE, kind=kind, rename="internal-schema-tool")
                )

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_workspace_target_specific_tool_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, INTERNAL_PACKAGE)["dependencies"].append(
            dependency(TOOL_PACKAGE, target="cfg(unix)")
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_workspace_optional_tool_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, INTERNAL_PACKAGE)["dependencies"].append(
            dependency(TOOL_PACKAGE, optional=True)
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_product_renamed_contract_dependency_is_allowed(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, "kanban-sqlite")["dependencies"].append(
            dependency("kanban-contract", rename="wire-contract")
        )

        dependency_policy.audit_metadata(metadata)

    def test_tool_dev_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, TOOL_PACKAGE)["dependencies"].append(
            dependency("kanban-server", kind="dev")
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_tool_build_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, TOOL_PACKAGE)["dependencies"].append(
            dependency("kanban-server", kind="build")
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_tool_normal_dependency_on_product_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, TOOL_PACKAGE)["dependencies"].append(
            dependency("kanban-server")
        )

        with self.assertRaisesRegex(
            dependency_policy.DependencyPolicyError, "direct normal dependencies"
        ):
            dependency_policy.audit_metadata(metadata)

    def test_tool_duplicate_allowed_package_alias_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, TOOL_PACKAGE)["dependencies"].append(
            tool_dependency("serde", rename="serde-shadow")
        )

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_tool_contract_same_name_different_source_or_path_is_rejected(self) -> None:
        mutations = (
            {"source": CRATES_IO_SOURCE},
            {"path": "/tmp/not-the-workspace-contract"},
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                metadata = valid_phase_one_metadata()
                contract = next(
                    dependency
                    for dependency in workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
                    if dependency["name"] == "kanban-contract"
                )
                contract.update(mutation)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_optional_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        sha2 = next(
            dependency
            for dependency in workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
            if dependency["name"] == "sha2"
        )
        sha2["optional"] = True

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_tool_target_specific_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        serde_json = next(
            dependency
            for dependency in workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
            if dependency["name"] == "serde_json"
        )
        serde_json["target"] = "cfg(unix)"

        with self.assertRaises(dependency_policy.DependencyPolicyError):
            dependency_policy.audit_metadata(metadata)

    def test_tool_kind_alias_requirement_or_registry_drift_is_rejected(self) -> None:
        mutations = (
            {"kind": "dev"},
            {"kind": "build"},
            {"rename": "serde-shadow"},
            {"req": "^1.0.999"},
            {"registry": "private-registry"},
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                metadata = valid_phase_one_metadata()
                serde = next(
                    dependency
                    for dependency in workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
                    if dependency["name"] == "serde"
                )
                serde.update(mutation)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_jsonschema_default_or_extra_feature_is_rejected(self) -> None:
        mutations = (
            {"uses_default_features": True},
            {"features": ["resolve-http"]},
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                metadata = valid_phase_one_metadata()
                jsonschema = next(
                    dependency
                    for dependency in workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
                    if dependency["name"] == "jsonschema"
                )
                jsonschema.update(mutation)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_contract_default_or_extra_feature_is_rejected(self) -> None:
        mutations = (
            {"uses_default_features": True},
            {"features": ["schema", "extra"]},
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                metadata = valid_phase_one_metadata()
                contract = next(
                    dependency
                    for dependency in workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
                    if dependency["name"] == "kanban-contract"
                )
                contract.update(mutation)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_unexpected_external_normal_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        workspace_package(metadata, TOOL_PACKAGE)["dependencies"].append(
            dependency("reqwest")
        )

        with self.assertRaisesRegex(
            dependency_policy.DependencyPolicyError, "reqwest"
        ):
            dependency_policy.audit_metadata(metadata)

    def test_tool_missing_required_normal_dependency_is_rejected(self) -> None:
        metadata = valid_phase_one_metadata()
        tool_dependencies = workspace_package(metadata, TOOL_PACKAGE)["dependencies"]
        tool_dependencies[:] = [
            record for record in tool_dependencies if record["name"] != "sha2"
        ]

        with self.assertRaisesRegex(
            dependency_policy.DependencyPolicyError, "missing=.*sha2"
        ):
            dependency_policy.audit_metadata(metadata)

    def test_full_metadata_fixture_has_cargo_resolve_identity_shape(self) -> None:
        metadata = valid_phase_one_metadata()

        self.assertEqual(metadata["resolve"]["root"], TOOL_ID)
        self.assertEqual(len(resolved_node(metadata, TOOL_ID)["deps"]), 5)
        self.assertIn("schema", resolved_node(metadata, CONTRACT_ID)["features"])
        self.assertEqual(
            resolved_edge(metadata, CONTRACT_ID, "schemars")["pkg"],
            registry_id("schemars"),
        )

    def test_resolve_rejects_missing_or_duplicate_package_and_node_records(self) -> None:
        mutations = []
        missing_package = valid_phase_one_metadata()
        missing_package["packages"] = [
            record
            for record in missing_package["packages"]
            if record["id"] != registry_id("sha2")
        ]
        mutations.append(missing_package)
        duplicate_package = valid_phase_one_metadata()
        duplicate_package["packages"].append(
            copy.deepcopy(package_record(duplicate_package, registry_id("serde")))
        )
        mutations.append(duplicate_package)
        missing_node = valid_phase_one_metadata()
        missing_node["resolve"]["nodes"] = [
            node
            for node in missing_node["resolve"]["nodes"]
            if node["id"] != registry_id("serde_json")
        ]
        mutations.append(missing_node)
        duplicate_node = valid_phase_one_metadata()
        duplicate_node["resolve"]["nodes"].append(
            copy.deepcopy(resolved_node(duplicate_node, registry_id("jsonschema")))
        )
        mutations.append(duplicate_node)

        for metadata in mutations:
            with self.subTest(packages=len(metadata["packages"])):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_resolve_identity_source_and_manifest_are_canonical(self) -> None:
        mutations = []
        wrong_root = valid_phase_one_metadata()
        wrong_root["resolve"]["root"] = CONTRACT_ID
        mutations.append(wrong_root)
        wrong_source = valid_phase_one_metadata()
        package_record(wrong_source, TOOL_ID)["source"] = CRATES_IO_SOURCE
        mutations.append(wrong_source)
        wrong_manifest = valid_phase_one_metadata()
        package_record(wrong_manifest, TOOL_ID)["manifest_path"] = (
            "/tmp/not-the-schema-tool/Cargo.toml"
        )
        mutations.append(wrong_manifest)

        for metadata in mutations:
            with self.subTest(root=metadata["resolve"]["root"]):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_resolved_direct_path_and_git_patches_are_rejected(self) -> None:
        for source in (None, "git+https://example.invalid/jsonschema?rev=bad#deadbeef"):
            with self.subTest(source=source):
                metadata = valid_phase_one_metadata()
                replace_resolved_registry_package(metadata, "jsonschema", source)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_resolved_edge_mapping_kind_and_alias_drift_are_rejected(self) -> None:
        mutations = []
        wrong_pkg = valid_phase_one_metadata()
        resolved_edge(wrong_pkg, TOOL_ID, "serde")["pkg"] = registry_id("sha2")
        mutations.append(wrong_pkg)
        wrong_alias = valid_phase_one_metadata()
        resolved_edge(wrong_alias, TOOL_ID, "serde_json")["name"] = "serde_shadow"
        mutations.append(wrong_alias)
        wrong_kind = valid_phase_one_metadata()
        resolved_edge(wrong_kind, TOOL_ID, "sha2")["dep_kinds"] = [
            {"kind": "dev", "target": None}
        ]
        mutations.append(wrong_kind)
        wrong_target = valid_phase_one_metadata()
        resolved_edge(wrong_target, TOOL_ID, "sha2")["dep_kinds"] = [
            {"kind": None, "target": "cfg(unix)"}
        ]
        mutations.append(wrong_target)

        for metadata in mutations:
            with self.subTest(tool_node=resolved_node(metadata, TOOL_ID)):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_resolved_edge_missing_and_duplicate_mappings_are_rejected(self) -> None:
        missing = valid_phase_one_metadata()
        tool_node = resolved_node(missing, TOOL_ID)
        sha_id = registry_id("sha2")
        tool_node["deps"] = [edge for edge in tool_node["deps"] if edge["pkg"] != sha_id]
        tool_node["dependencies"].remove(sha_id)

        duplicate = valid_phase_one_metadata()
        tool_node = resolved_node(duplicate, TOOL_ID)
        serde_edge = copy.deepcopy(resolved_edge(duplicate, TOOL_ID, "serde"))
        tool_node["deps"].append(serde_edge)
        tool_node["dependencies"].append(serde_edge["pkg"])

        for metadata in (missing, duplicate):
            with self.subTest(tool_node=resolved_node(metadata, TOOL_ID)):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_contract_resolve_identity_source_and_manifest_are_canonical(self) -> None:
        mutations = []
        wrong_source = valid_phase_one_metadata()
        package_record(wrong_source, CONTRACT_ID)["source"] = CRATES_IO_SOURCE
        mutations.append(wrong_source)
        wrong_manifest = valid_phase_one_metadata()
        package_record(wrong_manifest, CONTRACT_ID)["manifest_path"] = (
            "/tmp/not-the-workspace-contract/Cargo.toml"
        )
        mutations.append(wrong_manifest)
        wrong_pkg = valid_phase_one_metadata()
        resolved_edge(wrong_pkg, TOOL_ID, CONTRACT_PACKAGE)["pkg"] = registry_id(
            "serde"
        )
        mutations.append(wrong_pkg)

        for metadata in mutations:
            with self.subTest(contract=package_record(metadata, CONTRACT_ID)):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_tool_resolve_closure_rejects_path_and_git_transitive_overrides(self) -> None:
        for source in (None, "git+https://example.invalid/transport?rev=bad#deadbeef"):
            with self.subTest(source=source):
                metadata = valid_phase_one_metadata()
                add_transitive_override(metadata, source)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_contract_dependency_declaration_signature_is_exact(self) -> None:
        mutations = (
            {"uses_default_features": True},
            {"features": ["std", "derive", "preserve_order"]},
            {"source": None, "path": "/override/schemars"},
            {"optional": False},
            {"target": "cfg(unix)"},
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                metadata = valid_phase_one_metadata()
                schemars = next(
                    dependency
                    for dependency in workspace_package(metadata, CONTRACT_PACKAGE)["dependencies"]
                    if dependency["name"] == "schemars"
                )
                schemars.update(mutation)

                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_contract_schema_resolved_edge_is_enabled_unique_and_crates_io(self) -> None:
        mutations = []
        missing_feature = valid_phase_one_metadata()
        resolved_node(missing_feature, CONTRACT_ID)["features"].remove("schema")
        mutations.append(missing_feature)
        missing_edge = valid_phase_one_metadata()
        contract_node = resolved_node(missing_edge, CONTRACT_ID)
        schemars_id = registry_id("schemars")
        contract_node["deps"] = [edge for edge in contract_node["deps"] if edge["pkg"] != schemars_id]
        contract_node["dependencies"].remove(schemars_id)
        mutations.append(missing_edge)
        duplicate_edge = valid_phase_one_metadata()
        contract_node = resolved_node(duplicate_edge, CONTRACT_ID)
        schemars_edge = copy.deepcopy(
            resolved_edge(duplicate_edge, CONTRACT_ID, "schemars")
        )
        contract_node["deps"].append(schemars_edge)
        contract_node["dependencies"].append(schemars_id)
        mutations.append(duplicate_edge)
        wrong_pkg = valid_phase_one_metadata()
        resolved_edge(wrong_pkg, CONTRACT_ID, "schemars")["pkg"] = registry_id("serde")
        mutations.append(wrong_pkg)

        for metadata in mutations:
            with self.subTest(contract_node=resolved_node(metadata, CONTRACT_ID)):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

        for source in (None, "git+https://example.invalid/schemars#deadbeef"):
            with self.subTest(source=source):
                metadata = valid_phase_one_metadata()
                replace_resolved_registry_package(metadata, "schemars", source)
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_metadata_loader_selects_locked_full_tool_manifest_graph(self) -> None:
        completed = subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout=json.dumps(valid_phase_one_metadata()),
            stderr="",
        )
        with mock.patch.object(
            dependency_policy.subprocess, "run", return_value=completed
        ) as run:
            dependency_policy.load_metadata(ROOT)

        command = run.call_args.args[0]
        self.assertIn("--locked", command)
        self.assertNotIn("--no-deps", command)
        self.assertEqual(
            command[command.index("--manifest-path") + 1],
            "crates/kanban-schema-tool/Cargo.toml",
        )

    def test_metadata_loader_preserves_valid_inherited_lock_descriptor(self) -> None:
        lock_fd = fcntl.fcntl(0, fcntl.F_DUPFD, 90)
        completed = subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout=json.dumps(valid_phase_one_metadata()),
            stderr="",
        )
        try:
            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "KANBAN_CARGO_BUILD_LOCK_FD": str(lock_fd),
                        "KANBAN_CARGO_BUILD_LOCK_HELD": "1",
                    },
                ),
                mock.patch.object(
                    dependency_policy.subprocess, "run", return_value=completed
                ) as run,
            ):
                dependency_policy.load_metadata(ROOT)
        finally:
            os.close(lock_fd)

        self.assertTrue(run.call_args.kwargs["close_fds"])
        self.assertEqual(run.call_args.kwargs["pass_fds"], (lock_fd,))

    def test_metadata_loader_rejects_unusable_inherited_lock_descriptors(self) -> None:
        closed_fd = fcntl.fcntl(0, fcntl.F_DUPFD, 100)
        os.close(closed_fd)
        completed = subprocess.CompletedProcess(
            args=[],
            returncode=2,
            stdout="",
            stderr="error: KANBAN_CARGO_BUILD_LOCK_HELD requires an inherited lock proof\n",
        )

        for lock_fd_raw in ("not-a-fd", str(closed_fd), "9" * 5000, str(2**63)):
            with self.subTest(lock_fd=lock_fd_raw):
                with (
                    mock.patch.dict(
                        os.environ,
                        {
                            "KANBAN_CARGO_BUILD_LOCK_FD": lock_fd_raw,
                            "KANBAN_CARGO_BUILD_LOCK_HELD": "1",
                        },
                    ),
                    mock.patch.object(
                        dependency_policy.subprocess, "run", return_value=completed
                    ) as run,
                ):
                    with self.assertRaisesRegex(
                        dependency_policy.DependencyPolicyError,
                        "inherited lock proof",
                    ):
                        dependency_policy.load_metadata(ROOT)
                    self.assertTrue(run.call_args.kwargs["close_fds"])
                    self.assertEqual(run.call_args.kwargs["pass_fds"], ())

    def test_metadata_policy_rejects_null_or_missing_tool_resolve_node(self) -> None:
        mutations = []
        null_resolve = valid_phase_one_metadata()
        null_resolve["resolve"] = None
        mutations.append(null_resolve)
        missing_node = valid_phase_one_metadata()
        missing_node["resolve"] = {"root": "wrong-tool-id", "nodes": []}
        mutations.append(missing_node)

        for metadata in mutations:
            with self.subTest(resolve=metadata["resolve"]):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    dependency_policy.audit_metadata(metadata)

    def test_contract_manifest_mutations_require_a_dedicated_policy(self) -> None:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (ROOT / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            baseline = tomllib.load(handle)
        audit = getattr(
            dependency_policy, "audit_contract_manifest_data", lambda *_: None
        )

        mutations = []
        feature_drift = copy.deepcopy(baseline)
        feature_drift["features"]["schema"].append("dep:reqwest")
        feature_drift["dependencies"]["reqwest"] = {
            "workspace": True,
            "optional": True,
        }
        mutations.append((workspace, feature_drift))
        root_drift = copy.deepcopy(workspace)
        root_drift["workspace"]["dependencies"]["schemars"][
            "default-features"
        ] = True
        mutations.append((root_drift, baseline))

        for root, contract in mutations:
            with self.subTest(contract=contract):
                with self.assertRaises(dependency_policy.DependencyPolicyError):
                    audit(root, contract, ROOT)

    def test_product_tool_tree_is_rejected(self) -> None:
        completed, _ = self.run_gate(tool_leak_package="kanban-vector-lancedb")
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("schema tooling", completed.stderr)

    def test_feature_forwarded_schema_is_rejected(self) -> None:
        completed, _ = self.run_gate("schema")
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("schema tooling", completed.stderr)

    def test_feature_forwarded_schema_tool_is_rejected(self) -> None:
        completed, _ = self.run_gate("schema-tool")
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("schema tooling", completed.stderr)


if __name__ == "__main__":
    unittest.main()
