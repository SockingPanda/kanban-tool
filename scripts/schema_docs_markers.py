#!/usr/bin/env python3
"""Fail-closed audit for public JSON examples embedded in Markdown."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


MANIFEST = Path("schemas/json-schema/draft-2020-12/manifest.json")
EXACT_MARKER = re.compile(
    r"^<!-- schema-doc: contract=([^\s]+) fixture=([^\s]+) -->$"
)
IGNORE_MARKER = re.compile(r"^<!-- schema-doc-ignore: (\S(?:.*\S)?) -->$")
FENCE_OPEN = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")


class MarkerError(RuntimeError):
    """Raised when a public JSON example is not auditable."""


def _load_contract_fixtures(repo_root: Path) -> dict[str, str]:
    path = repo_root / MANIFEST
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise MarkerError(f"cannot read schema manifest {path}: {error}") from error
    roots = document.get("roots")
    if not isinstance(roots, list):
        raise MarkerError(f"schema manifest roots must be an array: {path}")
    fixtures: dict[str, str] = {}
    for root in roots:
        if not isinstance(root, dict):
            raise MarkerError(f"schema manifest root must be an object: {path}")
        contract = root.get("contract_id")
        fixture = root.get("schema_fixture")
        if not isinstance(contract, str) or not isinstance(fixture, str):
            raise MarkerError(f"schema manifest root lacks contract_id/schema_fixture: {path}")
        if contract in fixtures:
            raise MarkerError(f"schema manifest contains duplicate contract: {contract}")
        fixtures[contract] = fixture
    return fixtures


def _documentation_files(repo_root: Path) -> list[Path]:
    files = sorted((repo_root / "docs").rglob("*.md")) if (repo_root / "docs").is_dir() else []
    return sorted([*repo_root.glob("*.md"), *files])


def _read_json(path: Path, label: str) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise MarkerError(f"{label} is not readable JSON: {path}: {error}") from error


def _opening_fence(line: str) -> tuple[str, int, str] | None:
    match = FENCE_OPEN.fullmatch(line)
    if match is None:
        return None
    fence, raw_info = match.groups()
    if fence[0] == "`" and "`" in raw_info:
        return None
    info = raw_info.strip()
    first_token = info.split(maxsplit=1)[0] if info else ""
    return fence[0], len(fence), first_token


def _is_closing_fence(line: str, character: str, opening_length: int) -> bool:
    return re.fullmatch(
        rf"^ {{0,3}}{re.escape(character)}{{{opening_length},}}[ \t]*$", line
    ) is not None


def _audit_file(path: Path, repo_root: Path, contract_fixtures: dict[str, str]) -> tuple[int, int]:
    lines = path.read_text(encoding="utf-8").splitlines()
    exact_count = 0
    ignored_count = 0
    index = 0
    while index < len(lines):
        line = lines[index].strip()
        exact = EXACT_MARKER.fullmatch(line)
        ignored = IGNORE_MARKER.fullmatch(line)
        if line.startswith("<!-- schema-doc") and not exact and not ignored:
            raise MarkerError(f"malformed schema-doc marker: {path}:{index + 1}")
        if exact or ignored:
            opening = _opening_fence(lines[index + 1]) if index + 1 < len(lines) else None
            if opening is None or opening[2] != "json":
                raise MarkerError(
                    f"schema-doc marker must be immediately followed by a JSON fence: {path}:{index + 1}"
                )
            index += 1
            continue

        opening = _opening_fence(lines[index])
        if opening is None:
            index += 1
            continue

        character, opening_length, first_token = opening
        closing = index + 1
        while closing < len(lines) and not _is_closing_fence(
            lines[closing], character, opening_length
        ):
            closing += 1
        if closing == len(lines):
            raise MarkerError(f"unterminated fenced block: {path}:{index + 1}")
        if first_token != "json":
            index = closing + 1
            continue

        if index == 0:
            raise MarkerError(f"unmarked public JSON block: {path}:{index + 1}")
        marker_line = lines[index - 1].strip()
        exact = EXACT_MARKER.fullmatch(marker_line)
        ignored = IGNORE_MARKER.fullmatch(marker_line)
        if not exact and not ignored:
            raise MarkerError(f"unmarked public JSON block: {path}:{index + 1}")

        if ignored:
            ignored_count += 1
        else:
            assert exact is not None
            contract, fixture = exact.groups()
            expected_fixture = contract_fixtures.get(contract)
            if expected_fixture is None:
                raise MarkerError(f"unknown contract in schema-doc marker: {contract} ({path}:{index})")
            if fixture != expected_fixture:
                raise MarkerError(
                    f"schema-doc fixture mismatch for {contract}: marker={fixture}, manifest={expected_fixture} "
                    f"({path}:{index})"
                )
            fixture_path = repo_root / fixture
            expected = _read_json(fixture_path, "schema fixture")
            inline_text = "\n".join(lines[index + 1 : closing])
            try:
                inline = json.loads(inline_text)
            except json.JSONDecodeError as error:
                raise MarkerError(f"schema-doc block is not valid JSON: {path}:{index + 1}: {error}") from error
            if inline != expected:
                raise MarkerError(
                    f"schema-doc block does not match fixture {fixture}: {path}:{index + 1}"
                )
            exact_count += 1
        index = closing + 1
    return exact_count, ignored_count


def validate_repository(repo_root: Path) -> tuple[int, int]:
    repo_root = repo_root.resolve()
    contract_fixtures = _load_contract_fixtures(repo_root)
    exact_total = 0
    ignored_total = 0
    for path in _documentation_files(repo_root):
        exact, ignored = _audit_file(path, repo_root, contract_fixtures)
        exact_total += exact
        ignored_total += ignored
    return exact_total, ignored_total


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    exact, ignored = validate_repository(args.root)
    print(f"ok: schema docs markers exact={exact} intentionally_illustrative={ignored}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
