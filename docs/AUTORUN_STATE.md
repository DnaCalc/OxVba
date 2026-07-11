# Execution Control

Purpose: the sole volatile control surface for current execution mode, accepted worksets, terminal condition and resume context.

Mode: AutoRun
Active program manifest: docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json
Program root: bd-59co
Control epic: bd-59co.1
AutoRun terminal gate: bd-59co
Queue certification: certified — three current leaves are active; the actual ready queue contains exactly the seven current leaves listed below and no stale work.
Claim queue: br ready -l ideal-2026-07 -t task
Current tasks: `bd-59co.2.2.8` (pinned Linux x64 CI environment contract), `bd-59co.2.2.16` (transactional owning VbaRecord writes), and `bd-59co.3.1.5` (owned Windows test-resource policy).
Certified ready queue: `bd-59co.2.2.3`, `bd-59co.2.2.14`, `bd-59co.2.2.17`, `bd-59co.2.2.18`, `bd-59co.2.2.22`, `bd-59co.3.1.6`, and `bd-59co.3.15.4`.
Current capacity snapshot: `docs/evidence/programs/ideal-2026-07/capacity/POST_ROLLOUT_CAPACITY_2026-07-11.md` (three agents, source hash `19876d013a3d9d45`).
Terminal condition: all Core, Windows x64 and IDE profile roots close beneath `bd-59co`, or every remaining path is genuinely blocked and recorded through the repository blocker protocol.
Last completed tasks: `bd-59co.3.1.4` closed support-only after genuine loader-backed x64 PE/TLB admission, controller-owned digest binding, generalized fail-closed MSFT record parsing, 8 positive/54 mutation evidence and clean non-author review; all 57 artifacts/environments remain pending with no capability credit. `bd-59co.2.2.21` remains closed after safe full-payload/non-Send/Sync VariantCore proof and `.4` after the LF/snapshot contract. Exact open successors `.16`, `.18`, `.22`, `.2.4.4`, and `bd-59co.2.7.2` own transactional record writes, VMR05 provenance, VM3 fixture provenance, compiler UDT size/depth limits, and hidden object-carrier thread/apartment transit; no terminal row is closed by these slices.

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
