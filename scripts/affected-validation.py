#!/usr/bin/env python3
"""为当前 diff 受影响的文件规划并运行验证命令。"""

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
    full_gate_commands: list[Command]
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
        or path.startswith("docs/")
        or path.endswith(".md")
    )


def is_desktop(path: str) -> bool:
    return path.startswith("apps/desktop/")


CORE_CRATES = {
    "kanban-core",
    "kanban-service",
    "kanban-protocol",
    "kanban-client",
    "kanban-server",
    "kanban-cli",
    "kanban-mcp",
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
        or path.startswith("crates/kanban-core/docs/")
        or path.startswith("crates/kanban-service/docs/")
        or path.startswith("crates/kanban-protocol/docs/")
    )


def is_workspace_member_manifest(path: str) -> bool:
    return (
        path
        in {
            "crates/kanban-core/Cargo.toml",
            "crates/kanban-service/Cargo.toml",
            "crates/kanban-protocol/Cargo.toml",
            "crates/kanban-client/Cargo.toml",
            "crates/kanban-cli/Cargo.toml",
            "crates/kanban-mcp/Cargo.toml",
            "crates/kanban-server/Cargo.toml",
            "xtask/Cargo.toml",
        }
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
            SCHEMA_TOOL_REGISTRY_APPROVAL,
        }
        or is_workspace_member_manifest(path)
        or path.startswith("crates/kanban-protocol/")
        or path.startswith("xtask/")
        or path.startswith("schemas/")
        or path in {
            "scripts/schema_adoption_witnesses.py",
            "scripts/schema_dependency_policy.py",
            "scripts/test_schema_adoption_witnesses.py",
            "scripts/test-schema-cargo-tree.sh",
            "scripts/test_schema_dependency_isolation.py",
            "scripts/test_schema_recipe_witness.py",
        }
    )


def is_cli(path: str) -> bool:
    return path.startswith("crates/kanban-cli/")


def is_server_api(path: str) -> bool:
    return path.startswith("crates/kanban-server/")


def is_sqlite_core_state_machine(path: str) -> bool:
    return (
        path.startswith("crates/kanban-core/")
        or path.startswith("migrations/")
        or path.startswith("crates/kanban-core/docs/")
        or path.startswith("crates/kanban-service/docs/")
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
    "cli": is_cli,
    "server/api": is_server_api,
    "sqlite/core/state-machine": is_sqlite_core_state_machine,
    "scripts/packaging/release-sensitive": is_scripts_packaging_release_sensitive,
}


RULES = (
    Rule("desktop", (["just", "desktop-check"],)),
    Rule("core", (["just", "check-core"],)),
    Rule("schema-contract", (["just", "schema-contract"],)),
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

# `just ci-full` 是当前仓库的完整验证入口，覆盖 Rust workspace、Desktop
# Rust/前端、schema/docs、依赖和现有 smoke gate。它只表达人工复核建议，
# 不把 package/release 流程自动加入 affected 计划。
FULL_GATE_COMMANDS: tuple[Command, ...] = (["just", "ci-full"],)


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
        commands.append(["just", "docs-check"])
        commands.append(["just", "diff-check"])
    else:
        if any(path in WORKSPACE_RUST_FAST_PATTERNS for path in paths):
            commands.append(["just", "check-full"])
        for rule in RULES:
            if any(rule.matches(path) for path in paths):
                commands.extend(rule.commands)
        if any(path.endswith(".md") for path in paths):
            commands.append(["just", "docs-check"])
        if paths:
            commands.append(["just", "diff-check"])

    full_gate_recommended = any(has_full_gate_file(path) for path in paths)
    full_gate_commands = (
        [list(command) for command in FULL_GATE_COMMANDS]
        if full_gate_recommended
        else []
    )
    notes: list[str] = []
    if full_gate_recommended:
        notes.append(
            "建议按需人工运行当前完整仓库验证 `just ci-full`；"
            "affected 计划不自动执行 package/release。"
        )
    if not paths:
        notes.append("未检测到 base、staged、working tree 或 untracked 文件变更。")

    return {
        "base": base,
        "changed_files": paths,
        "classifications": classifications,
        "commands": dedupe_commands(commands),
        "full_gate_recommended": full_gate_recommended,
        "full_gate_commands": full_gate_commands,
        "notes": notes,
    }


def print_plan(plan: Plan) -> None:
    print(f"base: {plan['base']}")
    print(f"full_gate_recommended: {str(plan['full_gate_recommended']).lower()}")
    print("full_gate_commands:")
    full_gate_commands = plan["full_gate_commands"]
    if full_gate_commands:
        for command in full_gate_commands:
            print(f"  - {' '.join(command)}")
    else:
        print("  - <none>")
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
                    f"ok: {' '.join(command)} 没有 tests；仍需执行 check/clippy",
                    flush=True,
                )
                continue
            return completed.returncode
    return 0


