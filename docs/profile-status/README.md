# Profile Status Archive

This directory stores the versioned `PROFILE_STATUS_V*.md` records.

Usage policy:
- Keep these as immutable historical gate records for each profile step.
- Treat them as reference/provenance docs, not active implementation guidance.
- Put active status and orientation material in `docs/PHASE12_STATUS.md`,
  `docs/IMPLEMENTATION_LOG.md`, and `docs/status-tours/`.
- Treat `docs/AUTORUN_STATE.md` as a minimal active-ladder control file only, not a narrative status rollup.

When adding a new profile status file:
1. Add `PROFILE_STATUS_V<version>.md` here.
2. Add or update links in `docs/README.md`.
3. Keep narrative walkthroughs in `docs/status-tours/`.

Current published range includes historical files through `PROFILE_STATUS_V466.md`.
