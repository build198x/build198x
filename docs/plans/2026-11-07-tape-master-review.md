# Complete the approved tape master

Continue the agreed Spectrum tape-master design in PR #92 without widening its scope.

1. Review `crates/build198x/src/tape/sinclair_zx_spectrum.rs`, the `tape_command` path in `main.rs`, `tests/tape_cli.rs`, and the updated demand-gate record against the approved generated/authored loader behaviour.
2. Reproduce the public loader-floor helper's suspected arithmetic overflow with an oversized loader length before changing it. If confirmed, make impossible lengths remain above any Spectrum load address; retain the measured valid-load boundary and add an invariant regression.
3. Run tape unit/CLI tests, formatting and scoped Clippy. Rebuild the committed tape example byte for byte, retaining refusal tests for existing outputs and input aliases.
4. Push only the reviewed change and this plan onto the existing PR, recheck complete CI/native candidate qualification, and rebase-merge if green. Describe the release for readers and verify the resulting native archive before switching curriculum consumers.

Use the isolated review worktree and a task-owned target directory. Preserve the shared release checkout. Remove the worktree/cache only after verifying remote retention.
