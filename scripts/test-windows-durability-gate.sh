#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 -B - "$ROOT" <<'PY'
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


root = Path(sys.argv[1])
dump = subprocess.run(
    [
        "just",
        "--justfile",
        str(root / "justfile"),
        "--dump",
        "--dump-format",
        "json",
    ],
    cwd=root,
    check=False,
    capture_output=True,
    text=True,
)
if dump.returncode != 0:
    print(f"error: cannot parse justfile: {dump.stderr.strip()}", file=sys.stderr)
    raise SystemExit(1)
try:
    recipes = json.loads(dump.stdout)["recipes"]
except (KeyError, TypeError, json.JSONDecodeError) as error:
    print(f"error: invalid `just --dump` JSON: {error}", file=sys.stderr)
    raise SystemExit(1)
release = recipes.get("release")
if release is None or release.get("body") != [["scripts/release-cohort.sh"]]:
    print(
        "error: parsed release recipe must invoke only scripts/release-cohort.sh",
        file=sys.stderr,
    )
    raise SystemExit(1)

wrapper = (root / "scripts" / "release-cohort.sh").read_text(
    encoding="utf-8"
).splitlines()
required = "just check-windows-p kanban-local"
if sum(line == required for line in wrapper) != 1:
    print(
        "error: release cohort must contain one unconditional Windows cross-check",
        file=sys.stderr,
    )
    raise SystemExit(1)
PY

