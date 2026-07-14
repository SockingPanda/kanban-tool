#!/usr/bin/env python3
"""schema adoption witness gate 的持久负测。"""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from unittest import mock

import schema_adoption_witnesses as witness_gate


WORKSPACE_ROOT = Path("/workspace/kanban-tool")
CONTRACT_PATH = WORKSPACE_ROOT / "crates/kanban-contract"
CONTRACT_ID = f"path+file://{CONTRACT_PATH}#2.1.2"
ADOPTER_ID = f"path+file://{WORKSPACE_ROOT}/crates/runtime-adopter#0.1.0"


def metadata_with_dependency(
    kind: str | None,
    features: list[str] | None = None,
    *,
    source: str | None = None,
    path: str | None = str(CONTRACT_PATH),
    resolved_package: str = CONTRACT_ID,
    target: str | None = None,
    optional: bool = False,
) -> dict:
    return {
        "workspace_root": str(WORKSPACE_ROOT),
        "workspace_members": [CONTRACT_ID, ADOPTER_ID],
        "packages": [
            {
                "id": CONTRACT_ID,
                "name": "kanban-contract",
                "source": None,
                "manifest_path": str(CONTRACT_PATH / "Cargo.toml"),
            },
            {
                "id": ADOPTER_ID,
                "name": "runtime-adopter",
                "manifest_path": str(
                    WORKSPACE_ROOT / "crates/runtime-adopter/Cargo.toml"
                ),
                "dependencies": [
                    {
                        "name": "kanban-contract",
                        "kind": kind,
                        "features": features or [],
                        "source": source,
                        "path": path,
                        "target": target,
                        "optional": optional,
                    }
                ],
                "targets": [
                    {"name": "runtime_adopter", "kind": ["lib"]},
                    {"name": "contract_witness", "kind": ["test"]},
                ],
            }
        ],
        "resolve": {
            "nodes": [
                {
                    "id": ADOPTER_ID,
                    "deps": [
                        {
                            "name": "kanban_contract",
                            "pkg": resolved_package,
                            "dep_kinds": [{"kind": kind, "target": target}],
                        }
                    ],
                }
            ]
        },
    }


