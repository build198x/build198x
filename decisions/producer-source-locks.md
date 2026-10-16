# Release artifacts require producer source-lock evidence

**Status:** Active for the Build198x first rollout of umbrella #137, agreed on
2026-10-16. Follows the producer check implemented in Asm198x #684.

Both package tags and the two-package PR plan retain unsigned per-package,
per-target receipts. The packaging command must succeed and Cargo.lock must match
the checkout commit before and after it runs. Receipts bind committed/before/after
lock hashes, source commit, version, target, producer run/attempt/job and actual
archive/extracted-executable hashes. Empty plans, omitted packages, stale receipts
and missing/changed identities fail. This is source-lock evidence, not a compiled
per-binary dependency/features graph or a cryptographic signer claim.

Before hosting, the existing reusable package job independently verifies every
selected receipt on all five native hosts. Existing Homebrew install, checksum
rejection and native-output smoke tests remain required on both Linux hosts.
Qualification cannot be skipped, missing, cancelled or failed and still publish.
Receipts travel with producer artifacts and the existing release asset upload.
No release is implicitly authorised by this decision.

This narrowly overrides the direct `dist generate` CI maintenance instruction in
umbrella `decisions/project-skeleton-standard.md` and the former Cargo.toml comment.
Use `python scripts/generate_release_workflow.py` (or `--check`). cargo-dist 0.32.0
allows skipped custom jobs to reach hosting; the checked wrapper preserves its
full output, makes the package job wait for global artifacts, requires its success
at hosting, wraps local packaging and uploads receipts. It rejects changed template
anchors. `allow-dirty = ["ci"]` delegates complete comparison to this wrapper;
only its disposable generation copy removes the exception. Every candidate job
checks full regeneration. Revisit when upstream supplies the same strict gate.

Historical releases acquire no receipt or new qualification claim from this
change. Actual post-publication reconciliation needs a future authorised release;
this rollout does not manufacture build-time evidence for old binaries.
