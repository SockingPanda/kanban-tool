#!/usr/bin/env python3
"""用真实 just 与 fake executables 锁定产品/schema recipe 的执行调用图。"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable


ROOT = Path(__file__).resolve().parents[1]
REAL_JUST = shutil.which("just")
EXPECTED_JUST_VERSION = "just 1.57.0"
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
PROJECTION_RELEASE_FEATURES = "tantivy-backend,oxigraph-backend"
RELEASE_WRAPPER_SHA256 = (
    "20b6b07ebabd88dbef592a1a2ec1ce88eb579f6cd004582b6a52cd07a615f6d3"
)
Event = dict[str, object]
ExpectedBuilder = Callable[[Path, bool], list[Event]]


class RecipeWitnessError(RuntimeError):
    """recipe 实际执行序列偏离 canonical 调用图。"""


def _bash_function_body(wrapper: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^{re.escape(name)}\(\) \{{\n(.*?)^\}}\n",
        wrapper,
    )
    if match is None:
        raise RecipeWitnessError(f"release wrapper 缺少 canonical function: {name}")
    return match.group(1)


def _release_safe_publish_graph(wrapper: str) -> tuple[tuple[str, ...], ...]:
    collapsed = re.sub(
        r"\\\n[ \t]*",
        " ",
        _bash_function_body(wrapper, "publish_generation"),
    )
    commands: list[tuple[str, ...]] = []
    for line in collapsed.splitlines():
        marker = 'python3 "$SAFE_PATH" '
        position = line.find(marker)
        if position < 0:
            continue
        command_text = line[position:].strip()
        if command_text.endswith(')"'):
            command_text = command_text[:-2].rstrip()
        argv = shlex.split(command_text)
        if len(argv) < 3 or argv[:2] != ["python3", "$SAFE_PATH"]:
            raise RecipeWitnessError(
                f"release safe publish command 无法结构化解析: {command_text!r}"
            )
        commands.append(tuple(argv[2:]))
    return tuple(commands)


def _verify_sealed_release_tooling(wrapper: str) -> None:
    """锁定 fresh/resume 共用的 sealed release tooling，禁止回退 live wrapper。"""

    bind_body = _bash_function_body(wrapper, "bind_sealed_release_tools")
    bind_fragments = (
        'if [[ -d "$SOURCE_SNAPSHOT_ROOT" && ! -L "$SOURCE_SNAPSHOT_ROOT" ]]; then',
        'tools_root="$SOURCE_SNAPSHOT_ROOT"',
        'elif [[ "$RESUME_RELEASE" == "1" && -d "$STAGE_DIR/.release-tools"',
        'tools_root="$STAGE_DIR/.release-tools"',
        'elif [[ "$RESUME_PUBLISHED" == "1" && -d "$PUBLISHED_DIR/.release-tools"',
        'tools_root="$PUBLISHED_DIR/.release-tools"',
        'fail "immutable release tooling snapshot is unavailable"',
        'SEALED_ARTIFACT_MANIFEST="$tools_root/scripts/release-artifact-manifest.sh"',
        'SEALED_EMBED_DEB="$tools_root/scripts/embed-release-provenance-deb.sh"',
        'SEALED_SOURCE_GATE="$tools_root/scripts/release-source-gate.sh"',
        'SEALED_SAFE_PATH="$tools_root/scripts/release-safe-path.py"',
        'python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$tool"',
        '[[ -x "$tool" ]] || fail "sealed release tool is not executable: $tool"',
        'ARTIFACT_MANIFEST="$SEALED_ARTIFACT_MANIFEST"',
        'EMBED_DEB="$SEALED_EMBED_DEB"',
    )
    cursor = 0
    for fragment in bind_fragments:
        position = bind_body.find(fragment, cursor)
        if position < 0:
            raise RecipeWitnessError(
                "release wrapper sealed tooling 缺少或重排 fail-closed boundary: "
                f"{fragment!r}"
            )
        cursor = position + len(fragment)

    persist_body = _bash_function_body(wrapper, "persist_sealed_release_tools")
    persist_fragments = (
        '[[ "$SEALED_TOOLS_DIR" == "$STAGE_DIR/.release-tools" ]]',
        '--destination "$SEALED_TOOLS_DIR/scripts/release-artifact-manifest.sh" --mode 0555',
        '--destination "$SEALED_TOOLS_DIR/scripts/embed-release-provenance-deb.sh" --mode 0555',
        "for dependency in cargo-build-lock.sh release-safe-path.py release-source-gate.sh; do",
        '--source "$SOURCE_SNAPSHOT_ROOT/scripts/$dependency"',
        '--destination "$SEALED_TOOLS_DIR/scripts/$dependency" --mode 0555',
    )
    cursor = 0
    for fragment in persist_fragments:
        position = persist_body.find(fragment, cursor)
        if position < 0:
            raise RecipeWitnessError(
                "release wrapper sealed tooling persistence 不完整或重排: "
                f"{fragment!r}"
            )
        cursor = position + len(fragment)

    bind_dispatch = (
        'if [[ "$RESUME_PUBLISHED" != "1" && "$RESUME_RELEASE" != "1" ]]; then\n'
        "  create_source_snapshot\n"
        "  bind_sealed_release_tools\n"
        "  persist_sealed_release_tools\n"
        'elif [[ "$RESUME_RELEASE" == "1" ]]; then\n'
        "  bind_sealed_release_tools\n"
        'elif [[ "$RESUME_PUBLISHED" == "1" ]]; then\n'
        "  bind_sealed_release_tools\n"
        "fi"
    )
    if wrapper.count(bind_dispatch) != 1:
        raise RecipeWitnessError(
            "release wrapper fresh/resume 必须先绑定一次 canonical sealed tooling"
        )
    bound_tail = wrapper[wrapper.index(bind_dispatch) + len(bind_dispatch) :]
    source_gate_binding = bound_tail.find('SOURCE_GATE="$SEALED_SOURCE_GATE"')
    safe_path_binding = bound_tail.find(
        'export KANBAN_RELEASE_SAFE_PATH="$SEALED_SAFE_PATH"'
    )
    first_sealed_verify = bound_tail.find(
        '"$SOURCE_GATE" verify --manifest "$STAGE_DIR/source-provenance.json"'
    )
    if not (
        0 <= source_gate_binding < safe_path_binding < first_sealed_verify
    ):
        raise RecipeWitnessError(
            "release wrapper 未在 post-snapshot verify 前绑定 sealed source/safe-path tooling"
        )

    resume_body = _bash_function_body(wrapper, "resume_published_generation")
    resume_fragments = (
        'python3 "$SAFE_PATH" validate-published-dir --root "$TARGET_ROOT"',
        '--path "$PUBLISHED_DIR" --marker "$PUBLISHED_MARKER"',
        '--verify-command "$ARTIFACT_MANIFEST" verify-final',
        '--manifest "$PUBLISHED_DIR/release-artifacts.json"',
        '--stage-dir "$PUBLISHED_DIR"',
        '[[ "$generation_sha256" =~ ^[0-9a-f]{64}$ ]]',
    )
    cursor = 0
    for fragment in resume_fragments:
        position = resume_body.find(fragment, cursor)
        if position < 0:
            raise RecipeWitnessError(
                "release wrapper published resume 缺少 sealed verification boundary: "
                f"{fragment!r}"
            )
        cursor = position + len(fragment)
    published_resume = (
        'if [[ "$RESUME_PUBLISHED" == "1" ]]; then\n'
        "  resume_published_generation\n"
        "  finish_release\n"
        "  exit 0\n"
        "fi"
    )
    if wrapper.count(published_resume) != 1:
        raise RecipeWitnessError(
            "release wrapper published resume 必须执行 sealed artifact verification"
        )


def _verify_release_wrapper_semantics(wrapper: str) -> None:
    """锁定 release wrapper 的 fail-closed semantic→digest→publish 顺序。"""

    actual_hash = hashlib.sha256(wrapper.encode()).hexdigest()
    if actual_hash != RELEASE_WRAPPER_SHA256:
        raise RecipeWitnessError(
            "release wrapper whole-file hash drifted: "
            f"expected={RELEASE_WRAPPER_SHA256} actual={actual_hash}"
        )
    for forbidden in (
        'mv -f "$ARTIFACT_MANIFEST_PENDING"',
        'cp -f "$ARTIFACT_MANIFEST_PENDING"',
        'rm -f "$ARTIFACT_MANIFEST_FINAL"',
    ):
        if forbidden in wrapper:
            raise RecipeWitnessError(
                f"release wrapper 使用了非 safe-path manifest publish: {forbidden}"
            )
    expected_graph = (
        (
            "tree-digest",
            "--root",
            "$TARGET_ROOT",
            "--path",
            "$STAGE_DIR",
            "--require-sealed",
        ),
        (
            "publish-dir",
            "--root",
            "$TARGET_ROOT",
            "--source",
            "$STAGE_DIR",
            "--destination",
            "$PUBLISHED_DIR",
            "--expected-tree-sha256",
            "$GENERATION_SHA256",
            "--verify-command",
            "$ARTIFACT_MANIFEST",
            "verify-final",
            "--manifest",
            "$ARTIFACT_MANIFEST_FINAL",
            "--stage-dir",
            "$STAGE_DIR",
        ),
        (
            "validate-published-dir",
            "--root",
            "$TARGET_ROOT",
            "--path",
            "$PUBLISHED_DIR",
            "--marker",
            "$PUBLISHED_MARKER",
            "--expected-tree-sha256",
            "$GENERATION_SHA256",
        ),
    )
    actual_graph = _release_safe_publish_graph(wrapper)
    if actual_graph != expected_graph:
        raise RecipeWitnessError(
            "release safe publish structured command graph drifted: "
            f"expected={expected_graph!r} actual={actual_graph!r}"
        )
    normal_tail = wrapper[wrapper.index('"$ARTIFACT_MANIFEST" prepare') :]
    ordered_fragments = (
        '"$ARTIFACT_MANIFEST" prepare',
        '"$SOURCE_GATE" verify --manifest "$SOURCE_MANIFEST"',
        '--manifest "$ARTIFACT_MANIFEST_PENDING"',
        'python3 "$SAFE_PATH" publish-file',
        'python3 "$SAFE_PATH" seal-tree',
        "publish_generation\nfinish_release",
    )
    cursor = 0
    for fragment in ordered_fragments:
        position = normal_tail.find(fragment, cursor)
        if position < 0:
            raise RecipeWitnessError(
                "release wrapper normal path 缺少或重排 canonical boundary: "
                f"{fragment!r}"
            )
        cursor = position + len(fragment)
    resume_block = (
        'if [[ "$RESUME_RELEASE" == "1" ]]; then\n'
        "  publish_generation\n"
        "  finish_release\n"
        "  exit 0\n"
        "fi"
    )
    if wrapper.count(resume_block) != 1:
        raise RecipeWitnessError(
            "release wrapper resume path 必须无条件执行一次 canonical publish"
        )
    _verify_sealed_release_tooling(wrapper)


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
    check = _locked(root, "check", "--locked", "-p", TOOL_PACKAGE, "--tests")
    probe = _cargo(root, "nextest", "--version")
    if nextest:
        tests = [
            *probe,
            *_locked(
                root, "nextest", "run", "--locked", "-p", TOOL_PACKAGE, "--no-fail-fast"
            ),
        ]
    else:
        tests = [*probe, *_locked(root, "test", "--locked", "-p", TOOL_PACKAGE)]
    clippy = _locked(
        root,
        "clippy",
        "--locked",
        "-p",
        TOOL_PACKAGE,
        "--all-targets",
        "--",
        "-D",
        "warnings",
    )
    return [
        *check,
        *tests,
        *clippy,
    ]


def _feature_package(
    root: Path,
    nextest: bool,
    package: str,
    features: str,
) -> list[Event]:
    probe = _cargo(root, "nextest", "--version")
    if nextest:
        tests = _locked(
            root,
            "nextest",
            "run",
            "--locked",
            "-p",
            package,
            "--features",
            features,
            "--no-fail-fast",
            "--no-tests",
            "pass",
        )
    else:
        tests = _locked(
            root, "test", "--locked", "-p", package, "--features", features
        )
    clippy = _locked(
        root,
        "clippy",
        "--locked",
        "-p",
        package,
        "--all-targets",
        "--features",
        features,
        "--",
        "-D",
        "warnings",
    )
    return [*probe, *tests, *clippy]


def _feature_contract(root: Path, nextest: bool) -> list[Event]:
    return _feature_package(root, nextest, CONTRACT_PACKAGE, "schema")


def _projection_release_cohort(root: Path, nextest: bool) -> list[Event]:
    events: list[Event] = []
    for package in ("kanban-cli", "kanban-server"):
        events.extend(
            _nested(
                root,
                "feature-p",
                _feature_package(
                    root,
                    nextest,
                    package,
                    PROJECTION_RELEASE_FEATURES,
                ),
                package,
                PROJECTION_RELEASE_FEATURES,
            )
        )
    return events


def _schema_check(root: Path, _: bool) -> list[Event]:
    return _locked(
        root,
        "run",
        "--locked",
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
        "--locked",
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


def _schema_surface(root: Path, nextest: bool) -> list[Event]:
    events: list[Event] = []
    cases = (
        ("kanban-server", "api_route_catalog_matches_exact_contract_catalog"),
        ("kanban-cli", "clap_leaf_commands_match_exact_contract_catalog"),
        ("kanban-sqlite", "jsonl_export_discriminators_match_exact_contract_catalog"),
    )
    for package, test_filter in cases:
        events.extend(_cargo(root, "nextest", "--version"))
        if nextest:
            events.extend(
                _locked(
                    root,
                    "nextest",
                    "run",
                    "--locked",
                    "-p",
                    package,
                    test_filter,
                    "--no-fail-fast",
                )
            )
        else:
            events.extend(
                _locked(root, "test", "--locked", "-p", package, test_filter)
            )
    return events


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
            "--locked",
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
    return [
        {
            "kind": "script",
            "argv": [],
            "invoked_as": str(root / "scripts/release-cohort.sh"),
            "cwd": str(root),
        }
    ]


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
    (
        "projection-release-cohort",
        (),
        _projection_release_cohort,
        True,
    ),
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


# just --dump-format json --dump 先投影为稳定的执行语义：globals 保留
# assignments/first/groups/unexports，settings 只保留相对同版空 justfile 的
# override；recipe 保留 dependencies/body/parameters/
# attributes/priors/private/quiet/shebang，并规范化 parameter/dependency 的纯默认
# 字段。parser 的 source/namepath 等位置字段和新增默认噪音不进入 witness。
# 投影后的 canonical JSON（sorted keys、compact separators、末尾换行）做 SHA-256；
# 更新执行语义必须显式更新对应 hash，运行采样无法触达的 env/dead branch 仍然
# fail closed。
PROTECTED_RECIPE_AST_SHA256 = {
    "fmt": "775adb8877d364be2c9c5c5d8665ff9efdf27189584aea2241dcda504d372206",
    "fmt-check": "b2fdd96430312d9ee37d369ce26ff44dab2f6a10dfcbb29bb6380fa0c371d611",
    "fmt-full": "b30159e7d1235629c9a877ca31a1df661eaa4bc3670b0ae047350de6c06ae509",
    "check": "cddd49fe5e50b59502f0a2a54cfd4e2acb4bb9b891f4de0c056e78f52e0783a7",
    "check-core": "3412493908e5e51fc636899322a3da85cb88f95a11846e0043b2ac062ce0cbdf",
    "check-helpers": "72b99d6827202bff3079e478349fc476a6476885a84e68c1db5e2d54456440eb",
    "check-full": "a60a7ed88645351cc32236a5bca2c4edafa1d0631e63d2179d4bda6b17184208",
    "test": "d136c26efda43f4482000099f4917c98ba3407f2072d58ee7a6ffefebba798db",
    "test-p": "16d14ffcf302b2f745a4b4a0724a96e5a38b600870d58f36c344e185595a6b00",
    "check-p": "afb9f318d2aa509feed7efd372318cbd4c2993742a26f078c6f740349400e33e",
    "rust-fast": "9a013d3f1005dc4c21c44e1fc21e8d05dc36c9de73e79470021e4f62ac2801c9",
    "test-core": "d4dc0f1b0c16f285476fba46b1fb9761e6f30c9bd8ca081c10897febb82e5aa9",
    "test-helpers": "100355d58273d5bb201b81a27ac00b4f88bdec31c5e48e4f0aa8fe9cfe82e5f1",
    "test-full": "253dcab5f836d56a22b0616e9ee56905aed9468afd7c6f773509f16862b6e366",
    "clippy": "e487d817f0f09efb21d91eaa4e9793f05cb2883723cc59758c2eaa4314384010",
    "clippy-core": "bd35e34699dbb23c0d8ca688f26859f6fd1c993c1191bc69e49c53778ec3f4cd",
    "clippy-helpers": "09b9641653aadc6a4074e939800832daa1dba050001ce052e6f55f2129e4924d",
    "clippy-full": "64cf5ae07fe363d605275ab8997272d6d7c48e120e9cfcf1c24ecbb105d77da8",
    "rust-full": "05bd1a7769cf8d0ebde0582f37444132adfafd19ae8ff0682b1a8c45307b5288",
    "projection-release-cohort": "fa986fe568697b3f4fa7e62e65280b461f2d0b7f34d18a351d0cae6c330641f4",
    "feature-p": "90a48cdda4600c6cffd88614a139b22584936690af300aaec2a2e60bdc3fec09",
    "schema-generate": "49f543ca2ee8cd0dcc5d8a36427176e0f49bbdb42036c8a1460778fb80aa9f83",
    "schema-check": "353207a2d392277a1cd7f768c6b61f1485d866bc3280d35bd4142ff3cafbffb6",
    "spec-bundle-generate": "4ad6e58b6600690d8a85c9d5472efe999b8c356f2eeafc86978edd4ccbe24fa9",
    "spec-bundle-check": "deb622cf88b3dff62e5abbeaa87996cd5cc05cf83a9175c51376110f3f337bcb",
    "schema-docs": "8f54ff9bd7e05e820243e82418b68c46316c11380fcadcbf816ab9a1cdfe40ac",
    "schema-fmt": "ff27c09a697acc1fc1b7ea52c02d41035840492cb6b2ceb97162a828d1d62c28",
    "schema-tool": "eb1a84d4fd75ab1264dcff25d69c18790c2efa9a5683161fe4807a7563707c59",
    "schema-dependency-isolation-self-test": "1f791d177b30a450e938734f8b48480f3c1d3fe5e77d1e4588771c405383f934",
    "schema-dependency-isolation": "70f9346e1e8d1be4d169bf6568fc743c93c445739e98fc5599c244ad8ff789bb",
    "schema-adoption-witness-self-test": "221fd748cf1b93cb7a11f90b70b398bd08b9122df154f61012a4b92661e81fc4",
    "schema-adoption-witness": "1c55f076cb1632e69be3f6c1b98da5ddfb292a91f238324a5f51fc584e860e1d",
    "schema-surface-audit": "bf65e600ecefe5b05dddcf9ae165be6e5b00b54148f1d7abd0a7996041e786b0",
    "schema-contract": "5d0d2d21d1bc9a4543a9f938f8211a3387ec34812948c91cff6d7dee6e5521c7",
    "schema-audit-closed": "7f2f8074b1bf51095c5119ea342ad412fb50f667b59f970f68ef9239bda54f45",
    "release": "1dae2a83fb1776567c134c61e610556f3fc39cc21d9642beeaa3ee7b226d8a4d",
}
AST_ONLY_RECIPE_NAMES = {"check-p", "test-p"}
PROTECTED_JUST_GLOBALS_SHA256 = (
    "76483961d7be530d64e46198b2e15a495d851df2051ccdccf4afa1a5f2dafe2b"
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


PARSER_LOCATION_FIELDS = {
    "column",
    "line",
    "module_path",
    "namepath",
    "offset",
    "source",
    "source_column",
    "source_line",
    "source_offset",
    "source_path",
    "span",
}


def _reject_unknown_nondefault_fields(
    value: dict[str, object],
    known_fields: set[str],
    *,
    context: str,
) -> None:
    unsafe = []
    for field in set(value) - known_fields - PARSER_LOCATION_FIELDS:
        field_value = value[field]
        if (
            field_value is None
            or field_value is False
            or field_value == []
            or field_value == {}
        ):
            continue
        unsafe.append(field)
    if unsafe:
        raise RecipeWitnessError(
            f"{context} 出现未投影的非默认 parser 字段: "
            f"fields={sorted(unsafe)}"
        )


def _project_setting_overrides(
    settings: dict[str, object],
    parser_defaults: dict[str, object],
) -> dict[str, object]:
    unknown = set(settings) - set(parser_defaults)
    if unknown:
        raise RecipeWitnessError(
            "just settings 缺少 parser default 基线: "
            f"unknown={sorted(unknown)}"
        )
    return {
        name: value
        for name, value in settings.items()
        if value != parser_defaults[name]
    }


def _project_parameter_semantics(parameter: object) -> dict[str, object]:
    if not isinstance(parameter, dict):
        raise RecipeWitnessError("recipe parameter AST 必须是 object")
    name = parameter.get("name")
    if not isinstance(name, str) or not name:
        raise RecipeWitnessError("recipe parameter AST 缺少非空 name")

    defaults: dict[str, object] = {
        "default": None,
        "export": False,
        "flag": False,
        "help": None,
        "kind": "singular",
        "long": None,
        "max": None,
        "min": None,
        "multiple": False,
        "pattern": None,
        "short": None,
        "value": None,
    }
    _reject_unknown_nondefault_fields(
        parameter,
        {"name", *defaults},
        context="recipe parameter AST",
    )
    projection: dict[str, object] = {"name": name}
    for field, default in defaults.items():
        value = parameter.get(field, default)
        if value != default:
            projection[field] = value
    return projection


def _project_dependency_semantics(dependency: object) -> dict[str, object]:
    if not isinstance(dependency, dict):
        raise RecipeWitnessError("recipe dependency AST 必须是 object")
    recipe = dependency.get("recipe")
    if not isinstance(recipe, str) or not recipe:
        raise RecipeWitnessError("recipe dependency AST 缺少非空 recipe")
    _reject_unknown_nondefault_fields(
        dependency,
        {"arguments", "recipe", "star"},
        context="recipe dependency AST",
    )

    projection: dict[str, object] = {"recipe": recipe}
    arguments = dependency.get("arguments", [])
    if arguments != []:
        projection["arguments"] = arguments
    star = dependency.get("star")
    if star is not None:
        projection["star"] = star
    return projection


def _project_recipe_semantics(recipe: object) -> dict[str, object]:
    if not isinstance(recipe, dict):
        raise RecipeWitnessError("recipe AST 必须是 object")
    required = (
        "attributes",
        "body",
        "dependencies",
        "parameters",
        "priors",
        "private",
        "quiet",
        "shebang",
    )
    missing = [field for field in required if field not in recipe]
    if missing:
        raise RecipeWitnessError(
            f"recipe AST 缺少执行语义字段: missing={missing}"
        )
    _reject_unknown_nondefault_fields(
        recipe,
        {*required, "doc", "name"},
        context="recipe AST",
    )
    dependencies = recipe["dependencies"]
    parameters = recipe["parameters"]
    if not isinstance(dependencies, list):
        raise RecipeWitnessError("recipe dependencies AST 必须是 array")
    if not isinstance(parameters, list):
        raise RecipeWitnessError("recipe parameters AST 必须是 array")
    return {
        "attributes": recipe["attributes"],
        "body": recipe["body"],
        "dependencies": [
            _project_dependency_semantics(dependency)
            for dependency in dependencies
        ],
        "parameters": [
            _project_parameter_semantics(parameter) for parameter in parameters
        ],
        "priors": recipe["priors"],
        "private": recipe["private"],
        "quiet": recipe["quiet"],
        "shebang": recipe["shebang"],
    }


def _project_aliases_semantics(aliases: object) -> dict[str, object]:
    if not isinstance(aliases, dict):
        raise RecipeWitnessError("just aliases AST 必须是 object")
    projection: dict[str, object] = {}
    for name, alias in aliases.items():
        if not isinstance(alias, dict):
            raise RecipeWitnessError(f"just alias AST 必须是 object: {name}")
        _reject_unknown_nondefault_fields(
            alias,
            {"attributes", "name", "target"},
            context=f"just alias AST ({name})",
        )
        target = alias.get("target")
        if not isinstance(target, str) or not target:
            raise RecipeWitnessError(f"just alias AST 缺少 target: {name}")
        projection[name] = {
            "attributes": alias.get("attributes", []),
            "target": target,
        }
    return projection


def _project_assignments_semantics(assignments: object) -> dict[str, object]:
    if not isinstance(assignments, dict):
        raise RecipeWitnessError("just assignments AST 必须是 object")
    projection: dict[str, object] = {}
    for name, assignment in assignments.items():
        if not isinstance(assignment, dict) or "value" not in assignment:
            raise RecipeWitnessError(
                f"just assignment AST 缺少 value: {name}"
            )
        _reject_unknown_nondefault_fields(
            assignment,
            {"eager", "export", "name", "private", "value"},
            context=f"just assignment AST ({name})",
        )
        projection[name] = {
            "eager": assignment.get("eager", False),
            "export": assignment.get("export", False),
            "private": assignment.get("private", False),
            "value": assignment["value"],
        }
    return projection


def _project_modules_semantics(
    modules: object,
    parser_setting_defaults: dict[str, object],
) -> dict[str, object]:
    if not isinstance(modules, dict):
        raise RecipeWitnessError("just modules AST 必须是 object")
    projection: dict[str, object] = {}
    for name, module in modules.items():
        if not isinstance(module, dict):
            raise RecipeWitnessError(f"just module AST 必须是 object: {name}")
        _reject_unknown_nondefault_fields(
            module,
            {
                "aliases",
                "assignments",
                "doc",
                "first",
                "groups",
                "module_path",
                "modules",
                "recipes",
                "settings",
                "source",
                "unexports",
                "warnings",
            },
            context=f"just module AST ({name})",
        )
        settings = module.get("settings")
        recipes = module.get("recipes")
        if not isinstance(settings, dict) or not isinstance(recipes, dict):
            raise RecipeWitnessError(
                f"just module AST 缺少 settings/recipes: {name}"
            )
        projection[name] = {
            "aliases": _project_aliases_semantics(
                module.get("aliases", {})
            ),
            "assignments": _project_assignments_semantics(
                module.get("assignments", {})
            ),
            "first": module.get("first"),
            "groups": module.get("groups", []),
            "modules": _project_modules_semantics(
                module.get("modules", {}),
                parser_setting_defaults,
            ),
            "recipes": {
                recipe_name: _project_recipe_semantics(recipe)
                for recipe_name, recipe in recipes.items()
            },
            "settings": _project_setting_overrides(
                settings,
                parser_setting_defaults,
            ),
            "unexports": module.get("unexports", []),
        }
    return projection


def _project_global_semantics(
    dump: dict[str, object],
    parser_setting_defaults: dict[str, object],
) -> dict[str, object]:
    _reject_unknown_nondefault_fields(
        dump,
        {
            "aliases",
            "assignments",
            "doc",
            "first",
            "groups",
            "module_path",
            "modules",
            "recipes",
            "settings",
            "source",
            "unexports",
            "warnings",
        },
        context="just 顶层 AST",
    )
    settings = dump.get("settings")
    first = dump.get("first")
    groups = dump.get("groups")
    unexports = dump.get("unexports")
    if not isinstance(settings, dict):
        raise RecipeWitnessError("just parser AST dump 缺少 settings object")
    if first is not None and not isinstance(first, str):
        raise RecipeWitnessError("just parser AST first 必须为 string 或 null")
    if not isinstance(groups, list) or not isinstance(unexports, list):
        raise RecipeWitnessError(
            "just parser AST groups/unexports 必须为 array"
        )
    return {
        "aliases": _project_aliases_semantics(dump.get("aliases")),
        "assignments": _project_assignments_semantics(
            dump.get("assignments")
        ),
        "first": first,
        "groups": groups,
        "modules": _project_modules_semantics(
            dump.get("modules"),
            parser_setting_defaults,
        ),
        "settings": _project_setting_overrides(
            settings,
            parser_setting_defaults,
        ),
        "unexports": unexports,
    }


def _parse_just_dump(completed: subprocess.CompletedProcess[str]) -> dict[str, object]:
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
    return dump


def _dump_just_ast(root: Path, name: str, text: str) -> dict[str, object]:
    assert REAL_JUST is not None
    justfile = root / name
    justfile.write_text(text, encoding="utf-8")
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
    return _parse_just_dump(completed)


def verify_recipe_ast_projection(justfile_text: str) -> None:
    if REAL_JUST is None:
        raise RecipeWitnessError("PATH 中缺少真实 just executable")
    version = subprocess.run(
        [REAL_JUST, "--version"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if version.returncode != 0 or version.stdout.strip() != EXPECTED_JUST_VERSION:
        raise RecipeWitnessError(
            "just parser version 不匹配: "
            f"expected={EXPECTED_JUST_VERSION!r}, "
            f"actual={version.stdout.strip()!r}, stderr={version.stderr!r}"
        )

    expected_names = {name for name, _, _, _ in CASES} | AST_ONLY_RECIPE_NAMES
    configured_names = set(PROTECTED_RECIPE_AST_SHA256)
    if configured_names != expected_names:
        raise RecipeWitnessError(
            "recipe AST hash inventory 与执行矩阵不一致: "
            f"expected={sorted(expected_names)}, configured={sorted(configured_names)}"
        )

    with tempfile.TemporaryDirectory(prefix="schema-recipe-ast-") as temp_dir:
        root = Path(temp_dir)
        dump = _dump_just_ast(root, "justfile", justfile_text)
        parser_defaults = _dump_just_ast(root, "empty.just", "")
    default_settings = parser_defaults.get("settings")
    if not isinstance(default_settings, dict):
        raise RecipeWitnessError(
            "just parser default AST dump 缺少 settings object"
        )
    global_projection = _project_global_semantics(dump, default_settings)
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
        recipe = recipes.get(name)
        if not isinstance(recipe, dict):
            raise RecipeWitnessError(f"just parser AST 缺少受保护 recipe: {name}")
        projection = _project_recipe_semantics(recipe)
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
        _write_executable(root / "scripts/release-cohort.sh", FAKE_DIRECT)
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

    def test_top_level_assignment_drift_is_rejected_by_ast(self) -> None:
        self.assert_ast_mutation_rejected(
            'audit-ignore-flags := "--ignore RUSTSEC-2024-0370',
            'audit-ignore-flags := "--ignore RUSTSEC-2099-9999',
        )

    def test_semantic_recipe_projection_ignores_parser_defaults_and_locations(
        self,
    ) -> None:
        recipe = {
            "attributes": [],
            "body": [["cargo check"]],
            "dependencies": [
                {"arguments": [], "recipe": "preflight", "star": None}
            ],
            "doc": None,
            "name": "gate",
            "namepath": "gate",
            "parameters": [
                {
                    "default": None,
                    "export": False,
                    "kind": "singular",
                    "name": "package",
                }
            ],
            "priors": 0,
            "private": False,
            "quiet": False,
            "shebang": False,
        }
        parser_augmented = json.loads(json.dumps(recipe))
        parser_augmented.update(
            {
                "column": 7,
                "future_default": None,
                "line": 11,
                "namepath": "root::gate",
                "source": "/parser-specific/justfile",
            }
        )
        parser_augmented["dependencies"][0].update(
            {
                "column": 3,
                "source": "/parser-specific/justfile",
                "source_path": "/parser-specific/justfile",
            }
        )
        parser_augmented["parameters"][0].update(
            {
                "flag": False,
                "future_default": None,
                "help": None,
                "long": None,
                "max": None,
                "min": None,
                "multiple": False,
                "pattern": None,
                "short": None,
                "value": None,
            }
        )

        self.assertEqual(
            _project_recipe_semantics(recipe),
            _project_recipe_semantics(parser_augmented),
        )

    def test_semantic_recipe_projection_retains_execution_fields(self) -> None:
        recipe = {
            "attributes": [],
            "body": [["cargo check"]],
            "dependencies": [
                {"arguments": [], "recipe": "preflight", "star": None}
            ],
            "parameters": [
                {
                    "default": None,
                    "export": False,
                    "kind": "singular",
                    "name": "package",
                }
            ],
            "priors": 0,
            "private": False,
            "quiet": False,
            "shebang": False,
        }
        baseline = _project_recipe_semantics(recipe)
        mutations = (
            ("attributes", [["no-exit-message"]]),
            ("body", [["cargo test"]]),
            (
                "dependencies",
                [{"arguments": [], "recipe": "other", "star": None}],
            ),
            (
                "parameters",
                [
                    {
                        "default": None,
                        "export": False,
                        "kind": "star",
                        "name": "package",
                    }
                ],
            ),
            ("priors", 1),
            ("private", True),
            ("quiet", True),
            ("shebang", True),
        )
        for field, value in mutations:
            with self.subTest(field=field):
                mutation = dict(recipe)
                mutation[field] = value
                self.assertNotEqual(
                    baseline,
                    _project_recipe_semantics(mutation),
                )

    def test_semantic_projection_rejects_unknown_nondefault_fields(self) -> None:
        recipe = {
            "attributes": [],
            "body": [["cargo check"]],
            "dependencies": [
                {"arguments": [], "recipe": "preflight", "star": None}
            ],
            "parameters": [
                {
                    "default": None,
                    "export": False,
                    "kind": "singular",
                    "name": "package",
                }
            ],
            "priors": 0,
            "private": False,
            "quiet": False,
            "shebang": False,
        }
        mutations = []
        recipe_field = json.loads(json.dumps(recipe))
        recipe_field["future_execution_flag"] = True
        mutations.append(recipe_field)
        dependency_field = json.loads(json.dumps(recipe))
        dependency_field["dependencies"][0]["future_mode"] = "strict"
        mutations.append(dependency_field)
        parameter_field = json.loads(json.dumps(recipe))
        parameter_field["parameters"][0]["future_mode"] = "strict"
        mutations.append(parameter_field)

        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with self.assertRaises(RecipeWitnessError):
                    _project_recipe_semantics(mutation)

    def test_parameter_and_dependency_semantics_are_not_normalized_away(
        self,
    ) -> None:
        parameter = {
            "default": None,
            "export": False,
            "flag": False,
            "help": None,
            "kind": "singular",
            "long": None,
            "max": None,
            "min": None,
            "multiple": False,
            "name": "package",
            "pattern": None,
            "short": None,
            "value": None,
        }
        baseline_parameter = _project_parameter_semantics(parameter)
        parameter_mutations = {
            "default": "kanban-cli",
            "export": True,
            "flag": True,
            "help": "Cargo package",
            "kind": "star",
            "long": "package",
            "max": 2,
            "min": 1,
            "multiple": True,
            "pattern": ["kanban-*"],
            "short": "p",
            "value": "kanban-cli",
        }
        for field, value in parameter_mutations.items():
            with self.subTest(parameter=field):
                mutation = dict(parameter)
                mutation[field] = value
                self.assertNotEqual(
                    baseline_parameter,
                    _project_parameter_semantics(mutation),
                )

        dependency = {
            "arguments": [],
            "recipe": "preflight",
            "star": None,
        }
        baseline_dependency = _project_dependency_semantics(dependency)
        for field, value in (
            ("arguments", [[["string", "release"]]]),
            ("recipe", "other"),
            ("star", 0),
        ):
            with self.subTest(dependency=field):
                mutation = dict(dependency)
                mutation[field] = value
                self.assertNotEqual(
                    baseline_dependency,
                    _project_dependency_semantics(mutation),
                )

    def test_global_nested_unknown_nondefault_fields_fail_closed(self) -> None:
        parser_defaults = {"quiet": False}
        baseline = {
            "aliases": {},
            "assignments": {},
            "doc": None,
            "first": None,
            "groups": [],
            "module_path": "",
            "modules": {},
            "recipes": {},
            "settings": dict(parser_defaults),
            "source": "/parser-specific/justfile",
            "unexports": [],
            "warnings": [],
        }
        mutations = []
        top_level = json.loads(json.dumps(baseline))
        top_level["future_mode"] = "strict"
        mutations.append(top_level)
        alias = json.loads(json.dumps(baseline))
        alias["aliases"]["gate"] = {
            "attributes": [],
            "future_mode": "strict",
            "name": "gate",
            "target": "check",
        }
        mutations.append(alias)
        assignment = json.loads(json.dumps(baseline))
        assignment["assignments"]["flags"] = {
            "eager": False,
            "export": False,
            "future_mode": "strict",
            "name": "flags",
            "private": False,
            "value": "--locked",
        }
        mutations.append(assignment)
        module = json.loads(json.dumps(baseline))
        module["modules"]["nested"] = {
            "aliases": {},
            "assignments": {},
            "doc": None,
            "first": None,
            "future_mode": "strict",
            "groups": [],
            "module_path": "nested",
            "modules": {},
            "recipes": {},
            "settings": dict(parser_defaults),
            "source": "/parser-specific/nested.just",
            "unexports": [],
            "warnings": [],
        }
        mutations.append(module)

        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with self.assertRaises(RecipeWitnessError):
                    _project_global_semantics(mutation, parser_defaults)

    def test_setting_projection_ignores_new_parser_defaults(self) -> None:
        old_defaults = {
            "positional_arguments": False,
            "quiet": False,
            "shell": None,
        }
        old_settings = {
            **old_defaults,
            "positional_arguments": True,
            "shell": {"arguments": ["-cu"], "command": "bash"},
        }
        new_defaults = {
            **old_defaults,
            "future_false": False,
            "future_list": [],
            "future_null": None,
        }
        new_settings = {
            **old_settings,
            "future_false": False,
            "future_list": [],
            "future_null": None,
        }

        self.assertEqual(
            _project_setting_overrides(old_settings, old_defaults),
            _project_setting_overrides(new_settings, new_defaults),
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
            "scripts/cargo-build-lock.sh -- cargo check --locked -p kanban-schema-tool --tests",
            "schema-tool:\n    echo check omitted\n",
            "schema-tool",
            _schema_tool,
        )
        self.assert_mutation_rejected(
            "    scripts/cargo-build-lock.sh -- cargo clippy --locked -p kanban-schema-tool --all-targets -- -D warnings\n",
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
            "    scripts/cargo-build-lock.sh -- cargo run --locked -p kanban-schema-tool --bin kanban-schema -- audit --root . --require-closed\n",
            "    cargo run -p kanban-schema-tool --bin kanban-schema -- audit --root . --require-closed\n",
            "schema-audit-closed",
            _schema_audit_closed,
            delegate=False,
        )

    def test_release_cannot_omit_or_reorder_schema_contract(self) -> None:
        self.assert_mutation_rejected(
            "    scripts/release-cohort.sh\n",
            "    echo release cohort omitted\n",
            "release",
            _release,
            delegate=False,
        )
        self.assert_mutation_rejected(
            "    scripts/release-cohort.sh\n",
            "    scripts/release-source-gate.sh\n",
            "release",
            _release,
            delegate=False,
        )

    def test_release_nested_gate_order_is_exact(self) -> None:
        wrapper = (ROOT / "scripts/release-cohort.sh").read_text(encoding="utf-8")
        _verify_release_wrapper_semantics(wrapper)
        self.assertIn("just schema-contract", wrapper)
        self.assertLess(
            wrapper.index("just check-windows-p kanban-local"),
            wrapper.index("just projection-release-cohort"),
        )
        self.assertLess(
            wrapper.index("just desktop-package"),
            wrapper.index("just desktop-package-layout"),
        )

    def test_release_safe_publish_witness_rejects_negative_mutations(self) -> None:
        wrapper = (ROOT / "scripts/release-cohort.sh").read_text(encoding="utf-8")
        _verify_release_wrapper_semantics(wrapper)
        mutations = (
            (
                'python3 "$SAFE_PATH" seal-tree --root "$TARGET_ROOT" --path "$STAGE_DIR"',
                'chmod -R a-w "$STAGE_DIR"',
            ),
            (
                '--source "$ARTIFACT_MANIFEST_PENDING" \\\n'
                '  --destination "$ARTIFACT_MANIFEST_FINAL"',
                'mv -f "$ARTIFACT_MANIFEST_PENDING" "$ARTIFACT_MANIFEST_FINAL"',
            ),
            (
                '    --expected-tree-sha256 "$GENERATION_SHA256" \\\n'
                '    --verify-command "$ARTIFACT_MANIFEST" verify-final',
                '--expected-tree-sha256 "$GENERATION_SHA256"',
            ),
            (
                'python3 "$SAFE_PATH" tree-digest --root "$TARGET_ROOT"',
                'printf "%064d\\n" 0',
            ),
            (
                'python3 "$SAFE_PATH" seal-tree --root "$TARGET_ROOT" --path "$STAGE_DIR"',
                'python3 "$SAFE_PATH" seal-tree --root "$TARGET_ROOT" --path "$STAGE_DIR"\n'
                'python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" '
                '--path "$ARTIFACT_MANIFEST_FINAL"',
            ),
            (
                '''  python3 "$SAFE_PATH" publish-dir --root "$TARGET_ROOT" \\
    --source "$STAGE_DIR" --destination "$PUBLISHED_DIR" \\
    --expected-tree-sha256 "$GENERATION_SHA256" \\
    --verify-command "$ARTIFACT_MANIFEST" verify-final \\
      --manifest "$ARTIFACT_MANIFEST_FINAL" --stage-dir "$STAGE_DIR"''',
                '''  if false; then
    python3 "$SAFE_PATH" publish-dir --root "$TARGET_ROOT" \\
      --source "$STAGE_DIR" --destination "$PUBLISHED_DIR" \\
      --expected-tree-sha256 "$GENERATION_SHA256" \\
      --verify-command "$ARTIFACT_MANIFEST" verify-final \\
        --manifest "$ARTIFACT_MANIFEST_FINAL" --stage-dir "$STAGE_DIR"
  fi''',
            ),
            (
                "if [[ \"$RESUME_RELEASE\" == \"1\" ]]; then\n"
                "  publish_generation\n",
                "if [[ \"$RESUME_RELEASE\" == \"1\" ]]; then\n"
                "  if [[ \"${KANBAN_RELEASE_SKIP_PUBLISH:-0}\" != \"1\" ]]; then\n"
                "    publish_generation\n"
                "  fi\n",
            ),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                mutation = wrapper.replace(original, replacement, 1)
                self.assertNotEqual(mutation, wrapper)
                with self.assertRaises(RecipeWitnessError):
                    _verify_release_wrapper_semantics(mutation)
        sealed_mutations = (
            (
                'tools_root="$PUBLISHED_DIR/.release-tools"',
                'tools_root="$ROOT"',
            ),
            (
                "for dependency in cargo-build-lock.sh release-safe-path.py "
                "release-source-gate.sh; do",
                "for dependency in cargo-build-lock.sh release-safe-path.py; do",
            ),
            (
                'SOURCE_GATE="$SEALED_SOURCE_GATE"',
                'SOURCE_GATE="$ROOT/scripts/release-source-gate.sh"',
            ),
            (
                '--verify-command "$ARTIFACT_MANIFEST" verify-final \\\n'
                '        --manifest "$PUBLISHED_DIR/release-artifacts.json"',
                '--manifest "$PUBLISHED_DIR/release-artifacts.json"',
            ),
            (
                'if [[ "$RESUME_PUBLISHED" == "1" ]]; then\n'
                "  resume_published_generation\n",
                'if [[ "$RESUME_PUBLISHED" == "1" ]]; then\n'
                '  if [[ "${KANBAN_RELEASE_SKIP_VERIFY:-0}" != "1" ]]; then\n'
                "    resume_published_generation\n"
                "  fi\n",
            ),
        )
        for original, replacement in sealed_mutations:
            with self.subTest(sealed_original=original):
                mutation = wrapper.replace(original, replacement, 1)
                self.assertNotEqual(mutation, wrapper)
                with self.assertRaises(RecipeWitnessError):
                    _verify_sealed_release_tooling(mutation)

    def test_projection_release_cohort_cannot_drop_a_backend(self) -> None:
        self.assert_mutation_rejected(
            '    just feature-p kanban-cli "tantivy-backend,oxigraph-backend"\n',
            '    just feature-p kanban-cli "tantivy-backend"\n',
            "projection-release-cohort",
            _projection_release_cohort,
        )
        self.assert_mutation_rejected(
            '    just feature-p kanban-server "tantivy-backend,oxigraph-backend"\n',
            '    just feature-p kanban-server "oxigraph-backend"\n',
            "projection-release-cohort",
            _projection_release_cohort,
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
            "scripts/cargo-build-lock.sh -- cargo test --locked -p kanban-cli clap_leaf_commands_match_exact_contract_catalog",
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
