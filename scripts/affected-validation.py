#!/usr/bin/env python3
"""Plan and run validation commands for files affected by the current diff."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from pathlib import PurePosixPath
from typing import Iterable, NotRequired, TypedDict

sys.dont_write_bytecode = True

from spec_bundle import SOURCE_PATHS as SPEC_BUNDLE_SOURCE_PATHS


Command = list[str]
Classifications = dict[str, list[str]]
SCHEMA_TOOL_REGISTRY_APPROVAL = (
    "policy/schema-tool-registry-closure.json"
)


class Plan(TypedDict):
    base: str
    changed_files: list[str]
    classifications: Classifications
    commands: list[Command]
    full_gate_recommended: bool
    release_authoritative_command: str
    notes: list[str]
    sources: NotRequired[dict[str, list[str]]]


@dataclass(frozen=True)
class Rule:
    name: str
    commands: tuple[Command, ...]

    def matches(self, path: str) -> bool:
        return CLASSIFIERS[self.name](path)


def _parts(path: str) -> tuple[str, ...]:
    return PurePosixPath(path).parts


def is_docs_only(path: str) -> bool:
    return (
        path == "README.md"
        or path == "KANBAN_SPEC_BUNDLE.md"
        or path.startswith("docs/")
        or path.endswith(".md")
    )


def is_desktop(path: str) -> bool:
    return path.startswith("apps/desktop/")


CORE_CRATES = {
    "kanban-core",
    "kanban-contract",
    "kanban-entity",
    "kanban-indexer",
    "kanban-helper-protocol",
    "kanban-search",
    "kanban-graph",
    "kanban-vector",
    "kanban-derived-io",
    "kanban-labels",
    "kanban-context",
    "kanban-sqlite",
    "kanban-local",
    "kanban-server",
    "kanban-cli",
}


def crate_name(path: str) -> str | None:
    parts = _parts(path)
    if len(parts) >= 2 and parts[0] == "crates":
        return parts[1]
    return None


def is_core(path: str) -> bool:
    return (
        crate_name(path) in CORE_CRATES
        or path.startswith("migrations/")
        or path in {
            "docs/STATE_MACHINE.md",
            "docs/DATA_MODEL.md",
            "docs/CLI_SPEC.md",
            "docs/API_SPEC.md",
        }
    )


def is_workspace_member_manifest(path: str) -> bool:
    parts = _parts(path)
    return (
        len(parts) == 3 and parts[0] == "crates" and parts[2] == "Cargo.toml"
    ) or path == "apps/desktop/src-tauri/Cargo.toml"


def is_schema_contract(path: str) -> bool:
    return (
        path in {
            "Cargo.toml",
            "Cargo.lock",
            "justfile",
            ".cargo/config",
            ".cargo/config.toml",
            "AGENTS.md",
            "docs/ARCHITECTURE.md",
            "docs/SCHEMA_CONTRACTS.md",
            SCHEMA_TOOL_REGISTRY_APPROVAL,
        }
        or is_workspace_member_manifest(path)
        or path.startswith("crates/kanban-contract/")
        or path.startswith("crates/kanban-schema-tool/")
        or path.startswith("schemas/")
        or path in {
            "scripts/schema_adoption_witnesses.py",
            "scripts/schema_dependency_policy.py",
            "scripts/test_schema_adoption_witnesses.py",
            "scripts/test-schema-cargo-tree.sh",
            "scripts/test_schema_dependency_isolation.py",
            "scripts/test_schema_recipe_witness.py",
            "scripts/schema_docs_markers.py",
            "scripts/test_schema_docs_markers.py",
            "scripts/spec_bundle.py",
            "scripts/test_spec_bundle.py",
        }
    )


def is_schema_docs(path: str) -> bool:
    return (
        ("/" not in path and path.endswith(".md"))
        or (path.startswith("docs/") and path.endswith(".md"))
        or path in SPEC_BUNDLE_SOURCE_PATHS
        or path in {"scripts/spec_bundle.py", "scripts/test_spec_bundle.py"}
    )


def is_vector_helper(path: str) -> bool:
    return path.startswith((
        "crates/kanban-vector/",
        "crates/kanban-vector-lancedb/",
        "crates/kanban-derived-io/",
        "crates/kanban-helper-protocol/",
    ))


def is_graph_helper(path: str) -> bool:
    return path.startswith((
        "crates/kanban-graph/",
        "crates/kanban-graph-oxigraph/",
        "crates/kanban-derived-io/",
        "crates/kanban-helper-protocol/",
    ))


def is_cli(path: str) -> bool:
    return path.startswith("crates/kanban-cli/") or path == "docs/CLI_SPEC.md"


def is_server_api(path: str) -> bool:
    return path.startswith("crates/kanban-server/") or path == "docs/API_SPEC.md"


def is_sqlite_core_state_machine(path: str) -> bool:
    return (
        path.startswith("crates/kanban-core/")
        or path.startswith("crates/kanban-sqlite/")
        or path.startswith("migrations/")
        or path == "docs/STATE_MACHINE.md"
        or path == "docs/DATA_MODEL.md"
    )


def is_search_graph_vector_context(path: str) -> bool:
    needles = {"search", "graph", "vector", "context", "indexer", "entity"}
    parts = set(_parts(path))
    return any(part in needles for part in parts) or any(
        path.startswith(f"crates/kanban-{name}/")
        for name in ("search", "graph", "vector", "context", "indexer", "entity")
    )


def is_scripts_packaging_release_sensitive(path: str) -> bool:
    release_names = {
        "Cargo.toml",
        "Cargo.lock",
        "justfile",
        ".cargo/config",
        ".cargo/config.toml",
        "rust-toolchain.toml",
        ".config/nextest.toml",
        SCHEMA_TOOL_REGISTRY_APPROVAL,
    }
    file_name = PurePosixPath(path).name.lower()
    return (
        path in release_names
        or path.startswith(".github/workflows/")
        or path.startswith("scripts/")
        or path.startswith("apps/desktop/src-tauri/")
        or "release" in file_name
        or "version" in file_name
        or "package" in file_name
    )


CLASSIFIERS = {
    "docs-only": is_docs_only,
    "desktop": is_desktop,
    "core": is_core,
    "schema-contract": is_schema_contract,
    "schema-docs": is_schema_docs,
    "vector-helper": is_vector_helper,
    "graph-helper": is_graph_helper,
    "cli": is_cli,
    "server/api": is_server_api,
    "sqlite/core/state-machine": is_sqlite_core_state_machine,
    "search/graph/vector/context": is_search_graph_vector_context,
    "scripts/packaging/release-sensitive": is_scripts_packaging_release_sensitive,
}


RULES = (
    Rule("desktop", (["just", "desktop-check"],)),
    Rule("core", (["just", "check-core"],)),
    Rule("schema-contract", (["just", "schema-contract"],)),
    Rule("schema-docs", (["just", "schema-docs"],)),
    Rule("vector-helper", (["just", "check-p", "kanban-vector-lancedb"],)),
    Rule("graph-helper", (["just", "check-p", "kanban-graph-oxigraph"],)),
    Rule(
        "cli",
        (
            ["just", "check-p", "kanban-cli"],
            ["just", "test-p", "kanban-cli"],
            ["just", "clippy-p", "kanban-cli"],
        ),
    ),
    Rule(
        "server/api",
        (
            ["just", "check-p", "kanban-server"],
            ["just", "test-p", "kanban-server"],
            ["just", "clippy-p", "kanban-server"],
        ),
    ),
    Rule(
        "sqlite/core/state-machine",
        (
            ["just", "check-p", "kanban-core"],
            ["just", "test-p", "kanban-core"],
            ["just", "clippy-p", "kanban-core"],
            ["just", "check-p", "kanban-sqlite"],
            ["just", "test-p", "kanban-sqlite"],
            ["just", "clippy-p", "kanban-sqlite"],
        ),
    ),
    Rule(
        "search/graph/vector/context",
        (
            ["just", "check-p", "kanban-search"],
            ["just", "test-p", "kanban-search"],
            ["just", "clippy-p", "kanban-search"],
            ["just", "check-p", "kanban-graph"],
            ["just", "test-p", "kanban-graph"],
            ["just", "clippy-p", "kanban-graph"],
            ["just", "check-p", "kanban-vector"],
            ["just", "test-p", "kanban-vector"],
            ["just", "clippy-p", "kanban-vector"],
            ["just", "check-p", "kanban-context"],
            ["just", "test-p", "kanban-context"],
            ["just", "clippy-p", "kanban-context"],
            ["just", "check-p", "kanban-indexer"],
            ["just", "test-p", "kanban-indexer"],
            ["just", "clippy-p", "kanban-indexer"],
            ["just", "check-p", "kanban-entity"],
            ["just", "test-p", "kanban-entity"],
            ["just", "clippy-p", "kanban-entity"],
        ),
    ),
    Rule(
        "scripts/packaging/release-sensitive",
        (
            ["just", "affected-self-test"],
            ["just", "check-full"],
            ["just", "target-tools"],
        ),
    ),
)


FULL_GATE_PATTERNS = (
    "Cargo.toml",
    "Cargo.lock",
    "justfile",
    ".cargo/config",
    ".cargo/config.toml",
    "rust-toolchain.toml",
    ".config/nextest.toml",
    SCHEMA_TOOL_REGISTRY_APPROVAL,
)

WORKSPACE_RUST_FAST_PATTERNS = {
    "Cargo.toml",
    "Cargo.lock",
    ".cargo/config",
    ".cargo/config.toml",
    "rust-toolchain.toml",
    ".config/nextest.toml",
}


def run_git(args: list[str]) -> list[str]:
    completed = subprocess.run(
        ["git", *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return [line.strip() for line in completed.stdout.splitlines() if line.strip()]


def changed_files(base: str) -> dict[str, list[str]]:
    sources = {
        "base": run_git(["diff", "--name-only", f"{base}...HEAD"]),
        "staged": run_git(["diff", "--name-only", "--cached"]),
        "working_tree": run_git(["diff", "--name-only"]),
        "untracked": run_git(["ls-files", "--others", "--exclude-standard"]),
    }
    all_files: list[str] = []
    seen: set[str] = set()
    for source_files in sources.values():
        for path in source_files:
            if path not in seen:
                all_files.append(path)
                seen.add(path)
    sources["all"] = all_files
    return sources


def classify(paths: Iterable[str]) -> Classifications:
    results: Classifications = {name: [] for name in CLASSIFIERS}
    for path in paths:
        for name, predicate in CLASSIFIERS.items():
            if predicate(path):
                results[name].append(path)
    return {name: files for name, files in results.items() if files}


def has_full_gate_file(path: str) -> bool:
    file_name = PurePosixPath(path).name.lower()
    return (
        PurePosixPath(path).name in {"Cargo.toml", "Cargo.lock"}
        or path in FULL_GATE_PATTERNS
        or path.startswith("scripts/release")
        or path.startswith("scripts/package")
        or "release" in file_name
        or "version" in file_name
        or "package" in file_name
    )


def dedupe_commands(commands: Iterable[Command]) -> list[Command]:
    deduped: list[Command] = []
    seen: set[tuple[str, ...]] = set()
    for command in commands:
        key = tuple(command)
        if key not in seen:
            deduped.append(command)
            seen.add(key)
    return deduped


def build_plan(base: str, paths: list[str]) -> Plan:
    classifications = classify(paths)
    docs_only = bool(paths) and set(classifications) == {"docs-only"}
    commands: list[Command] = []

    if docs_only:
        commands.append(["just", "diff-check"])
    else:
        if any(path in WORKSPACE_RUST_FAST_PATTERNS for path in paths):
            commands.append(["just", "check-full"])
        for rule in RULES:
            if any(rule.matches(path) for path in paths):
                commands.extend(rule.commands)
        if paths:
            commands.append(["just", "diff-check"])

    full_gate_recommended = any(has_full_gate_file(path) for path in paths)
    notes: list[str] = []
    if full_gate_recommended:
        notes.append(
            "`just release` remains the authoritative release gate for release-sensitive diffs."
        )
    if not paths:
        notes.append("No base, staged, working tree, or untracked file changes detected.")

    return {
        "base": base,
        "changed_files": paths,
        "classifications": classifications,
        "commands": dedupe_commands(commands),
        "full_gate_recommended": full_gate_recommended,
        "release_authoritative_command": "just release",
        "notes": notes,
    }


def print_plan(plan: Plan) -> None:
    print(f"base: {plan['base']}")
    print(f"full_gate_recommended: {str(plan['full_gate_recommended']).lower()}")
    print("changed_files:")
    changed = plan["changed_files"]
    if changed:
        for path in changed:
            print(f"  - {path}")
    else:
        print("  - <none>")

    print("classifications:")
    classifications = plan["classifications"]
    if classifications:
        for name, files in classifications.items():
            print(f"  {name}:")
            for path in files:
                print(f"    - {path}")
    else:
        print("  - <none>")

    print("commands:")
    commands = plan["commands"]
    if commands:
        for command in commands:
            print(f"  - {' '.join(command)}")
    else:
        print("  - <none>")

    notes = plan["notes"]
    if notes:
        print("notes:")
        for note in notes:
            print(f"  - {note}")


def execute(plan: Plan) -> int:
    commands = plan["commands"]

    for command in commands:
        print(f"+ {' '.join(command)}", flush=True)
        completed = subprocess.run(command)
        if completed.returncode != 0:
            if is_allowed_empty_test_result(command, completed.returncode):
                print(
                    f"ok: {' '.join(command)} has no tests; check/clippy remain required",
                    flush=True,
                )
                continue
            return completed.returncode
    return 0


ALLOW_EMPTY_TEST_PACKAGES = {
    "kanban-search",
    "kanban-graph",
    "kanban-entity",
}


def is_allowed_empty_test_result(command: Command, returncode: int) -> bool:
    return (
        returncode == 4
        and len(command) == 3
        and command[:2] == ["just", "test-p"]
        and command[2] in ALLOW_EMPTY_TEST_PACKAGES
    )


def self_test() -> None:
    cases = [
        (
            "docs only",
            ["docs/SPEC.md", "README.md"],
            {"docs-only", "schema-docs"},
            [["just", "schema-docs"], ["just", "diff-check"]],
            False,
        ),
        (
            "desktop",
            ["apps/desktop/src/App.tsx"],
            {"desktop"},
            [["just", "desktop-check"], ["just", "diff-check"]],
            False,
        ),
        (
            "cli",
            ["crates/kanban-cli/src/main.rs"],
            {"core", "cli"},
            [["just", "check-core"], ["just", "check-p", "kanban-cli"], ["just", "diff-check"]],
            False,
        ),
        (
            "cli spec",
            ["docs/CLI_SPEC.md"],
            {"docs-only", "schema-docs", "core", "cli"},
            [["just", "schema-docs"], ["just", "check-core"], ["just", "check-p", "kanban-cli"], ["just", "diff-check"]],
            False,
        ),
        (
            "server api",
            ["crates/kanban-server/src/router.rs", "docs/API_SPEC.md"],
            {"core", "server/api", "docs-only", "schema-docs"},
            [["just", "schema-docs"], ["just", "check-core"], ["just", "check-p", "kanban-server"], ["just", "diff-check"]],
            False,
        ),
        (
            "sqlite state",
            ["crates/kanban-sqlite/src/service/transitions.rs", "migrations/004_priority_levels.sql"],
            {"core", "sqlite/core/state-machine"},
            [["just", "check-core"], ["just", "check-p", "kanban-core"], ["just", "check-p", "kanban-sqlite"]],
            False,
        ),
        (
            "search graph vector context",
            [
                "crates/kanban-vector/src/lib.rs",
                "crates/kanban-context/src/lib.rs",
                "crates/kanban-indexer/src/lib.rs",
                "crates/kanban-entity/src/lib.rs",
            ],
            {"core", "search/graph/vector/context"},
            [
                ["just", "check-core"],
                ["just", "check-p", "kanban-vector"],
                ["just", "check-p", "kanban-context"],
                ["just", "check-p", "kanban-indexer"],
                ["just", "check-p", "kanban-entity"],
            ],
            False,
        ),
        (
            "vector base crate propagates to helper",
            ["crates/kanban-vector/src/lib.rs"],
            {"core", "search/graph/vector/context", "vector-helper"},
            [
                ["just", "check-core"],
                ["just", "check-p", "kanban-vector"],
                ["just", "check-p", "kanban-vector-lancedb"],
                ["just", "diff-check"],
            ],
            False,
        ),
        (
            "graph base crate propagates to helper",
            ["crates/kanban-graph/src/lib.rs"],
            {"core", "search/graph/vector/context", "graph-helper"},
            [
                ["just", "check-core"],
                ["just", "check-p", "kanban-graph"],
                ["just", "check-p", "kanban-graph-oxigraph"],
                ["just", "diff-check"],
            ],
            False,
        ),
        (
            "derived io propagates to both helpers",
            ["crates/kanban-derived-io/src/lib.rs"],
            {"core", "vector-helper", "graph-helper"},
            [
                ["just", "check-core"],
                ["just", "check-p", "kanban-vector-lancedb"],
                ["just", "check-p", "kanban-graph-oxigraph"],
                ["just", "diff-check"],
            ],
            False,
        ),
        (
            "helper protocol propagates to core and both helpers",
            ["crates/kanban-helper-protocol/src/lib.rs"],
            {"core", "vector-helper", "graph-helper"},
            [
                ["just", "check-core"],
                ["just", "check-p", "kanban-vector-lancedb"],
                ["just", "check-p", "kanban-graph-oxigraph"],
                ["just", "diff-check"],
            ],
            False,
        ),
        (
            "vector helper",
            ["crates/kanban-vector-lancedb/src/lib.rs"],
            {"vector-helper"},
            [["just", "check-p", "kanban-vector-lancedb"], ["just", "diff-check"]],
            False,
        ),
        (
            "graph helper",
            ["crates/kanban-graph-oxigraph/src/lib.rs"],
            {"graph-helper"},
            [["just", "check-p", "kanban-graph-oxigraph"], ["just", "diff-check"]],
            False,
        ),
        (
            "both helpers",
            ["crates/kanban-vector-lancedb/src/lib.rs", "crates/kanban-graph-oxigraph/src/lib.rs"],
            {"vector-helper", "graph-helper"},
            [
                ["just", "check-p", "kanban-vector-lancedb"],
                ["just", "check-p", "kanban-graph-oxigraph"],
                ["just", "diff-check"],
            ],
            False,
        ),
        (
            "schema contract",
            [
                "crates/kanban-contract/src/schema.rs",
                "schemas/fixtures/api/error-response.v1.valid.json",
            ],
            {"core", "schema-contract"},
            [
                ["just", "check-core"],
                ["just", "schema-contract"],
                ["just", "diff-check"],
            ],
            False,
        ),
        (
            "schema tool source",
            ["crates/kanban-schema-tool/src/lib.rs"],
            {"schema-contract"},
            [["just", "schema-contract"], ["just", "diff-check"]],
            False,
        ),
        (
            "schema tool manifest",
            ["crates/kanban-schema-tool/Cargo.toml"],
            {"schema-contract"},
            [["just", "schema-contract"], ["just", "diff-check"]],
            True,
        ),
        (
            "schema registry approval",
            [SCHEMA_TOOL_REGISTRY_APPROVAL],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [
                ["just", "schema-contract"],
                ["just", "affected-self-test"],
                ["just", "check-full"],
                ["just", "target-tools"],
                ["just", "diff-check"],
            ],
            True,
        ),
        (
            "release sensitive",
            ["justfile", "scripts/package-cli-linux.sh"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "affected-self-test"], ["just", "check-full"], ["just", "target-tools"], ["just", "diff-check"]],
            True,
        ),
        (
            "workspace manifest",
            ["Cargo.toml"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "check-full"], ["just", "affected-self-test"], ["just", "diff-check"]],
            True,
        ),
        (
            "workspace lockfile",
            ["Cargo.lock"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "check-full"], ["just", "affected-self-test"], ["just", "diff-check"]],
            True,
        ),
        (
            "cargo source config",
            [".cargo/config"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [
                ["just", "check-full"],
                ["just", "schema-contract"],
                ["just", "affected-self-test"],
                ["just", "target-tools"],
                ["just", "diff-check"],
            ],
            True,
        ),
        (
            "cargo source config toml",
            [".cargo/config.toml"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [
                ["just", "check-full"],
                ["just", "schema-contract"],
                ["just", "affected-self-test"],
                ["just", "target-tools"],
                ["just", "diff-check"],
            ],
            True,
        ),
        (
            "nested crate manifest",
            ["crates/kanban-cli/Cargo.toml"],
            {"core", "cli", "schema-contract"},
            [["just", "check-core"], ["just", "schema-contract"], ["just", "check-p", "kanban-cli"], ["just", "diff-check"]],
            True,
        ),
        (
            "desktop tauri manifest",
            ["apps/desktop/src-tauri/Cargo.toml"],
            {"desktop", "schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "desktop-check"], ["just", "schema-contract"], ["just", "affected-self-test"], ["just", "diff-check"]],
            True,
        ),
        (
            "schema witness gate script",
            ["scripts/schema_adoption_witnesses.py"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "affected-self-test"], ["just", "diff-check"]],
            False,
        ),
        (
            "schema dependency policy script",
            ["scripts/schema_dependency_policy.py"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "affected-self-test"], ["just", "diff-check"]],
            False,
        ),
        (
            "schema dependency isolation self-test",
            ["scripts/test_schema_dependency_isolation.py"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "affected-self-test"], ["just", "diff-check"]],
            False,
        ),
        (
            "schema recipe execution witness",
            ["scripts/test_schema_recipe_witness.py"],
            {"schema-contract", "scripts/packaging/release-sensitive"},
            [["just", "schema-contract"], ["just", "affected-self-test"], ["just", "diff-check"]],
            False,
        ),
        (
            "schema architecture policy docs",
            ["AGENTS.md", "docs/ARCHITECTURE.md", "docs/SCHEMA_CONTRACTS.md"],
            {"docs-only", "schema-contract"},
            [["just", "schema-contract"], ["just", "diff-check"]],
            False,
        ),
    ]

    for name, paths, expected_classes, expected_commands, expected_full_gate in cases:
        plan = build_plan("main", paths)
        classes = set(plan["classifications"])
        missing_classes = expected_classes - classes
        if missing_classes:
            raise AssertionError(f"{name}: missing classifications {missing_classes}")
        commands = plan["commands"]
        for command in expected_commands:
            if command not in commands:
                raise AssertionError(f"{name}: missing command {command}; got {commands}")
        if plan["full_gate_recommended"] is not expected_full_gate:
            raise AssertionError(
                f"{name}: full_gate_recommended expected {expected_full_gate}, "
                f"got {plan['full_gate_recommended']}"
            )

    testless_plan = build_plan(
        "main",
        [
            "crates/kanban-search/src/lib.rs",
            "crates/kanban-graph/src/lib.rs",
            "crates/kanban-entity/src/lib.rs",
        ],
    )
    for package in ("kanban-search", "kanban-graph", "kanban-entity"):
        command = ["just", "test-p", package]
        if command not in testless_plan["commands"]:
            raise AssertionError(
                f"testless package {package} must retain test command {command}"
            )
        if not is_allowed_empty_test_result(command, 4):
            raise AssertionError(f"testless package {package} must accept nextest exit 4")
        if is_allowed_empty_test_result(command, 1):
            raise AssertionError(f"test failure for {package} must remain fatal")
    if is_allowed_empty_test_result(["just", "test-p", "kanban-vector"], 4):
        raise AssertionError("packages outside the explicit allowlist must reject exit 4")

    root = Path(__file__).resolve().parents[1]
    with (root / "Cargo.toml").open("rb") as manifest_file:
        workspace = tomllib.load(manifest_file)["workspace"]
    schema_inputs = [
        "Cargo.toml",
        "Cargo.lock",
        "justfile",
        ".cargo/config",
        ".cargo/config.toml",
        "scripts/test_schema_recipe_witness.py",
        "scripts/schema_docs_markers.py",
        "scripts/test_schema_docs_markers.py",
        "scripts/spec_bundle.py",
        "scripts/test_spec_bundle.py",
        "AGENTS.md",
        "docs/ARCHITECTURE.md",
        "docs/SCHEMA_CONTRACTS.md",
        *(f"{member}/Cargo.toml" for member in workspace["members"]),
    ]
    for path in schema_inputs:
        plan = build_plan("main", [path])
        if "schema-contract" not in plan["classifications"]:
            raise AssertionError(
                f"schema manifest routing missing classification: {path}"
            )
        if ["just", "schema-contract"] not in plan["commands"]:
            raise AssertionError(
                f"schema manifest routing missing command: {path}; got {plan['commands']}"
            )

    for path in SPEC_BUNDLE_SOURCE_PATHS:
        plan = build_plan("main", [path])
        if "schema-docs" not in plan["classifications"]:
            raise AssertionError(
                f"SPEC bundle source missing schema-docs classification: {path}"
            )
        if ["just", "schema-docs"] not in plan["commands"]:
            raise AssertionError(
                f"SPEC bundle source missing schema-docs command: {path}; "
                f"got {plan['commands']}"
            )

    duplicate_plan = build_plan("main", ["crates/kanban-cli/src/main.rs", "crates/kanban-cli/tests/task.rs"])
    command_keys = [tuple(command) for command in duplicate_plan["commands"]]
    if len(command_keys) != len(set(command_keys)):
        raise AssertionError(f"duplicate commands in plan: {duplicate_plan['commands']}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="main", help="base ref for committed branch diff")
    parser.add_argument(
        "--mode",
        choices=("plan", "json", "run"),
        default="plan",
        help="print a human plan, print JSON, or execute the planned commands",
    )
    parser.add_argument("--self-test", action="store_true", help="run script self-tests")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        print("affected-validation self-test passed")
        return 0

    base = args.base.removeprefix("base=")
    sources = changed_files(base)
    plan = build_plan(base, sources["all"])
    plan["sources"] = {key: value for key, value in sources.items() if key != "all"}

    if args.mode == "json":
        print(json.dumps(plan, indent=2, sort_keys=True))
        return 0
    if args.mode == "plan":
        print_plan(plan)
        return 0
    return execute(plan)


if __name__ == "__main__":
    raise SystemExit(main())
