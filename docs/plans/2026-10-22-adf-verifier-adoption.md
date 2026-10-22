# Adopt the native bitmap fix

Update the two existing ADF consumers after Format198x 0.3.4 is published.
This is an approved dependency update, not a new dependency or API.

1. Set the ADF requirement in `crates/build198x/Cargo.toml` and
   `crates/build198x-adf/Cargo.toml` to 0.3.4. Update only that package in
   `Cargo.lock`; inspect the diff for unrelated changes.
2. Update the standalone CLI's library label in
   `crates/build198x-adf/src/main.rs`. Add a regression that compares the emitted
   label with the resolved ADF version in Cargo.lock. Verify it fails with an
   incorrect label and passes with the correct one.
3. Run workspace tests, formatting and Clippy. Build both CLI paths. Verify
   retained native images unchanged, require empty problem lists, and retain
   the live-block, final-padding and stale-checksum corruption controls.
4. Write a short changelog entry, push a checked PR, merge under the approved
   delivery scope, and review the generated patch release. Verify its published
   archive and CLI metadata before closing AMOS issue #6.

Keep the private media outside Git. Bound build/test runs. Remove disposable
disk copies, downloaded archives and task-owned processes when finished. Do not
change conversion, mastering, parser or emulator behaviour in this work.
