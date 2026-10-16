#!/usr/bin/env python3
"""Regenerate complete pinned cargo-dist CI with checked producer/publication guards."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path

WORKFLOW = Path(".github/workflows/build198x-release.yml")
PERMISSIVE = "(needs.custom-test-linux-packages.result == 'skipped' || needs.custom-test-linux-packages.result == 'success')"
REQUIRED = "(needs.custom-test-linux-packages.result == 'success')"
BUILD = "          dist build ${{ needs.plan.outputs.tag-flag }} --print=linkage --output-format=json ${{ matrix.dist_args }} > dist-manifest.json"
GUARDED_BUILD = "          python scripts/source_lock.py build \"${{ join(matrix.targets, ',') }}\" dist-manifest.json target/distrib -- dist build ${{ needs.plan.outputs.tag-flag }} --print=linkage --output-format=json ${{ matrix.dist_args }}"


def transform(text: str) -> str:
    dependency = "  custom-test-linux-packages:\n    needs:\n      - plan\n      - build-local-artifacts\n"
    rust = "      - name: Install Rust non-interactively if not already installed"
    upload = "          name: artifacts-build-local-${{ join(matrix.targets, '_') }}\n          path: |\n            ${{ steps.cargo-dist.outputs.paths }}\n            ${{ env.BUILD_MANIFEST_NAME }}"
    for anchor in (PERMISSIVE, dependency, rust, upload, BUILD):
        if text.count(anchor) != 1:
            raise ValueError(
                "cargo-dist template changed; review producer/publication guards"
            )
    return (
        text.replace(PERMISSIVE, REQUIRED)
        .replace(dependency, dependency + "      - build-global-artifacts\n")
        .replace(BUILD, GUARDED_BUILD)
        .replace(
            rust,
            "      - uses: actions/setup-python@v6\n        with:\n          python-version: '3.13'\n"
            + rust,
        )
        .replace(
            upload,
            upload
            + "\n            target/distrib/*-${{ join(matrix.targets, ',') }}-source-lock.json",
        )
    )


def generate(root: Path, check: bool) -> None:
    files = (
        subprocess.run(
            [
                "git",
                "-c",
                "core.fsmonitor=false",
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
            cwd=root,
            capture_output=True,
            check=True,
            timeout=10,
        )
        .stdout.decode()
        .split("\0")
    )
    with tempfile.TemporaryDirectory(prefix="build198x-release-generation-") as tmp:
        copy = Path(tmp)
        for name in set(files) - {""}:
            source = root / name
            if source.is_file() and not source.is_symlink():
                destination = copy / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, destination)
        cargo = copy / "Cargo.toml"
        config = cargo.read_text()
        if config.count('allow-dirty = ["ci"]') != 1:
            raise ValueError("missing checked generation exception")
        cargo.write_text(config.replace('allow-dirty = ["ci"]', "allow-dirty = []"))
        subprocess.run(
            ["dist", "generate", "--mode", "ci"],
            cwd=copy,
            check=True,
            timeout=120,
            capture_output=True,
        )
        result = transform((copy / WORKFLOW).read_text())
        if check:
            if (root / WORKFLOW).read_text() != result:
                raise ValueError(
                    "release workflow is stale; run python scripts/generate_release_workflow.py"
                )
        else:
            (root / WORKFLOW).write_text(result)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    generate(Path(__file__).resolve().parent.parent, parser.parse_args().check)
