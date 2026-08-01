#!/usr/bin/env python3
"""Dirfd-anchored filesystem primitives for release packaging.

Every path component is opened with O_NOFOLLOW and every mutation is performed
relative to a retained directory descriptor.  Whole-generation publication
also holds Linux read leases on every regular file while semantic verification,
snapshot hashing, the atomic rename, and post-publish verification run.

The release filesystem must be writable only by a dedicated release OS identity
or an equivalent isolated CI job.  Descriptor and inode checks provide path
confinement, unknown-overwrite prevention, and observable drift detection; they
do not establish exclusive authority across the mkdir-to-first-observation gap
or protect against hostile same-UID, CAP_DAC_OVERRIDE, or same-inode ABA actors.
"""

from __future__ import annotations

import argparse
import ctypes
import errno
import fcntl
import hashlib
import os
import pathlib
import secrets
import signal
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Sequence


class UnsafePath(RuntimeError):
    pass


DIR_FLAGS = (
    os.O_RDONLY
    | getattr(os, "O_DIRECTORY", 0)
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
FILE_FLAGS = (
    os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
)
RENAME_NOREPLACE = 1
RENAME_EXCHANGE = 2
RETENTION_MARKER = ".kanban-release-retain"
_ACTIVE_LEASE_SET: LeaseSet | None = None


def test_event(name: str, path: pathlib.Path) -> None:
    trace_path = os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_TRACE")
    if trace_path:
        with open(trace_path, "a", encoding="utf-8") as handle:
            handle.write(f"{name}\t{path}\n")
    if os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT") == name:
        raise OSError(errno.EIO, f"injected release durability failure at {name}")
    if os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT") == name:
        os._exit(86)
    if os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT") == name:
        marker = os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER")
        continuation = os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE")
        if not marker or not continuation:
            raise UnsafePath(f"test pause {name} requires marker and continuation paths")
        pathlib.Path(marker).write_text(name + "\n", encoding="utf-8")
        deadline = time.monotonic() + 30
        while not os.path.exists(continuation):
            if time.monotonic() >= deadline:
                raise UnsafePath(f"timed out waiting to continue test pause {name}")
            time.sleep(0.01)


def absolute(raw: str, label: str) -> pathlib.Path:
    path = pathlib.Path(raw)
    if not path.is_absolute():
        raise UnsafePath(f"{label} must be absolute: {path}")
    if ".." in path.parts:
        raise UnsafePath(f"{label} must not contain parent traversal: {path}")
    return path


def within(root: pathlib.Path, path: pathlib.Path, label: str) -> None:
    try:
        common = os.path.commonpath((os.fspath(root), os.fspath(path)))
    except ValueError as error:
        raise UnsafePath(f"{label} is not comparable with root: {path}") from error
    if common != os.fspath(root):
        raise UnsafePath(f"{label} escapes the allowed root {root}: {path}")


def same_inode(left: os.stat_result, right: os.stat_result) -> bool:
    return left.st_dev == right.st_dev and left.st_ino == right.st_ino


def open_absolute_directory(path: pathlib.Path) -> int:
    descriptor = os.open("/", DIR_FLAGS)
    current = pathlib.Path("/")
    try:
        for component in path.parts[1:]:
            current /= component
            try:
                child = os.open(component, DIR_FLAGS, dir_fd=descriptor)
            except OSError as error:
                raise UnsafePath(
                    f"directory component is missing, symlinked, or unsafe: {current}"
                ) from error
            os.close(descriptor)
            descriptor = child
        metadata = os.fstat(descriptor)
        if not stat.S_ISDIR(metadata.st_mode):
            raise UnsafePath(f"path is not a directory: {path}")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


class RootAnchor:
    def __init__(self, path: pathlib.Path):
        self.path = path
        self.fd = open_absolute_directory(path)
        self.metadata = os.fstat(self.fd)

    def close(self) -> None:
        if self.fd >= 0:
            os.close(self.fd)
            self.fd = -1

    def __enter__(self) -> RootAnchor:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def relative_parts(self, path: pathlib.Path, label: str) -> tuple[str, ...]:
        within(self.path, path, label)
        try:
            relative = path.relative_to(self.path)
        except ValueError as error:
            raise UnsafePath(f"{label} escapes the allowed root {self.path}: {path}") from error
        return relative.parts

    def assert_root_identity(self) -> None:
        reopened = open_absolute_directory(self.path)
        try:
            if not same_inode(self.metadata, os.fstat(reopened)):
                raise UnsafePath(f"release root identity changed: {self.path}")
        finally:
            os.close(reopened)

    def open_dir_parts(self, parts: Sequence[str], display: pathlib.Path) -> int:
        descriptor = os.dup(self.fd)
        current = self.path
        try:
            for component in parts:
                current /= component
                try:
                    child = os.open(component, DIR_FLAGS, dir_fd=descriptor)
                except OSError as error:
                    raise UnsafePath(
                        f"directory component is missing, symlinked, or unsafe: {current}"
                    ) from error
                os.close(descriptor)
                descriptor = child
            metadata = os.fstat(descriptor)
            if not stat.S_ISDIR(metadata.st_mode):
                raise UnsafePath(f"path is not a directory: {display}")
            return descriptor
        except BaseException:
            os.close(descriptor)
            raise

    def open_dir(self, path: pathlib.Path, label: str) -> int:
        parts = self.relative_parts(path, label)
        return self.open_dir_parts(parts, path)

    def open_parent(
        self, path: pathlib.Path, label: str
    ) -> tuple[int, str, pathlib.Path]:
        parts = self.relative_parts(path, label)
        if not parts:
            raise UnsafePath(f"{label} must name an entry below root: {path}")
        parent_path = path.parent
        return self.open_dir_parts(parts[:-1], parent_path), parts[-1], parent_path

    def assert_dir_identity(
        self, path: pathlib.Path, descriptor: int, label: str
    ) -> None:
        self.assert_root_identity()
        if path == self.path:
            reopened = open_absolute_directory(path)
        else:
            reopened = self.open_dir(path, label)
        try:
            if not same_inode(os.fstat(descriptor), os.fstat(reopened)):
                raise UnsafePath(f"{label} identity changed: {path}")
        finally:
            os.close(reopened)


def reject_xattrs(descriptor: int, path: pathlib.Path) -> None:
    try:
        names = sorted(os.listxattr(descriptor))
    except (AttributeError, OSError) as error:
        raise UnsafePath(
            f"cannot inspect release xattrs/ACLs/capabilities: {path}"
        ) from error
    if names:
        rendered = ", ".join(names)
        raise UnsafePath(
            f"release entries must have no xattrs, ACLs, or capabilities: "
            f"{path} ({rendered})"
        )


def open_regular_at(
    parent_descriptor: int,
    name: str,
    path: pathlib.Path,
    *,
    single_link: bool,
) -> tuple[int, os.stat_result]:
    try:
        metadata = os.stat(name, dir_fd=parent_descriptor, follow_symlinks=False)
    except FileNotFoundError as error:
        raise UnsafePath(f"regular file does not exist: {path}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise UnsafePath(f"path is not a no-follow regular file: {path}")
    if single_link and metadata.st_nlink != 1:
        raise UnsafePath(
            f"multiply linked release file is forbidden: {path} "
            f"(nlink={metadata.st_nlink})"
        )
    try:
        descriptor = os.open(name, FILE_FLAGS, dir_fd=parent_descriptor)
    except OSError as error:
        raise UnsafePath(f"cannot open no-follow release file: {path}") from error
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISREG(opened.st_mode)
        or not same_inode(metadata, opened)
        or (single_link and opened.st_nlink != 1)
    ):
        os.close(descriptor)
        raise UnsafePath(f"release file identity changed while opening: {path}")
    return descriptor, opened


def open_absolute_regular(
    path: pathlib.Path, *, single_link: bool
) -> tuple[int, os.stat_result]:
    parent_descriptor = open_absolute_directory(path.parent)
    try:
        return open_regular_at(
            parent_descriptor, path.name, path, single_link=single_link
        )
    finally:
        os.close(parent_descriptor)


def regular_entry_matches(
    parent_descriptor: int,
    name: str,
    path: pathlib.Path,
    expected: os.stat_result,
) -> bool:
    try:
        descriptor, current = open_regular_at(
            parent_descriptor,
            name,
            path,
            single_link=True,
        )
    except (OSError, UnsafePath):
        return False
    try:
        return same_inode(expected, current) and same_inode(
            expected, os.fstat(descriptor)
        )
    finally:
        os.close(descriptor)


def directory_entry_matches(
    parent_descriptor: int,
    name: str,
    expected: os.stat_result,
) -> bool:
    try:
        current = os.stat(
            name,
            dir_fd=parent_descriptor,
            follow_symlinks=False,
        )
    except OSError:
        return False
    if not stat.S_ISDIR(current.st_mode) or not same_inode(expected, current):
        return False
    try:
        descriptor = os.open(name, DIR_FLAGS, dir_fd=parent_descriptor)
    except OSError:
        return False
    try:
        opened = os.fstat(descriptor)
        return stat.S_ISDIR(opened.st_mode) and same_inode(expected, opened)
    finally:
        os.close(descriptor)


def sync_file(descriptor: int, path: pathlib.Path, checkpoint: str) -> None:
    test_event(checkpoint, path)
    test_event("fsync-file", path)
    os.fsync(descriptor)


def sync_directory_descriptor(
    descriptor: int, path: pathlib.Path, checkpoint: str
) -> None:
    test_event(checkpoint, path)
    test_event("fsync-dir", path)
    os.fsync(descriptor)


def ensure_directory(root: RootAnchor, path: pathlib.Path, mode: int) -> None:
    parts = root.relative_parts(path, "directory")
    root.assert_root_identity()
    descriptors = [os.dup(root.fd)]
    created: list[tuple[int, str, pathlib.Path, os.stat_result]] = []
    current_path = root.path
    try:
        for component in parts:
            child_path = current_path / component
            created_now = False
            try:
                child = os.open(component, DIR_FLAGS, dir_fd=descriptors[-1])
            except FileNotFoundError:
                root.assert_dir_identity(
                    current_path, descriptors[-1], "output directory parent"
                )
                os.mkdir(component, mode, dir_fd=descriptors[-1])
                observed_metadata = os.stat(
                    component,
                    dir_fd=descriptors[-1],
                    follow_symlinks=False,
                )
                if not stat.S_ISDIR(observed_metadata.st_mode):
                    raise UnsafePath(
                        "observed output directory entry is not a directory: "
                        f"{child_path}"
                    )
                created.append(
                    (
                        len(descriptors) - 1,
                        component,
                        child_path,
                        observed_metadata,
                    )
                )
                test_event("ensure-dir-after-observe-before-open", child_path)
                child = os.open(component, DIR_FLAGS, dir_fd=descriptors[-1])
                if not same_inode(observed_metadata, os.fstat(child)):
                    os.close(child)
                    raise UnsafePath(
                        "output directory identity changed after observation: "
                        f"{child_path}"
                    )
                created_now = True
            except OSError as error:
                raise UnsafePath(f"unsafe output directory component: {child_path}") from error
            descriptors.append(child)
            if created_now:
                sync_directory_descriptor(child, child_path, "ensure-dir-new")
                sync_directory_descriptor(
                    descriptors[-2], current_path, "ensure-dir-parent"
                )
            current_path = child_path
        root.assert_dir_identity(path, descriptors[-1], "output directory")
    except BaseException as operation_error:
        retained: list[pathlib.Path] = []
        for parent_index, component, child_path, expected in reversed(created):
            try:
                if directory_entry_matches(
                    descriptors[parent_index],
                    component,
                    expected,
                ):
                    os.rmdir(component, dir_fd=descriptors[parent_index])
                    sync_directory_descriptor(
                        descriptors[parent_index],
                        child_path.parent,
                        "ensure-dir-cleanup-parent",
                    )
                else:
                    retained.append(child_path)
            except OSError:
                retained.append(child_path)
        if retained:
            rendered = ", ".join(os.fspath(item) for item in retained)
            raise UnsafePath(
                "output directory cleanup retained entries whose identity "
                f"could not be proven: {rendered}"
            ) from operation_error
        raise
    finally:
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def private_directory(
    root: RootAnchor, parent: pathlib.Path, prefix: str
) -> tuple[pathlib.Path, os.stat_result]:
    if not prefix or pathlib.Path(prefix).name != prefix or prefix in (".", ".."):
        raise UnsafePath(f"private directory prefix is unsafe: {prefix!r}")
    parent_descriptor = root.open_dir(parent, "private directory parent")
    name = ""
    path = parent
    child = -1
    observed_metadata: os.stat_result | None = None
    try:
        root.assert_dir_identity(parent, parent_descriptor, "private directory parent")
        for _ in range(128):
            name = f"{prefix}{secrets.token_hex(8)}"
            try:
                os.mkdir(name, 0o700, dir_fd=parent_descriptor)
                break
            except FileExistsError:
                continue
        else:
            raise UnsafePath(f"cannot allocate private release directory below {parent}")
        path = parent / name
        try:
            observed_metadata = os.stat(
                name,
                dir_fd=parent_descriptor,
                follow_symlinks=False,
            )
            if not stat.S_ISDIR(observed_metadata.st_mode):
                raise UnsafePath(
                    f"observed private release entry is not a directory: {path}"
                )
            test_event("private-dir-after-observe-before-open", path)
            child = os.open(name, DIR_FLAGS, dir_fd=parent_descriptor)
            if not same_inode(observed_metadata, os.fstat(child)):
                os.close(child)
                child = -1
                raise UnsafePath(
                    f"private directory identity changed after observation: {path}"
                )
            os.fchmod(child, 0o700)
            sync_directory_descriptor(child, path, "private-dir-new")
            sync_directory_descriptor(parent_descriptor, parent, "private-dir-parent")
            if not directory_entry_matches(
                parent_descriptor,
                name,
                observed_metadata,
            ):
                raise UnsafePath(
                    f"private directory identity changed before commit: {path}"
                )
            root.assert_dir_identity(
                parent,
                parent_descriptor,
                "private directory parent",
            )
            if not directory_entry_matches(
                parent_descriptor,
                name,
                observed_metadata,
            ):
                raise UnsafePath(
                    f"private directory identity changed at commit: {path}"
                )
            return path, observed_metadata
        except BaseException as operation_error:
            if child >= 0:
                os.close(child)
                child = -1
            try:
                if observed_metadata is not None and directory_entry_matches(
                    parent_descriptor,
                    name,
                    observed_metadata,
                ):
                    os.rmdir(name, dir_fd=parent_descriptor)
                    sync_directory_descriptor(
                        parent_descriptor, parent, "private-dir-cleanup-parent"
                    )
                else:
                    raise UnsafePath(
                        "private directory cleanup retained an entry whose "
                        f"identity could not be proven: {path}"
                    ) from operation_error
            except OSError:
                raise UnsafePath(
                    "private directory cleanup retained an entry that could "
                    f"not be removed safely: {path}"
                ) from operation_error
            raise
        finally:
            if child >= 0:
                os.close(child)
    finally:
        os.close(parent_descriptor)


def validate_file(root: RootAnchor, path: pathlib.Path) -> None:
    parent_descriptor, name, _ = root.open_parent(path, "file")
    try:
        descriptor, _ = open_regular_at(
            parent_descriptor, name, path, single_link=True
        )
        try:
            reject_xattrs(descriptor, path)
        finally:
            os.close(descriptor)
    finally:
        os.close(parent_descriptor)


def safe_relative(raw: str, label: str) -> tuple[str, ...]:
    path = pathlib.PurePosixPath(raw)
    if path.is_absolute() or not path.parts or ".." in path.parts:
        raise UnsafePath(f"{label} must be a non-empty safe relative path: {raw}")
    if any(component in ("", ".", "..") for component in path.parts):
        raise UnsafePath(f"{label} contains an unsafe component: {raw}")
    return path.parts


def validate_tree_file(tree_descriptor: int, relative: str) -> None:
    try:
        root_metadata = os.fstat(tree_descriptor)
    except OSError as error:
        raise UnsafePath(f"pinned tree fd is unavailable: {tree_descriptor}") from error
    if not stat.S_ISDIR(root_metadata.st_mode):
        raise UnsafePath(f"pinned tree fd is not a directory: {tree_descriptor}")
    parts = safe_relative(relative, "pinned tree file")
    parent = os.dup(tree_descriptor)
    display = pathlib.Path(f"/proc/self/fd/{tree_descriptor}")
    try:
        for component in parts[:-1]:
            display /= component
            child = os.open(component, DIR_FLAGS, dir_fd=parent)
            os.close(parent)
            parent = child
        display /= parts[-1]
        descriptor, _ = open_regular_at(
            parent, parts[-1], display, single_link=True
        )
        try:
            reject_xattrs(descriptor, display)
        finally:
            os.close(descriptor)
    finally:
        os.close(parent)


def directory_identity(
    root: RootAnchor,
    path: pathlib.Path,
    expected_dev: int | None = None,
    expected_ino: int | None = None,
) -> tuple[int, int]:
    if (expected_dev is None) != (expected_ino is None):
        raise UnsafePath(
            "expected directory identity requires both --expected-dev "
            "and --expected-ino"
        )
    if (
        expected_dev is not None
        and expected_ino is not None
        and (expected_dev < 0 or expected_ino < 0)
    ):
        raise UnsafePath("expected directory identity values must be non-negative")
    descriptor = root.open_dir(path, "directory identity")
    try:
        root.assert_dir_identity(path, descriptor, "directory identity")
        metadata = os.fstat(descriptor)
        identity = (metadata.st_dev, metadata.st_ino)
        if (
            expected_dev is not None
            and expected_ino is not None
            and identity != (expected_dev, expected_ino)
        ):
            raise UnsafePath(
                f"directory identity does not match expected token: {path}"
            )
        return identity
    finally:
        os.close(descriptor)


def descriptor_mount_id(descriptor: int, path: pathlib.Path) -> int:
    try:
        with open(
            f"/proc/self/fdinfo/{descriptor}",
            encoding="utf-8",
        ) as descriptor_info:
            for line in descriptor_info:
                key, separator, value = line.partition(":")
                if key == "mnt_id" and separator:
                    return int(value.strip())
    except (OSError, ValueError) as error:
        raise UnsafePath(
            f"cannot establish cleanup mount identity: {path}"
        ) from error
    raise UnsafePath(f"cleanup mount identity is unavailable: {path}")


def cleanup_metadata_matches(
    expected: os.stat_result,
    actual: os.stat_result,
    *,
    allow_directory_mutation: bool,
) -> bool:
    stable = (
        same_inode(expected, actual)
        and expected.st_mode == actual.st_mode
        and expected.st_uid == actual.st_uid
        and expected.st_gid == actual.st_gid
        and expected.st_rdev == actual.st_rdev
    )
    if not stable:
        return False
    if allow_directory_mutation and stat.S_ISDIR(expected.st_mode):
        return True
    return (
        expected.st_nlink == actual.st_nlink
        and expected.st_size == actual.st_size
        and expected.st_mtime_ns == actual.st_mtime_ns
        and expected.st_ctime_ns == actual.st_ctime_ns
    )


def cleanup_file_digest(
    descriptor: int,
    metadata: os.stat_result,
    path: pathlib.Path,
) -> str:
    os.lseek(descriptor, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    while True:
        chunk = os.read(descriptor, 1024 * 1024)
        if not chunk:
            break
        digest.update(chunk)
    if not cleanup_metadata_matches(
        metadata,
        os.fstat(descriptor),
        allow_directory_mutation=False,
    ):
        raise UnsafePath(
            f"cleanup file changed while authorizing deletion: {path}"
        )
    return digest.hexdigest()


@dataclass
class CleanupEntry:
    relative: tuple[str, ...]
    display: pathlib.Path
    kind: str
    descriptor: int
    metadata: os.stat_result
    mount_id: int
    digest: str | None = None
    link_target: str | None = None


class CleanupAuthority:
    """Pinned exact authority for entries that remove-tree may delete."""

    def __init__(
        self,
        path: pathlib.Path,
        entries: dict[tuple[str, ...], CleanupEntry],
    ):
        self.path = path
        self.entries = entries
        children: dict[tuple[str, ...], list[str]] = {}
        for relative in entries:
            children.setdefault(relative, [])
            if relative:
                children.setdefault(relative[:-1], []).append(relative[-1])
        self.children = {
            relative: tuple(sorted(names))
            for relative, names in children.items()
        }

    @classmethod
    def capture(
        cls,
        directory_descriptor: int,
        path: pathlib.Path,
    ) -> CleanupAuthority:
        entries: dict[tuple[str, ...], CleanupEntry] = {}
        root_copy = os.dup(directory_descriptor)
        root_device = os.fstat(root_copy).st_dev
        root_mount = descriptor_mount_id(root_copy, path)
        path_only = getattr(os, "O_PATH", None)
        if not isinstance(path_only, int):
            os.close(root_copy)
            raise primitive_failure(
                "open(O_PATH|O_NOFOLLOW)",
                errno.ENOSYS,
                context="while authorizing cleanup symlinks",
            )

        def visit_directory(
            descriptor: int,
            relative: tuple[str, ...],
            display: pathlib.Path,
        ) -> None:
            os.fchmod(descriptor, 0o700)
            metadata = os.fstat(descriptor)
            mount_id = descriptor_mount_id(descriptor, display)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_dev != root_device
                or mount_id != root_mount
            ):
                raise UnsafePath(
                    f"cleanup directory crosses its authorized mount: {display}"
                )
            entries[relative] = CleanupEntry(
                relative,
                display,
                "directory",
                descriptor,
                metadata,
                mount_id,
            )
            for name in sorted(os.listdir(descriptor)):
                child_relative = (*relative, name)
                child_path = display / name
                metadata = os.stat(
                    name,
                    dir_fd=descriptor,
                    follow_symlinks=False,
                )
                if stat.S_ISDIR(metadata.st_mode):
                    child = os.open(name, DIR_FLAGS, dir_fd=descriptor)
                    try:
                        opened = os.fstat(child)
                        if not same_inode(metadata, opened):
                            raise UnsafePath(
                                "cleanup directory identity changed while "
                                f"authorizing deletion: {child_path}"
                            )
                        visit_directory(child, child_relative, child_path)
                    except BaseException:
                        if child_relative not in entries:
                            os.close(child)
                        raise
                elif stat.S_ISREG(metadata.st_mode):
                    child, opened = open_regular_at(
                        descriptor,
                        name,
                        child_path,
                        single_link=False,
                    )
                    try:
                        mount_id = descriptor_mount_id(child, child_path)
                        if opened.st_dev != root_device or mount_id != root_mount:
                            raise UnsafePath(
                                "cleanup file crosses its authorized mount: "
                                f"{child_path}"
                            )
                        digest = cleanup_file_digest(child, opened, child_path)
                        entries[child_relative] = CleanupEntry(
                            child_relative,
                            child_path,
                            "file",
                            child,
                            opened,
                            mount_id,
                            digest=digest,
                        )
                    except BaseException:
                        if child_relative not in entries:
                            os.close(child)
                        raise
                elif stat.S_ISLNK(metadata.st_mode):
                    flags = (
                        path_only
                        | getattr(os, "O_CLOEXEC", 0)
                        | getattr(os, "O_NOFOLLOW", 0)
                    )
                    child = os.open(name, flags, dir_fd=descriptor)
                    try:
                        opened = os.fstat(child)
                        mount_id = descriptor_mount_id(child, child_path)
                        if (
                            not stat.S_ISLNK(opened.st_mode)
                            or not same_inode(metadata, opened)
                            or opened.st_dev != root_device
                            or mount_id != root_mount
                        ):
                            raise UnsafePath(
                                "cleanup symlink identity or mount changed while "
                                f"authorizing deletion: {child_path}"
                            )
                        entries[child_relative] = CleanupEntry(
                            child_relative,
                            child_path,
                            "symlink",
                            child,
                            opened,
                            mount_id,
                            link_target=os.readlink(
                                name,
                                dir_fd=descriptor,
                            ),
                        )
                    except BaseException:
                        if child_relative not in entries:
                            os.close(child)
                        raise
                else:
                    raise UnsafePath(
                        "cleanup authority rejects non-file, non-directory, "
                        f"non-symlink entry: {child_path}"
                    )

        try:
            visit_directory(root_copy, (), path)
            authority = cls(path, entries)
            authority.verify_exact()
            return authority
        except BaseException:
            for entry in reversed(list(entries.values())):
                try:
                    os.close(entry.descriptor)
                except OSError:
                    pass
            if () not in entries:
                os.close(root_copy)
            raise

    @property
    def root(self) -> CleanupEntry:
        return self.entries[()]

    def close(self) -> None:
        for entry in reversed(list(self.entries.values())):
            os.close(entry.descriptor)
        self.entries.clear()

    def _verify_pinned(
        self,
        entry: CleanupEntry,
        *,
        allow_directory_mutation: bool,
    ) -> None:
        metadata = os.fstat(entry.descriptor)
        if (
            not cleanup_metadata_matches(
                entry.metadata,
                metadata,
                allow_directory_mutation=allow_directory_mutation,
            )
            or descriptor_mount_id(entry.descriptor, entry.display)
            != entry.mount_id
        ):
            raise UnsafePath(
                f"cleanup authority inode or mount drifted: {entry.display}"
            )
        if entry.kind == "file":
            assert entry.digest is not None
            if (
                cleanup_file_digest(
                    entry.descriptor,
                    entry.metadata,
                    entry.display,
                )
                != entry.digest
            ):
                raise UnsafePath(
                    f"cleanup authority digest drifted: {entry.display}"
                )

    def _verify_public_entry(
        self,
        relative: tuple[str, ...],
        *,
        allow_directory_mutation: bool,
    ) -> None:
        entry = self.entries[relative]
        parent = self.entries[relative[:-1]]
        current = os.stat(
            relative[-1],
            dir_fd=parent.descriptor,
            follow_symlinks=False,
        )
        if not cleanup_metadata_matches(
            entry.metadata,
            current,
            allow_directory_mutation=allow_directory_mutation,
        ):
            raise UnsafePath(
                f"cleanup authority public entry drifted: {entry.display}"
            )
        self._verify_pinned(
            entry,
            allow_directory_mutation=allow_directory_mutation,
        )
        if entry.kind == "symlink":
            current_target = os.readlink(
                relative[-1],
                dir_fd=parent.descriptor,
            )
            if current_target != entry.link_target:
                raise UnsafePath(
                    f"cleanup authority symlink target drifted: {entry.display}"
                )

    def verify_exact(self) -> None:
        for relative, entry in self.entries.items():
            self._verify_pinned(
                entry,
                allow_directory_mutation=False,
            )
            if relative:
                self._verify_public_entry(
                    relative,
                    allow_directory_mutation=False,
                )
            if entry.kind == "directory":
                current_names = tuple(sorted(os.listdir(entry.descriptor)))
                if current_names != self.children[relative]:
                    raise UnsafePath(
                        f"cleanup authority entry set drifted: {entry.display}"
                    )

    def delete_authorized_entries(self) -> None:
        self.verify_exact()

        def clear(relative: tuple[str, ...]) -> None:
            directory = self.entries[relative]
            current_names = tuple(sorted(os.listdir(directory.descriptor)))
            if current_names != self.children[relative]:
                raise UnsafePath(
                    "cleanup tree contains an entry absent from the exact "
                    f"authority token: {directory.display}"
                )
            for name in self.children[relative]:
                child_relative = (*relative, name)
                child = self.entries[child_relative]
                if child.kind == "directory":
                    self._verify_public_entry(
                        child_relative,
                        allow_directory_mutation=False,
                    )
                    clear(child_relative)
                    self._verify_public_entry(
                        child_relative,
                        allow_directory_mutation=True,
                    )
                    if os.listdir(child.descriptor):
                        raise UnsafePath(
                            "cleanup directory gained an unauthorized entry: "
                            f"{child.display}"
                        )
                    os.rmdir(name, dir_fd=directory.descriptor)
                else:
                    self._verify_public_entry(
                        child_relative,
                        allow_directory_mutation=False,
                    )
                    os.unlink(name, dir_fd=directory.descriptor)
            if os.listdir(directory.descriptor):
                raise UnsafePath(
                    "cleanup directory gained an unauthorized entry: "
                    f"{directory.display}"
                )
            sync_directory_descriptor(
                directory.descriptor,
                directory.display,
                "remove-tree-directory",
            )

        clear(())


def remove_tree(
    root: RootAnchor,
    path: pathlib.Path,
    expected_dev: int,
    expected_ino: int,
) -> None:
    parent, name, parent_path = root.open_parent(path, "cleanup tree")
    descriptor = -1
    authority: CleanupAuthority | None = None

    try:
        descriptor = os.open(name, DIR_FLAGS, dir_fd=parent)
        metadata = os.fstat(descriptor)
        if (metadata.st_dev, metadata.st_ino) != (expected_dev, expected_ino):
            raise UnsafePath(
                "cleanup tree identity does not match expected identity "
                f"observation token: {path}"
            )
        root.assert_dir_identity(path, descriptor, "cleanup tree")
        try:
            os.stat(RETENTION_MARKER, dir_fd=descriptor, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise UnsafePath(
                f"cleanup tree is retained after an unsafe publish rollback: {path}"
            )
        authority = CleanupAuthority.capture(descriptor, path)
        test_event("remove-tree-before-delete", path)
        authority.delete_authorized_entries()
        root.assert_dir_identity(parent_path, parent, "cleanup tree parent")
        current = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if not same_inode(current, metadata):
            raise UnsafePath(f"cleanup tree public entry was replaced: {path}")
        os.rmdir(name, dir_fd=parent)
        sync_directory_descriptor(parent, parent_path, "remove-tree-parent")
    finally:
        if authority is not None:
            authority.close()
        if descriptor >= 0:
            os.close(descriptor)
        os.close(parent)


def source_metadata_stable(
    before: os.stat_result, after: os.stat_result, *, allow_ctime_change: bool = False
) -> bool:
    return (
        same_inode(before, after)
        and after.st_nlink == before.st_nlink == 1
        and after.st_mode == before.st_mode
        and after.st_size == before.st_size
        and after.st_mtime_ns == before.st_mtime_ns
        and (allow_ctime_change or after.st_ctime_ns == before.st_ctime_ns)
    )


def mark_directory_for_retention(
    directory: int,
    path: pathlib.Path,
    reason: str,
) -> None:
    document = f"kanban-release-retain-v1\t{reason}\n".encode()
    marker_path = path / RETENTION_MARKER
    try:
        descriptor = os.open(
            RETENTION_MARKER,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o400,
            dir_fd=directory,
        )
    except FileExistsError:
        return
    try:
        view = memoryview(document)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fchmod(descriptor, 0o400)
        reject_xattrs(descriptor, marker_path)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.fsync(directory)


def copy_file(
    root: RootAnchor,
    source: pathlib.Path,
    destination: pathlib.Path,
    mode: int,
) -> None:
    destination_parent, destination_name, destination_parent_path = root.open_parent(
        destination, "copy destination"
    )
    source_descriptor, source_metadata = open_absolute_regular(
        source, single_link=True
    )
    destination_descriptor = -1
    destination_metadata: os.stat_result | None = None
    created = False
    try:
        reject_xattrs(source_descriptor, source)
        root.assert_dir_identity(
            destination_parent_path,
            destination_parent,
            "copy destination parent",
        )
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        destination_descriptor = os.open(
            destination_name, flags, 0o600, dir_fd=destination_parent
        )
        created = True
        destination_metadata = os.fstat(destination_descriptor)
        if (
            not stat.S_ISREG(destination_metadata.st_mode)
            or destination_metadata.st_nlink != 1
        ):
            raise UnsafePath(
                f"new copy destination is not a single-link regular file: {destination}"
            )
        while True:
            chunk = os.read(source_descriptor, 1024 * 1024)
            if not chunk:
                break
            view = memoryview(chunk)
            while view:
                written = os.write(destination_descriptor, view)
                view = view[written:]
        os.fchmod(destination_descriptor, mode)
        reject_xattrs(destination_descriptor, destination)
        sync_file(destination_descriptor, destination, "copy-file-data")
        if not source_metadata_stable(source_metadata, os.fstat(source_descriptor)):
            raise UnsafePath(
                f"release source changed while creating stable copy: {source}"
            )
        reject_xattrs(source_descriptor, source)
        root.assert_dir_identity(
            destination_parent_path,
            destination_parent,
            "copy destination parent",
        )
        if not regular_entry_matches(
            destination_parent,
            destination_name,
            destination,
            destination_metadata,
        ):
            raise UnsafePath(
                f"copy destination identity changed before commit: {destination}"
            )
        sync_directory_descriptor(
            destination_parent, destination_parent_path, "copy-file-parent"
        )
        root.assert_dir_identity(
            destination_parent_path,
            destination_parent,
            "copy destination parent",
        )
        if not regular_entry_matches(
            destination_parent,
            destination_name,
            destination,
            destination_metadata,
        ):
            raise UnsafePath(
                f"copy destination identity changed at commit: {destination}"
            )
    except BaseException as operation_error:
        if destination_descriptor >= 0:
            os.close(destination_descriptor)
            destination_descriptor = -1
        retained = False
        if created:
            try:
                assert destination_metadata is not None
                if regular_entry_matches(
                    destination_parent,
                    destination_name,
                    destination,
                    destination_metadata,
                ):
                    os.unlink(destination_name, dir_fd=destination_parent)
                    sync_directory_descriptor(
                        destination_parent,
                        destination_parent_path,
                        "copy-file-cleanup-parent",
                    )
                else:
                    retained = True
            except OSError:
                retained = True
        if retained:
            raise UnsafePath(
                "copy cleanup retained the owned file and unknown public "
                f"destination after identity drift: {destination}"
            ) from operation_error
        raise
    finally:
        os.close(source_descriptor)
        if destination_descriptor >= 0:
            os.close(destination_descriptor)
        os.close(destination_parent)


def injected_errno(variable: str) -> int | None:
    raw = os.environ.get(variable)
    if raw is None:
        return None
    if raw != "ENOSYS":
        raise UnsafePath(
            f"{variable} only accepts the symbolic errno ENOSYS"
        )
    return errno.ENOSYS


def primitive_failure(
    primitive: str,
    error_number: int,
    *,
    context: str = "",
) -> UnsafePath:
    suffix = f" {context}" if context else ""
    if error_number == errno.ENOSYS:
        return UnsafePath(
            f"required Linux primitive unavailable: {primitive}: "
            f"ENOSYS ({os.strerror(errno.ENOSYS)}); no fallback{suffix}"
        )
    return UnsafePath(
        f"{primitive} failed: errno={error_number} "
        f"({os.strerror(error_number)}){suffix}"
    )


def required_linux_constant(
    module: object,
    name: str,
    primitive: str,
    *,
    context: str = "",
    missing_fixture: str | None = None,
) -> int:
    if (
        missing_fixture is not None
        and os.environ.get(missing_fixture) == "1"
    ):
        value = None
    else:
        value = getattr(module, name, None)
    if not isinstance(value, int):
        raise primitive_failure(
            primitive,
            errno.ENOSYS,
            context=context,
        )
    return value


def load_renameat2(primitive: str) -> ctypes._CFuncPtr:
    libc = ctypes.CDLL(None, use_errno=True)
    renameat2 = getattr(libc, "renameat2", None)
    if (
        renameat2 is None
        or os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_RENAMEAT2_MISSING")
        == "1"
    ):
        raise UnsafePath(
            f"required Linux primitive unavailable: {primitive}: "
            "symbol missing; no fallback"
        )
    renameat2.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    renameat2.restype = ctypes.c_int
    return renameat2


def invoke_renameat2(
    renameat2: ctypes._CFuncPtr,
    source_parent: int,
    source_name: str,
    destination_parent: int,
    destination_name: str,
    flags: int,
) -> tuple[int, int]:
    forced = injected_errno("KANBAN_RELEASE_SAFE_PATH_TEST_RENAMEAT2_ERRNO")
    if forced is not None:
        ctypes.set_errno(forced)
        result = -1
    else:
        ctypes.set_errno(0)
        result = renameat2(
            source_parent,
            os.fsencode(source_name),
            destination_parent,
            os.fsencode(destination_name),
            flags,
        )
    return result, ctypes.get_errno()


def rename_noreplace(
    source_parent: int,
    source_name: str,
    destination_parent: int,
    destination_name: str,
    label: str,
    destination_display: pathlib.Path,
) -> None:
    primitive = "renameat2(RENAME_NOREPLACE)"
    renameat2 = load_renameat2(primitive)
    result, error_number = invoke_renameat2(
        renameat2,
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        RENAME_NOREPLACE,
    )
    if result == 0:
        return
    if error_number in (errno.EEXIST, errno.ENOTEMPTY):
        raise UnsafePath(f"{label} destination already exists: {destination_display}")
    raise primitive_failure(
        primitive,
        error_number,
        context=f"while {label}",
    )


def rename_exchange(
    source_parent: int,
    source_name: str,
    destination_parent: int,
    destination_name: str,
    label: str,
) -> None:
    primitive = "renameat2(RENAME_EXCHANGE)"
    renameat2 = load_renameat2(primitive)
    result, error_number = invoke_renameat2(
        renameat2,
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        RENAME_EXCHANGE,
    )
    if result == 0:
        return
    raise primitive_failure(
        primitive,
        error_number,
        context=f"while {label}",
    )


def sync_rename_parents(
    source_parent: int,
    source_path: pathlib.Path,
    destination_parent: int,
    destination_path: pathlib.Path,
    checkpoint: str,
) -> None:
    sync_directory_descriptor(destination_parent, destination_path, checkpoint)
    if not same_inode(os.fstat(source_parent), os.fstat(destination_parent)):
        sync_directory_descriptor(source_parent, source_path, checkpoint)


def _lease_signal(_signal_number: int, _frame: object) -> None:
    if _ACTIVE_LEASE_SET is not None:
        _ACTIVE_LEASE_SET.broken = True


class LeaseSet:
    def __init__(self, files: Sequence[tuple[int, pathlib.Path]]):
        self.files = list(files)
        self.acquired: list[tuple[int, pathlib.Path]] = []
        self.broken = False
        self.setlease: int | None = None
        self.unlock: int | None = None

    def __enter__(self) -> LeaseSet:
        global _ACTIVE_LEASE_SET
        if _ACTIVE_LEASE_SET is not None:
            raise UnsafePath("nested release lease sets are forbidden")
        if not self.files:
            raise UnsafePath(
                "release publication requires at least one leased regular file"
            )
        sigio = required_linux_constant(
            signal,
            "SIGIO",
            "signal(SIGIO)",
        )
        try:
            signal.signal(sigio, _lease_signal)
        except OSError as error:
            raise primitive_failure(
                "signal(SIGIO)",
                error.errno if error.errno is not None else errno.EIO,
            ) from error
        _ACTIVE_LEASE_SET = self
        try:
            for descriptor, path in self.files:
                try:
                    setown = required_linux_constant(
                        fcntl,
                        "F_SETOWN",
                        "fcntl(F_SETOWN)",
                        context=f"while leasing {path}",
                    )
                    self.setlease = required_linux_constant(
                        fcntl,
                        "F_SETLEASE",
                        "fcntl(F_SETLEASE)",
                        context=f"while leasing {path}",
                        missing_fixture=(
                            "KANBAN_RELEASE_SAFE_PATH_TEST_SETLEASE_MISSING"
                        ),
                    )
                    read_lock = required_linux_constant(
                        fcntl,
                        "F_RDLCK",
                        "fcntl(F_SETLEASE)",
                        context=f"while leasing {path}",
                    )
                    self.unlock = required_linux_constant(
                        fcntl,
                        "F_UNLCK",
                        "fcntl(F_SETLEASE)",
                        context=f"while leasing {path}",
                    )
                    fcntl.fcntl(descriptor, setown, os.getpid())
                    forced_errno = injected_errno(
                        "KANBAN_RELEASE_SAFE_PATH_TEST_SETLEASE_ERRNO"
                    )
                    if forced_errno is not None:
                        raise OSError(
                            forced_errno,
                            os.strerror(forced_errno),
                        )
                    fcntl.fcntl(
                        descriptor,
                        self.setlease,
                        read_lock,
                    )
                except OSError as error:
                    raise primitive_failure(
                        "fcntl(F_SETLEASE)",
                        error.errno if error.errno is not None else errno.EIO,
                        context=f"while leasing {path}",
                    ) from error
                self.acquired.append((descriptor, path))
            self.check()
            return self
        except BaseException:
            self.release()
            raise

    def check_fast(self) -> None:
        if self.broken:
            raise UnsafePath(
                "release snapshot write lease break requested (SIGIO); "
                "publication aborted"
            )

    def check(self) -> None:
        self.check_fast()
        for descriptor, path in self.acquired:
            try:
                getlease = required_linux_constant(
                    fcntl,
                    "F_GETLEASE",
                    "fcntl(F_GETLEASE)",
                    context=f"while inspecting lease for {path}",
                )
                read_lock = required_linux_constant(
                    fcntl,
                    "F_RDLCK",
                    "fcntl(F_GETLEASE)",
                    context=f"while inspecting lease for {path}",
                )
                forced_errno = injected_errno(
                    "KANBAN_RELEASE_SAFE_PATH_TEST_GETLEASE_ERRNO"
                )
                if forced_errno is not None:
                    raise OSError(
                        forced_errno,
                        os.strerror(forced_errno),
                    )
                state = fcntl.fcntl(descriptor, getlease)
            except OSError as error:
                raise primitive_failure(
                    "fcntl(F_GETLEASE)",
                    error.errno if error.errno is not None else errno.EIO,
                    context=f"while inspecting lease for {path}",
                ) from error
            if state != read_lock:
                self.broken = True
                raise UnsafePath(
                    f"release snapshot write conflict detected for {path}"
                )

    def release(self) -> None:
        global _ACTIVE_LEASE_SET
        for descriptor, _ in reversed(self.acquired):
            try:
                if self.setlease is not None and self.unlock is not None:
                    fcntl.fcntl(descriptor, self.setlease, self.unlock)
            except OSError:
                pass
        self.acquired.clear()
        if _ACTIVE_LEASE_SET is self:
            _ACTIVE_LEASE_SET = None

    def __exit__(self, *_: object) -> None:
        self.release()


def committed_process_exit() -> None:
    """Exit without unwinding so leases and pinned fds live to kernel teardown."""

    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(0)


@dataclass
class SnapshotEntry:
    relative: tuple[str, ...]
    display: pathlib.Path
    kind: str
    descriptor: int
    metadata: os.stat_result


def metadata_matches_snapshot(
    expected: SnapshotEntry,
    actual: os.stat_result,
    *,
    allow_root_ctime_change: bool,
) -> bool:
    before = expected.metadata
    common = (
        same_inode(before, actual)
        and before.st_mode == actual.st_mode
        and before.st_nlink == actual.st_nlink
        and before.st_mtime_ns == actual.st_mtime_ns
    )
    if not common:
        return False
    if not (
        allow_root_ctime_change and expected.kind == "directory" and not expected.relative
    ) and before.st_ctime_ns != actual.st_ctime_ns:
        return False
    if expected.kind == "file" and before.st_size != actual.st_size:
        return False
    return True


class TreeSnapshot:
    def __init__(self, path: pathlib.Path, entries: dict[tuple[str, ...], SnapshotEntry]):
        self.path = path
        self.entries = entries

    @classmethod
    def capture(
        cls,
        directory_descriptor: int,
        path: pathlib.Path,
        *,
        require_sealed: bool,
    ) -> TreeSnapshot:
        entries: dict[tuple[str, ...], SnapshotEntry] = {}
        root_device = os.fstat(directory_descriptor).st_dev

        def visit(
            descriptor: int,
            relative: tuple[str, ...],
            display: pathlib.Path,
        ) -> None:
            metadata = os.fstat(descriptor)
            if not stat.S_ISDIR(metadata.st_mode):
                raise UnsafePath(f"unsafe directory in release generation: {display}")
            if metadata.st_dev != root_device:
                raise UnsafePath(f"cross-filesystem release tree entry is forbidden: {display}")
            if require_sealed and metadata.st_mode & 0o222:
                raise UnsafePath(f"release generation directory is not sealed: {display}")
            reject_xattrs(descriptor, display)
            entries[relative] = SnapshotEntry(
                relative, display, "directory", descriptor, metadata
            )
            try:
                names = sorted(os.listdir(descriptor))
            except OSError as error:
                raise UnsafePath(f"cannot enumerate release directory: {display}") from error
            for name in names:
                candidate = display / name
                candidate_relative = (*relative, name)
                metadata = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                if stat.S_ISDIR(metadata.st_mode):
                    child = os.open(name, DIR_FLAGS, dir_fd=descriptor)
                    try:
                        opened = os.fstat(child)
                        if not same_inode(metadata, opened):
                            raise UnsafePath(
                                "release directory identity changed while opening: "
                                f"{candidate}"
                            )
                        visit(child, candidate_relative, candidate)
                    except BaseException:
                        if candidate_relative not in entries:
                            os.close(child)
                        raise
                elif stat.S_ISREG(metadata.st_mode):
                    child, opened = open_regular_at(
                        descriptor, name, candidate, single_link=True
                    )
                    try:
                        if opened.st_dev != root_device:
                            raise UnsafePath(
                                "cross-filesystem release tree entry is forbidden: "
                                f"{candidate}"
                            )
                        if require_sealed and opened.st_mode & 0o222:
                            raise UnsafePath(
                                f"release generation file is not sealed: {candidate}"
                            )
                        reject_xattrs(child, candidate)
                        entries[candidate_relative] = SnapshotEntry(
                            candidate_relative, candidate, "file", child, opened
                        )
                    except BaseException:
                        if candidate_relative not in entries:
                            os.close(child)
                        raise
                else:
                    raise UnsafePath(
                        f"non-regular release tree entry is forbidden: {candidate}"
                    )

        root_copy = os.dup(directory_descriptor)
        try:
            visit(root_copy, (), path)
            return cls(path, entries)
        except BaseException:
            for entry in entries.values():
                try:
                    os.close(entry.descriptor)
                except OSError:
                    pass
            if () not in entries:
                os.close(root_copy)
            raise

    @property
    def root(self) -> SnapshotEntry:
        return self.entries[()]

    @property
    def leased_files(self) -> list[tuple[int, pathlib.Path]]:
        return [
            (entry.descriptor, entry.display)
            for entry in self.entries.values()
            if entry.kind == "file"
        ]

    def close(self) -> None:
        for entry in reversed(list(self.entries.values())):
            os.close(entry.descriptor)
        self.entries.clear()

    def verify(self, *, allow_root_ctime_change: bool) -> None:
        current = TreeSnapshot.capture(
            self.root.descriptor, self.path, require_sealed=True
        )
        try:
            if set(current.entries) != set(self.entries):
                raise UnsafePath(
                    "release snapshot entry set changed during publication"
                )
            for relative, expected in self.entries.items():
                actual_entry = current.entries[relative]
                if actual_entry.kind != expected.kind or not metadata_matches_snapshot(
                    expected,
                    actual_entry.metadata,
                    allow_root_ctime_change=allow_root_ctime_change,
                ):
                    raise UnsafePath(
                        f"release snapshot identity or metadata changed: {expected.display}"
                    )
                pinned = os.fstat(expected.descriptor)
                if not metadata_matches_snapshot(
                    expected,
                    pinned,
                    allow_root_ctime_change=allow_root_ctime_change,
                ):
                    raise UnsafePath(
                        f"pinned release snapshot inode changed: {expected.display}"
                    )
                reject_xattrs(expected.descriptor, expected.display)
        finally:
            current.close()

    def digest(
        self,
        lease_set: LeaseSet | None,
        *,
        allow_root_ctime_change: bool,
    ) -> str:
        digest = hashlib.sha256()
        for relative in sorted(self.entries):
            entry = self.entries[relative]
            metadata = os.fstat(entry.descriptor)
            if not metadata_matches_snapshot(
                entry,
                metadata,
                allow_root_ctime_change=allow_root_ctime_change,
            ):
                raise UnsafePath(
                    f"release snapshot changed while hashing: {entry.display}"
                )
            reject_xattrs(entry.descriptor, entry.display)
            relative_text = "/".join(relative)
            mode = stat.S_IMODE(metadata.st_mode)
            if entry.kind == "directory":
                digest.update(f"D\0{relative_text}\0{mode:04o}\0".encode())
                continue
            os.lseek(entry.descriptor, 0, os.SEEK_SET)
            content = hashlib.sha256()
            while True:
                chunk = os.read(entry.descriptor, 1024 * 1024)
                if not chunk:
                    break
                content.update(chunk)
                if lease_set is not None:
                    lease_set.check_fast()
            if lease_set is not None:
                lease_set.check()
            final = os.fstat(entry.descriptor)
            if not metadata_matches_snapshot(
                entry,
                final,
                allow_root_ctime_change=allow_root_ctime_change,
            ):
                raise UnsafePath(
                    f"release file changed while hashing: {entry.display}"
                )
            reject_xattrs(entry.descriptor, entry.display)
            digest.update(
                (
                    f"F\0{relative_text}\0{mode:04o}\0{metadata.st_size}\0"
                    f"{content.hexdigest()}\0"
                ).encode()
            )
        if lease_set is not None:
            lease_set.check()
        return digest.hexdigest()


def sync_snapshot(snapshot: TreeSnapshot) -> None:
    files = sorted(
        (entry for entry in snapshot.entries.values() if entry.kind == "file"),
        key=lambda entry: entry.relative,
    )
    directories = sorted(
        (entry for entry in snapshot.entries.values() if entry.kind == "directory"),
        key=lambda entry: (len(entry.relative), entry.relative),
        reverse=True,
    )
    for entry in files:
        sync_file(entry.descriptor, entry.display, "durable-tree-file")
    for entry in directories:
        sync_directory_descriptor(
            entry.descriptor, entry.display, "durable-tree-directory"
        )


def tree_digest(
    root: RootAnchor, path: pathlib.Path, *, require_sealed: bool
) -> str:
    descriptor = root.open_dir(path, "tree")
    try:
        root.assert_dir_identity(path, descriptor, "release tree")
        snapshot = TreeSnapshot.capture(
            descriptor, path, require_sealed=require_sealed
        )
        try:
            value = snapshot.digest(None, allow_root_ctime_change=False)
            snapshot.verify(allow_root_ctime_change=False)
        finally:
            snapshot.close()
        root.assert_dir_identity(path, descriptor, "release tree")
        return value
    finally:
        os.close(descriptor)


def seal_tree(root: RootAnchor, path: pathlib.Path) -> None:
    descriptor = root.open_dir(path, "tree")
    try:
        root.assert_dir_identity(path, descriptor, "release tree")
        snapshot = TreeSnapshot.capture(descriptor, path, require_sealed=False)
        try:
            files = sorted(
                (entry for entry in snapshot.entries.values() if entry.kind == "file"),
                key=lambda entry: entry.relative,
            )
            directories = sorted(
                (
                    entry
                    for entry in snapshot.entries.values()
                    if entry.kind == "directory"
                ),
                key=lambda entry: (len(entry.relative), entry.relative),
                reverse=True,
            )
            for entry in files:
                executable = bool(entry.metadata.st_mode & 0o111)
                os.fchmod(entry.descriptor, 0o555 if executable else 0o444)
                reject_xattrs(entry.descriptor, entry.display)
                sync_file(entry.descriptor, entry.display, "seal-file")
            for entry in directories:
                os.fchmod(entry.descriptor, 0o555)
                reject_xattrs(entry.descriptor, entry.display)
                sync_directory_descriptor(
                    entry.descriptor, entry.display, "seal-directory"
                )
        finally:
            snapshot.close()
        sealed = TreeSnapshot.capture(descriptor, path, require_sealed=True)
        sealed.close()
        root.assert_dir_identity(path, descriptor, "release tree")
    finally:
        os.close(descriptor)


def inherited_build_lock_fd() -> int | None:
    """Return and exclusively acquire the inherited Cargo lock descriptor.

    The release shell wrappers prove the descriptor identity before invoking
    the safe path helper.  Re-acquire the open file description here as an
    independent authority check: an inherited descriptor held by the wrapper
    succeeds, an unlocked same-inode spoof may be acquired safely, and a
    competing holder fails before any filesystem mutation.
    """

    if os.environ.get("KANBAN_CARGO_BUILD_LOCK_HELD") != "1":
        return None

    target_raw = os.environ.get("CARGO_TARGET_DIR")
    lock_raw = os.environ.get("KANBAN_CARGO_BUILD_LOCK_PATH")
    fd_raw = os.environ.get("KANBAN_CARGO_BUILD_LOCK_FD")
    if not target_raw or not lock_raw or not fd_raw:
        raise UnsafePath("inherited Cargo build lock proof is incomplete")
    target = pathlib.Path(target_raw)
    if (
        not target.is_absolute()
        or ".." in target.parts
        or os.path.normpath(target_raw) != target_raw
    ):
        raise UnsafePath("inherited Cargo target directory is not canonical")
    expected_lock = os.fspath(target / ".build.lock")
    if lock_raw != expected_lock:
        raise UnsafePath("inherited Cargo build lock path is not canonical")
    if not fd_raw.isascii() or not fd_raw.isdecimal() or int(fd_raw) < 3:
        raise UnsafePath("inherited Cargo build lock descriptor is invalid")
    lock_fd = int(fd_raw)

    try:
        descriptor_metadata = os.fstat(lock_fd)
    except OSError as error:
        raise UnsafePath(
            "inherited Cargo build lock descriptor is not open"
        ) from error
    if (
        not stat.S_ISREG(descriptor_metadata.st_mode)
        or descriptor_metadata.st_nlink != 1
    ):
        raise UnsafePath(
            "inherited Cargo build lock descriptor is not a single-linked regular file"
        )

    try:
        lock_metadata = os.lstat(lock_raw)
    except OSError as error:
        raise UnsafePath(
            "inherited Cargo build lock path is unavailable"
        ) from error
    if (
        stat.S_ISLNK(lock_metadata.st_mode)
        or not stat.S_ISREG(lock_metadata.st_mode)
        or lock_metadata.st_nlink != 1
    ):
        raise UnsafePath(
            "inherited Cargo build lock path is not a single-linked regular file"
        )
    if not same_inode(lock_metadata, descriptor_metadata):
        raise UnsafePath("inherited Cargo build lock descriptor identity changed")

    proc_fd_path = f"/proc/self/fd/{lock_fd}"
    try:
        proc_metadata = os.stat(proc_fd_path)
    except OSError as error:
        raise UnsafePath(
            "inherited Cargo build lock descriptor is not observable"
        ) from error
    if not same_inode(lock_metadata, proc_metadata):
        raise UnsafePath("inherited Cargo build lock descriptor identity changed")
    test_event("inherited-lock-before-flock", pathlib.Path(lock_raw))
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as error:
        raise UnsafePath(
            "inherited Cargo build lock is held by another process"
        ) from error
    except OSError as error:
        raise UnsafePath("inherited Cargo build lock could not be acquired") from error

    try:
        descriptor_after = os.fstat(lock_fd)
        lock_after = os.lstat(lock_raw)
        proc_after = os.stat(proc_fd_path)
    except OSError as error:
        raise UnsafePath(
            "inherited Cargo build lock identity changed after acquisition"
        ) from error
    for label, metadata in (
        ("descriptor", descriptor_after),
        ("path", lock_after),
        ("proc descriptor", proc_after),
    ):
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            raise UnsafePath(
                "inherited Cargo build lock "
                f"{label} is not a single-linked regular file after acquisition"
            )
    if (
        not same_inode(lock_after, lock_metadata)
        or not same_inode(descriptor_after, descriptor_metadata)
        or not same_inode(proc_after, proc_metadata)
        or not same_inode(lock_after, descriptor_after)
        or not same_inode(lock_after, proc_after)
    ):
        raise UnsafePath("inherited Cargo build lock identity changed after acquisition")
    return lock_fd


def run_verifier(
    command: Sequence[str],
    lease_set: LeaseSet,
    *,
    phase: str,
    source: pathlib.Path,
    destination: pathlib.Path,
    pinned_directory: int,
) -> None:
    if not command:
        raise UnsafePath("release publish requires a semantic verifier command")
    normalized = list(command)
    if normalized and normalized[0] == "--":
        normalized = normalized[1:]
    if not normalized:
        raise UnsafePath("release publish requires a semantic verifier command")
    pinned_path = f"/proc/self/fd/{pinned_directory}"

    def pinned_argument(argument: str) -> str:
        for public in (os.fspath(source), os.fspath(destination)):
            if argument == public:
                return pinned_path
            prefix = public + os.sep
            if argument.startswith(prefix):
                return pinned_path + os.sep + argument[len(prefix) :]
        return argument

    normalized = [pinned_argument(argument) for argument in normalized]
    pinned_metadata = os.fstat(pinned_directory)
    environment = os.environ.copy()
    environment["KANBAN_RELEASE_PUBLISH_PHASE"] = phase
    environment["KANBAN_RELEASE_PUBLISH_SOURCE"] = os.fspath(source)
    environment["KANBAN_RELEASE_PUBLISH_DESTINATION"] = os.fspath(destination)
    environment["KANBAN_RELEASE_PINNED_STAGE_FD"] = str(pinned_directory)
    environment["KANBAN_RELEASE_PINNED_STAGE_DEV"] = str(pinned_metadata.st_dev)
    environment["KANBAN_RELEASE_PINNED_STAGE_INO"] = str(pinned_metadata.st_ino)
    pass_fds_set: set[int] = set()
    if os.environ.get("KANBAN_RELEASE_SAFE_PATH_TEST_DROP_PINNED_FD") == "1":
        pass
    else:
        pass_fds_set.add(pinned_directory)
    build_lock_fd = inherited_build_lock_fd()
    if build_lock_fd is not None:
        pass_fds_set.add(build_lock_fd)
    pass_fds = tuple(sorted(pass_fds_set))
    process = subprocess.Popen(
        normalized,
        env=environment,
        close_fds=True,
        pass_fds=pass_fds,
        start_new_session=True,
    )
    try:
        while True:
            return_code = process.poll()
            if return_code is not None:
                break
            try:
                lease_set.check()
            except BaseException:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=5)
                except (ProcessLookupError, subprocess.TimeoutExpired):
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.wait()
                raise
            time.sleep(0.02)
        lease_set.check()
        if return_code != 0:
            raise UnsafePath(
                f"release semantic verifier failed during {phase} publication phase "
                f"(exit={return_code})"
            )
    finally:
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            process.wait()


def publish_file(
    root: RootAnchor,
    source: pathlib.Path,
    destination: pathlib.Path,
    replace: bool,
    expected_source_parent_dev: int | None = None,
    expected_source_parent_ino: int | None = None,
) -> None:
    if (expected_source_parent_dev is None) != (
        expected_source_parent_ino is None
    ):
        raise UnsafePath(
            "expected publish source parent identity requires both "
            "--expected-source-parent-dev and --expected-source-parent-ino"
        )
    if (
        expected_source_parent_dev is not None
        and expected_source_parent_ino is not None
        and (
            expected_source_parent_dev < 0
            or expected_source_parent_ino < 0
        )
    ):
        raise UnsafePath(
            "expected publish source parent identity values must be non-negative"
        )
    source_parent, source_name, source_parent_path = root.open_parent(
        source, "publish source"
    )
    destination_parent, destination_name, destination_parent_path = root.open_parent(
        destination, "publish destination"
    )
    source_descriptor = -1
    destination_descriptor = -1
    destination_metadata: os.stat_result | None = None
    published = False
    exchanged = False
    try:
        source_parent_identity = os.fstat(source_parent)
        if (
            expected_source_parent_dev is not None
            and expected_source_parent_ino is not None
            and (
                source_parent_identity.st_dev,
                source_parent_identity.st_ino,
            )
            != (
                expected_source_parent_dev,
                expected_source_parent_ino,
            )
        ):
            raise UnsafePath(
                "publish source parent identity does not match expected "
                f"identity observation token: {source_parent_path}"
            )
        source_descriptor, source_metadata = open_regular_at(
            source_parent, source_name, source, single_link=True
        )
        reject_xattrs(source_descriptor, source)
        if os.fstat(source_parent).st_dev != os.fstat(destination_parent).st_dev:
            raise UnsafePath("release publish must stay on one filesystem")
        try:
            destination_metadata = os.stat(
                destination_name,
                dir_fd=destination_parent,
                follow_symlinks=False,
            )
            destination_exists = True
        except FileNotFoundError:
            destination_exists = False
        if destination_exists:
            if not replace:
                raise UnsafePath(f"release destination already exists: {destination}")
            destination_descriptor, destination_metadata = open_regular_at(
                destination_parent,
                destination_name,
                destination,
                single_link=True,
            )
            reject_xattrs(destination_descriptor, destination)
        lease_files = [(source_descriptor, source)]
        if destination_descriptor >= 0:
            lease_files.append((destination_descriptor, destination))
        with LeaseSet(lease_files) as leases:
            sync_file(source_descriptor, source, "publish-file-source")
            leases.check()
            root.assert_dir_identity(
                source_parent_path, source_parent, "publish source parent"
            )
            root.assert_dir_identity(
                destination_parent_path,
                destination_parent,
                "publish destination parent",
            )
            test_event("publish-file-before-rename", source)
            leases.check()
            root.assert_dir_identity(
                source_parent_path, source_parent, "publish source parent"
            )
            root.assert_dir_identity(
                destination_parent_path,
                destination_parent,
                "publish destination parent",
            )
            latest = os.stat(
                source_name, dir_fd=source_parent, follow_symlinks=False
            )
            if not source_metadata_stable(source_metadata, latest):
                raise UnsafePath(f"release source changed before publish: {source}")
            if destination_exists:
                assert destination_metadata is not None
                latest_destination = os.stat(
                    destination_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if not same_inode(destination_metadata, latest_destination):
                    raise UnsafePath(
                        f"release replacement destination identity changed: {destination}"
                    )
                reject_xattrs(destination_descriptor, destination)
            if destination_exists:
                rename_exchange(
                    source_parent,
                    source_name,
                    destination_parent,
                    destination_name,
                    "release file replacement",
                )
                exchanged = True
            else:
                rename_noreplace(
                    source_parent,
                    source_name,
                    destination_parent,
                    destination_name,
                    "release file",
                    destination,
                )
            published = True
            try:
                if exchanged:
                    assert destination_metadata is not None
                    old_at_source = os.stat(
                        source_name,
                        dir_fd=source_parent,
                        follow_symlinks=False,
                    )
                    new_at_destination = os.stat(
                        destination_name,
                        dir_fd=destination_parent,
                        follow_symlinks=False,
                    )
                    if not same_inode(old_at_source, destination_metadata) or not same_inode(
                        new_at_destination, source_metadata
                    ):
                        raise UnsafePath(
                            "conditional release replacement observed an unknown target"
                        )
                test_event("publish-file-after-rename", destination)
                sync_rename_parents(
                    source_parent,
                    source_parent_path,
                    destination_parent,
                    destination_parent_path,
                    "publish-file-parent",
                )
                test_event("publish-file-before-final-check", destination)
                leases.check()
                new_at_destination = os.stat(
                    destination_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if not same_inode(new_at_destination, source_metadata):
                    raise UnsafePath(
                        f"published release file identity changed: {destination}"
                    )
                final = os.fstat(source_descriptor)
                if not source_metadata_stable(
                    source_metadata, final, allow_ctime_change=True
                ):
                    raise UnsafePath(
                        f"release source changed during publish: {destination}"
                    )
                reject_xattrs(source_descriptor, destination)
                if exchanged:
                    assert destination_metadata is not None
                    old_at_source = os.stat(
                        source_name,
                        dir_fd=source_parent,
                        follow_symlinks=False,
                    )
                    if not same_inode(old_at_source, destination_metadata):
                        raise UnsafePath(
                            "retained replacement rollback target identity changed"
                        )
                    reject_xattrs(destination_descriptor, source)
                root.assert_dir_identity(
                    source_parent_path,
                    source_parent,
                    "publish source parent",
                )
                root.assert_dir_identity(
                    destination_parent_path,
                    destination_parent,
                    "publish destination parent",
                )
                public_destination = os.stat(
                    destination_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if not same_inode(public_destination, source_metadata):
                    raise UnsafePath(
                        f"published release file public identity changed: {destination}"
                    )
                if exchanged:
                    assert destination_metadata is not None
                    public_source = os.stat(
                        source_name,
                        dir_fd=source_parent,
                        follow_symlinks=False,
                    )
                    if not same_inode(public_source, destination_metadata):
                        raise UnsafePath(
                            "retained replacement target is not public at commit"
                        )
                else:
                    try:
                        os.stat(
                            source_name,
                            dir_fd=source_parent,
                            follow_symlinks=False,
                        )
                    except FileNotFoundError:
                        pass
                    else:
                        raise UnsafePath(
                            f"publish source reappeared before commit: {source}"
                        )
                leases.check()
                committed_process_exit()
            except BaseException as publish_error:
                try:
                    if exchanged:
                        assert destination_metadata is not None
                        current_source = os.stat(
                            source_name,
                            dir_fd=source_parent,
                            follow_symlinks=False,
                        )
                        current_destination = os.stat(
                            destination_name,
                            dir_fd=destination_parent,
                            follow_symlinks=False,
                        )
                        if not same_inode(
                            current_source, destination_metadata
                        ) or not same_inode(current_destination, source_metadata):
                            raise UnsafePath(
                                "release replacement rollback endpoints drifted"
                            )
                        rename_exchange(
                            source_parent,
                            source_name,
                            destination_parent,
                            destination_name,
                            "release file replacement rollback",
                        )
                    else:
                        current_destination = os.stat(
                            destination_name,
                            dir_fd=destination_parent,
                            follow_symlinks=False,
                        )
                        if not same_inode(current_destination, source_metadata):
                            raise UnsafePath(
                                "release file rollback destination drifted"
                            )
                        try:
                            os.stat(
                                source_name,
                                dir_fd=source_parent,
                                follow_symlinks=False,
                            )
                        except FileNotFoundError:
                            pass
                        else:
                            raise UnsafePath(
                                "release file rollback source was replaced"
                            )
                        rename_noreplace(
                            destination_parent,
                            destination_name,
                            source_parent,
                            source_name,
                            "release file rollback",
                            source,
                        )
                    if exchanged:
                        assert destination_metadata is not None
                        restored_source = os.stat(
                            source_name,
                            dir_fd=source_parent,
                            follow_symlinks=False,
                        )
                        restored_destination = os.stat(
                            destination_name,
                            dir_fd=destination_parent,
                            follow_symlinks=False,
                        )
                        if not same_inode(
                            restored_source, source_metadata
                        ) or not same_inode(
                            restored_destination, destination_metadata
                        ):
                            raise UnsafePath(
                                "release replacement rollback identity is invalid"
                            )
                    else:
                        restored_source = os.stat(
                            source_name,
                            dir_fd=source_parent,
                            follow_symlinks=False,
                        )
                        if not same_inode(restored_source, source_metadata):
                            raise UnsafePath(
                                "release file rollback source identity is invalid"
                            )
                        try:
                            os.stat(
                                destination_name,
                                dir_fd=destination_parent,
                                follow_symlinks=False,
                            )
                        except FileNotFoundError:
                            pass
                        else:
                            raise UnsafePath(
                                "release file rollback destination still exists"
                            )
                    sync_rename_parents(
                        destination_parent,
                        destination_parent_path,
                        source_parent,
                        source_parent_path,
                        "publish-file-rollback-parent",
                    )
                except BaseException as rollback_error:
                    try:
                        mark_directory_for_retention(
                            source_parent,
                            source_parent_path,
                            "publish-file-rollback-identity-drift",
                        )
                    except BaseException as retention_error:
                        published = False
                        raise UnsafePath(
                            "release file publish failed; rollback was unsafe and "
                            "the private stage could not be marked for retention"
                        ) from retention_error
                    published = False
                    raise UnsafePath(
                        "release file publish failed; rollback endpoints drifted or "
                        "atomic rollback failed, so the private stage was retained"
                    ) from rollback_error
                published = False
                raise publish_error
    finally:
        if published:
            # Success exits through committed_process_exit() before unwinding.
            raise AssertionError("published release file unwound past commit boundary")
        if destination_descriptor >= 0:
            os.close(destination_descriptor)
        if source_descriptor >= 0:
            os.close(source_descriptor)
        os.close(destination_parent)
        os.close(source_parent)


def marker_document(
    expected_tree_sha256: str,
    source_name: str,
    destination_name: str,
    tree_metadata: os.stat_result,
) -> bytes:
    return (
        f"kanban-release-v1\t{expected_tree_sha256}\t{source_name}\t"
        f"{destination_name}\t"
        f"{tree_metadata.st_dev}\t{tree_metadata.st_ino}\n"
    ).encode()


def descriptor_bytes(
    descriptor: int, path: pathlib.Path, lease_set: LeaseSet | None
) -> bytes:
    os.lseek(descriptor, 0, os.SEEK_SET)
    chunks: list[bytes] = []
    while True:
        chunk = os.read(descriptor, 1024 * 1024)
        if not chunk:
            break
        chunks.append(chunk)
        if lease_set is not None:
            lease_set.check_fast()
    if lease_set is not None:
        lease_set.check()
    reject_xattrs(descriptor, path)
    return b"".join(chunks)


def parse_marker_document(
    document: bytes,
    *,
    source_name: str | None,
    destination_name: str,
    tree_metadata: os.stat_result,
    expected_tree_sha256: str | None,
) -> str:
    try:
        text = document.decode("utf-8")
    except UnicodeDecodeError as error:
        raise UnsafePath("release publication intent is not UTF-8") from error
    fields = text.removesuffix("\n").split("\t")
    if len(fields) != 6 or not text.endswith("\n"):
        raise UnsafePath("release publication intent has an invalid field layout")
    magic, digest, recorded_source, recorded_destination, raw_dev, raw_ino = fields
    if magic != "kanban-release-v1":
        raise UnsafePath("release publication intent has an invalid version")
    if (
        len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
        or (expected_tree_sha256 is not None and digest != expected_tree_sha256)
    ):
        raise UnsafePath("release publication intent has an invalid tree digest")
    if (
        not recorded_source
        or pathlib.PurePosixPath(recorded_source).name != recorded_source
        or recorded_source in (".", "..")
        or (source_name is not None and recorded_source != source_name)
    ):
        raise UnsafePath("release publication intent has an invalid source identity")
    if recorded_destination != destination_name:
        raise UnsafePath(
            "release publication intent has an invalid destination identity"
        )
    try:
        recorded_dev = int(raw_dev)
        recorded_ino = int(raw_ino)
    except ValueError as error:
        raise UnsafePath(
            "release publication intent has an invalid inode identity"
        ) from error
    if (recorded_dev, recorded_ino) != (
        tree_metadata.st_dev,
        tree_metadata.st_ino,
    ):
        raise UnsafePath(
            "release publication intent does not bind the current tree inode"
        )
    return digest


def open_or_create_publish_intent(
    parent: int,
    parent_path: pathlib.Path,
    name: str,
    document: bytes,
) -> tuple[pathlib.Path, int, os.stat_result]:
    path = parent_path / name
    try:
        writer = os.open(
            name,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
            dir_fd=parent,
        )
    except FileExistsError:
        descriptor, metadata = open_regular_at(
            parent, name, path, single_link=True
        )
        try:
            if descriptor_bytes(descriptor, path, None) != document:
                raise UnsafePath(
                    f"existing release publication intent is not reusable: {path}"
                )
            reject_xattrs(descriptor, path)
            return path, descriptor, metadata
        except BaseException:
            os.close(descriptor)
            raise
    try:
        view = memoryview(document)
        while view:
            written = os.write(writer, view)
            view = view[written:]
        os.fchmod(writer, 0o444)
        reject_xattrs(writer, path)
        sync_file(writer, path, "publish-dir-intent-file")
    finally:
        os.close(writer)
    sync_directory_descriptor(parent, parent_path, "publish-dir-intent-parent")
    descriptor, metadata = open_regular_at(parent, name, path, single_link=True)
    return path, descriptor, metadata


def marker_matches(
    parent: int,
    name: str,
    descriptor: int,
    metadata: os.stat_result,
    expected: bytes,
    path: pathlib.Path,
    lease_set: LeaseSet,
) -> bool:
    current = os.stat(name, dir_fd=parent, follow_symlinks=False)
    return (
        same_inode(current, metadata)
        and same_inode(os.fstat(descriptor), metadata)
        and descriptor_bytes(descriptor, path, lease_set) == expected
    )


def recover_directory_publish(
    root: RootAnchor,
    source: pathlib.Path,
    destination: pathlib.Path,
    expected_tree_sha256: str | None,
) -> str:
    source_parent, source_name, source_parent_path = root.open_parent(
        source, "directory recovery source"
    )
    destination_parent, destination_name, destination_parent_path = root.open_parent(
        destination, "directory recovery destination"
    )
    destination_descriptor = -1
    intent_descriptor = -1
    snapshot: TreeSnapshot | None = None
    renamed = False
    intent_name = destination_name + ".publishing"
    intent_path = destination_parent_path / intent_name
    marker_name = destination_name + ".published"
    try:
        try:
            os.stat(source_name, dir_fd=source_parent, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise UnsafePath(
                f"release recovery source already exists: {source}"
            )
        try:
            os.stat(marker_name, dir_fd=destination_parent, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise UnsafePath(
                f"authoritative release marker already exists: "
                f"{destination_parent_path / marker_name}"
            )
        destination_descriptor = os.open(
            destination_name, DIR_FLAGS, dir_fd=destination_parent
        )
        destination_metadata = os.fstat(destination_descriptor)
        current_destination = os.stat(
            destination_name,
            dir_fd=destination_parent,
            follow_symlinks=False,
        )
        if (
            not stat.S_ISDIR(current_destination.st_mode)
            or not same_inode(current_destination, destination_metadata)
        ):
            raise UnsafePath(
                f"uncommitted release destination identity changed: {destination}"
            )
        if os.fstat(source_parent).st_dev != os.fstat(destination_parent).st_dev:
            raise UnsafePath("release directory recovery must stay on one filesystem")
        snapshot = TreeSnapshot.capture(
            destination_descriptor, destination, require_sealed=True
        )
        intent_descriptor, intent_metadata = open_regular_at(
            destination_parent,
            intent_name,
            intent_path,
            single_link=True,
        )
        reject_xattrs(intent_descriptor, intent_path)
        intent_document = descriptor_bytes(intent_descriptor, intent_path, None)
        recorded_digest = parse_marker_document(
            intent_document,
            source_name=source_name,
            destination_name=destination_name,
            tree_metadata=snapshot.root.metadata,
            expected_tree_sha256=expected_tree_sha256,
        )
        with LeaseSet(
            [*snapshot.leased_files, (intent_descriptor, intent_path)]
        ) as leases:
            snapshot.verify(allow_root_ctime_change=False)
            if (
                snapshot.digest(leases, allow_root_ctime_change=False)
                != recorded_digest
            ):
                raise UnsafePath(
                    "uncommitted release destination does not match its durable intent"
                )
            if not marker_matches(
                destination_parent,
                intent_name,
                intent_descriptor,
                intent_metadata,
                intent_document,
                intent_path,
                leases,
            ):
                raise UnsafePath(
                    "durable release publication intent changed during recovery"
                )
            root.assert_dir_identity(
                source_parent_path,
                source_parent,
                "directory recovery source parent",
            )
            root.assert_dir_identity(
                destination_parent_path,
                destination_parent,
                "directory recovery destination parent",
            )
            try:
                os.stat(
                    source_name,
                    dir_fd=source_parent,
                    follow_symlinks=False,
                )
            except FileNotFoundError:
                pass
            else:
                raise UnsafePath(
                    f"release recovery source appeared before rename: {source}"
                )
            current_destination = os.stat(
                destination_name,
                dir_fd=destination_parent,
                follow_symlinks=False,
            )
            if not same_inode(current_destination, destination_metadata):
                raise UnsafePath(
                    "uncommitted release destination drifted before recovery"
                )
            test_event("publish-dir-before-crash-recovery", destination)
            rename_noreplace(
                destination_parent,
                destination_name,
                source_parent,
                source_name,
                "journaled release crash recovery",
                source,
            )
            renamed = True
            try:
                sync_rename_parents(
                    destination_parent,
                    destination_parent_path,
                    source_parent,
                    source_parent_path,
                    "publish-dir-crash-recovery-parent",
                )
                current_source = os.stat(
                    source_name,
                    dir_fd=source_parent,
                    follow_symlinks=False,
                )
                if not same_inode(current_source, destination_metadata):
                    raise UnsafePath(
                        "journaled release recovery restored the wrong source"
                    )
                leases.check()
            except BaseException:
                current_source = os.stat(
                    source_name,
                    dir_fd=source_parent,
                    follow_symlinks=False,
                )
                try:
                    os.stat(
                        destination_name,
                        dir_fd=destination_parent,
                        follow_symlinks=False,
                    )
                except FileNotFoundError:
                    destination_absent = True
                else:
                    destination_absent = False
                if (
                    not same_inode(current_source, destination_metadata)
                    or not destination_absent
                ):
                    raise UnsafePath(
                        "journaled release recovery failed with identity drift"
                    )
                rename_noreplace(
                    source_parent,
                    source_name,
                    destination_parent,
                    destination_name,
                    "journaled release recovery rollback",
                    destination,
                )
                renamed = False
                sync_rename_parents(
                    source_parent,
                    source_parent_path,
                    destination_parent,
                    destination_parent_path,
                    "publish-dir-crash-recovery-rollback-parent",
                )
                raise
        renamed = False
        return recorded_digest
    finally:
        if renamed:
            raise AssertionError(
                "journaled release recovery unwound with a moved generation"
            )
        if snapshot is not None:
            snapshot.close()
        if intent_descriptor >= 0:
            os.close(intent_descriptor)
        if destination_descriptor >= 0:
            os.close(destination_descriptor)
        os.close(destination_parent)
        os.close(source_parent)


def publish_directory(
    root: RootAnchor,
    source: pathlib.Path,
    destination: pathlib.Path,
    expected_tree_sha256: str,
    verify_command: Sequence[str],
) -> None:
    # Validate the inherited build proof before creating the durable
    # `.publishing` intent.  run_verifier repeats the check after snapshotting
    # so a path/descriptor race cannot turn an invalid proof authoritative.
    inherited_build_lock_fd()
    source_parent, source_name, source_parent_path = root.open_parent(
        source, "directory publish source"
    )
    destination_parent, destination_name, destination_parent_path = root.open_parent(
        destination, "directory publish destination"
    )
    source_descriptor = -1
    snapshot: TreeSnapshot | None = None
    renamed = False
    marker_published = False
    unsafe_publish_retained = False
    marker_descriptor = -1
    marker_pending_name = destination_name + ".publishing"
    marker_pending_path = destination_parent_path / marker_pending_name
    marker_metadata: os.stat_result | None = None
    marker_name = destination_name + ".published"
    marker_path = destination_parent_path / marker_name
    try:
        root.assert_dir_identity(
            destination_parent_path,
            destination_parent,
            "release destination parent",
        )
        try:
            os.stat(marker_name, dir_fd=destination_parent, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise UnsafePath(
                f"authoritative release marker already exists: {marker_path}"
            )
        try:
            source_descriptor = os.open(source_name, DIR_FLAGS, dir_fd=source_parent)
            source_exists = True
        except FileNotFoundError:
            source_exists = False
        try:
            destination_entry = os.stat(
                destination_name,
                dir_fd=destination_parent,
                follow_symlinks=False,
            )
            destination_exists = True
        except FileNotFoundError:
            destination_exists = False
        if not source_exists:
            if not destination_exists or not stat.S_ISDIR(destination_entry.st_mode):
                raise UnsafePath(f"release source directory does not exist: {source}")
            recover_directory_publish(
                root,
                source,
                destination,
                expected_tree_sha256,
            )
            source_descriptor = os.open(source_name, DIR_FLAGS, dir_fd=source_parent)
            destination_exists = False
        elif destination_exists:
            raise UnsafePath(
                "both release source and unmarked destination exist; "
                "automatic recovery is ambiguous"
            )
        source_entry = os.stat(
            source_name, dir_fd=source_parent, follow_symlinks=False
        )
        if not same_inode(source_entry, os.fstat(source_descriptor)):
            raise UnsafePath(
                f"release source directory identity changed while opening: {source}"
            )
        root.assert_dir_identity(source, source_descriptor, "release source directory")
        if os.fstat(source_parent).st_dev != os.fstat(destination_parent).st_dev:
            raise UnsafePath("release directory publish must stay on one filesystem")
        snapshot = TreeSnapshot.capture(
            source_descriptor, source, require_sealed=True
        )
        sync_snapshot(snapshot)
        marker_expected = marker_document(
            expected_tree_sha256,
            source_name,
            destination_name,
            snapshot.root.metadata,
        )
        (
            marker_pending_path,
            marker_descriptor,
            marker_metadata,
        ) = open_or_create_publish_intent(
            destination_parent,
            destination_parent_path,
            marker_pending_name,
            marker_expected,
        )
        with LeaseSet(
            [*snapshot.leased_files, (marker_descriptor, marker_pending_path)]
        ) as leases:
            snapshot.verify(allow_root_ctime_change=False)
            run_verifier(
                verify_command,
                leases,
                phase="pre",
                source=source,
                destination=destination,
                pinned_directory=snapshot.root.descriptor,
            )
            snapshot.verify(allow_root_ctime_change=False)
            test_event("publish-dir-before-digest", source)
            actual_tree_sha256 = snapshot.digest(
                leases, allow_root_ctime_change=False
            )
            if actual_tree_sha256 != expected_tree_sha256:
                raise UnsafePath(
                    "sealed release generation changed after final verification: "
                    f"expected={expected_tree_sha256} actual={actual_tree_sha256}"
                )
            test_event("publish-dir-after-final-digest", source)
            leases.check()
            snapshot.verify(allow_root_ctime_change=False)
            root.assert_dir_identity(source, source_descriptor, "release source directory")
            root.assert_dir_identity(
                source_parent_path, source_parent, "release source parent"
            )
            root.assert_dir_identity(
                destination_parent_path,
                destination_parent,
                "release destination parent",
            )
            test_event("publish-dir-before-rename", source)
            leases.check()
            snapshot.verify(allow_root_ctime_change=False)
            latest = os.stat(
                source_name, dir_fd=source_parent, follow_symlinks=False
            )
            if not same_inode(latest, os.fstat(source_descriptor)):
                raise UnsafePath(
                    f"release source directory identity changed before publish: {source}"
                )
            rename_noreplace(
                source_parent,
                source_name,
                destination_parent,
                destination_name,
                "release generation",
                destination,
            )
            renamed = True
            try:
                test_event("publish-dir-after-rename", destination)
                sync_rename_parents(
                    source_parent,
                    source_parent_path,
                    destination_parent,
                    destination_parent_path,
                    "publish-dir-parent",
                )
                leases.check()
                root.assert_dir_identity(
                    destination,
                    source_descriptor,
                    "published release generation",
                )
                snapshot.verify(allow_root_ctime_change=True)
                post_tree_sha256 = snapshot.digest(
                    leases, allow_root_ctime_change=True
                )
                if post_tree_sha256 != expected_tree_sha256:
                    raise UnsafePath(
                        "published release generation differs from leased snapshot: "
                        f"expected={expected_tree_sha256} actual={post_tree_sha256}"
                    )
                run_verifier(
                    verify_command,
                    leases,
                    phase="post",
                    source=source,
                    destination=destination,
                    pinned_directory=snapshot.root.descriptor,
                )
                test_event("publish-dir-post-publish-verified", destination)
                leases.check()
                root.assert_dir_identity(
                    destination,
                    source_descriptor,
                    "published release generation",
                )
                snapshot.verify(allow_root_ctime_change=True)
                if (
                    snapshot.digest(leases, allow_root_ctime_change=True)
                    != expected_tree_sha256
                ):
                    raise UnsafePath(
                        "published release generation changed during final semantic verify"
                    )
                if marker_metadata is None or not marker_matches(
                    destination_parent,
                    marker_pending_name,
                    marker_descriptor,
                    marker_metadata,
                    marker_expected,
                    marker_pending_path,
                    leases,
                ):
                    raise UnsafePath(
                        "pending authoritative release marker changed before commit"
                    )
                leases.check()
                rename_noreplace(
                    destination_parent,
                    marker_pending_name,
                    destination_parent,
                    marker_name,
                    "authoritative release marker",
                    marker_path,
                )
                marker_published = True
                sync_directory_descriptor(
                    destination_parent,
                    destination_parent_path,
                    "publish-dir-marker-commit-parent",
                )
                test_event(
                    "publish-dir-after-marker-commit-parent",
                    destination,
                )
                test_event(
                    "publish-dir-before-final-public-check",
                    destination,
                )
                leases.check()
                current_generation = os.stat(
                    destination_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if not same_inode(
                    current_generation,
                    snapshot.root.metadata,
                ):
                    raise UnsafePath(
                        "published release generation identity changed at commit"
                    )
                if marker_metadata is None or not marker_matches(
                    destination_parent,
                    marker_name,
                    marker_descriptor,
                    marker_metadata,
                    marker_expected,
                    marker_path,
                    leases,
                ):
                    raise UnsafePath(
                        "authoritative release marker identity changed during commit"
                    )
                leases.check()
                root.assert_root_identity()
                root.assert_dir_identity(
                    destination_parent_path,
                    destination_parent,
                    "release destination parent",
                )
                root.assert_dir_identity(
                    destination,
                    source_descriptor,
                    "published release generation",
                )
                current_marker = os.stat(
                    marker_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if not same_inode(current_marker, marker_metadata):
                    raise UnsafePath(
                        "authoritative release marker identity changed during commit"
                    )
                current_generation = os.stat(
                    destination_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if not same_inode(
                    current_generation,
                    snapshot.root.metadata,
                ):
                    raise UnsafePath(
                        "published release generation identity changed at commit"
                    )
                leases.check()
                committed_process_exit()
            except BaseException:
                try:
                    if marker_published:
                        if marker_metadata is None or not marker_matches(
                            destination_parent,
                            marker_name,
                            marker_descriptor,
                            marker_metadata,
                            marker_expected,
                            marker_path,
                            leases,
                        ):
                            raise UnsafePath(
                                "authoritative release marker identity "
                                "drifted before rollback"
                            )
                        rename_noreplace(
                            destination_parent,
                            marker_name,
                            destination_parent,
                            marker_pending_name,
                            "authoritative release marker rollback",
                            marker_pending_path,
                        )
                        marker_published = False
                    current = os.stat(
                        destination_name,
                        dir_fd=destination_parent,
                        follow_symlinks=False,
                    )
                    if not same_inode(current, os.fstat(source_descriptor)):
                        raise UnsafePath(
                            "published generation identity changed before rollback"
                        )
                    rename_noreplace(
                        destination_parent,
                        destination_name,
                        source_parent,
                        source_name,
                        "release generation rollback",
                        source,
                    )
                    renamed = False
                    sync_rename_parents(
                        destination_parent,
                        destination_parent_path,
                        source_parent,
                        source_parent_path,
                        "publish-dir-rollback-parent",
                    )
                except BaseException as rollback_error:
                    unsafe_publish_retained = True
                    raise UnsafePath(
                        "release generation publish failed; rollback authority "
                        "drifted or atomic rollback failed, so all unknown "
                        "public entries were retained in place"
                    ) from rollback_error
                raise
    finally:
        if (renamed or marker_published) and not unsafe_publish_retained:
            raise AssertionError(
                "published release generation unwound past commit boundary"
            )
        if marker_descriptor >= 0:
            os.close(marker_descriptor)
        if marker_pending_name and not unsafe_publish_retained:
            try:
                current_marker = os.stat(
                    marker_pending_name,
                    dir_fd=destination_parent,
                    follow_symlinks=False,
                )
                if marker_metadata is not None and same_inode(
                    current_marker, marker_metadata
                ):
                    os.unlink(marker_pending_name, dir_fd=destination_parent)
                    os.fsync(destination_parent)
            except FileNotFoundError:
                pass
        if snapshot is not None:
            snapshot.close()
        if source_descriptor >= 0:
            os.close(source_descriptor)
        os.close(destination_parent)
        os.close(source_parent)


def validate_published_directory(
    root: RootAnchor,
    path: pathlib.Path,
    marker: pathlib.Path,
    expected_tree_sha256: str | None,
    verify_command: Sequence[str],
) -> None:
    if marker.parent != path.parent:
        raise UnsafePath(
            "published release generation and marker must share one parent"
        )
    directory_descriptor = root.open_dir(path, "published release generation")
    marker_parent, marker_name, marker_parent_path = root.open_parent(
        marker, "published release marker"
    )
    marker_descriptor = -1
    snapshot: TreeSnapshot | None = None
    try:
        root.assert_dir_identity(
            path, directory_descriptor, "published release generation"
        )
        marker_descriptor, marker_metadata = open_regular_at(
            marker_parent, marker_name, marker, single_link=True
        )
        snapshot = TreeSnapshot.capture(
            directory_descriptor, path, require_sealed=True
        )
        with LeaseSet(
            [*snapshot.leased_files, (marker_descriptor, marker)]
        ) as leases:
            test_event("validate-published-before-final-check", path)
            leases.check()
            root.assert_dir_identity(
                path, directory_descriptor, "published release generation"
            )
            snapshot.verify(allow_root_ctime_change=False)
            recorded_digest = parse_marker_document(
                descriptor_bytes(marker_descriptor, marker, leases),
                source_name=None,
                destination_name=path.name,
                tree_metadata=snapshot.root.metadata,
                expected_tree_sha256=expected_tree_sha256,
            )
            if (
                snapshot.digest(leases, allow_root_ctime_change=False)
                != recorded_digest
            ):
                raise UnsafePath(
                    "authoritative release marker does not match published tree digest"
                )
            if verify_command:
                run_verifier(
                    verify_command,
                    leases,
                    phase="resume",
                    source=path,
                    destination=path,
                    pinned_directory=snapshot.root.descriptor,
                )
                snapshot.verify(allow_root_ctime_change=False)
                if (
                    snapshot.digest(leases, allow_root_ctime_change=False)
                    != recorded_digest
                ):
                    raise UnsafePath(
                        "published release generation changed during "
                        "resume verification"
                    )
            if expected_tree_sha256 is None:
                sync_directory_descriptor(
                    marker_parent,
                    marker_parent_path,
                    "validate-published-resume-parent",
                )
            root.assert_root_identity()
            root.assert_dir_identity(
                marker_parent_path,
                marker_parent,
                "published release parent",
            )
            root.assert_dir_identity(
                path,
                directory_descriptor,
                "published release generation",
            )
            current_marker = os.stat(
                marker_name,
                dir_fd=marker_parent,
                follow_symlinks=False,
            )
            if (
                not same_inode(current_marker, marker_metadata)
                or not same_inode(os.fstat(marker_descriptor), marker_metadata)
            ):
                raise UnsafePath(
                    "authoritative release marker identity is invalid"
                )
            current_generation = os.stat(
                path.name,
                dir_fd=marker_parent,
                follow_symlinks=False,
            )
            if not same_inode(current_generation, snapshot.root.metadata):
                raise UnsafePath(
                    "published release generation public identity is invalid"
                )
            leases.check()
            if expected_tree_sha256 is None:
                print(recorded_digest)
            committed_process_exit()
    finally:
        if snapshot is not None:
            snapshot.close()
        if marker_descriptor >= 0:
            os.close(marker_descriptor)
        os.close(marker_parent)
        os.close(directory_descriptor)


def parse_mode(raw: str) -> int:
    try:
        mode = int(raw, 8)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"invalid octal mode: {raw}") from error
    if mode < 0 or mode > 0o777:
        raise argparse.ArgumentTypeError(f"invalid octal mode: {raw}")
    return mode


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    subcommands = result.add_subparsers(dest="command", required=True)

    ensure = subcommands.add_parser("ensure-dir")
    ensure.add_argument("--root", required=True)
    ensure.add_argument("--path", required=True)
    ensure.add_argument("--mode", type=parse_mode, default=0o755)

    private = subcommands.add_parser("private-dir")
    private.add_argument("--root", required=True)
    private.add_argument("--parent", required=True)
    private.add_argument("--prefix", required=True)
    private.add_argument("--print-identity", action="store_true")

    validate = subcommands.add_parser("validate-file")
    validate.add_argument("--root", required=True)
    validate.add_argument("--path", required=True)

    validate_tree = subcommands.add_parser("validate-tree-file")
    validate_tree.add_argument("--tree-fd", type=int, required=True)
    validate_tree.add_argument("--relative", required=True)

    identity = subcommands.add_parser("dir-identity")
    identity.add_argument("--root", required=True)
    identity.add_argument("--path", required=True)
    identity.add_argument("--expected-dev", type=int)
    identity.add_argument("--expected-ino", type=int)

    remove = subcommands.add_parser("remove-tree")
    remove.add_argument("--root", required=True)
    remove.add_argument("--path", required=True)
    remove.add_argument("--expected-dev", type=int, required=True)
    remove.add_argument("--expected-ino", type=int, required=True)

    copy = subcommands.add_parser("copy-file")
    copy.add_argument("--root", required=True)
    copy.add_argument("--source", required=True)
    copy.add_argument("--destination", required=True)
    copy.add_argument("--mode", type=parse_mode, required=True)

    publish = subcommands.add_parser("publish-file")
    publish.add_argument("--root", required=True)
    publish.add_argument("--source", required=True)
    publish.add_argument("--destination", required=True)
    publish.add_argument("--replace", action="store_true")
    publish.add_argument("--expected-source-parent-dev", type=int)
    publish.add_argument("--expected-source-parent-ino", type=int)

    publish_dir = subcommands.add_parser("publish-dir")
    publish_dir.add_argument("--root", required=True)
    publish_dir.add_argument("--source", required=True)
    publish_dir.add_argument("--destination", required=True)
    publish_dir.add_argument("--expected-tree-sha256", required=True)
    publish_dir.add_argument("--verify-command", nargs=argparse.REMAINDER, required=True)

    recover_dir = subcommands.add_parser("recover-publish-dir")
    recover_dir.add_argument("--root", required=True)
    recover_dir.add_argument("--source", required=True)
    recover_dir.add_argument("--destination", required=True)

    validate_published = subcommands.add_parser("validate-published-dir")
    validate_published.add_argument("--root", required=True)
    validate_published.add_argument("--path", required=True)
    validate_published.add_argument("--marker", required=True)
    validate_published.add_argument("--expected-tree-sha256")
    validate_published.add_argument(
        "--verify-command",
        nargs=argparse.REMAINDER,
        default=(),
    )

    seal = subcommands.add_parser("seal-tree")
    seal.add_argument("--root", required=True)
    seal.add_argument("--path", required=True)

    tree = subcommands.add_parser("tree-digest")
    tree.add_argument("--root", required=True)
    tree.add_argument("--path", required=True)
    tree.add_argument("--require-sealed", action="store_true")
    return result


def main() -> int:
    arguments = parser().parse_args()
    # Validate the inherited proof before entering any command.  This keeps
    # even the read-only verifier paths from becoming a confused deputy when
    # a caller supplies only a marker/target environment.  Mutating paths
    # additionally re-check at their final mutation boundary.
    inherited_build_lock_fd()
    if arguments.command == "validate-tree-file":
        validate_tree_file(arguments.tree_fd, arguments.relative)
        return 0
    root_path = absolute(arguments.root, "root")
    with RootAnchor(root_path) as root:
        if arguments.command == "ensure-dir":
            ensure_directory(
                root, absolute(arguments.path, "directory"), arguments.mode
            )
        elif arguments.command == "private-dir":
            parent = absolute(arguments.parent, "private directory parent")
            path, metadata = private_directory(root, parent, arguments.prefix)
            print(path)
            if arguments.print_identity:
                print(metadata.st_dev)
                print(metadata.st_ino)
        elif arguments.command == "validate-file":
            validate_file(root, absolute(arguments.path, "file"))
        elif arguments.command == "dir-identity":
            dev, ino = directory_identity(
                root,
                absolute(arguments.path, "directory identity"),
                arguments.expected_dev,
                arguments.expected_ino,
            )
            print(dev, ino)
        elif arguments.command == "remove-tree":
            remove_tree(
                root,
                absolute(arguments.path, "cleanup tree"),
                arguments.expected_dev,
                arguments.expected_ino,
            )
        elif arguments.command == "copy-file":
            copy_file(
                root,
                absolute(arguments.source, "copy source"),
                absolute(arguments.destination, "copy destination"),
                arguments.mode,
            )
        elif arguments.command == "publish-file":
            publish_file(
                root,
                absolute(arguments.source, "publish source"),
                absolute(arguments.destination, "publish destination"),
                arguments.replace,
                arguments.expected_source_parent_dev,
                arguments.expected_source_parent_ino,
            )
        elif arguments.command == "publish-dir":
            if len(arguments.expected_tree_sha256) != 64 or any(
                character not in "0123456789abcdef"
                for character in arguments.expected_tree_sha256
            ):
                raise UnsafePath(
                    "expected tree SHA-256 must be 64 lowercase hex characters"
                )
            publish_directory(
                root,
                absolute(arguments.source, "directory publish source"),
                absolute(arguments.destination, "directory publish destination"),
                arguments.expected_tree_sha256,
                arguments.verify_command,
            )
        elif arguments.command == "recover-publish-dir":
            print(
                recover_directory_publish(
                    root,
                    absolute(arguments.source, "directory recovery source"),
                    absolute(
                        arguments.destination,
                        "directory recovery destination",
                    ),
                    None,
                )
            )
        elif arguments.command == "validate-published-dir":
            if arguments.expected_tree_sha256 is not None and (
                len(arguments.expected_tree_sha256) != 64
                or any(
                    character not in "0123456789abcdef"
                    for character in arguments.expected_tree_sha256
                )
            ):
                raise UnsafePath(
                    "expected tree SHA-256 must be 64 lowercase hex characters"
                )
            validate_published_directory(
                root,
                absolute(arguments.path, "published release generation"),
                absolute(arguments.marker, "published release marker"),
                arguments.expected_tree_sha256,
                arguments.verify_command,
            )
        elif arguments.command == "seal-tree":
            seal_tree(root, absolute(arguments.path, "tree"))
        elif arguments.command == "tree-digest":
            print(
                tree_digest(
                    root,
                    absolute(arguments.path, "tree"),
                    require_sealed=arguments.require_sealed,
                )
            )
        else:
            raise AssertionError(arguments.command)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, UnsafePath) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