class WitnessGateNegativeTests(unittest.TestCase):
    def test_registry_dependency_cannot_impersonate_workspace_contract(self) -> None:
        metadata = metadata_with_dependency(
            None,
            source="registry+https://github.com/rust-lang/crates.io-index",
            path=None,
            resolved_package="registry+https://github.com/rust-lang/crates.io-index#kanban-contract@2.1.2",
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "workspace kanban-contract"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_git_dependency_cannot_impersonate_workspace_contract(self) -> None:
        metadata = metadata_with_dependency(
            None,
            source="git+https://example.invalid/kanban-contract#deadbeef",
            path=None,
            resolved_package="git+https://example.invalid/kanban-contract#deadbeef",
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "workspace kanban-contract"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_other_local_path_cannot_impersonate_workspace_contract(self) -> None:
        metadata = metadata_with_dependency(
            None,
            path="/tmp/other/kanban-contract",
            resolved_package="path+file:///tmp/other/kanban-contract#2.1.2",
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "workspace kanban-contract"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_resolved_package_identity_drift_is_rejected(self) -> None:
        metadata = metadata_with_dependency(
            None,
            resolved_package="path+file:///tmp/identity-drift/kanban-contract#2.1.2",
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "package identity"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_dev_only_dependency_is_rejected(self) -> None:
        metadata = metadata_with_dependency("dev")

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "normal dependency"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_target_specific_normal_dependency_is_rejected(self) -> None:
        metadata = metadata_with_dependency(None, target='cfg(target_os = "windows")')

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "unconditional non-optional normal dependency"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_target_specific_resolve_edge_is_rejected(self) -> None:
        metadata = metadata_with_dependency(None)
        metadata["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["target"] = (
            'cfg(target_os = "windows")'
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "unconditional non-optional normal dependency"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_target_specific_declaration_with_unconditional_dev_dependency_is_rejected(self) -> None:
        metadata = metadata_with_dependency(None, target='cfg(target_os = "windows")')
        metadata["packages"][1]["dependencies"].append(
            {"name": "kanban-contract", "kind": "dev", "target": None, "optional": False}
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "unconditional non-optional normal dependency"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_target_specific_declaration_with_unconditional_resolve_edge_is_rejected(self) -> None:
        metadata = metadata_with_dependency(None, target='cfg(target_os = "windows")')
        metadata["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["target"] = None

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "unconditional non-optional normal dependency"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_optional_only_dependency_is_rejected_even_with_dev_dependency(self) -> None:
        metadata = metadata_with_dependency(None, optional=True)
        metadata["packages"][1]["dependencies"].append(
            {"name": "kanban-contract", "kind": "dev", "target": None, "optional": False}
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "unconditional non-optional normal dependency"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_unconditional_non_optional_dependency_allows_target_specific_sibling(self) -> None:
        metadata = metadata_with_dependency(None)
        metadata["packages"][1]["dependencies"].append(
            {
                "name": "kanban-contract",
                "kind": None,
                "target": 'cfg(target_os = "windows")',
                "optional": False,
                "source": None,
                "path": str(CONTRACT_PATH),
                "features": [],
            }
        )

        witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_schema_feature_on_runtime_dependency_is_rejected(self) -> None:
        metadata = metadata_with_dependency(None, ["schema"])

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "schema"
        ):
            witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_current_dev_schema_feature_is_allowed_with_clean_normal_dependency(self) -> None:
        metadata = metadata_with_dependency(None)
        metadata["packages"][1]["dependencies"].append(
            {
                "name": "kanban-contract",
                "kind": "dev",
                "features": ["schema"],
            }
        )

        witness_gate.require_runtime_dependency(metadata, "runtime-adopter")

    def test_schema_tool_owner_cannot_impersonate_runtime_adopter(self) -> None:
        metadata = metadata_with_dependency(None)
        metadata["packages"][1]["name"] = "kanban-schema-tool"

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "schema tooling owner"
        ):
            witness_gate.require_runtime_dependency(metadata, "kanban-schema-tool")

    def test_witness_plan_is_loaded_from_leaf_tool_binary(self) -> None:
        with mock.patch.object(
            witness_gate, "run_checked", return_value="[]"
        ) as run:
            self.assertEqual(witness_gate.load_witness_plan(WORKSPACE_ROOT), [])

        command = run.call_args.args[0]
        self.assertEqual(command[command.index("-p") + 1], "kanban-schema-tool")
        self.assertNotIn("--features", command)
        self.assertEqual(command[command.index("--bin") + 1], "kanban-schema")

    def test_metadata_load_keeps_full_locked_resolve_graph(self) -> None:
        metadata = metadata_with_dependency(None)

        with mock.patch.object(
            witness_gate, "run_checked", return_value=json.dumps(metadata)
        ) as run:
            loaded = witness_gate.load_cargo_metadata(WORKSPACE_ROOT)

        self.assertEqual(loaded, metadata)
        command = run.call_args.args[0]
        self.assertIn("--locked", command)
        self.assertNotIn("--no-deps", command)

    def test_missing_contract_in_runtime_tree_is_rejected(self) -> None:
        with self.assertRaisesRegex(witness_gate.WitnessGateError, "未出现"):
            witness_gate.require_runtime_tree("runtime-adopter\n", "runtime-adopter")

    def test_forwarded_schema_feature_in_runtime_tree_is_rejected(self) -> None:
        tree = 'runtime-adopter\n└── kanban-contract feature "schema"\n'

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "schema tooling"
        ):
            witness_gate.require_runtime_tree(tree, "runtime-adopter")

    def test_all_target_tree_rejects_target_specific_schema_tooling(self) -> None:
        tree = (
            f"runtime-adopter\n├── kanban-contract v2.1.2 ({CONTRACT_PATH})\n"
            "└── schemars v1.2.1\n"
        )

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "schema tooling"
        ):
            witness_gate.require_runtime_tree(tree, "runtime-adopter", CONTRACT_PATH)

    def test_jsonschema_in_runtime_tree_is_rejected(self) -> None:
        tree = "runtime-adopter\n└── jsonschema v0.47.0\n"

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "schema tooling"
        ):
            witness_gate.require_runtime_tree(tree, "runtime-adopter")

    def test_leaf_schema_tool_in_runtime_tree_is_rejected(self) -> None:
        tree = "runtime-adopter\n└── kanban-schema-tool v2.1.2\n"

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "schema tooling"
        ):
            witness_gate.require_runtime_tree(tree, "runtime-adopter")

    def test_clean_runtime_tree_is_accepted(self) -> None:
        tree = "runtime-adopter\n└── kanban-contract v2.1.2\n"

        witness_gate.require_runtime_tree(tree, "runtime-adopter")

    def test_runtime_tree_rejects_same_name_without_workspace_path(self) -> None:
        tree = "runtime-adopter\n└── kanban-contract v2.1.2\n"

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "workspace kanban-contract"
        ):
            witness_gate.require_runtime_tree(
                tree, "runtime-adopter", CONTRACT_PATH
            )

    def test_runtime_tree_uses_package_identity_and_all_targets(self) -> None:
        metadata = metadata_with_dependency(None)
        tree = (
            f"runtime-adopter\n└── kanban-contract v2.1.2 ({CONTRACT_PATH})\n"
        )

        with mock.patch.object(witness_gate, "run_checked", return_value=tree) as run:
            witness_gate.validate_runtime_graph(
                WORKSPACE_ROOT, metadata, "runtime-adopter"
            )

        command = run.call_args.args[0]
        self.assertEqual(command[command.index("-p") + 1], ADOPTER_ID)
        self.assertIn("--all-features", command)
        self.assertEqual(command[command.index("--target") + 1], "all")
        self.assertEqual(command[command.index("--edges") + 1], "normal,features")
        self.assertIn("--locked", command)

    def test_nonexistent_test_target_is_rejected(self) -> None:
        metadata = metadata_with_dependency(None)

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "test target"
        ):
            witness_gate.require_test_target(
                metadata, "runtime-adopter", "missing-target"
            )

    def test_zero_exact_tests_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "0 tests"
        ):
            witness_gate.require_exact_test("", "tests::producer_contract")

    def test_different_exact_test_is_rejected(self) -> None:
        output = "tests::another_contract: test\n"

        with self.assertRaisesRegex(
            witness_gate.WitnessGateError, "0 tests"
        ):
            witness_gate.require_exact_test(output, "tests::producer_contract")

    def test_exact_test_is_accepted(self) -> None:
        witness_gate.require_exact_test(
            "tests::producer_contract: test\n", "tests::producer_contract"
        )

    def test_zero_executed_tests_is_rejected(self) -> None:
        output = (
            "running 0 tests\n\n"
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n"
        )

        with self.assertRaisesRegex(witness_gate.WitnessGateError, "1 test"):
            witness_gate.require_executed_test(output, "tests::producer_contract")

    def test_one_executed_test_is_accepted(self) -> None:
        output = (
            "running 1 test\n"
            "test tests::producer_contract ... ok\n\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n"
        )

        witness_gate.require_executed_test(output, "tests::producer_contract")


if __name__ == "__main__":
    unittest.main()
