# OxVba Ideal Worksets Stock-take

Date: 2026-07-14  
Integrated baseline: `b5604905`  
Program: `ideal-2026-07` / `bd-59co`  
Authority: status and execution handoff; this document does not advance a capability matrix row

## Executive assessment

OxVba is a substantial, broad implementation under an intentionally conservative recertification program. The compiler and binder remain the strongest product foundation. VM3 is a broad reference-runtime candidate, and the Cranelift JIT is a real whole-program backend rather than a fallback façade. The repository also contains extensive runtime, VBA-library, project/reference, COM, native, host, wrapper, corpus, and historical oracle assets.

The requested end state is not yet achieved. Current product trust boundaries, dual-runtime proof, Windows x64 JIT interop, persistent JIT sessions/cache, and clean-stack language services remain open. The correct overall status is `in-progress`, with no architectural barrier currently known.

The most useful shorthand is:

> substantial implementation breadth; early current-stack certification; strong control-plane progress; material capability delivery still ahead

This is why the repository can contain a mature compiler and broad runtimes while the new canonical program still reports nearly every row as planned. Historical implementation and evidence are inputs to the new proof, not automatic completion credit.

## Measured program truth

### Canonical capability matrices

The fifteen manifest-owned matrices contain 193 rows:

| Profile | Matrices | Rows | Planned | Verified |
|---|---:|---:|---:|---:|
| Core | 5 | 59 | 58 | 1 |
| Windows x64 | 6 | 57 | 57 | 0 |
| IDE foundation | 4 | 77 | 77 | 0 |
| **Total** | **15** | **193** | **192** | **1** |

The one verified row is the Core clean/spec/VBA semantic-authority row. The remaining rows retain explicit accepted residual scope and owners. See the generated [Ideal Program Derived Validation Summary](validation/IDEAL_PROGRAM_DERIVED_SUMMARY_LATEST.md).

### Execution beads

The `ideal-2026-07` graph currently has 192 labelled beads:

| Scope | Total | Closed | Active | Open |
|---|---:|---:|---:|---:|
| All profiles | 192 | 37 | 3 | 152 |
| Core | 64 | 25 | 2 | 37 |
| Windows x64 | 82 | 7 | 1 | 74 |
| IDE foundation | 40 | 0 | 0 | 40 |

The profile counts are execution topology, not capability percentages. Windows includes extensive certification and cross-row evidence work; the IDE matrix has more rows than its bead count because one delivery bead may own several related direct/LSP or reference tranches.

At this baseline the active leaves are:

- `bd-59co.2.2.23`: bind Core gate execution to admitted file instances;
- `bd-59co.2.2.26`: harden the current rt-abi raw-pointer helper boundary;
- `bd-59co.3.15.6`: promote the owned Excel64 VBE oracle supervisor.

The only ready leaf is `bd-59co.3.1.2`, the Windows x64 development/oracle host characterization. It remains unclaimed while the serialized Excel/VBE lane is active.

## Subsystem stock-take

| Subsystem | Practical current status | Ideal-workset gap |
|---|---|---|
| Syntax and conditional compilation | Strong lossless-CST foundation with broad tests and length-preserving active-source offsets | Complete fail-closed source/provenance/CST contract and current UTF-8/CRLF/editor evidence |
| Symbols, compiler, and binder | Mature foundation with provider-based project/library/COM/Declare resolution and rich Core IR | Versioned `AnalysisResultV1`, stable use-site/type/call facts, exact diagnostics/provenance, remaining language legality and reference cases |
| VBA base library | Broad implemented subset exercised by large corpora | Member-by-member inventory, typed binding, host/locale/error coverage, and current Excel/VBA evidence across VM3/JIT |
| Core IR and OxIR | Rich explicit semantic tree plus typed CFG shared by VM3 and JIT | Complete admitted vocabulary, effect/ownership descriptors, verifier rejection matrix, and backend-neutral helper/session contracts |
| OxImage | Real serialized project-closure artifact | Bounded decoding, owning sealed verified handles, deterministic identity/ABI/provenance, verified-only consumers, distributable package proof |
| Runtime carriers/eval/rt-abi | Broad Variant/BSTR/SAFEARRAY/object/record support and shared helper substrate | Current raw-pointer helper boundary must become honestly unsafe; ideal versioned helper catalog and session-owned descriptors follow in CORE-5/CORE-7 |
| VM3 | Broad reference-runtime candidate with extensive focused/corpus coverage | Close admitted-vocabulary, entry/link, full Err/Erl, lifecycle/session, package-trust, structural differential, and current VBA-oracle rows |
| JIT | Real direct OxIR-to-Cranelift backend with whole linked-program compilation and broad portable feature subsets | Inspectable lowering plan, typed primary entries, universal thunk boundary, safe recursion, full Err/Erl/ByRef/direct calls, sessions/cache/object output, complete structural parity |
| Windows COM/native | Valuable VM-era code, fixtures, typelibs, wrappers, and historical evidence exist | One verified x64 interop plan shared by VM3/JIT; real JIT late/early COM, events, serving, callbacks/Declare, carrier proof, native/wrapped outputs, Excel64 certification |
| Language services | No active clean-stack LS or LSP product | Rebuild from compiler facts: immutable snapshots, overlays, direct queries, project/reference/COM/OxImage coverage, thin LSP, editor/host smoke, cancellation/performance |

