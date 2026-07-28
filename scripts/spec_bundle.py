#!/usr/bin/env python3
"""Generate or check the deterministic KANBAN_SPEC_BUNDLE.md snapshot."""

from __future__ import annotations

import argparse
import os
import posixpath
import re
import tempfile
from pathlib import Path


BUNDLE_PATH = "KANBAN_SPEC_BUNDLE.md"
SOURCE_PATHS = (
    "README.md",
    "docs/SPEC.md",
    "docs/ARCHITECTURE.md",
    "docs/STATE_MACHINE.md",
    "docs/DATA_MODEL.md",
    "docs/CLI_SPEC.md",
    "docs/API_SPEC.md",
    "docs/SCHEMA_CONTRACTS.md",
    "docs/ADR.md",
    "migrations/001_initial.sql",
    "migrations/003_comment_author_identity.sql",
)

AUTHORITY_NOTE = (
    "`docs/SPEC.md`、`docs/CLI_SPEC.md`、`docs/API_SPEC.md`、"
    "`docs/STATE_MACHINE.md` 和 `docs/SCHEMA_CONTRACTS.md` 等分主题文档"
    "是当前行为的权威来源；"
    "本文件是这些源文档的同步快照，便于一次性阅读和离线传递。"
)


class BundleError(RuntimeError):
    """The bundle could not be rendered or validated."""


class BundleDrift(BundleError):
    """The committed bundle differs from its canonical source documents."""


_INLINE_LINK_DESTINATION = re.compile(
    r"(?P<prefix>\]\()(?P<destination>[^)\s]+)(?P<suffix>\))"
)
_URI_SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")


def _read_source(root: Path, relative_path: str) -> str:
    path = root / relative_path
    if not path.is_file():
        raise BundleError(f"missing bundle source: {relative_path}")
    return path.read_text(encoding="utf-8").rstrip("\n")


def _rebase_relative_links(content: str, source_path: str) -> str:
    source_directory = posixpath.dirname(source_path)

    def replace(match: re.Match[str]) -> str:
        destination = match.group("destination")
        if (
            destination.startswith(("#", "/", "//"))
            or _URI_SCHEME.match(destination)
        ):
            return match.group(0)

        path_and_query, fragment_separator, fragment = destination.partition("#")
        link_path, query_separator, query = path_and_query.partition("?")
        if not link_path:
            return match.group(0)

        rebased_path = posixpath.normpath(
            posixpath.join(source_directory, link_path)
        )
        rebased_destination = rebased_path
        if query_separator:
            rebased_destination += f"?{query}"
        if fragment_separator:
            rebased_destination += f"#{fragment}"
        return (
            f"{match.group('prefix')}{rebased_destination}{match.group('suffix')}"
        )

    return _INLINE_LINK_DESTINATION.sub(replace, content)


def render_bundle(root: Path) -> str:
    header = [
        "# Kanban Tool 规范合集",
        "",
        "本文档由以下文件合并而成：",
        "",
        *(f"- {source}" for source in SOURCE_PATHS),
        "",
        AUTHORITY_NOTE,
    ]
    sections: list[str] = []
    for source in SOURCE_PATHS:
        content = _read_source(root, source)
        content = _rebase_relative_links(content, source)
        if source.endswith(".sql"):
            content = f"```sql\n{content}\n```"
        sections.append(f"---\n\n# 文件：{source}\n\n{content}")
    return "\n".join(header) + "\n\n\n" + "\n\n\n".join(sections) + "\n"


def check_bundle(root: Path) -> None:
    expected = render_bundle(root)
    path = root / BUNDLE_PATH
    if not path.is_file():
        raise BundleDrift(f"{BUNDLE_PATH} is out of date: file is missing")
    actual = path.read_text(encoding="utf-8")
    if actual != expected:
        raise BundleDrift(
            f"{BUNDLE_PATH} is out of date; run `just spec-bundle-generate`"
        )


def write_bundle(root: Path) -> None:
    rendered = render_bundle(root)
    destination = root / BUNDLE_PATH
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=destination.parent,
            prefix=f".{destination.name}.",
            delete=False,
        ) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write(rendered)
        temporary_path.chmod(0o644)
        os.replace(temporary_path, destination)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    try:
        if args.write:
            write_bundle(root)
            print(f"updated {BUNDLE_PATH}")
        else:
            check_bundle(root)
            print(f"checked {BUNDLE_PATH}")
    except BundleError as error:
        print(f"spec bundle error: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
