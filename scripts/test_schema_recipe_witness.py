#!/usr/bin/env python3
"""用真实 just 与 fake executables 锁定产品/schema recipe 的执行调用图。"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable


ROOT = Path(__file__).resolve().parents[1]
REAL_JUST = shutil.which("just")
CORE_PACKAGES = (
    "kanban-core",
    "kanban-contract",
    "kanban-entity",
    "kanban-indexer",
    "kanban-search",
    "kanban-graph",
    "kanban-vector",
    "kanban-derived-io",
    "kanban-helper-protocol",
    "kanban-labels",
    "kanban-context",
    "kanban-sqlite",
    "kanban-local",
    "kanban-server",
    "kanban-cli",
)
HELPER_PACKAGES = ("kanban-vector-lancedb", "kanban-graph-oxigraph")
TOOL_PACKAGE = "kanban-schema-tool"
CONTRACT_PACKAGE = "kanban-contract"
Event = dict[str, object]
ExpectedBuilder = Callable[[Path, bool], list[Event]]


class RecipeWitnessError(RuntimeError):
    """recipe 实际执行序列偏离 canonical 调用图。"""


def _event(root: Path, kind: str, argv: list[str]) -> Event:
    invoked_as = {
        "cargo": root / "bin/cargo",
        "just": root / "bin/just",
        "build-lock": root / "scripts/cargo-build-lock.sh",
        "python3": root / "bin/python3",
        "script": root / "scripts/test-schema-cargo-tree.sh",
    }[kind]
    return {
        "kind": kind,
        "argv": argv,
        "invoked_as": str(invoked_as),
        "cwd": str(root),
    }


def _cargo(root: Path, *argv: str) -> list[Event]:
    return [_event(root, "cargo", list(argv))]


def _locked(root: Path, *argv: str) -> list[Event]:
    cargo_argv = list(argv)
    return [
        _event(root, "build-lock", ["--", "cargo", *cargo_argv]),
        _event(root, "cargo", cargo_argv),
    ]


def _nested(
    root: Path,
    recipe: str,
    expected: list[Event],
    *args: str,
) -> list[Event]:
    return [_event(root, "just", [recipe, *args]), *expected]


def _package_args(packages: tuple[str, ...]) -> list[str]:
    result: list[str] = []
    for package in packages:
        result.extend(("-p", package))
    return result


def _fmt_events(root: Path, packages: tuple[str, ...]) -> list[Event]:
    return _cargo(root, "fmt", *_package_args(packages), "--", "--check")


def _fmt(root: Path, _: bool) -> list[Event]:
    return _fmt_events(root, CORE_PACKAGES)


def _fmt_full(root: Path, _: bool) -> list[Event]:
    return _fmt_events(root, (*CORE_PACKAGES, *HELPER_PACKAGES))


def _schema_fmt(root: Path, _: bool) -> list[Event]:
    return _fmt_events(root, (CONTRACT_PACKAGE, TOOL_PACKAGE))


def _test_events(
    root: Path,
    packages: tuple[str, ...],
    nextest: bool,
) -> list[Event]:
    probe = _cargo(root, "nextest", "--version")
    package_args = _package_args(packages)
    if nextest:
        return [
            *probe,
            *_locked(root, "nextest", "run", *package_args, "--no-fail-fast"),
        ]
    return [*probe, *_locked(root, "test", *package_args)]


def _check_core(root: Path, _: bool) -> list[Event]:
    return _locked(root, "check", "--tests", *_package_args(CORE_PACKAGES))


def _check_helpers(root: Path, _: bool) -> list[Event]:
    return _locked(root, "check", "--tests", *_package_args(HELPER_PACKAGES))


def _check_full(root: Path, nextest: bool) -> list[Event]:
    return [
        *_nested(root, "check-core", _check_core(root, nextest)),
        *_nested(root, "check-helpers", _check_helpers(root, nextest)),
    ]


def _test_core(root: Path, nextest: bool) -> list[Event]:
    return _test_events(root, CORE_PACKAGES, nextest)


def _test_helpers(root: Path, nextest: bool) -> list[Event]:
    return _test_events(root, HELPER_PACKAGES, nextest)


def _test_full(root: Path, nextest: bool) -> list[Event]:
    probe = _cargo(root, "nextest", "--version")
    excludes = (
        "--workspace",
        "--exclude",
        "kanban-desktop",
        "--exclude",
        TOOL_PACKAGE,
    )
    if nextest:
        return [
            *probe,
            *_locked(root, "nextest", "run", *excludes, "--no-fail-fast"),
        ]
    return [*probe, *_locked(root, "test", *excludes)]


def _clippy_core(root: Path, _: bool) -> list[Event]:
    return _locked(
        root,
        "clippy",
        "--all-targets",
        *_package_args(CORE_PACKAGES),
        "--",
        "-D",
        "warnings",
    )


def _clippy_helpers(root: Path, _: bool) -> list[Event]:
    return _locked(
        root,
        "clippy",
        "--all-targets",
        *_package_args(HELPER_PACKAGES),
        "--",
        "-D",
        "warnings",
    )


def _clippy_full(root: Path, nextest: bool) -> list[Event]:
    return [
        *_nested(root, "clippy-core", _clippy_core(root, nextest)),
        *_nested(root, "clippy-helpers", _clippy_helpers(root, nextest)),
    ]


def _rust_fast(root: Path, nextest: bool) -> list[Event]:
    return [
        *_nested(root, "fmt", _fmt(root, nextest)),
        *_nested(root, "check-core", _check_core(root, nextest)),
        *_nested(root, "test-core", _test_core(root, nextest)),
        *_nested(root, "clippy-core", _clippy_core(root, nextest)),
    ]


def _rust_full(root: Path, nextest: bool) -> list[Event]:
    return [
        *_nested(root, "fmt-full", _fmt_full(root, nextest)),
        *_nested(root, "check-full", _check_full(root, nextest)),
        *_nested(root, "test-full", _test_full(root, nextest)),
        *_nested(root, "clippy-full", _clippy_full(root, nextest)),
    ]


def _schema_tool(root: Path, nextest: bool) -> list[Event]:
    check = _locked(root, "check", "-p", TOOL_PACKAGE, "--tests")
    probe = _cargo(root, "nextest", "--version")
    if nextest:
        tests = [
            *probe,
            *_locked(
                root, "nextest", "run", "-p", TOOL_PACKAGE, "--no-fail-fast"
            ),
        ]
    else:
        tests = [*probe, *_locked(root, "test", "-p", TOOL_PACKAGE)]
    clippy = _locked(
        root,
        "clippy",
        "-p",
        TOOL_PACKAGE,
        "--all-targets",
        "--",
        "-D",
        "warnings",
    )
    return [
        *_nested(root, "check-p", check, TOOL_PACKAGE),
        *_nested(root, "test-p", tests, TOOL_PACKAGE),
        *clippy,
    ]


def _feature_contract(root: Path, nextest: bool) -> list[Event]:
    probe = _cargo(root, "nextest", "--version")
    if nextest:
        tests = _locked(
            root,
            "nextest",
            "run",
            "-p",
            CONTRACT_PACKAGE,
            "--features",
            "schema",
            "--no-fail-fast",
            "--no-tests",
            "pass",
        )
    else:
        tests = _locked(
            root, "test", "-p", CONTRACT_PACKAGE, "--features", "schema"
        )
    clippy = _locked(
        root,
        "clippy",
        "-p",
        CONTRACT_PACKAGE,
        "--all-targets",
        "--features",
        "schema",
        "--",
        "-D",
        "warnings",
    )
    return [*probe, *tests, *clippy]


def _schema_check(root: Path, _: bool) -> list[Event]:
    return _locked(
        root,
        "run",
        "-p",
        TOOL_PACKAGE,
        "--bin",
        "kanban-schema",
        "--",
        "check",
        "--root",
        ".",
    )


def _schema_generate(root: Path, _: bool) -> list[Event]:
    return _locked(
        root,
        "run",
        "-p",
        TOOL_PACKAGE,
        "--bin",
        "kanban-schema",
        "--",
        "generate",
        "--root",
        ".",
    )


def _spec_bundle_generate(root: Path, _: bool) -> list[Event]:
    return [
        _event(
            root,
            "python3",
            ["-B", "scripts/spec_bundle.py", "--root", ".", "--write"],
        )
    ]


def _spec_bundle_check(root: Path, _: bool) -> list[Event]:
    return [
        _event(root, "python3", ["-B", "scripts/test_spec_bundle.py"]),
        _event(
            root,
            "python3",
            ["-B", "scripts/spec_bundle.py", "--root", ".", "--check"],
        ),
    ]


def _schema_docs(root: Path, _: bool) -> list[Event]:
    return [
        _event(root, "just", ["spec-bundle-check"]),
        _event(root, "python3", ["-B", "scripts/test_schema_docs_markers.py"]),
        _event(root, "python3", ["-B", "scripts/schema_docs_markers.py", "--root", "."]),
    ]


def _schema_contract(root: Path, _: bool) -> list[Event]:
    calls = (
        ("schema-dependency-isolation",),
        ("schema-fmt",),
        ("feature-p", CONTRACT_PACKAGE, "schema"),
        ("schema-tool",),
        ("schema-check",),
        ("schema-docs",),
        ("schema-surface-audit",),
        ("schema-adoption-witness",),
    )
    return [_event(root, "just", list(call)) for call in calls]


def _schema_dependency_self_test(root: Path, _: bool) -> list[Event]:
    return [
        _event(
            root,
            "python3",
            ["-B", "scripts/test_schema_dependency_isolation.py"],
        ),
        _event(
            root,
            "python3",
            ["-B", "scripts/test_schema_recipe_witness.py"],
        ),
    ]


def _schema_dependency(root: Path, _: bool) -> list[Event]:
    return [
        _event(root, "just", ["schema-dependency-isolation-self-test"]),
        _event(root, "python3", ["-B", "scripts/schema_dependency_policy.py"]),
        _event(root, "script", []),
    ]


def _schema_surface(root: Path, _: bool) -> list[Event]:
    return [
        _event(
            root,
            "just",
            [
                "test-p",
                "kanban-server",
                "api_route_catalog_matches_exact_contract_catalog",
            ],
        ),
        _event(
            root,
            "just",
            [
                "test-p",
                "kanban-cli",
                "clap_leaf_commands_match_exact_contract_catalog",
            ],
        ),
        _event(
            root,
            "just",
            [
                "test-p",
                "kanban-sqlite",
                "jsonl_export_discriminators_match_exact_contract_catalog",
            ],
        ),
    ]


def _schema_adoption_self_test(root: Path, _: bool) -> list[Event]:
    return [
        _event(
            root,
            "python3",
            ["-B", "scripts/test_schema_adoption_witnesses.py"],
        )
    ]


def _schema_adoption(root: Path, _: bool) -> list[Event]:
    return [
        _event(root, "just", ["schema-adoption-witness-self-test"]),
        _event(
            root,
            "python3",
            ["-B", "scripts/schema_adoption_witnesses.py", "--root", "."],
        ),
    ]


def _schema_audit_closed(root: Path, _: bool) -> list[Event]:
    return [
        _event(root, "just", ["schema-adoption-witness"]),
        *_locked(
            root,
            "run",
            "-p",
            TOOL_PACKAGE,
            "--bin",
            "kanban-schema",
            "--",
            "audit",
            "--root",
            ".",
            "--require-closed",
        ),
    ]


def _release(root: Path, _: bool) -> list[Event]:
    calls = (
        "affected-self-test",
        "schema-contract",
        "audit",
        "rust-full",
        "bench-check",
        "target-tools",
        "cli-package",
        "cli-package-layout",
        "desktop-package-config",
        "desktop-package",
        "desktop-package-layout",
        "smoke",
        "diff-check",
    )
    return [_event(root, "just", [recipe]) for recipe in calls]


CASES: tuple[tuple[str, tuple[str, ...], ExpectedBuilder, bool], ...] = (
    ("fmt", (), _fmt, True),
    ("fmt-check", (), _fmt, True),
    ("fmt-full", (), _fmt_full, True),
    ("check", (), _check_core, True),
    ("check-core", (), _check_core, True),
    ("check-helpers", (), _check_helpers, True),
    ("check-full", (), _check_full, True),
    (
        "test",
        (),
        lambda root, nextest: _nested(
            root, "test-core", _test_core(root, nextest)
        ),
        True,
    ),
    ("test-core", (), _test_core, True),
    ("test-helpers", (), _test_helpers, True),
    ("test-full", (), _test_full, True),
    (
        "clippy",
        (),
        lambda root, nextest: _nested(
            root, "clippy-core", _clippy_core(root, nextest)
        ),
        True,
    ),
    ("clippy-core", (), _clippy_core, True),
    ("clippy-helpers", (), _clippy_helpers, True),
    ("clippy-full", (), _clippy_full, True),
    ("rust-fast", (), _rust_fast, True),
    ("rust-full", (), _rust_full, True),
    ("feature-p", (CONTRACT_PACKAGE, "schema"), _feature_contract, True),
    ("schema-generate", (), _schema_generate, True),
    ("schema-check", (), _schema_check, True),
    ("spec-bundle-generate", (), _spec_bundle_generate, False),
    ("spec-bundle-check", (), _spec_bundle_check, False),
    ("schema-docs", (), _schema_docs, False),
    ("schema-fmt", (), _schema_fmt, True),
    ("schema-tool", (), _schema_tool, True),
    (
        "schema-dependency-isolation-self-test",
        (),
        _schema_dependency_self_test,
        False,
    ),
    ("schema-dependency-isolation", (), _schema_dependency, False),
    ("schema-surface-audit", (), _schema_surface, False),
    (
        "schema-adoption-witness-self-test",
        (),
        _schema_adoption_self_test,
        False,
    ),
    ("schema-adoption-witness", (), _schema_adoption, False),
    ("schema-contract", (), _schema_contract, False),
    ("schema-audit-closed", (), _schema_audit_closed, False),
    ("release", (), _release, False),
)


# just --dump-format json --dump 对全局 parser contract 与每个受保护
# recipe AST 的 canonical JSON（sorted keys、compact separators、末尾换行）
# 做 SHA-256。更新 setting/recipe 必须显式更新对应 hash；运行采样无法触达的
# env/dead branch 也因此 fail closed。
PROTECTED_RECIPE_AST_SHA256 = {
    "fmt": "e602aa4629d31c23849fd7ac6ce8426ada610686ac17b67136154851d3638793",
    "fmt-check": "41e18d94d6309dc6df573f39df07a69adefbc8af245224ba46de2869f6e5c931",
    "fmt-full": "2cd01fd70cfce948fa356205e63cd593c658e1bddfc042cd1bb0d87cc557c878",
    "check": "70377f1313282fbffb1d0f39658c373b080dd18afa0f30a6e023bc63f591d1d8",
    "check-core": "7763aa46f7e81f69a746ff7645cab5476bc22a74b354de3b4fb5a005b729bcbb",
    "check-helpers": "c7c753226684f610d1f0b37735544d7f0ed69e4cce301f2799aedcbade76e161",
    "check-full": "04378c09c886e1829631a0a51f809d9222d4c0fdcee63d1ae0c85e3e7dec40f3",
    "test": "d7364e1d31ff9b6ce18122fa82b6d91433ed1e9bd6d4705ff4dcfd314281bcaf",
    "test-p": "d096ac1d15d2a323322236b9d2edb2faa00b2a4cb2ea40afe670232fef6f744a",
    "check-p": "3247d65a2ac334095a727baca0829ec884cdfdcbc09048fbbc8b5d31c3e8fbe9",
    "rust-fast": "6bbfcbb125926c84e3066e835b390216497d9752147ecefecc6ee912a7d9c76a",
    "test-core": "ef26a32ae07d30020638cff485a4a4362a1c35cc7a251ca57918d2e4eadd1159",
    "test-helpers": "0db865a71b5839289e886854ab5ea5bf0f4f34516c62b37c06f2cddb3cf816e5",
    "test-full": "5ced7cec91f083c1a13b7ecb409a7b58f66985d5103fc94dbeb0bdc7d56888da",
    "clippy": "b68e87fa19b16feb752c1930747e890f496a9b6d7c199e18a014cce921c6b800",
    "clippy-core": "fd2da09da06b2928d6e160858f8a199c78a496b643041506040ac5daf99068e6",
    "clippy-helpers": "43f5df0a0691c2b222539a4e78d99715565ed1715d1204160c287df1d5b16599",
    "clippy-full": "f729c7b8f04a8a8144874ded54eaf8c51a9588f9bc4ccba60f4f4240ac4c3860",
    "rust-full": "874c085fd5ce1da73fa0f2a665d06a238b0ad27eb4bc2b068fd1e0e00648abb2",
    "feature-p": "c9339ff530f2193a835b480967d2179e5e0e3448cec569d0e4ed019a4bb1ebe6",
    "schema-generate": "062dfc46dc85dc6634716bb49c83eb54c5652a59a040d68adf0b54978581d11e",
    "schema-check": "25533121f2c4a69bb80ecce953650ea4d3a07e935f94ea71aedd253bd55e273a",
    "spec-bundle-generate": "8be9071c562468a80334de0e774394e9dfd2b07c756dd9a0c8dca0f758d03305",
    "spec-bundle-check": "b2f7b1b38e32082c6c55f8677cfbabc85e8775afa5d67aacfdf021787ed798f7",
    "schema-docs": "bc47df7c05f552862d34da3555da73590293cf5ca7483d290bb935dc9065b652",
    "schema-fmt": "9034d88a64881b90be3933a912c18b8c0a077f113758bca17f7d9d85d9ef5096",
    "schema-tool": "34d3cb160f5942a198d3d7ec046279f9b86ec5b8e2a2c643c9f04cbc3c631fa8",
    "schema-dependency-isolation-self-test": "09d7e70be11489d5fe1bf2f7328e782c9a1859e78b6dc4e0d48bec67f7db2fff",
    "schema-dependency-isolation": "d5e87173eba132003a4ab5128bfca5bd4e3f57c5379ae666faf06f42c5c18217",
    "schema-adoption-witness-self-test": "dfd3dbc862cf5c192d5d3b4cafce39a65fa7919e0673749fcbf180ec6b90ad05",
    "schema-adoption-witness": "b8a079ff467fb15f447e5649ca2ccc015b6207ad8ccf6dadeb889de59f84745c",
    "schema-surface-audit": "d0d4e8990e2032199e23cfc6274664b6dd0477fec0c84d859aee2300daa14905",
    "schema-contract": "ddd8227b389f91fc388fcfc3cd6f624a2801131bbd72b7d017024a8f1b7dd519",
    "schema-audit-closed": "2411b312998d3c56c6542271f2002d360050197399880606734e44e773e54006",
    "release": "9363953e0f1d4e81ac99662d2f6ba708e9b301506ea5c5495b356d3f3ad1f6bc",
}
AST_ONLY_RECIPE_NAMES = {"check-p", "test-p"}
PROTECTED_JUST_GLOBALS_SHA256 = (
    "1bae71edc87ce712743cfed74fca0071974fc1661f401b0e96e28ab870660125"
)

FAKE_CARGO = r'''#!/usr/bin/env python3
import json
import os
import sys

event = {
    "kind": "cargo",
    "argv": sys.argv[1:],
    "invoked_as": os.path.abspath(sys.argv[0]),
    "cwd": os.getcwd(),
}
with open(os.environ["WITNESS_LOG"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(event, sort_keys=True) + "\n")
if sys.argv[1:] == ["nextest", "--version"]:
    raise SystemExit(0 if os.environ["WITNESS_NEXTEST"] == "1" else 1)
raise SystemExit(0)
'''

FAKE_BUILD_LOCK = r'''#!/usr/bin/env python3
import json
import os
import sys

argv = sys.argv[1:]
event = {
    "kind": "build-lock",
    "argv": argv,
    "invoked_as": os.path.abspath(sys.argv[0]),
    "cwd": os.getcwd(),
}
with open(os.environ["WITNESS_LOG"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(event, sort_keys=True) + "\n")
if not argv or argv[0] != "--" or len(argv) == 1:
    raise SystemExit(64)
child = argv[1:]
os.execvpe(child[0], child, os.environ)
'''

FAKE_JUST = r'''#!/usr/bin/env python3
import json
import os
import sys

event = {
    "kind": "just",
    "argv": sys.argv[1:],
    "invoked_as": os.path.abspath(sys.argv[0]),
    "cwd": os.getcwd(),
}
with open(os.environ["WITNESS_LOG"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(event, sort_keys=True) + "\n")
if os.environ["WITNESS_JUST_DELEGATE"] == "1":
    real_just = os.environ["WITNESS_REAL_JUST"]
    command = [
        real_just,
        "--justfile",
        os.environ["WITNESS_JUSTFILE"],
        "--working-directory",
        os.environ["WITNESS_ROOT"],
        *sys.argv[1:],
    ]
    os.execv(real_just, command)
raise SystemExit(0)
'''

FAKE_DIRECT = r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

name = Path(sys.argv[0]).name
kind = "python3" if name == "python3" else "script"
event = {
    "kind": kind,
    "argv": sys.argv[1:],
    "invoked_as": os.path.abspath(sys.argv[0]),
    "cwd": os.getcwd(),
}
with open(os.environ["WITNESS_LOG"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(event, sort_keys=True) + "\n")
raise SystemExit(0)
'''


def _write_executable(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = content.replace("#!/usr/bin/env python3", f"#!{sys.executable}", 1)
    path.write_text(content, encoding="utf-8")
    path.chmod(0o755)


def verify_recipe_ast_projection(justfile_text: str) -> None:
    if REAL_JUST is None:
        raise RecipeWitnessError("PATH 中缺少真实 just executable")

    expected_names = {name for name, _, _, _ in CASES} | AST_ONLY_RECIPE_NAMES
    configured_names = set(PROTECTED_RECIPE_AST_SHA256)
    if configured_names != expected_names:
        raise RecipeWitnessError(
            "recipe AST hash inventory 与执行矩阵不一致: "
            f"expected={sorted(expected_names)}, configured={sorted(configured_names)}"
        )

    with tempfile.TemporaryDirectory(prefix="schema-recipe-ast-") as temp_dir:
        root = Path(temp_dir)
        justfile = root / "justfile"
        justfile.write_text(justfile_text, encoding="utf-8")
        completed = subprocess.run(
            [
                REAL_JUST,
                "--justfile",
                str(justfile),
                "--working-directory",
                str(root),
                "--dump-format",
                "json",
                "--dump",
            ],
            cwd=root,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    if completed.returncode != 0:
        raise RecipeWitnessError(
            "just parser AST dump 失败 "
            f"({completed.returncode}): stderr={completed.stderr!r}"
        )
    try:
        dump = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RecipeWitnessError(f"just parser AST dump JSON 无效: {error}") from error
    if not isinstance(dump, dict):
        raise RecipeWitnessError("just parser AST dump 顶层必须是 object")
    global_projection = {
        name: dump.get(name) for name in ("settings", "aliases", "modules")
    }
    global_canonical = (
        json.dumps(
            global_projection,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    )
    global_hash = hashlib.sha256(global_canonical.encode("utf-8")).hexdigest()
    if global_hash != PROTECTED_JUST_GLOBALS_SHA256:
        raise RecipeWitnessError(
            "just global parser AST 漂移: "
            f"expected_sha256={PROTECTED_JUST_GLOBALS_SHA256}, "
            f"actual_sha256={global_hash}, "
            f"projection={global_canonical.rstrip()}"
        )

    recipes = dump.get("recipes")
    if not isinstance(recipes, dict):
        raise RecipeWitnessError("just parser AST dump 缺少 recipes object")

    for name, expected_hash in PROTECTED_RECIPE_AST_SHA256.items():
        projection = recipes.get(name)
        if not isinstance(projection, dict):
            raise RecipeWitnessError(f"just parser AST 缺少受保护 recipe: {name}")
        canonical = (
            json.dumps(
                projection,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        )
        actual_hash = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
        if actual_hash != expected_hash:
            raise RecipeWitnessError(
                f"{name} parser AST 漂移: expected_sha256={expected_hash}, "
                f"actual_sha256={actual_hash}, projection={canonical.rstrip()}"
            )


def _validate_build_lock_transparency(events: list[Event]) -> None:
    for index, event in enumerate(events):
        if event["kind"] != "build-lock":
            continue
        if index + 1 >= len(events):
            raise RecipeWitnessError("build-lock 后缺少被透明转发的 cargo event")
        cargo = events[index + 1]
        argv = event["argv"]
        if (
            cargo["kind"] != "cargo"
            or not isinstance(argv, list)
            or argv[:2] != ["--", "cargo"]
            or argv[2:] != cargo["argv"]
            or event["cwd"] != cargo["cwd"]
        ):
            raise RecipeWitnessError(
                "build-lock 未与下一条 cargo argv 一一透明对应: "
                f"lock={event}, next={cargo}"
            )


def verify_case(
    justfile_text: str,
    recipe: str,
    args: tuple[str, ...],
    expected_builder: ExpectedBuilder,
    *,
    nextest: bool,
    delegate_nested_just: bool,
) -> None:
    if REAL_JUST is None:
        raise RecipeWitnessError("PATH 中缺少真实 just executable")

    with tempfile.TemporaryDirectory(prefix="schema-recipe-witness-") as temp_dir:
        root = Path(temp_dir)
        justfile = root / "justfile"
        log = root / "events.jsonl"
        justfile.write_text(justfile_text, encoding="utf-8")
        _write_executable(root / "bin/cargo", FAKE_CARGO)
        _write_executable(root / "bin/just", FAKE_JUST)
        _write_executable(root / "bin/python3", FAKE_DIRECT)
        _write_executable(root / "scripts/cargo-build-lock.sh", FAKE_BUILD_LOCK)
        _write_executable(
            root / "scripts/test-schema-cargo-tree.sh",
            FAKE_DIRECT,
        )
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{root / 'bin'}:{env.get('PATH', '')}",
                "WITNESS_LOG": str(log),
                "WITNESS_NEXTEST": "1" if nextest else "0",
                "WITNESS_JUST_DELEGATE": "1" if delegate_nested_just else "0",
                "WITNESS_REAL_JUST": REAL_JUST,
                "WITNESS_JUSTFILE": str(justfile),
                "WITNESS_ROOT": str(root),
            }
        )
        command = [
            REAL_JUST,
            "--justfile",
            str(justfile),
            "--working-directory",
            str(root),
            recipe,
            *args,
        ]
        completed = subprocess.run(
            command,
            cwd=root,
            env=env,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        events = []
        if log.exists():
            events = [
                json.loads(line)
                for line in log.read_text(encoding="utf-8").splitlines()
                if line
            ]
        if completed.returncode != 0:
            raise RecipeWitnessError(
                f"{recipe} 真实 just 执行失败 ({completed.returncode}); "
                f"stdout={completed.stdout!r}; stderr={completed.stderr!r}; "
                f"events={events}"
            )
        _validate_build_lock_transparency(events)
        expected = expected_builder(root, nextest)
        if events != expected:
            raise RecipeWitnessError(
                f"{recipe} execution trace 漂移 (nextest={nextest}):\n"
                f"expected={json.dumps(expected, ensure_ascii=False, indent=2)}\n"
                f"actual={json.dumps(events, ensure_ascii=False, indent=2)}\n"
                f"stdout={completed.stdout!r}; stderr={completed.stderr!r}"
            )


def audit_recipe_witness(justfile_text: str) -> None:
    verify_recipe_ast_projection(justfile_text)
    for nextest in (True, False):
        for recipe, args, expected, delegate in CASES:
            verify_case(
                justfile_text,
                recipe,
                args,
                expected,
                nextest=nextest,
                delegate_nested_just=delegate,
            )


class SchemaRecipeWitnessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.baseline = (ROOT / "justfile").read_text(encoding="utf-8")

    def assert_mutation_rejected(
        self,
        original: str,
        replacement: str,
        recipe: str,
        expected: ExpectedBuilder,
        *,
        args: tuple[str, ...] = (),
        nextest: bool = True,
        delegate: bool = True,
    ) -> None:
        mutation = self.baseline.replace(original, replacement, 1)
        self.assertNotEqual(mutation, self.baseline)
        with self.assertRaises(RecipeWitnessError):
            verify_case(
                mutation,
                recipe,
                args,
                expected,
                nextest=nextest,
                delegate_nested_just=delegate,
            )

    def assert_ast_mutation_rejected(
        self,
        original: str,
        replacement: str,
    ) -> str:
        mutation = self.baseline.replace(original, replacement, 1)
        self.assertNotEqual(mutation, self.baseline)
        with self.assertRaises(RecipeWitnessError):
            verify_recipe_ast_projection(mutation)
        return mutation

    def test_canonical_ast_and_execution_matrix_are_exact(self) -> None:
        audit_recipe_witness(self.baseline)

    def test_global_shell_setting_drift_is_rejected_by_ast(self) -> None:
        self.assert_ast_mutation_rejected(
            'set shell := ["bash", "-cu"]\n',
            'set shell := ["sh", "-cu"]\n',
        )

    def test_env_gated_extra_cargo_is_rejected_by_ast(self) -> None:
        mutation = self.assert_ast_mutation_rejected(
            "check-core:\n    scripts/cargo-build-lock.sh -- cargo check --tests \\\n",
            "check-core:\n"
            "    if printenv WITNESS_EXTRA_CARGO >/dev/null 2>&1; then "
            "cargo check --workspace; fi\n"
            "    scripts/cargo-build-lock.sh -- cargo check --tests \\\n",
        )
        verify_case(
            mutation,
            "check-core",
            (),
            _check_core,
            nextest=True,
            delegate_nested_just=True,
        )

    def test_dead_branch_extra_cargo_is_rejected_by_ast(self) -> None:
        mutation = self.assert_ast_mutation_rejected(
            "check-core:\n    scripts/cargo-build-lock.sh -- cargo check --tests \\\n",
            "check-core:\n"
            "    if false; then cargo check --workspace; fi\n"
            "    scripts/cargo-build-lock.sh -- cargo check --tests \\\n",
        )
        verify_case(
            mutation,
            "check-core",
            (),
            _check_core,
            nextest=True,
            delegate_nested_just=True,
        )

    def test_commented_nested_call_cannot_spoof_execution(self) -> None:
        self.assert_mutation_rejected(
            "check-full:\n    just check-core\n",
            "check-full:\n    # just check-core\n",
            "check-full",
            _check_full,
        )

    def test_echoed_nested_call_cannot_spoof_execution(self) -> None:
        self.assert_mutation_rejected(
            "check-full:\n    just check-core\n",
            "check-full:\n    echo just check-core\n",
            "check-full",
            _check_full,
        )

    def test_test_full_nextest_branch_cannot_run_cargo_test(self) -> None:
        self.assert_mutation_rejected(
            "if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --workspace",
            "if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo test --workspace",
            "test-full",
            _test_full,
            nextest=True,
        )

    def test_test_full_fallback_package_cannot_drift(self) -> None:
        self.assert_mutation_rejected(
            "else scripts/cargo-build-lock.sh -- cargo test --workspace --exclude kanban-desktop --exclude kanban-schema-tool",
            "else scripts/cargo-build-lock.sh -- cargo test --workspace --exclude kanban-desktop --exclude kanban-contract",
            "test-full",
            _test_full,
            nextest=False,
        )

    def test_default_check_alias_cannot_drift(self) -> None:
        self.assert_mutation_rejected(
            "check: check-core\n",
            "check:\n",
            "check",
            _check_core,
        )

    def test_fmt_cannot_be_replaced_by_echo(self) -> None:
        canonical = "cargo fmt " + " ".join(_package_args(CORE_PACKAGES))
        canonical += " -- --check"
        self.assert_mutation_rejected(
            f"fmt:\n    {canonical}\n",
            f"fmt:\n    echo {canonical}\n",
            "fmt",
            _fmt,
        )

    def test_fmt_cannot_fall_back_to_workspace_selection(self) -> None:
        canonical = "cargo fmt " + " ".join(_package_args(CORE_PACKAGES))
        canonical += " -- --check"
        self.assert_mutation_rejected(
            f"fmt:\n    {canonical}\n",
            "fmt:\n    cargo fmt -- --check\n",
            "fmt",
            _fmt,
        )

    def test_fmt_check_cannot_restore_workspace_selection(self) -> None:
        self.assert_mutation_rejected(
            "fmt-check: fmt\n",
            "fmt-check:\n    cargo fmt -- --check\n",
            "fmt-check",
            _fmt,
        )

    def test_fmt_full_must_cover_only_core_and_helpers(self) -> None:
        packages = (*CORE_PACKAGES, *HELPER_PACKAGES)
        canonical = "cargo fmt " + " ".join(_package_args(packages))
        canonical += " -- --check"
        self.assert_mutation_rejected(
            f"fmt-full:\n    {canonical}\n",
            "fmt-full:\n    cargo fmt -- --check\n",
            "fmt-full",
            _fmt_full,
        )

    def test_schema_fmt_must_cover_only_contract_and_leaf(self) -> None:
        canonical = "cargo fmt " + " ".join(
            _package_args((CONTRACT_PACKAGE, TOOL_PACKAGE))
        )
        canonical += " -- --check"
        self.assert_mutation_rejected(
            f"schema-fmt:\n    {canonical}\n",
            "schema-fmt:\n    cargo fmt -p kanban-contract -- --check\n",
            "schema-fmt",
            _schema_fmt,
        )

    def test_rust_full_must_use_fmt_full(self) -> None:
        self.assert_mutation_rejected(
            "rust-full:\n    just fmt-full\n",
            "rust-full:\n    just fmt\n",
            "rust-full",
            _rust_full,
        )

    def test_schema_tool_cannot_omit_check_or_clippy_gate(self) -> None:
        self.assert_mutation_rejected(
            "schema-tool:\n    just check-p kanban-schema-tool\n",
            "schema-tool:\n    echo check omitted\n",
            "schema-tool",
            _schema_tool,
        )
        self.assert_mutation_rejected(
            "    scripts/cargo-build-lock.sh -- cargo clippy -p kanban-schema-tool --all-targets -- -D warnings\n",
            "    echo clippy omitted\n",
            "schema-tool",
            _schema_tool,
        )

    def test_schema_contract_cannot_omit_or_reorder_dependency_preflight(self) -> None:
        self.assert_mutation_rejected(
            "schema-contract:\n    just schema-dependency-isolation\n",
            "schema-contract:\n    echo dependency gate omitted\n",
            "schema-contract",
            _schema_contract,
            delegate=False,
        )
        self.assert_mutation_rejected(
            "    just schema-dependency-isolation\n    just schema-fmt\n",
            "    just schema-fmt\n    just schema-dependency-isolation\n",
            "schema-contract",
            _schema_contract,
            delegate=False,
        )

    def test_schema_contract_cannot_omit_schema_fmt(self) -> None:
        self.assert_mutation_rejected(
            "    just schema-fmt\n",
            "    echo schema fmt omitted\n",
            "schema-contract",
            _schema_contract,
            delegate=False,
        )

    def test_schema_audit_closed_protects_witness_and_locked_audit(self) -> None:
        self.assert_mutation_rejected(
            "schema-audit-closed:\n    just schema-adoption-witness\n",
            "schema-audit-closed:\n    echo adoption witness omitted\n",
            "schema-audit-closed",
            _schema_audit_closed,
            delegate=False,
        )
        self.assert_mutation_rejected(
            "    scripts/cargo-build-lock.sh -- cargo run -p kanban-schema-tool --bin kanban-schema -- audit --root . --require-closed\n",
            "    cargo run -p kanban-schema-tool --bin kanban-schema -- audit --root . --require-closed\n",
            "schema-audit-closed",
            _schema_audit_closed,
            delegate=False,
        )

    def test_release_cannot_omit_or_reorder_schema_contract(self) -> None:
        self.assert_mutation_rejected(
            "    just schema-contract\n",
            "    echo schema contract omitted\n",
            "release",
            _release,
            delegate=False,
        )
        self.assert_mutation_rejected(
            "    just affected-self-test\n    just schema-contract\n",
            "    just schema-contract\n    just affected-self-test\n",
            "release",
            _release,
            delegate=False,
        )

    def test_release_nested_gate_order_is_exact(self) -> None:
        self.assert_mutation_rejected(
            "    just bench-check\n    just target-tools\n",
            "    just target-tools\n    just bench-check\n",
            "release",
            _release,
            delegate=False,
        )

    def test_schema_dependency_internal_policy_cannot_be_removed(self) -> None:
        self.assert_mutation_rejected(
            "    python3 -B scripts/schema_dependency_policy.py\n",
            "    echo metadata policy omitted\n",
            "schema-dependency-isolation",
            _schema_dependency,
            delegate=False,
        )
        self.assert_mutation_rejected(
            "    python3 -B scripts/test_schema_recipe_witness.py\n",
            "    echo recipe witness omitted\n",
            "schema-dependency-isolation-self-test",
            _schema_dependency_self_test,
            delegate=False,
        )

    def test_schema_surface_internal_call_cannot_be_removed(self) -> None:
        self.assert_mutation_rejected(
            "    just test-p kanban-cli clap_leaf_commands_match_exact_contract_catalog\n",
            "    echo cli surface omitted\n",
            "schema-surface-audit",
            _schema_surface,
            delegate=False,
        )

    def test_schema_adoption_internal_policy_cannot_be_removed(self) -> None:
        self.assert_mutation_rejected(
            "    python3 -B scripts/schema_adoption_witnesses.py --root .\n",
            "    echo adoption witness omitted\n",
            "schema-adoption-witness",
            _schema_adoption,
            delegate=False,
        )
        self.assert_mutation_rejected(
            "    python3 -B scripts/test_schema_adoption_witnesses.py\n",
            "    echo adoption self-test omitted\n",
            "schema-adoption-witness-self-test",
            _schema_adoption_self_test,
            delegate=False,
        )

    def test_extra_workspace_cargo_command_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            "rust-fast:\n    just fmt\n",
            "rust-fast:\n    just fmt\n    cargo check --workspace\n",
            "rust-fast",
            _rust_fast,
        )

    def test_test_core_branch_package_mismatch_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            "-p kanban-core -p kanban-contract -p kanban-entity -p kanban-indexer -p kanban-search \\\n",
            "-p kanban-core -p kanban-contract -p kanban-entity -p kanban-indexer -p kanban-cli \\\n",
            "test-core",
            _test_core,
            nextest=False,
        )

    def test_header_prerequisite_cannot_extend_gate(self) -> None:
        self.assert_ast_mutation_rejected(
            "check-full:\n",
            "check-full: schema-tool\n",
        )
        self.assert_mutation_rejected(
            "check-full:\n",
            "check-full: schema-tool\n",
            "check-full",
            _check_full,
        )

    def test_absolute_just_bypass_is_rejected(self) -> None:
        assert REAL_JUST is not None
        self.assert_mutation_rejected(
            "check-full:\n    just check-core\n",
            f"check-full:\n    {REAL_JUST} check-core\n",
            "check-full",
            _check_full,
        )

    def test_build_lock_bypass_is_rejected(self) -> None:
        self.assert_mutation_rejected(
            "scripts/cargo-build-lock.sh -- cargo check --tests \\\n",
            "cargo check --tests \\\n",
            "check-core",
            _check_core,
        )


    def test_internal_gate_deletion_is_rejected_by_ast(self) -> None:
        self.assert_ast_mutation_rejected(
            "    python3 -B scripts/schema_dependency_policy.py\n",
            "    # metadata policy deleted\n",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