The detailed basis remains the [Post-JIT Status Review](OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md). The [System Contract](spec/OXVBA_SYSTEM_CONTRACT_V1.md), [Architecture](ARCHITECTURE.md), and rewritten worksets are the destination authority.

## Progress since the post-JIT review

The review found a red baseline and weak execution truth. The program has since made concrete foundational progress without converting support work into capability credit:

- the three x64-only worksets, contract clauses, fifteen canonical matrices, 42 execution epics, rollout validators, traceability graph, generated summary, and AutoRun control surface are live;
- legacy open work was reconciled into bounded current owners, and stale queue selection is validator-blocked;
- the Core baseline repaired line endings, carrier-balance isolation, BSTR ownership, stale host/JIT assertions, HAL/LongPtr/Variant/VM3 fixture issues, and contextual `Explicit` parsing/provenance;
- the versioned cross-platform Core gate runner now has Windows Job and Linux pidfd/subreaper containment plus exact retained input-instance binding;
- the Windows current-stack ledger characterizes all 57 rows without granting JIT credit for VM3 or historical evidence;
- the Windows certification manifest has one fail-closed case and six observable axes per row, all still blocked pending real producers, fixtures, and the pinned certification VM;
- the Windows owned-resource policy now proves exact, resumable, nonrecursive teardown for Registry64, files, harmless processes, and logical COM/UIA resources;
- the strict-Clippy sequence closed behavior-preserving symbol/OxIR and project/binder repairs, the rt-abi raw-pointer tranche, and the JIT/VM3 follow-on tranches; the aggregate all-target Clippy and ordinary workspace baseline now passes.

The aggregate technical baseline is now green. Cross-platform certification is
still open because the actual Windows development and pinned Linux CI
transcripts, followed by terminal reconciliation, remain separate required
gates. This distinction keeps support evidence from prematurely verifying the
five CORE-1 capability rows.

## Reading the three worksets now

### Core

Core has the most closed execution beads because control, baseline, and bounded correctness repairs came first. That does not mean Core conformance is almost complete. The major producer spine—source/provenance, compiler facts, sealed OxImage, helper/session contracts, complete VM3/JIT architecture, cache/object output, differential/oracle certification, and terminal profile—remains ahead.

### Windows x64

Windows has good scope definition, residual characterization, fixture/certification structure, and exact resource policy. Real current-stack capability delivery remains early. All 57 rows are planned, including late and early COM clients, incoming/outgoing events, late/dual serving, Declare and callbacks, exact carriers, wrappers, native DLL/EXE output, deployment, safety, and Excel64 certification.

### IDE foundation

IDE delivery has not started on the clean stack. This is deliberate rather than neglect: the first vertical slice consumes stable source identity and compiler facts from Core. There is reusable CST, symbol, provider, project/reference, COM metadata, diagnostic DTO, and historical LS/LSP test material, but no active immutable snapshot/query/LSP product today.

The correct first IDE slice remains a real two-module Unicode/CRLF project flowing through shared compiler facts into immutable snapshots, diagnostics, cross-module definition, thin LSP projection, and embedded-host/VS Code smoke. It must not recreate a second semantic model.

## Immediate execution order

1. Execute the actual Windows x64 development and pinned Linux x64 baseline transcripts, then reconcile CORE-1 terminal truth.
2. Close WIN-0 terminal control truth and live-prove the owned Excel/VBE supervisor with modal capture and zero-owned-resource cleanup.
3. Provision, qualify and seal the clean pinned Windows x64/Excel64 certification VM; the characterized development host remains noncertifying.
4. Resume the permanent producer spine: source/provenance, compiler facts, sealed verification/OxImage identity, helper catalog/session ownership, and lowering-plan/typed-entry skeleton.
5. Start the first IDE vertical slice once its Core identity/fact inputs are stable; start Windows metadata/type-layout producers in parallel, while serialized Excel/Registry/JIT/VM3 resources remain controlled.

## Readiness conclusion

OxVba is suitable for continued construction of the ideal architecture and already contains enough real implementation to make that program credible. It is not yet suitable for a fully conforming, complete-toolchain claim or as an unquestioned DNA Calc foundation.

The three worksets remain a plausible route to that destination. No fundamental barrier has emerged; the limiting factors are delivery volume, unsafe/product-boundary hardening, current Excel/VBA authority, Windows JIT interop, and rebuilding language services over the shared compiler facts. The program should continue to describe itself as a strong, broad, in-progress VBA toolchain until all three terminal profiles close.
