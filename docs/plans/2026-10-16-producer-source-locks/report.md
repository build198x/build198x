# Build198x producer source-lock control

Both real native macOS ARM64 archives pass producer receipt verification at
`b87d9cb`. Extracted archive executables also pass the existing BASIC golden-byte
and ADF create/verify/info smoke checks. The two package versions remain 0.2.11;
cargo-dist remains 0.32.0. No release was published.

[Producer receipts](producer-receipts.json) retain actual source, committed/before/
after lock hashes, archive/executable identities and native target.
[Local verification](local-verification.json) retains independent Git-blob and
archive digest checks, smoke output identities and deliberate missing-receipt,
wrong-source and wrong-archive rejections. Removing the second package's receipt
cannot leave a partial successful verification.

The same record compares the exact old and new hosting expressions, holding
successful plan/local/global prerequisites fixed. Previously all six tested
qualification states allowed hosting because the hook ran after publication.
Now only success permits it; failure, skipped, cancelled, empty and missing
results reject. The checked generation wrapper preserves the complete pinned
upstream workflow and fails on template drift.

All thirteen local tests pass, including both separately tagged packages, the
two-package plan, all five archive architectures, before/after lock mutations,
failed packaging, absent committed locks and every missing/altered receipt field.
Ruff, formatting, complete regeneration and diff checks pass; the repository's
pre-push workspace Clippy hook passes. The first Windows CI run correctly
rejected a test fixture whose text writer produced CRLF; the fixture now writes
exact LF bytes. Production checkout bytes were already governed by the existing
`.gitattributes` and the guard was not weakened.

Fresh five-host receipt results and the existing two Linux Homebrew install,
checksum-rejection and output smoke results are inspected and retained with
[PR #66](https://github.com/build198x/build198x/pull/66) before review readiness.
PR builds exercise both CLI packages; normal release runs select the package
named by the tag. Single-package scope is independently covered by the controls.
Actual hosting/post-publication reconciliation is not exercised by a PR.

These are unsigned source-lock receipts, not per-binary compiled package/features
or signer evidence. They do not grant historical releases new claims. This is the
Build198x rollout of [#137](https://github.com/stevehill1981/198x/issues/137);
Emu198x remains separate. Task-owned compile caches, packaging output and report
downloads are removed before completion; compact evidence is retained here and
in the PR. Existing target caches are preserved.
