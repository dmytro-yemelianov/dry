# Ralph review decisions

## D0001 — Worktree-specific graph project

- **Decision:** Index this worktree as `dry-conscious-chess` rather than overwriting the existing global `dry` project.
- **Reason:** The existing project represented another checkout and stale HEAD; review evidence must match the frozen baseline.

## D0002 — Preserve the pre-existing dirty file

- **Decision:** Treat `docs/04-tasks.md` as user-owned baseline state. Review it where relevant, but do not modify or revert it.
- **Reason:** The file was already modified before Phase 0 began.

## D0003 — Review-only controller authority

- **Decision:** The Ralph controller owns state, routing, retries, and gate evidence but does not patch or self-certify.
- **Reason:** Material changes require a scoped implementation owner and an independent reviewer.

## D0004 — Standalone target wording

- **Decision:** Describe Wasm, Python, cloud, and verify-runner as independently validated roots with dedicated CI jobs; do not claim all four have unit tests.
- **Reason:** The cloud job currently formats, lints, and builds for wasm32 but has no unit-test step.
