# Execution Control

Purpose: the sole volatile control surface for current execution mode, accepted worksets, terminal condition and resume context.

Mode: AutoRun
Active program manifest: docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json
Program root: bd-59co
Control epic: bd-59co.1
AutoRun terminal gate: bd-59co
Queue certification: certified — three current leaves are active; the actual ready queue contains exactly the two current leaves listed below and no stale work.
Claim queue: br ready -l ideal-2026-07 -t task
Current tasks: `bd-59co.2.2.9` (versioned cross-platform gate runner), `bd-59co.3.1.5` (owned Windows test-resource policy), and `bd-59co.3.1.6` (Windows current-stack residual inventory).
Certified ready queue: `bd-59co.2.2.3` and `bd-59co.3.15.4`.
Current capacity snapshot: `docs/evidence/programs/ideal-2026-07/capacity/POST_ROLLOUT_CAPACITY_2026-07-11.md` (three agents, source hash `19876d013a3d9d45`).
Terminal condition: all Core, Windows x64 and IDE profile roots close beneath `bd-59co`, or every remaining path is genuinely blocked and recorded through the repository blocker protocol.
Last completed tasks: `bd-59co.2.2.18` closed after MS-VBAL authority established `Explicit` as a legal contextual identifier, the parser accepted it, VMR05 executed, and focused syntax/symbol tests proved exact active-token UTF-8 offsets under LF/CRLF; broader source, CST, and EOL rows remain planned. `bd-59co.2.2.22` remains closed after the VM3 fake foreign-IUnknown fixture switched to complete-allocation provenance and passed a focused 1→2→3→2→1→0 lifecycle proof under Miri; this support-only repair advances no VM3 capability row, and strict workspace Clippy remains `.3`. `bd-59co.2.2.14` remains closed after stable boxed native VARIANT cells, pointer-independent BSTR accounting, `VariantClear`-before-token cleanup, RAII native SAFEARRAY construction, 128-pin/rehash and extreme-bound regressions; broader Windows pointer/native parity remains under `bd-59co.3.11.1`. `bd-59co.2.2.17` remains closed after destructor-free BSTR/SAFEARRAY borrow projections and explicit raw ownership transfers. `bd-59co.2.2.8` remains closed after the digest-pinned Linux x64 CI contract; exact baseline execution remains blocking under `.11`. Exact open successors `.9`, `.23`, `.11`, `.2.4.4`, `bd-59co.2.7.2`, and the broader CORE-2 owner `bd-59co.2.3.1` own the portable gate runner, identity-bound input consumption, Linux execution, compiler UDT size/depth limits, hidden object-carrier thread/apartment transit, and full source/provenance/CST realization; no terminal row is closed by these slices.

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
