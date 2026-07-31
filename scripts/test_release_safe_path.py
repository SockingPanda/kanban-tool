#!/usr/bin/env python3
"""Independent tests for the release-safe-path filesystem primitives.

The release filesystem is assumed to be writable only by a dedicated release
OS identity (or an equivalent isolated CI job).  These tests cover path
confinement, unknown-overwrite prevention, observable different-inode drift,
and cooperative concurrency.  They do not claim protection from hostile
same-UID processes, CAP_DAC_OVERRIDE, the mkdir-to-first-observation gap, or
same-inode move/mutate/put-back ABA.
"""

from __future__ import annotations

import os
import pathlib
import stat
import subprocess
import sys
import tempfile
import time
import unittest


HELPER = pathlib.Path(__file__).with_name("release-safe-path.py")


class ReleaseSafePathTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="kanban-release-safe-path-test."
        )
        self.addCleanup(self.temporary.cleanup)
        self.base = pathlib.Path(self.temporary.name)
        self.root = self.base / "root"
        self.root.mkdir(mode=0o700)

    def run_helper(
        self,
        *arguments: object,
        env: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        command = [
            sys.executable,
            "-B",
            os.fspath(HELPER),
            *(os.fspath(argument) for argument in arguments),
        ]
        return subprocess.run(
            command,
            check=False,
            capture_output=True,
            env=env,
            text=True,
            timeout=10,
        )

    def assert_rejected(
        self,
        *arguments: object,
        env: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        result = self.run_helper(*arguments, env=env)
        self.assertNotEqual(
            result.returncode,
            0,
            msg=f"unsafe operation succeeded: {result.args!r}",
        )
        self.assertTrue(
            result.stderr.startswith("error: "),
            msg=f"unsafe operation had no fail-closed diagnostic: {result.stderr!r}",
        )
        return result

    def paused_environment(
        self,
        pause_at: str,
        *,
        fail_at: str | None = None,
        trace: pathlib.Path | None = None,
    ) -> tuple[dict[str, str], pathlib.Path, pathlib.Path]:
        marker = self.base / f"{pause_at}.paused"
        continuation = self.base / f"{pause_at}.continue"
        environment = os.environ.copy()
        environment.update(
            {
                "KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT": pause_at,
                "KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER": os.fspath(marker),
                "KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE": os.fspath(continuation),
            }
        )
        if fail_at is not None:
            environment["KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT"] = fail_at
        if trace is not None:
            environment["KANBAN_RELEASE_SAFE_PATH_TEST_TRACE"] = os.fspath(trace)
        return environment, marker, continuation

    def start_helper(
        self,
        *arguments: object,
        env: dict[str, str],
    ) -> subprocess.Popen[str]:
        process = subprocess.Popen(
            [
                sys.executable,
                "-B",
                os.fspath(HELPER),
                *(os.fspath(argument) for argument in arguments),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            text=True,
        )
        self.addCleanup(
            lambda: process.kill() if process.poll() is None else None
        )
        return process

    def wait_for_pause(
        self,
        process: subprocess.Popen[str],
        marker: pathlib.Path,
    ) -> None:
        deadline = time.monotonic() + 5
        while not marker.exists() and process.poll() is None:
            if time.monotonic() >= deadline:
                self.fail("safe-path helper did not reach the injected race checkpoint")
            time.sleep(0.01)
        self.assertIsNone(
            process.poll(),
            msg="safe-path helper exited before the injected race checkpoint",
        )

    def test_absolute_paths_and_parent_traversal_are_required(self) -> None:
        self.assert_rejected(
            "ensure-dir",
            "--root",
            "relative-root",
            "--path",
            "relative-root/output",
        )
        traversing = os.fspath(self.root / "nested" / ".." / "escape")
        self.assert_rejected(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            traversing,
        )
        self.assertFalse((self.root / "escape").exists())

    def test_symlink_component_and_symlink_leaf_are_rejected(self) -> None:
        outside = self.base / "outside"
        outside.mkdir()
        (self.root / "linked").symlink_to(outside, target_is_directory=True)
        self.assert_rejected(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            self.root / "linked" / "escaped",
        )
        self.assertFalse((outside / "escaped").exists())

        regular = self.root / "regular"
        regular.write_text("evidence\n", encoding="utf-8")
        linked_file = self.root / "linked-file"
        linked_file.symlink_to(regular)
        self.assert_rejected(
            "validate-file",
            "--root",
            self.root,
            "--path",
            linked_file,
        )

    def test_non_directory_component_is_rejected(self) -> None:
        component = self.root / "not-a-directory"
        component.write_text("sentinel\n", encoding="utf-8")
        self.assert_rejected(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            component / "child",
        )
        self.assertEqual(component.read_text(encoding="utf-8"), "sentinel\n")

    def test_copy_is_no_overwrite_and_preserves_existing_destination(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"new payload\n")
        destination = self.root / "destination"
        destination.write_bytes(b"existing payload\n")
        before = destination.stat()

        self.assert_rejected(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
        )

        after = destination.stat()
        self.assertEqual(destination.read_bytes(), b"existing payload\n")
        self.assertEqual((before.st_dev, before.st_ino), (after.st_dev, after.st_ino))

    def test_hardlinked_source_is_rejected_without_mutation(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"hardlinked source\n")
        alias = self.base / "source-alias"
        os.link(source, alias)
        destination = self.root / "destination"

        self.assert_rejected(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
        )

        self.assertEqual(source.read_bytes(), b"hardlinked source\n")
        self.assertEqual(alias.read_bytes(), b"hardlinked source\n")
        self.assertEqual(source.stat().st_nlink, 2)
        self.assertFalse(destination.exists())

    def test_validate_file_rejects_hardlinked_leaf_without_mutation(self) -> None:
        leaf = self.root / "leaf"
        leaf.write_bytes(b"hardlinked validation leaf\n")
        alias = self.root / "leaf-alias"
        os.link(leaf, alias)

        self.assert_rejected(
            "validate-file",
            "--root",
            self.root,
            "--path",
            leaf,
        )

        self.assertEqual(leaf.read_bytes(), b"hardlinked validation leaf\n")
        self.assertEqual(alias.read_bytes(), b"hardlinked validation leaf\n")
        self.assertEqual(leaf.stat().st_nlink, 2)

    def test_copy_rejects_hardlinked_destination_without_mutation(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"new copy bytes\n")
        destination = self.root / "destination"
        destination.write_bytes(b"hardlinked destination\n")
        alias = self.base / "destination-alias"
        os.link(destination, alias)

        self.assert_rejected(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
        )

        self.assertEqual(destination.read_bytes(), b"hardlinked destination\n")
        self.assertEqual(alias.read_bytes(), b"hardlinked destination\n")
        self.assertEqual(destination.stat().st_nlink, 2)

    def test_safe_directory_creation_and_copy(self) -> None:
        destination_parent = self.root / "one" / "two"
        created = self.run_helper(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            destination_parent,
            "--mode",
            "0750",
        )
        self.assertEqual(created.returncode, 0, msg=created.stderr)
        self.assertTrue(destination_parent.is_dir())
        self.assertEqual(stat.S_IMODE(destination_parent.stat().st_mode), 0o750)

        source = self.base / "source"
        source.write_bytes(b"stable release bytes\n")
        destination = destination_parent / "copied"
        copied = self.run_helper(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0640",
        )
        self.assertEqual(copied.returncode, 0, msg=copied.stderr)
        self.assertEqual(destination.read_bytes(), source.read_bytes())
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o640)
        self.assertEqual(destination.stat().st_nlink, 1)

    def test_private_directory_identity_token_rejects_replacement(self) -> None:
        parent = self.root / "private-token"
        parent.mkdir()

        created = self.run_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            parent,
            "--prefix",
            ".stage.",
            "--print-identity",
        )
        self.assertEqual(created.returncode, 0, msg=created.stderr)
        token = created.stdout.splitlines()
        self.assertEqual(len(token), 3, msg=created.stdout)
        destination = pathlib.Path(token[0])
        expected_dev = int(token[1])
        expected_ino = int(token[2])
        observed = destination.stat()
        self.assertEqual(
            (expected_dev, expected_ino),
            (observed.st_dev, observed.st_ino),
        )

        matched = self.run_helper(
            "dir-identity",
            "--root",
            self.root,
            "--path",
            destination,
            "--expected-dev",
            str(expected_dev),
            "--expected-ino",
            str(expected_ino),
        )
        self.assertEqual(matched.returncode, 0, msg=matched.stderr)
        self.assertEqual(
            matched.stdout.split(),
            [str(expected_dev), str(expected_ino)],
        )

        detached = parent / "detached-original"
        destination.rename(detached)
        destination.mkdir(mode=0o700)
        replacement = destination.stat()
        self.assertNotEqual(
            (replacement.st_dev, replacement.st_ino),
            (expected_dev, expected_ino),
        )

        rejected = self.assert_rejected(
            "dir-identity",
            "--root",
            self.root,
            "--path",
            destination,
            "--expected-dev",
            str(expected_dev),
            "--expected-ino",
            str(expected_ino),
        )
        self.assertIn("does not match expected token", rejected.stderr)

        cleanup_rejected = self.assert_rejected(
            "remove-tree",
            "--root",
            self.root,
            "--path",
            destination,
            "--expected-dev",
            str(expected_dev),
            "--expected-ino",
            str(expected_ino),
        )
        self.assertIn(
            "does not match expected identity observation token",
            cleanup_rejected.stderr,
        )
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_directory_identity_requires_complete_expected_token(self) -> None:
        destination = self.root / "identity-token"
        destination.mkdir()
        observed = destination.stat()

        for arguments in (
            ("--expected-dev", str(observed.st_dev)),
            ("--expected-ino", str(observed.st_ino)),
        ):
            with self.subTest(arguments=arguments):
                rejected = self.assert_rejected(
                    "dir-identity",
                    "--root",
                    self.root,
                    "--path",
                    destination,
                    *arguments,
                )
                self.assertIn(
                    "expected directory identity requires both",
                    rejected.stderr,
                )

    def test_publish_file_binds_retained_source_parent_identity(self) -> None:
        output = self.root / "output"
        output.mkdir()
        created = self.run_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            self.root,
            "--prefix",
            ".publish-stage.",
            "--print-identity",
        )
        self.assertEqual(created.returncode, 0, msg=created.stderr)
        token = created.stdout.splitlines()
        self.assertEqual(len(token), 3, msg=created.stdout)
        stage = pathlib.Path(token[0])
        expected_dev = int(token[1])
        expected_ino = int(token[2])
        source = stage / "artifact"
        destination = output / "artifact"
        source.write_bytes(b"original artifact\n")

        prechecked = self.run_helper(
            "dir-identity",
            "--root",
            self.root,
            "--path",
            stage,
            "--expected-dev",
            str(expected_dev),
            "--expected-ino",
            str(expected_ino),
        )
        self.assertEqual(prechecked.returncode, 0, msg=prechecked.stderr)

        detached = self.root / "original-stage"
        stage.rename(detached)
        stage.mkdir(mode=0o700)
        source.write_bytes(b"replacement artifact\n")

        rejected = self.assert_rejected(
            "publish-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--expected-source-parent-dev",
            str(expected_dev),
            "--expected-source-parent-ino",
            str(expected_ino),
        )
        self.assertIn(
            "publish source parent identity does not match expected "
            "identity observation token",
            rejected.stderr,
        )
        self.assertEqual(source.read_bytes(), b"replacement artifact\n")
        self.assertEqual(
            (detached / "artifact").read_bytes(),
            b"original artifact\n",
        )
        self.assertFalse(destination.exists())

    def test_publish_file_source_parent_identity_arguments(self) -> None:
        output = self.root / "publish-output"
        output.mkdir()
        created = self.run_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            self.root,
            "--prefix",
            ".valid-publish-stage.",
            "--print-identity",
        )
        self.assertEqual(created.returncode, 0, msg=created.stderr)
        token = created.stdout.splitlines()
        self.assertEqual(len(token), 3, msg=created.stdout)
        stage = pathlib.Path(token[0])
        source = stage / "artifact"
        destination = output / "bound-artifact"
        source.write_bytes(b"bound artifact\n")

        published = self.run_helper(
            "publish-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--expected-source-parent-dev",
            token[1],
            "--expected-source-parent-ino",
            token[2],
        )
        self.assertEqual(published.returncode, 0, msg=published.stderr)
        self.assertFalse(source.exists())
        self.assertEqual(destination.read_bytes(), b"bound artifact\n")

        unbound_stage = self.root / "unbound-publish-stage"
        unbound_stage.mkdir(mode=0o700)
        unbound_source = unbound_stage / "artifact"
        unbound_destination = output / "unbound-artifact"
        unbound_source.write_bytes(b"unbound compatibility artifact\n")
        unbound = self.run_helper(
            "publish-file",
            "--root",
            self.root,
            "--source",
            unbound_source,
            "--destination",
            unbound_destination,
        )
        self.assertEqual(unbound.returncode, 0, msg=unbound.stderr)
        self.assertFalse(unbound_source.exists())
        self.assertEqual(
            unbound_destination.read_bytes(),
            b"unbound compatibility artifact\n",
        )

        invalid_stage = self.root / "invalid-publish-stage"
        invalid_stage.mkdir(mode=0o700)
        invalid_source = invalid_stage / "artifact"
        invalid_source.write_bytes(b"invalid token artifact\n")
        for arguments, expected_message in (
            (
                ("--expected-source-parent-dev", "1"),
                "requires both --expected-source-parent-dev",
            ),
            (
                (
                    "--expected-source-parent-dev",
                    "-1",
                    "--expected-source-parent-ino",
                    "1",
                ),
                "identity values must be non-negative",
            ),
        ):
            with self.subTest(arguments=arguments):
                rejected = self.assert_rejected(
                    "publish-file",
                    "--root",
                    self.root,
                    "--source",
                    invalid_source,
                    "--destination",
                    output / "invalid-artifact",
                    *arguments,
                )
                self.assertIn(expected_message, rejected.stderr)
                self.assertEqual(
                    invalid_source.read_bytes(),
                    b"invalid token artifact\n",
                )
                self.assertFalse((output / "invalid-artifact").exists())

    def test_copy_fails_closed_when_public_parent_is_replaced(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"race payload\n")
        stage = self.root / "stage"
        stage.mkdir()
        detached = self.root / "detached"
        outside = self.base / "outside"
        outside.mkdir()
        destination = stage / "copied"

        environment, marker, continuation = self.paused_environment(
            "copy-file-data"
        )
        process = self.start_helper(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        stage.rename(detached)
        stage.symlink_to(outside, target_is_directory=True)
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertFalse((outside / "copied").exists())
        self.assertFalse((detached / "copied").exists())

    def test_copy_success_path_rejects_public_leaf_replacement(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"owned release payload\n")
        destination = self.root / "destination"
        detached = self.root / "detached-owned"
        environment, marker, continuation = self.paused_environment(
            "copy-file-data"
        )
        process = self.start_helper(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        destination.rename(detached)
        destination.write_bytes(b"attacker payload\n")
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertEqual(destination.read_bytes(), b"attacker payload\n")
        self.assertEqual(detached.read_bytes(), b"owned release payload\n")

    def test_copy_commit_recheck_rejects_late_leaf_replacement(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"owned release payload\n")
        destination = self.root / "destination"
        detached = self.root / "detached-owned"
        environment, marker, continuation = self.paused_environment(
            "copy-file-parent"
        )
        process = self.start_helper(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        destination.rename(detached)
        destination.write_bytes(b"late attacker payload\n")
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertEqual(destination.read_bytes(), b"late attacker payload\n")
        self.assertEqual(detached.read_bytes(), b"owned release payload\n")

    def test_copy_error_cleanup_does_not_unlink_replacement(self) -> None:
        source = self.base / "source"
        source.write_bytes(b"original source\n")
        destination = self.root / "destination"
        detached = self.root / "detached-owned"
        environment, marker, continuation = self.paused_environment(
            "copy-file-data"
        )
        process = self.start_helper(
            "copy-file",
            "--root",
            self.root,
            "--source",
            source,
            "--destination",
            destination,
            "--mode",
            "0644",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        destination.rename(detached)
        destination.write_bytes(b"attacker payload\n")
        source.write_bytes(b"mutated source with a different size\n")
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertEqual(destination.read_bytes(), b"attacker payload\n")
        self.assertEqual(detached.read_bytes(), b"original source\n")

    def test_ensure_directory_cleanup_preserves_replacement(self) -> None:
        destination = self.root / "created"
        detached = self.root / "detached-created"
        environment, marker, continuation = self.paused_environment(
            "ensure-dir-new",
            fail_at="ensure-dir-parent",
        )
        process = self.start_helper(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            destination,
            "--mode",
            "0755",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_ensure_directory_post_observation_replacement_is_rejected(self) -> None:
        destination = self.root / "observed-created"
        detached = self.root / "observed-detached"
        environment, marker, continuation = self.paused_environment(
            "ensure-dir-after-observe-before-open"
        )
        process = self.start_helper(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            destination,
            "--mode",
            "0755",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_ensure_directory_post_observation_error_preserves_replacement(
        self,
    ) -> None:
        destination = self.root / "observed-error-created"
        detached = self.root / "observed-error-detached"
        environment, marker, continuation = self.paused_environment(
            "ensure-dir-after-observe-before-open",
            fail_at="ensure-dir-new",
        )
        process = self.start_helper(
            "ensure-dir",
            "--root",
            self.root,
            "--path",
            destination,
            "--mode",
            "0755",
            env=environment,
        )
        self.wait_for_pause(process, marker)

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_private_directory_cleanup_preserves_replacement(self) -> None:
        parent = self.root / "private"
        parent.mkdir()
        trace = self.base / "private.trace"
        detached = self.root / "detached-private"
        environment, marker, continuation = self.paused_environment(
            "private-dir-new",
            fail_at="private-dir-parent",
            trace=trace,
        )
        process = self.start_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            parent,
            "--prefix",
            ".stage.",
            env=environment,
        )
        self.wait_for_pause(process, marker)
        private_paths = [
            pathlib.Path(line.split("\t", 1)[1])
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line.startswith("private-dir-new\t")
        ]
        self.assertEqual(len(private_paths), 1)
        destination = private_paths[0]

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_private_directory_success_rejects_leaf_replacement(self) -> None:
        parent = self.root / "private-success"
        parent.mkdir()
        trace = self.base / "private-success.trace"
        detached = self.root / "detached-private-success"
        environment, marker, continuation = self.paused_environment(
            "private-dir-parent",
            trace=trace,
        )
        process = self.start_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            parent,
            "--prefix",
            ".stage.",
            env=environment,
        )
        self.wait_for_pause(process, marker)
        private_paths = [
            pathlib.Path(line.split("\t", 1)[1])
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line.startswith("private-dir-new\t")
        ]
        self.assertEqual(len(private_paths), 1)
        destination = private_paths[0]

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_private_directory_post_observation_replacement_is_rejected(self) -> None:
        parent = self.root / "private-observed"
        parent.mkdir()
        trace = self.base / "private-observed.trace"
        detached = self.root / "private-observed-detached"
        environment, marker, continuation = self.paused_environment(
            "private-dir-after-observe-before-open",
            trace=trace,
        )
        process = self.start_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            parent,
            "--prefix",
            ".stage.",
            env=environment,
        )
        self.wait_for_pause(process, marker)
        private_paths = [
            pathlib.Path(line.split("\t", 1)[1])
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line.startswith("private-dir-after-observe-before-open\t")
        ]
        self.assertEqual(len(private_paths), 1)
        destination = private_paths[0]

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())

    def test_private_directory_post_observation_error_preserves_replacement(
        self,
    ) -> None:
        parent = self.root / "private-observed-error"
        parent.mkdir()
        trace = self.base / "private-observed-error.trace"
        detached = self.root / "private-observed-error-detached"
        environment, marker, continuation = self.paused_environment(
            "private-dir-after-observe-before-open",
            fail_at="private-dir-new",
            trace=trace,
        )
        process = self.start_helper(
            "private-dir",
            "--root",
            self.root,
            "--parent",
            parent,
            "--prefix",
            ".stage.",
            env=environment,
        )
        self.wait_for_pause(process, marker)
        private_paths = [
            pathlib.Path(line.split("\t", 1)[1])
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line.startswith("private-dir-after-observe-before-open\t")
        ]
        self.assertEqual(len(private_paths), 1)
        destination = private_paths[0]

        destination.rename(detached)
        destination.mkdir()
        continuation.write_text("continue\n", encoding="utf-8")
        _, stderr = process.communicate(timeout=10)

        self.assertNotEqual(process.returncode, 0, msg=stderr)
        self.assertTrue(stderr.startswith("error: "), msg=stderr)
        self.assertTrue(destination.is_dir())
        self.assertTrue(detached.is_dir())


if __name__ == "__main__":
    unittest.main(verbosity=2)
