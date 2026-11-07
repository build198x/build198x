# Recover Apple Silicon release packaging

Both 0.2.15 release runs stop before starting Apple Silicon packaging. GitHub's annotation says `The configuration 'macos-14-arm64-us-default' is not supported`; the runner was retired on 2 November 2026 (https://github.blog/changelog/2026-10-01-github-actions-macos-14-runner-image-retirement/). No build step ran. Other targets are not the cause.

1. In `scripts/generate_release_workflow.py`, extend the checked template transform so the packaging job maps only the retired `macos-14` runner to stable `macos-15`. Fail if the single upstream runner anchor drifts. This lets the existing immutable-tag dispatch recover already-tagged sources without moving tags or changing source-lock evidence.
2. Add an explicit Apple Silicon `macos-15` override in `Cargo.toml` for future plans and regenerate `.github/workflows/build198x-release.yml` with the pinned generator. Keep all target hosts, producer receipts and publication gates.
3. In `scripts/test_source_lock.py`, exercise the actual workflow runner expression against the retired label and unchanged Intel/Linux/Windows labels. Establish red first; retain full regeneration/drift and dispatch rejection controls. Run all Python tests, Ruff, regeneration and actionlint.
4. Land a checked PR after all current checks and candidate qualification. Dispatch existing `build198x-v0.2.15` and `build198x-adf-v0.2.15` tags from the repaired workflow. Require all native producer/qualification jobs before publication; download the Apple Silicon archive, verify checksum and receipt, run version/tape smoke against the committed fixture, then clean task workspaces and caches.

No dependency upgrade, tag rewrite, dropped target or relaxed release guard. This is release recovery; it does not change the tagged program source.
