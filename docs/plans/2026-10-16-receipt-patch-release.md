# Prepare receipt-bearing patch 0.2.12

Steve explicitly approved preparing 0.2.12 on 2026-10-16. The only changes since
0.2.11 are producer controls #66; CI-only filtering correctly opens no automatic
release PR. Select this patch deliberately to publish new build-time receipts.
Do not modify release-plz filters or retrofit evidence onto older archives.

1. Bump the workspace in Cargo.toml and refresh Cargo.lock with offline metadata.
   Prove external package versions/checksums and mediaspec198x remain unchanged.
2. Author 0.2.12 notes for both CLI changelogs; preserve historical entries.
3. Run source-lock rejection tests, full workflow regeneration verification and
   locked metadata. Push a checked release PR, covering both CLI tags and the
   build198x-adf registry publication. Keep publication pending separate approval.
4. After approved publication, reconcile actual public archives/receipts against
   producer evidence. Record outcomes under umbrella #137; compiled dependency
   graph remains unknown and no umbrella release set changes automatically.

Use this existing checkout and its normal target cache; remove task-owned scratch.
No new dependency, runtime change or independently versioned library release.