check_workflow() {
  local workflow="$1"
  local result_job="$2"

  python3 -B - "$workflow" "$result_job" <<'PY'
from __future__ import annotations

import re
import sys
from pathlib import Path


workflow = Path(sys.argv[1])
result_job_name = sys.argv[2]
lines = workflow.read_text(encoding="utf-8").splitlines()
required_runs = (
    "cargo test --locked -p kanban-local durable_",
    "cargo test --locked -p kanban-local --test database_lifecycle",
    "cargo test --locked -p kanban-sqlite --test lifecycle_connection",
    "cargo test --locked -p kanban-sqlite --test raw_file_open_audit",
    "cargo test --locked -p kanban-sqlite --features "
    "'tantivy-backend,oxigraph-backend' --lib",
    "cargo test --locked -p kanban-sqlite --features "
    "'tantivy-backend,oxigraph-backend' --test all "
    "suite::maintenance_runtime::",
    "cargo test --locked -p kanban-sqlite --features "
    "'tantivy-backend,oxigraph-backend' --test all suite::projection_v2::",
)
job_key = re.compile(r"^  ([A-Za-z0-9_-]+):(?:\s+#.*)?$")
field_key = re.compile(r"^([A-Za-z0-9_-]+):(.*)$")
result_expression = "${{ needs.rust-windows-durability.result }}"
expected_result_scripts = {
    "pr-result": "\n".join(
        [
            "changes_result='${{ needs.changes.result }}'",
            "repo_meta_result='${{ needs.repo-meta.result }}'",
            "rust_result='${{ needs.rust-default.result }}'",
            f"windows_durability_result='{result_expression}'",
            "desktop_result='${{ needs.desktop.result }}'",
            "desktop_rust_result='${{ needs.desktop-rust.result }}'",
            "dependency_audit_result='${{ needs.dependency-audit.result }}'",
            'if [[ "$changes_result" != "success" ]]; then',
            'echo "Changed-path detection finished with result: $changes_result"',
            "exit 1",
            "fi",
            'for result in "$repo_meta_result" "$rust_result" '
            '"$windows_durability_result" "$desktop_result" '
            '"$desktop_rust_result" "$dependency_audit_result"; do',
            'case "$result" in',
            "success|skipped) ;;",
            '*) echo "A gated PR job finished with result: $result"; exit 1 ;;',
            "esac",
            "done",
            'echo "PR checks passed"',
        ]
    ),
    "full-ci-result": "\n".join(
        [
            "for result in \\",
            "'${{ needs.rust-default.result }}' \\",
            "'${{ needs.rust-features.result }}' \\",
            f"'{result_expression}' \\",
            "'${{ needs.desktop.result }}' \\",
            "'${{ needs.smoke.result }}' \\",
            "'${{ needs.dependency-audit.result }}'",
            "do",
            'case "$result" in',
            "success) ;;",
            '*) echo "A full CI job finished with result: $result"; exit 1 ;;',
            "esac",
            "done",
            'echo "Full CI passed"',
        ]
    ),
}


def fail(message: str) -> None:
    print(f"error: {message} in {workflow}", file=sys.stderr)
    raise SystemExit(1)


def indentation(line: str) -> int:
    prefix = line[: len(line) - len(line.lstrip())]
    if "\t" in prefix:
        fail("tabs are not allowed in workflow indentation")
    return len(line) - len(line.lstrip(" "))


def scalar(value: str) -> str:
    value = value.strip()
    if " #" in value:
        value = value.split(" #", 1)[0].rstrip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        value = value[1:-1]
    return value


jobs_markers = [
    index
    for index, line in enumerate(lines)
    if indentation(line) == 0 and line.strip() == "jobs:"
]
if len(jobs_markers) != 1:
    fail("workflow must contain exactly one top-level jobs mapping")
jobs_start = jobs_markers[0] + 1
jobs_end = len(lines)
for index in range(jobs_start, len(lines)):
    if lines[index].strip() and indentation(lines[index]) == 0:
        jobs_end = index
        break

jobs: dict[str, list[str]] = {}
current_name: str | None = None
current_lines: list[str] = []
for line in lines[jobs_start:jobs_end]:
    match = job_key.fullmatch(line)
    if match:
        if current_name is not None:
            jobs[current_name] = current_lines
        current_name = match.group(1)
        if current_name in jobs:
            fail(f"duplicate job {current_name}")
        current_lines = []
    elif current_name is not None:
        current_lines.append(line)
if current_name is not None:
    jobs[current_name] = current_lines


def direct_fields(block: list[str]) -> dict[str, tuple[int, str]]:
    fields: dict[str, tuple[int, str]] = {}
    for index, line in enumerate(block):
        if indentation(line) != 4:
            continue
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        match = field_key.fullmatch(line.strip())
        if match is None:
            fail(f"unsupported job field syntax: {line.strip()!r}")
        name, value = match.groups()
        if name in fields:
            fail(f"job field {name!r} must occur at most once")
        fields[name] = (index, value.strip())
    return fields


def direct_field(block: list[str], field: str) -> tuple[int, str]:
    fields = direct_fields(block)
    if field not in fields:
        fail(f"job field {field!r} must occur exactly once")
    return fields[field]


def parse_step_fields(step: list[str]) -> dict[str, tuple[int, str]]:
    fields: dict[str, tuple[int, str]] = {}
    for index, line in enumerate(step):
        indent = indentation(line)
        stripped = line.strip()
        if index == 0:
            if indent != 6 or not stripped.startswith("-"):
                fail("steps entries must begin at indentation six")
            stripped = stripped[1:].strip()
            if not stripped:
                continue
        elif indent != 8:
            continue
        if not stripped or stripped.startswith("#"):
            continue
        match = field_key.fullmatch(stripped)
        if match is None:
            fail(f"unsupported step field syntax: {stripped!r}")
        name, value = match.groups()
        if name in fields:
            fail(f"step field {name!r} must occur at most once")
        fields[name] = (index, value.strip())
    return fields


def step_blocks(block: list[str]) -> list[list[str]]:
    steps_index, steps_value = direct_field(block, "steps")
    if steps_value:
        fail("steps must be an indented sequence")
    result: list[list[str]] = []
    current: list[str] | None = None
    for line in block[steps_index + 1 :]:
        indent = indentation(line)
        if line.strip() and indent <= 4:
            break
        if indent == 6 and line.strip().startswith("-"):
            if current is not None:
                result.append(current)
            current = [line]
        elif current is not None:
            current.append(line)
    if current is not None:
        result.append(current)
    return result


def step_run(step: list[str]) -> tuple[str, bool] | None:
    fields = parse_step_fields(step)
    if "run" not in fields:
        return None
    run_index, run_value = fields["run"]
    if run_value not in {"|", "|-", "|+", ">", ">-", ">+"}:
        return scalar(run_value), False
    run_indent = indentation(step[run_index])
    body: list[str] = []
    for line in step[run_index + 1 :]:
        if line.strip() and indentation(line) <= run_indent:
            break
        body.append(line.strip())
    return "\n".join(body), True


def needs(block: list[str]) -> set[str]:
    needs_index, needs_value = direct_field(block, "needs")
    if needs_value:
        value = scalar(needs_value)
        if value.startswith("[") and value.endswith("]"):
            return {
                scalar(part)
                for part in value[1:-1].split(",")
                if scalar(part)
            }
        return {value}
    result: set[str] = set()
    for line in block[needs_index + 1 :]:
        if line.strip() and indentation(line) <= 4:
            break
        if indentation(line) == 6 and line.strip().startswith("- "):
            result.add(scalar(line.strip()[2:]))
    return result


durability = jobs.get("rust-windows-durability")
if durability is None:
    fail("missing jobs.rust-windows-durability")
durability_fields = direct_fields(durability)
durability_condition = scalar(durability_fields.get("if", (-1, ""))[1])
expected_condition = (
    "needs.changes.outputs.rust == 'true'" if result_job_name == "pr-result" else ""
)
if durability_condition != expected_condition:
    fail(
        "jobs.rust-windows-durability has an unapproved condition; "
        f"expected {expected_condition or 'none'}"
    )
if "continue-on-error" in durability_fields:
    fail("jobs.rust-windows-durability must be fail-fast")
if "defaults" in durability_fields:
    fail("jobs.rust-windows-durability must use the native default shell")
_, runner = direct_field(durability, "runs-on")
if scalar(runner) != "windows-latest":
    fail("jobs.rust-windows-durability.runs-on must be windows-latest")

observed_runs: list[str] = []
observed_steps: list[list[str]] = []
for step in step_blocks(durability):
    run = step_run(step)
    if run is None:
        continue
    command, multiline = run
    if multiline:
        fail("durability job run steps must be single-line commands")
    observed_runs.append(command)
    observed_steps.append(step)
if tuple(observed_runs) != required_runs:
    fail(
        "durability job run trace drifted; expected exactly "
        f"{required_runs!r}, observed {tuple(observed_runs)!r}"
    )
for command, step in zip(required_runs, observed_steps):
    fields = parse_step_fields(step)
    if "if" in fields:
        fail(f"required command step must be unconditional: {command}")
    if "continue-on-error" in fields:
        fail(f"required command step must be fail-fast: {command}")
    if "shell" in fields:
        fail(f"required command step must use the native default shell: {command}")

result_job = jobs.get(result_job_name)
if result_job is None:
    fail(f"missing jobs.{result_job_name}")
if "rust-windows-durability" not in needs(result_job):
    fail(f"jobs.{result_job_name}.needs must include rust-windows-durability")
result_fields = direct_fields(result_job)
if scalar(result_fields.get("if", (-1, ""))[1]) not in {
    "always()",
    "${{ always() }}",
}:
    fail(f"jobs.{result_job_name}.if must be always()")
if "continue-on-error" in result_fields:
    fail(f"jobs.{result_job_name} must propagate failures")
if "defaults" in result_fields:
    fail(f"jobs.{result_job_name} must use the audited native shell")
result_steps = step_blocks(result_job)
if len(result_steps) != 1:
    fail(f"jobs.{result_job_name} must contain exactly one pinned result-check step")
result_step_fields = parse_step_fields(result_steps[0])
if set(result_step_fields) != {"name", "run"}:
    fail(
        f"jobs.{result_job_name} result-check step may contain only pinned name/run fields"
    )
if scalar(result_step_fields["name"][1]) != "Check required jobs":
    fail(f"jobs.{result_job_name} result-check step name drifted")
if result_step_fields["run"][1] != "|":
    fail(f"jobs.{result_job_name} result-check step must use a literal block")
result_run = step_run(result_steps[0])
if (
    result_run is None
    or not result_run[1]
    or result_run[0] != expected_result_scripts[result_job_name]
):
    fail(
        f"jobs.{result_job_name} result-check script drifted from the audited "
        "fail-closed form"
    )
PY
}

check_workflow "$ROOT/.github/workflows/pr.yml" pr-result
check_workflow "$ROOT/.github/workflows/full-ci.yml" full-ci-result

echo "ok: Windows projection durability is cross-compiled and natively tested"