ALLOW_EMPTY_TEST_PACKAGES: set[str] = set()


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
            "architecture docs",
            ["docs/architecture.md"],
            {"docs-only"},
            [["just", "docs-check"], ["just", "diff-check"]],
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
            "cli guide",
            ["crates/kanban-cli/README.md"],
            {"docs-only", "core", "cli"},
            [["just", "docs-check"], ["just", "check-core"], ["just", "check-p", "kanban-cli"], ["just", "diff-check"]],
            False,
        ),
        (
            "server api",
            ["crates/kanban-server/src/router.rs", "crates/kanban-server/README.md"],
            {"core", "server/api", "docs-only"},
            [["just", "docs-check"], ["just", "check-core"], ["just", "check-p", "kanban-server"], ["just", "diff-check"]],
            False,
        ),
        (
            "state machine",
            ["crates/kanban-core/src/state.rs", "migrations/004_priority_levels.sql"],
            {"core", "sqlite/core/state-machine"},
            [["just", "check-core"], ["just", "diff-check"]],
            False,
        ),
        (
            "schema contract",
            [
                "crates/kanban-protocol/src/schema.rs",
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
            "xtask source",
            ["xtask/src/lib.rs"],
            {"schema-contract"},
            [["just", "schema-contract"], ["just", "diff-check"]],
            False,
        ),
        (
            "xtask manifest",
            ["xtask/Cargo.toml"],
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
            ["AGENTS.md", "docs/architecture.md"],
            {"docs-only"},
            [["just", "docs-check"], ["just", "diff-check"]],
            False,
        ),
    ]

    for name, paths, expected_classes, expected_commands, expected_full_gate in cases:
        plan = build_plan("main", paths)
        classes = set(plan["classifications"])
        missing_classes = expected_classes - classes
        if missing_classes:
            raise AssertionError(f"{name}: 缺少分类 {missing_classes}")
        commands = plan["commands"]
        for command in expected_commands:
            if command not in commands:
                raise AssertionError(f"{name}: 缺少命令 {command}；实际为 {commands}")
        if plan["full_gate_recommended"] is not expected_full_gate:
            raise AssertionError(
                f"{name}: full_gate_recommended 预期为 {expected_full_gate}，"
                f"实际为 {plan['full_gate_recommended']}"
            )
        expected_full_gate_commands = (
            [list(command) for command in FULL_GATE_COMMANDS]
            if expected_full_gate
            else []
        )
        if plan["full_gate_commands"] != expected_full_gate_commands:
            raise AssertionError(
                f"{name}: full_gate_commands 预期为 {expected_full_gate_commands}，"
                f"实际为 {plan['full_gate_commands']}"
            )

    if is_allowed_empty_test_result(["just", "test-p", "kanban-client"], 4):
        raise AssertionError("显式 allowlist 之外的 package 必须拒绝退出码 4")

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
        *(f"{member}/Cargo.toml" for member in workspace["members"]),
    ]
    for path in schema_inputs:
        plan = build_plan("main", [path])
        if "schema-contract" not in plan["classifications"]:
            raise AssertionError(
                f"schema manifest 路由缺少分类: {path}"
            )
        if ["just", "schema-contract"] not in plan["commands"]:
            raise AssertionError(
                f"schema manifest 路由缺少命令: {path}；实际为 {plan['commands']}"
            )

    duplicate_plan = build_plan("main", ["crates/kanban-cli/src/main.rs", "crates/kanban-cli/tests/task.rs"])
    command_keys = [tuple(command) for command in duplicate_plan["commands"]]
    if len(command_keys) != len(set(command_keys)):
        raise AssertionError(f"计划中存在重复命令: {duplicate_plan['commands']}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="main", help="已提交分支 diff 使用的 base ref")
    parser.add_argument(
        "--mode",
        choices=("plan", "json", "run"),
        default="plan",
        help="打印人类可读计划、打印 JSON，或执行计划中的命令",
    )
    parser.add_argument("--self-test", action="store_true", help="运行脚本 self-test")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        print("affected-validation self-test 已通过")
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
