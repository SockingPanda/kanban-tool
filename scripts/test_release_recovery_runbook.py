#!/usr/bin/env python3
"""Read-only contract checks for the site-evidence recovery runbook section."""

from __future__ import annotations

import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
RUNBOOK = ROOT / "docs" / "release" / "DERIVED_PROJECTION_V2_RECOVERY.md"


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def section_eight(document: str) -> str:
    start_marker = "## 8. 同 cohort 部署和严格串行 rebuild"
    end_marker = "\n## 9."
    start = document.find(start_marker)
    end = document.find(end_marker, start + len(start_marker))
    if start < 0 or end < 0:
        fail("runbook section 8 boundaries are missing")
    return document[start:end]


def main() -> int:
    document = RUNBOOK.read_text(encoding="utf-8")
    section = section_eight(document)

    required_phrases = (
        "本 runbook\n不执行、也不验证任何现场 primitive",
        "现场输入和证据是唯一允许的部署接口",
        "operation_id, started_at_utc, finished_at_utc, actor, decision",
        "deploy.dry_run.exit, deploy.dry_run.stdout, deploy.dry_run.stderr",
        "rollback.dry_run.exit, rollback.dry_run.stdout, rollback.dry_run.stderr",
        "post_deploy.machine_comparison, post_deploy.helper_binding",
        "post_deploy.unit_metadata(optional, raw; never inferred)",
        "canonical 六 roles",
        "cli_binary",
        "lancedb_helper",
        "oxigraph_helper",
        "desktop_binary",
        "cli_deb",
        "desktop_deb",
        "operation_evidence.path, operation_evidence.mode, operation_evidence.uid",
        "wrapper_stat.mode,uid,gid,dev,ino,nlink",
        "deploy.post_bind.json.absolute_path, deploy.post_bind.json.content_sha256",
        "deploy.post_bind.json.schema",
        "rollback.post_bind.json.absolute_path, rollback.post_bind.json.content_sha256",
        "rollback.post_bind.json.schema",
        "cohort.source_provenance_sha256, cohort.source_map_sha256",
        "不存在、0700、no-symlink、no-overwrite",
        "绝对路径、`lstat` 得到",
        "argv 数组",
        "`shell=false`、无 `eval`",
        "保持生产只读",
        "fresh post-bind",
        'deploy.argv_template = ["<wrapper>", "--dry-run", "--cohort"',
        'deploy.apply.argv_template = ["<wrapper>", "--cohort"',
        'rollback.argv_template = ["<wrapper>", "--dry-run", "--cohort"',
        'rollback.apply.argv_template = ["<wrapper>", "--cohort"',
        '"--evidence", "<operation_evidence.path>", "--post-bind", "<post_bind.json.absolute_path>"',
        "release/bundle/cohort/<generation_key>",
        "canonical SQLite restore 是独立的高风险 database replacement",
    )
    for phrase in required_phrases:
        if phrase not in section:
            fail(f"section 8 is missing required contract wording: {phrase!r}")

    evidence_start = section.index("operation_id, started_at_utc, finished_at_utc, actor, decision")
    evidence_end = section.index("~~~", evidence_start)
    evidence = section[evidence_start:evidence_end]
    field_order = (
        "operation_id, started_at_utc, finished_at_utc, actor, decision",
        "cohort.absolute_path, cohort.generation_key",
        "artifacts[].role",
        "deploy.wrapper_path",
        "deploy.dry_run.exit",
        "deploy.apply.exit",
        "rollback.predecessor.absolute_path, rollback.predecessor.generation_key",
        "rollback.wrapper_path",
        "rollback.dry_run.exit",
        "rollback.apply.exit",
        "post_deploy.machine_comparison",
    )
    positions = [evidence.index(field) for field in field_order]
    if positions != sorted(positions):
        fail("evidence fields are not ordered from operation to post-deploy proof")

    if "site-deployment-gate" in document:
        fail("runbook must not claim a repository-owned deployment executor")
    if re.search(r"(?m)^\s*(?:sudo\s+)?(?:systemctl|dpkg(?:-deb)?)\s+", section):
        fail("section 8 must not contain executable systemctl/dpkg command lines")
    if "<absolute-site-approved" in section or "<absolute-approved-previous" in section:
        fail("section 8 contains executable-looking deployment placeholders")

    for removed in (
        ROOT / "scripts" / "site-deployment-gate.py",
        ROOT / "scripts" / "test_site_deployment_gate.py",
    ):
        if removed.exists() or removed.is_symlink():
            fail(f"removed repository executor still exists: {removed}")
    justfile = (ROOT / "justfile").read_text(encoding="utf-8")
    if "release-recovery-runbook-contract:" not in justfile:
        fail("justfile is missing the explicit read-only runbook contract recipe")
    if "target-tools:\n    just release-recovery-runbook-contract" in justfile:
        fail("runbook contract recipe must not expand the default target-tools gate")
    release_lines = justfile.splitlines()
    try:
        release_index = release_lines.index("release:")
    except ValueError:
        fail("justfile is missing the canonical release recipe")
    release_body = []
    for line in release_lines[release_index + 1 :]:
        if line.startswith("    "):
            release_body.append(line.strip())
        elif line == "":
            break
        else:
            break
    if release_body != ["scripts/release-cohort.sh"]:
        fail("release recipe call graph must remain the single cohort wrapper")
    if "just release-recovery-runbook-contract" not in section:
        fail("section 8 must name the explicit pre-release runbook contract recipe")

    print("ok: recovery runbook records site primitive evidence without a repository executor")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
