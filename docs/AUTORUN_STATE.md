# Execution Control

Purpose: the sole volatile control surface for current execution mode, accepted worksets, terminal condition and resume context.

Mode: AutoRun
Active program manifest: docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json
Program root: bd-59co
Control epic: bd-59co.1
AutoRun terminal gate: bd-59co
Queue certification: certified — two current leaves are active; the actual ready queue contains exactly the two current leaves listed below and no stale work.
Claim queue: br ready -l ideal-2026-07 -t task
Current tasks: `bd-59co.2.2.23` (identity-bound Core gate inputs) and `bd-59co.3.15.6` (owned Excel64 VBE oracle supervisor). The parent strict-baseline gate `bd-59co.2.2.3` remains open and blocked by the newly exposed high-risk rt-abi boundary bead `.26`.
Certified ready queue: `bd-59co.2.2.26` and `bd-59co.3.1.2`.
Current capacity snapshot: `docs/evidence/programs/ideal-2026-07/capacity/POST_ROLLOUT_CAPACITY_2026-07-11.md` (three agents, source hash `19876d013a3d9d45`).
Terminal condition: all Core, Windows x64 and IDE profile roots close beneath `bd-59co`, or every remaining path is genuinely blocked and recorded through the repository blocker protocol.
Last completed tasks: `bd-59co.2.2.24` and `.25` closed after strict focused Clippy passed without suppression, 229 symbol/OxIR and 512 project/binder neighboring tests passed, and independent review confirmed the argument bundles, condition rewrite, path split, and integer type set are behavior-preserving. The full workspace baseline remains open: reaching `oxvba-rt-abi` exposed 44 raw-pointer/helper-boundary findings now owned by the high-risk delivery bead `.26`; no warning or capability row was hidden. `bd-59co.2.2.9` closed after its versioned cross-platform runner passed the exact All suite and final Windows Core/WSL containment checks; Windows Job objects and Linux pidfd/subreaper containment prove failure, timeout, descendant, early-exit, hostile-PATH, post-retention confirmation-failure, cleanup, evidence, and Cargo-serialization behavior without a bless route. Exact input-instance consumption remains open under `.23`, and actual platform baseline transcripts/terminal ownership remain `.10`, `.11`, and `.12`, so the canonical row stays planned. `bd-59co.3.1.5` remains closed after its exact x64 owned-resource journal and normative policy passed 81 assertions and 65 fail-closed mutations over real HKCU Registry64 values, local files, and harmless children; fresh-eyes repair added retained process/file/HKEY authority, resumable transaction state, immutable lease/ticket binding, lease-safe contention observation, and exact nonrecursive teardown, with zero final temp/registry residue. This support result grants no COM, native, carrier, or release capability credit. `bd-59co.3.15.4` remains closed after all 57 canonical Windows x64 rows received one fail-closed certification case with producer, fixture, pinned-environment, command, artifact, locale, process/apartment, and six-axis expectations; all 57 cases and 342 axes remain pending/blocked, twelve mutations reject premature promotion, and the support manifest grants no capability or certification credit. `bd-59co.3.1.6` remains closed after all 57 Windows x64 rows received current code, test, historical-evidence, gap, and live-owner dispositions; every row remains planned, VM3/historical assets earn no JIT credit, the synchronous ByRef event blocker and imported callback/event routes remain explicit, and ten mutations enforce the boundary. `bd-59co.2.2.18` remains closed after MS-VBAL authority established `Explicit` as a legal contextual identifier, the parser accepted it, VMR05 executed, and focused syntax/symbol tests proved exact active-token UTF-8 offsets under LF/CRLF; broader source, CST, and EOL rows remain planned. `bd-59co.2.2.22` remains closed after the VM3 fake foreign-IUnknown fixture switched to complete-allocation provenance and passed a focused 1→2→3→2→1→0 lifecycle proof under Miri; this support-only repair advances no VM3 capability row, and strict workspace Clippy remains `.3`. `bd-59co.2.2.14` remains closed after stable boxed native VARIANT cells, pointer-independent BSTR accounting, `VariantClear`-before-token cleanup, RAII native SAFEARRAY construction, 128-pin/rehash and extreme-bound regressions; broader Windows pointer/native parity remains under `bd-59co.3.11.1`. `bd-59co.2.2.17` remains closed after destructor-free BSTR/SAFEARRAY borrow projections and explicit raw ownership transfers. `bd-59co.2.2.8` remains closed after the digest-pinned Linux x64 CI contract; exact baseline execution remains blocking under `.11`. Exact open successors `.23`, `.26`, `.11`, `.2.4.4`, `bd-59co.2.7.2`, and the broader CORE-2 owner `bd-59co.2.3.1` own identity-bound input consumption, raw-pointer helper-boundary safety, Linux execution, compiler UDT size/depth limits, hidden object-carrier thread/apartment transit, and full source/provenance/CST realization; no terminal row is closed by these slices.

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
