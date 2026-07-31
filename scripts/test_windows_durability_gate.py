#!/usr/bin/env python3
"""Hostile fixtures for the Windows durability workflow structure gate."""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path


REQUIRED_RUNS = (
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
WORKFLOWS = (("pr.yml", "pr-result"), ("full-ci.yml", "full-ci-result"))
WorkflowMutation = Callable[[str, str], str]


def canonical_result_script(result_job: str) -> list[str]:
    if result_job == "pr-result":
        return [
            "changes_result='${{ needs.changes.result }}'",
            "repo_meta_result='${{ needs.repo-meta.result }}'",
            "rust_result='${{ needs.rust-default.result }}'",
            "windows_durability_result="
            "'${{ needs.rust-windows-durability.result }}'",
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
    return [
        "for result in \\",
        "'${{ needs.rust-default.result }}' \\",
        "'${{ needs.rust-features.result }}' \\",
        "'${{ needs.rust-windows-durability.result }}' \\",
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


def valid_workflow(result_job: str) -> str:
    lines = [
        "name: fixture",
        "jobs:",
        "  rust-windows-durability:",
    ]
    if result_job == "pr-result":
        lines.append("    if: needs.changes.outputs.rust == 'true'")
    lines.extend(["    runs-on: windows-latest", "    steps:"])
    for index, command in enumerate(REQUIRED_RUNS):
        lines.extend(
            [
                f"      - name: required {index}",
                f"        run: {command}",
            ]
        )
    lines.extend(
        [
            f"  {result_job}:",
            "    needs:",
            "      - rust-windows-durability",
            "    if: always()",
            "    runs-on: ubuntu-latest",
            "    steps:",
            "      - name: Check required jobs",
            "        run: |",
        ]
    )
    lines.extend(f"          {line}" for line in canonical_result_script(result_job))
    return "\n".join(lines) + "\n"


def add_job_if_false(source: str, _result_job: str) -> str:
    canonical = "    if: needs.changes.outputs.rust == 'true'\n"
    if canonical in source:
        return source.replace(canonical, "    if: false\n", 1)
    return source.replace("    runs-on:", "    if: false\n    runs-on:", 1)


def add_step_if_false(source: str, _result_job: str) -> str:
    return source.replace(
        "      - name: required 0\n        run:",
        "      - name: required 0\n        if: false\n        run:",
        1,
    )


def add_job_continue_on_error(source: str, _result_job: str) -> str:
    return source.replace(
        "  rust-windows-durability:\n",
        "  rust-windows-durability:\n    continue-on-error: true\n",
        1,
    )


def add_step_continue_on_error(source: str, _result_job: str) -> str:
    return source.replace(
        "      - name: required 0\n        run:",
        "      - name: required 0\n        continue-on-error: true\n        run:",
        1,
    )


def add_step_shell_override(source: str, _result_job: str) -> str:
    return source.replace(
        "      - name: required 0\n        run:",
        "      - name: required 0\n        shell: powershell {0}; exit 0\n        run:",
        1,
    )


def swap_durability_steps(source: str, _result_job: str) -> str:
    first = (
        f"      - name: required 1\n        run: {REQUIRED_RUNS[1]}\n"
        f"      - name: required 2\n        run: {REQUIRED_RUNS[2]}\n"
    )
    second = (
        f"      - name: required 2\n        run: {REQUIRED_RUNS[2]}\n"
        f"      - name: required 1\n        run: {REQUIRED_RUNS[1]}\n"
    )
    return source.replace(first, second, 1)


def add_unexpected_durability_step(source: str, result_job: str) -> str:
    needle = f"        run: {REQUIRED_RUNS[-1]}\n  {result_job}:"
    replacement = (
        f"        run: {REQUIRED_RUNS[-1]}\n"
        "      - name: unexpected durability command\n"
        "        run: echo falsely green\n"
        f"  {result_job}:"
    )
    return source.replace(needle, replacement, 1)


def remove_raw_opener_audit(source: str, _result_job: str) -> str:
    return source.replace(
        f"      - name: required 3\n        run: {REQUIRED_RUNS[3]}\n",
        "",
        1,
    )


def hide_run_in_unreachable_powershell(source: str, _result_job: str) -> str:
    command = REQUIRED_RUNS[0]
    return source.replace(
        f"        run: {command}",
        "\n".join(
            [
                "        run: |",
                "          if ($false) {",
                f"            {command}",
                "          }",
            ]
        ),
        1,
    )


def replace_result_check_with_echo(source: str, result_job: str) -> str:
    prefix, _separator, _result = source.partition(f"  {result_job}:")
    return (
        prefix
        + "\n".join(
            [
                f"  {result_job}:",
                "    needs:",
                "      - rust-windows-durability",
                "    if: always()",
                "    runs-on: ubuntu-latest",
                "    steps:",
                "      - run: echo falsely green",
                "",
            ]
        )
    )


def add_result_step_continue_on_error(source: str, _result_job: str) -> str:
    return source.replace(
        "      - name: Check required jobs\n        run:",
        "      - name: Check required jobs\n        continue-on-error: true\n        run:",
        1,
    )


def add_result_step_shell_override(source: str, _result_job: str) -> str:
    return source.replace(
        "      - name: Check required jobs\n        run:",
        "      - name: Check required jobs\n"
        "        shell: bash {0}; exit 0\n"
        "        run:",
        1,
    )


def add_result_job_shell_override(source: str, result_job: str) -> str:
    return source.replace(
        f"  {result_job}:\n",
        f"  {result_job}:\n"
        "    defaults:\n"
        "      run:\n"
        "        shell: bash {0}; exit 0\n",
        1,
    )


def fold_result_script(source: str, _result_job: str) -> str:
    return source.replace("        run: |\n", "        run: >\n", 1)


def add_result_early_success_exit(source: str, _result_job: str) -> str:
    return source.replace(
        "        run: |\n",
        "        run: |\n          exit 0\n",
        1,
    )


def make_result_check_unreachable(source: str, _result_job: str) -> str:
    wrapped = source.replace(
        "        run: |\n",
        "        run: |\n          if false; then\n",
        1,
    )
    return wrapped.rstrip() + "\n          fi\n"


def make_result_job_conditional(source: str, result_job: str) -> str:
    return source.replace(
        f"  {result_job}:\n    needs:\n"
        "      - rust-windows-durability\n"
        "    if: always()",
        f"  {result_job}:\n    needs:\n"
        "      - rust-windows-durability\n"
        "    if: false",
        1,
    )


def move_evidence_to_decoy_job(source: str, result_job: str) -> str:
    lines = [
        "name: hostile fixture",
        "jobs:",
        "  rust-windows-durability:",
        "    runs-on: ubuntu-latest",
        "    steps:",
        "      - run: echo skipped all required durability tests",
        "  decoy:",
        "    runs-on: windows-latest",
        "    steps:",
    ]
    for index, command in enumerate(REQUIRED_RUNS):
        lines.extend(
            [
                f"      - name: decoy {index}",
                f"        run: {command}",
            ]
        )
    lines.extend(
        [
            f"  {result_job}:",
            "    needs:",
            "      - decoy",
            "    if: always()",
            "    runs-on: ubuntu-latest",
            "    steps:",
            "      - run: echo falsely green",
        ]
    )
    return "\n".join(lines) + "\n"


class WindowsDurabilityGateTests(unittest.TestCase):
    def run_fixture(
        self,
        *,
        mutate_file: str | None = None,
        mutation: WorkflowMutation | None = None,
        release_recipe: str | None = "scripts/release-cohort.sh",
    ) -> subprocess.CompletedProcess[str]:
        repository_root = Path(__file__).resolve().parent.parent
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            (root / ".github" / "workflows").mkdir(parents=True)
            shutil.copy2(
                repository_root / "scripts" / "test-windows-durability-gate.sh",
                root / "scripts" / "test-windows-durability-gate.sh",
            )
            # This unrelated recipe is an intentional decoy: the release
            # linkage must be read from the parsed `release` recipe itself.
            justfile = ["target-tools:", "    scripts/release-cohort.sh"]
            if release_recipe is not None:
                justfile.extend(["", "release:", f"    {release_recipe}"])
            (root / "justfile").write_text(
                "\n".join(justfile) + "\n",
                encoding="utf-8",
            )
            (root / "scripts" / "release-cohort.sh").write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "just check-windows-p kanban-local\n",
                encoding="utf-8",
            )
            for filename, result_job in WORKFLOWS:
                source = valid_workflow(result_job)
                if filename == mutate_file and mutation is not None:
                    source = mutation(source, result_job)
                (root / ".github" / "workflows" / filename).write_text(
                    source,
                    encoding="utf-8",
                )
            return subprocess.run(
                [str(root / "scripts" / "test-windows-durability-gate.sh")],
                cwd=root,
                check=False,
                capture_output=True,
                text=True,
            )

    def assert_rejected_individually(self, mutation: WorkflowMutation) -> None:
        for filename, _result_job in WORKFLOWS:
            with self.subTest(workflow=filename):
                result = self.run_fixture(mutate_file=filename, mutation=mutation)
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_valid_structured_jobs_pass(self) -> None:
        result = self.run_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_decoy_jobs_cannot_satisfy_the_named_job_or_result_gate(self) -> None:
        self.assert_rejected_individually(move_evidence_to_decoy_job)

    def test_job_if_false_is_rejected(self) -> None:
        self.assert_rejected_individually(add_job_if_false)

    def test_step_if_false_is_rejected(self) -> None:
        self.assert_rejected_individually(add_step_if_false)

    def test_job_continue_on_error_is_rejected(self) -> None:
        self.assert_rejected_individually(add_job_continue_on_error)

    def test_step_continue_on_error_is_rejected(self) -> None:
        self.assert_rejected_individually(add_step_continue_on_error)

    def test_step_shell_override_is_rejected(self) -> None:
        self.assert_rejected_individually(add_step_shell_override)

    def test_durability_run_trace_must_remain_ordered(self) -> None:
        self.assert_rejected_individually(swap_durability_steps)

    def test_durability_job_cannot_add_an_unpinned_run_step(self) -> None:
        self.assert_rejected_individually(add_unexpected_durability_step)

    def test_production_raw_opener_audit_is_required(self) -> None:
        self.assert_rejected_individually(remove_raw_opener_audit)

    def test_unreachable_multiline_powershell_is_rejected(self) -> None:
        self.assert_rejected_individually(hide_run_in_unreachable_powershell)

    def test_result_job_must_propagate_the_durability_failure(self) -> None:
        self.assert_rejected_individually(replace_result_check_with_echo)

    def test_result_check_cannot_continue_on_error(self) -> None:
        self.assert_rejected_individually(add_result_step_continue_on_error)

    def test_result_check_cannot_override_the_shell(self) -> None:
        self.assert_rejected_individually(add_result_step_shell_override)

    def test_result_job_cannot_override_the_shell(self) -> None:
        self.assert_rejected_individually(add_result_job_shell_override)

    def test_result_check_must_remain_a_literal_script(self) -> None:
        self.assert_rejected_individually(fold_result_script)

    def test_result_check_cannot_exit_success_before_validation(self) -> None:
        self.assert_rejected_individually(add_result_early_success_exit)

    def test_result_check_cannot_be_unreachable(self) -> None:
        self.assert_rejected_individually(make_result_check_unreachable)

    def test_result_job_must_run_after_failure(self) -> None:
        self.assert_rejected_individually(make_result_job_conditional)

    def test_release_recipe_is_required(self) -> None:
        result = self.run_fixture(release_recipe=None)
        self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_release_recipe_must_invoke_the_cohort_wrapper(self) -> None:
        result = self.run_fixture(release_recipe="echo falsely-green")
        self.assertNotEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
