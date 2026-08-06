#!/usr/bin/env python3
"""dependency owner policy 的 metadata fixture 回归测试。"""

from __future__ import annotations

import copy
import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check-dependency-owners.py")
SPEC = importlib.util.spec_from_file_location("dependency_owner_gate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


def _dependency(name: str, req: str, *, default: bool, features: list[str]) -> dict[str, object]:
    return {
        "name": name,
        "source": GATE.CRATES_IO_SOURCE,
        "req": req,
        "kind": None,
        "rename": None,
        "optional": False,
        "uses_default_features": default,
        "features": features,
        "target": None,
        "registry": None,
    }


def _metadata() -> dict[str, object]:
    policies = GATE.DEPENDENCY_POLICIES
    owners = {
        policy["owner"]: dependency_name
        for dependency_name, policy in policies.items()
    }
    packages: list[dict[str, object]] = []
    workspace_members: list[str] = []
    nodes: list[dict[str, object]] = []
    for owner, dependency_name in owners.items():
        owner_id = f"path+file:///repo#{owner}@2.1.3"
        dependency_policy = policies[dependency_name]
        root = dependency_policy["root"]
        feature_policy = dependency_policy["features"]
        external_id = f"registry+{dependency_name}"
        resolved_versions = {
            "turso": "0.7.2",
            "axum": "0.7.9",
            "ureq": "2.12.1",
            "rmcp": "3.1.0",
            "tauri": "2.11.2",
        }
        packages.append(
            {
                "id": owner_id,
                "name": owner,
                "version": "2.1.3",
                "source": None,
                "dependencies": [
                    _dependency(
                        dependency_name,
                        GATE._normalized_req(root),
                        default=feature_policy["uses_default_features"],
                        features=sorted(feature_policy["features"]),
                    )
                ],
            }
        )
        packages.append(
            {
                "id": external_id,
                "name": dependency_name,
                "version": resolved_versions[dependency_name],
                "source": GATE.CRATES_IO_SOURCE,
                "dependencies": [],
            }
        )
        workspace_members.append(owner_id)
        nodes.append(
            {
                "id": owner_id,
                "deps": [
                    {
                        "name": dependency_name,
                        "pkg": external_id,
                        "dep_kinds": [{"kind": None, "target": None}],
                    }
                ],
                "dependencies": [external_id],
                "features": [],
            }
        )
        nodes.append(
            {"id": external_id, "deps": [], "dependencies": [], "features": []}
        )
    return {
        "packages": packages,
        "workspace_members": workspace_members,
        "resolve": {"root": None, "nodes": nodes},
    }


class DependencyOwnerGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        root_dependencies = []
        for dependency_name, policy in GATE.DEPENDENCY_POLICIES.items():
            root = policy["root"]
            if isinstance(root, str):
                root_dependencies.append(f'{dependency_name} = "{root}"')
            else:
                values = [f'version = "{root["version"]}"']
                if root.get("default-features") is False:
                    values.append("default-features = false")
                root_dependencies.append(
                    f"{dependency_name} = {{ {', '.join(values)} }}"
                )
        self.root.joinpath("Cargo.toml").write_text(
            "[workspace]\nmembers = []\n\n[workspace.dependencies]\n"
            + "\n".join(root_dependencies)
            + "\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_metadata_policy_accepts_unique_owners_and_features(self) -> None:
        GATE.audit_metadata(_metadata(), self.root)

    def test_duplicate_owner_is_rejected(self) -> None:
        metadata = _metadata()
        server = next(
            package
            for package in metadata["packages"]
            if package.get("name") == "kanban-server"
        )
        server["dependencies"].append(
            _dependency("turso", "=0.7.2", default=False, features=["fts"])
        )
        with self.assertRaises(GATE.DependencyOwnerPolicyError):
            GATE.audit_metadata(metadata, self.root)

    def test_leaf_feature_drift_is_rejected(self) -> None:
        metadata = _metadata()
        service = next(
            package
            for package in metadata["packages"]
            if package.get("name") == "kanban-service"
        )
        service["dependencies"][0]["features"] = []
        with self.assertRaises(GATE.DependencyOwnerPolicyError):
            GATE.audit_metadata(metadata, self.root)

    def test_non_normal_dependency_kind_is_rejected(self) -> None:
        metadata = _metadata()
        service = next(
            package
            for package in metadata["packages"]
            if package.get("name") == "kanban-service"
        )
        service["dependencies"][0]["kind"] = "dev"
        with self.assertRaises(GATE.DependencyOwnerPolicyError):
            GATE.audit_metadata(metadata, self.root)

    def test_resolved_source_drift_is_rejected(self) -> None:
        metadata = copy.deepcopy(_metadata())
        tauri = next(
            package
            for package in metadata["packages"]
            if package.get("name") == "tauri"
        )
        tauri["source"] = "git+https://example.invalid/tauri"
        with self.assertRaises(GATE.DependencyOwnerPolicyError):
            GATE.audit_metadata(metadata, self.root)

    def test_workspace_identity_drift_is_rejected(self) -> None:
        root = self.root.joinpath("Cargo.toml")
        text = root.read_text(encoding="utf-8").replace('axum = "0.7"', 'axum = "0.8"')
        root.write_text(text, encoding="utf-8")
        with self.assertRaises(GATE.DependencyOwnerPolicyError):
            GATE.audit_metadata(_metadata(), self.root)


if __name__ == "__main__":
    unittest.main()
