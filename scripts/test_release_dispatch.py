"""Run the actual dispatch guard against real Git refs and wrong checkouts."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from release_dispatch import IDENTITY_SCRIPT, REPLACEMENTS, add_dispatch


class DispatchTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory(prefix="build198x-dispatch-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.git("init", "--quiet")
        (self.root / "fixture").write_text("source\n")
        self.commit()
        self.git("tag", "build198x-adf-v0.2.13")

    def git(self, *args: str) -> bytes:
        return subprocess.run(
            ["git", *args], cwd=self.root, check=True, capture_output=True, timeout=10
        ).stdout

    def commit(self) -> None:
        self.git("add", "fixture")
        # Only this disposable fixture lacks a signer; commit hooks remain enabled.
        self.git(
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Fixture source",
        )

    def guard(
        self, tag: str, event: str = "workflow_dispatch"
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", "-c", IDENTITY_SCRIPT],
            cwd=self.root,
            env={
                **os.environ,
                "RELEASE_TAG": tag,
                "RELEASE_EVENT": event,
                "GITHUB_OUTPUT": str(self.root / "output"),
            },
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )

    def test_existing_tag_binds_the_actual_commit(self) -> None:
        self.assertEqual(self.guard("build198x-adf-v0.2.13").returncode, 0)
        self.assertEqual(
            (self.root / "output").read_text(),
            f"commit={self.git('rev-parse', 'HEAD').decode().strip()}\n",
        )

    def test_bad_or_missing_tag_cannot_reach_release_work(self) -> None:
        for tag in (
            "",
            "main",
            "refs/tags/build198x-adf-v0.2.13",
            "other-v0.2.13",
            "build198x-v0.2.99",
            "build198x-adf-v0.2.13;echo unsafe",
        ):
            with self.subTest(tag=tag):
                self.assertNotEqual(self.guard(tag).returncode, 0)
                self.assertFalse((self.root / "output").exists())

    def test_wrong_checkout_is_rejected(self) -> None:
        (self.root / "fixture").write_text("different source\n")
        self.commit()
        self.assertNotEqual(self.guard("build198x-adf-v0.2.13").returncode, 0)
        self.assertFalse((self.root / "output").exists())

    def test_ordinary_events_retain_their_checkout_identity(self) -> None:
        self.assertEqual(self.guard("", "pull_request").returncode, 0)
        self.assertIn(
            self.git("rev-parse", "HEAD").decode().strip(),
            (self.root / "output").read_text(),
        )

    def test_missing_checkout_anchor_is_not_silently_dropped(self) -> None:
        root = Path(__file__).resolve().parent.parent
        original = (root / ".github/workflows/build198x-release.yml").read_text()
        for old, new, _ in reversed(REPLACEMENTS):
            original = original.replace(new, old)
        with self.assertRaises(ValueError):
            add_dispatch(
                original.replace("          persist-credentials: false\n", "", 1)
            )
        output = add_dispatch(original)
        self.assertEqual(
            output.count("          ref: ${{ inputs.release_tag || github.ref }}"), 1
        )
        self.assertIn("source_ref: ${{ needs.plan.outputs.source }}", output)
        self.assertEqual(
            output.count("          ref: ${{ needs.plan.outputs.source }}"), 4
        )
        self.assertIn('RELEASE_COMMIT: "${{ needs.plan.outputs.source }}"', output)


if __name__ == "__main__":
    unittest.main()
