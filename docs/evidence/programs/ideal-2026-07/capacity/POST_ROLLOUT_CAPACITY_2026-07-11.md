# Ideal Program Post-Rollout Capacity Snapshot

Date: 2026-07-11

Command: `bv --robot-capacity --agents 3 --capacity-label ideal-2026-07 -f json`

Source data hash: `19876d013a3d9d45` (`bv` v0.15.2).

The current graph contains 164 open nodes and 170,402 estimated minutes. The
three-agent structural simulation reports 18,497 serial minutes, 151,905
parallel minutes and 89.15% parallelizable work. Its 144.025-day projection is
planning evidence, not a calendar commitment: it includes non-claimable epics
and support nodes, and claims continue to come only from `br ready`.

The 17-node structural critical path is:

`bd-59co -> bd-59co.3 -> bd-59co.3.1 -> bd-59co.3.1.3 -> bd-59co.3.1.2 -> bd-59co.3.15.5 -> bd-59co.3.15.12 -> bd-59co.3.15.13 -> bd-59co.3.15.14 -> bd-59co.3.15.3 -> bd-59co.3.15.21 -> bd-59co.3.15.22 -> bd-59co.3.15.23 -> bd-59co.3.15.30 -> bd-59co.3.15.32 -> bd-59co.3.15.34 -> bd-59co.3.15.2`.

The first three-worker wave therefore includes `bd-59co.3.1.3`, the first
claimable critical-path leaf, alongside the two highest-impact ready Core
delivery leaves: `bd-2cjy` and `bd-59co.2.2.5`. This respects the two-Rust-writer
ceiling. Workspace-wide Cargo remains serialized by the controller.

The capacity and critical path are refreshed at each epic boundary.
