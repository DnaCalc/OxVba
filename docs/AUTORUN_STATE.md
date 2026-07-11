# Execution Control

Purpose: the sole volatile control surface for current execution mode, accepted worksets, terminal condition and resume context.

Mode: AutoRun
Active program manifest: docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json
Program root: bd-59co
Control epic: bd-59co.1
AutoRun terminal gate: bd-59co
Queue certification: certified — three first-wave leaves are active; the actual ready queue contains exactly the seven remaining current leaves listed below and no stale work.
Claim queue: br ready -l ideal-2026-07 -t task
Current tasks: `bd-2cjy` (SafeArray strict-Clippy/ownership), `bd-59co.2.2.5` (isolated carrier-balance protocol), and `bd-59co.3.1.3` (x64 matrix stewardship/authority).
Certified ready queue: `bd-59co.2.2.4`, `bd-59co.2.2.7`, `bd-59co.2.2.8`, `bd-59co.2.2.13`, `bd-59co.3.1.4`, `bd-59co.3.1.5`, and `bd-59co.3.15.4`.
Current capacity snapshot: `docs/evidence/programs/ideal-2026-07/capacity/POST_ROLLOUT_CAPACITY_2026-07-11.md` (three agents, source hash `19876d013a3d9d45`).
Terminal condition: all Core, Windows x64 and IDE profile roots close beneath `bd-59co`, or every remaining path is genuinely blocked and recorded through the repository blocker protocol.
Last completed tasks: rollout leaves `bd-59co.2.2.1`, `bd-59co.3.1.1`, and `bd-59co.3.15.1` closed after clean graph, truth, negative-fixture and independent semantic review gates.

## Accepted umbrella and worksets under AutoRun

- umbrella bead: `bd-59co`

- `docs/worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md`
- `docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md`
- `docs/worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`

## Current truth surfaces

- `CHARTER.md`
- `OPERATIONS.md`
- `docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md`
- `docs/ARCHITECTURE.md`
- `docs/OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`

Historical ladders, IP-08/IP-08B plans, MACH-1000 profiles and earlier JIT/frontend/language-service worksets remain provenance only unless PROGRAM-0 explicitly imports a residual into `bd-59co`.

## Resume rule

Read the current truth surfaces and active bead, then claim work only from `br ready -l ideal-2026-07 -t task`. Respect the two-Rust-writer ceiling and serialize workspace-wide Cargo, Excel/VBE automation, registry mutation, certification-VM provisioning and large JIT/VM3/differential/rt-abi writers. Use `bv` only for topology/capacity, refresh its critical path at each epic boundary, and never use it as a claim source. Continue implement -> evidence/docs -> checks -> independent fresh-eyes -> bead truth -> commit/push cycles until the terminal condition.
