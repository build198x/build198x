# Recover native archives from existing tags

Approved 22 October 2026 after the workflow-token tag pushes did not start
native archive builds. Registry 0.2.13 is published and independently verified.
Do not move or recreate either existing tag.

1. Extend the checked generator with a required `release_tag` manual input.
   Accept only an existing stable build198x/build198x-adf version tag. Resolve
   its commit after checkout and require it to match HEAD before release work.
2. Use that tag for all five generated source checkouts and dist commands.
   Pass the source ref to the reusable package qualification workflow so its
   checks compare the same source/lock. Pass the selected tag to notifications;
   the dispatcher branch is not the released source or announcement tag.
3. Keep every producer receipt, host matrix, lock check and publication gate.
   Add rejection tests for missing/invalid refs and changed generator anchors.
   Run full regeneration and existing source-lock tests; push a reviewed PR.
4. After CI and merge, dispatch both existing 0.2.13 tags. Read qualification
   receipts, published archive checksums and actual CLI manifests; replay the
   native images and corruption controls. Preserve private media and clean
   disposable builds. Track unattended authentication separately in #79.

The local guard tests execute the exact workflow shell against real temporary
Git refs. They accept the selected tag and reject missing/invalid tags, branch
names, injected shell text and a checkout at a different commit. Ordinary PR
identity remains supported. Drift controls reject a missing checkout anchor.

All 18 Python tests pass, full regeneration matches, and Ruff passes. Actionlint
reports the same five upstream generated ShellCheck alerts as main: SC2086
(word splitting) and SC2129 (repeated redirects). No new alert was introduced;
those unrelated generated scripts are not changed here. Early checks caught an
over-broad Install dist anchor and a test counting source_ref as ref; the final
checks use exact anchors and indented checkout fields.
