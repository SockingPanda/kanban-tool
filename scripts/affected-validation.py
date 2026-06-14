#!/usr/bin/env python3
"""Plan and run validation commands for files affected by the current diff."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import PurePosixPath
from typing import Iterable


Command = list[str]


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


def is_cli(path: str) -> bool:
    return path.startswith("crates/kanban-cli/")


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
        "rust-toolchain.toml",
        ".config/nextest.toml",
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
    "cli": is_cli,
    "server/api": is_server_api,
    "sqlite/core/state-machine": is_sqlite_core_state_machine,
    "search/graph/vector/context": is_search_graph_vector_context,
    "scripts/packaging/release-sensitive": is_scripts_packaging_release_sensitive,
}


RULES = (
    Rule("desktop", (["just", "desktop-check"],)),
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
        ),
    ),
    Rule(
        "scripts/packaging/release-sensitive",
        (
            ["just", "affected-self-test"],
            ["just", "target-tools"],
        ),
    ),
)


FULL_GATE_PATTERNS = (
    "Cargo.toml",
    "Cargo.lock",
    "justfile",
    "rust-toolchain.toml",
    ".config/nextest.toml",
)

WORKSPACE_RUST_FAST_PATTERNS = {
    "Cargo.toml",
    "Cargo.lock",
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


def classify(paths: Iterable[str]) -> dict[str, list[str]]:
    results = {name: [] for name in CLASSIFIERS}
    for path in paths:
        for name, predicate in CLASSIFIERS.items():
            if predicate(path):
                results[name].append(path)
    return {name: files for name, files in results.items() if files}


def has_full_gate_file(path: str) -> bool:
    file_name = PurePosixPath(path).name.lower()
    return (
        path in FULL_GATE_PATTERNS
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


def build_plan(base: str, paths: list[str]) -> dict[str, object]:
    classifications = classify(paths)
    docs_only = bool(paths) and set(classifications) == {"docs-only"}
    commands: list[Command] = []

    if docs_only:
        commands.append(["just", "diff-check"])
    else:
        if any(path in WORKSPACE_RUST_FAST_PATTERNS for path in paths):
            commands.append(["just", "rust-fast"])
        for rule in RULES:
            if any(rule.matches(path) for path in paths):
                commands.extend(rule.commands)
        if paths:
            commands.append(["just", "diff-check"])

    full_gate_recommended = any(has_full_gate_file(path) for path in paths)
    notes: list[str] = []
    if full_gate_recommended:
        notes.append(
            "`just release` remains the authoritative full gate for release-sensitive diffs."
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


def print_plan(plan: dict[str, object]) -> None:
    print(f"base: {plan['base']}")
    print(f"full_gate_recommended: {str(plan['full_gate_recommended']).lower()}")
    print("changed_files:")
    changed = plan["changed_files"]
    if isinstance(changed, list) and changed:
        for path in changed:
            print(f"  - {path}")
    else:
        print("  - <none>")

    print("classifications:")
    classifications = plan["classifications"]
    if isinstance(classifications, dict) and classifications:
        for name, files in classifications.items():
            print(f"  {name}:")
            for path in files:
                print(f"    - {path}")
    else:
        print("  - <none>")

    print("commands:")
    commands = plan["commands"]
    if isinstance(commands, list) and commands:
        for command in commands:
            print(f"  - {' '.join(command)}")
    else:
        print("  - <none>")

    notes = plan["notes"]
    if isinstance(notes, list) and notes:
        print("notes:")
        for note in notes:
            print(f"  - {note}")


def execute(plan: dict[str, object]) -> int:
    commands = plan["commands"]
    if not isinstance(commands, list):
        print("invalid plan: commands must be a list", file=sys.stderr)
        return 2

    for command in commands:
        if not isinstance(command, list) or not all(isinstance(part, str) for part in command):
            print(f"invalid command in plan: {command!r}", file=sys.stderr)
            return 2
        print(f"+ {' '.join(command)}", flush=True)
        completed = subprocess.run(command)
        if completed.returncode != 0:
            return completed.returncode
    return 0


def self_test() -> None:
    cases = [
        (
            "docs only",
            ["docs/SPEC.md", "README.md"],
            {"docs-only"},
            [["just", "diff-check"]],
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
            {"cli"},
            [["just", "check-p", "kanban-cli"], ["just", "diff-check"]],
            False,
        ),
        (
            "server api",
            ["crates/kanban-server/src/router.rs", "docs/API_SPEC.md"],
            {"server/api", "docs-only"},
            [["just", "check-p", "kanban-server"], ["just", "diff-check"]],
            False,
        ),
        (
            "sqlite state",
            ["crates/kanban-sqlite/src/service/transitions.rs", "migrations/004_priority_levels.sql"],
            {"sqlite/core/state-machine"},
            [["just", "check-p", "kanban-core"], ["just", "check-p", "kanban-sqlite"]],
            False,
        ),
        (
            "search graph vector context",
            ["crates/kanban-vector/src/lib.rs", "crates/kanban-context/src/lib.rs"],
            {"search/graph/vector/context"},
            [["just", "check-p", "kanban-vector"], ["just", "check-p", "kanban-context"]],
            False,
        ),
        (
            "release sensitive",
            ["justfile", "scripts/package-cli-linux.sh"],
            {"scripts/packaging/release-sensitive"},
            [["just", "affected-self-test"], ["just", "target-tools"], ["just", "diff-check"]],
            True,
        ),
        (
            "workspace manifest",
            ["Cargo.toml"],
            {"scripts/packaging/release-sensitive"},
            [["just", "rust-fast"], ["just", "affected-self-test"], ["just", "diff-check"]],
            True,
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
