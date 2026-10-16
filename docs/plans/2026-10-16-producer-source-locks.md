# Bind Build198x packages to the producer lock

Implement the approved Build198x rollout of umbrella #137 after Asm198x #684.
Keep cargo-dist 0.32.0, both package versions, fixtures and dependencies unchanged.

1. Add stdlib Python producer/receipt checks in `scripts/source_lock.py` with
   executable controls in `scripts/test_source_lock.py`. Match committed lock
   bytes before/after packaging. Bind each actual selected package archive and
   executable to source, lock, target and producer run/attempt/job. Support both
   CLI tags and the two-CLI PR plan, without zero-artifact/partial-package passes.
2. Adopt the existing family's checked generation-wrapper approach in
   `scripts/generate_release_workflow.py`: preserve upstream output, wrap local
   builds and upload receipts, require global package checks before hosting.
   Move the current host-stage hook to global-artifacts and require success.
   Document this narrow generator override in a local decision.
3. Extend `.github/workflows/test-linux-packages.yml` to five native receipt
   checks, retaining Homebrew install/smoke/checksum checks on both Linux hosts.
   Native PR builds use the same packaging guard; release qualification consumes
   the actual downloaded artifacts. Retain bounded JSON evidence for 90 days.
4. Verify source/receipt rejection controls, exact regeneration, and a real native
   package. Commit/push a focused PR; inspect all five host reports, source lock
   and artifact identities plus existing Linux output tests before review.
   Do not publish a release, merge without approval or change Emu198x here.

The receipt is unsigned source-lock evidence, not a compiled per-binary graph.
Scratch uses task-owned `target/producer-source-locks/`, `target/distrib/`, and
new native dist caches only; existing target caches are preserved. Test/generation
scratch must be removed by context managers. Clean task output before completion;
retain compact evidence in Git/PR records. No services or worktrees are planned.
