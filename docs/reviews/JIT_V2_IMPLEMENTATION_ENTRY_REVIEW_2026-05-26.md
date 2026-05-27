# JIT v2 Implementation-Entry Review

Status: `planning-review-complete`
Date: 2026-05-26
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)

## Purpose

Record the planning review pass that turns the JIT v2 Cranelift package into an
implementation-entry baseline. This is not JIT execution evidence and does not
activate Cranelift or change `oxvba-jit` behavior.

## Entry Decision

The planning package is ready to open the first JIT v2 support-scaffolding
implementation workset. That first implementation workset should start with
support-query diagnostics, `ProcLoweringIr` data structures, the
`ProcLoweringIr` verifier, and the helper ABI manifest before any
Cranelift-generated procedure is executable.

Executable tracer work remains gated by the executable semantic package VM
strengthening slices. For any touched tracer fixture, package identity,
procedure identity, bytecode digest, signature descriptors, and slot
descriptors must be visible in VM evidence before the JIT lowerer consumes those
facts.

The package is not a claim that any tracer bullet has executed through native
JIT code. Each tracer bullet still closes only when VM/JIT differential evidence
passes for that tracer's required surfaces.

## P0/P1 Decision Ledger

| Question | Decision for slice 1 | Evidence |
|---|---|---|
| First supported target | Windows x64 only; all other targets return deterministic unavailable diagnostics. | `JIT_V2_SUPPORT_MATRIX_V1.csv` |
| Entry ABI | Uniform `extern "C" fn(vmctx: *mut JitVmContext, frame: *mut JitFrame) -> JitStatus`. | `JIT_V2_IMPLEMENTATION_DESIGN_V1.md` |
| Executable semantic package | VM and JIT consume the same bytecode-plus-metadata package; the current `OxBundle` is the seed, and missing tracer facts must be added to the package rather than reconstructed in the JIT. | `EXECUTABLE_SEMANTIC_PACKAGE_V1.md` |
| Runtime value carrier | Declared typed carriers are authoritative in `ProcLoweringIr`; primitive and UDT lanes are first-class; retained `Variant` snapshots are evidence/deopt materialization, and COM VARIANT layout is required only for declared `Variant`/boundary projection. | `JIT_V2_SEMANTIC_CONTRACT_AND_FACT_PACK_V1.md` |
| Procedure-lowering IR | `ProcLoweringIr` is mandatory before CLIF lowering; it is built from package facts, and direct bytecode-to-CLIF lowering is out of bounds. | `JIT_V2_PROC_LOWERING_IR_V1.md` |
| Helper binding | Versioned helper table and descriptor manifest; no ambient helper symbol lookup. | `JIT_V2_HELPER_ABI_CATALOG_V1.md` |
| COM/native strategy | Descriptor-backed helpers are the first semantic path for late COM, early COM, native Declare, and exported callable projection. | Workset epics plus helper ABI catalog |
| Error routing | VM-equivalent `Err` state, resume target, and failure status are frame state, not backend-local behavior. | Semantic contract and TB03 plan |
| Cleanup/deopt | Explicit cleanup stack, safepoints, live-carrier maps, ByRef maps, and deopt snapshots are required. | ProcLoweringIr and implementation design |
| Cranelift memory flags | Conservative by default; stronger flags require named proof per carrier/boundary. | Workset execution policy |
| Verifiers | `ProcLoweringIr` verifier runs before CLIF; Cranelift verifier runs for every compiled test/debug function. | ProcLoweringIr verifier plan and workset policy |
| Debug policy | JIT disabled by default in debug sessions until a conservative debug profile is accepted. | Implementation design |
| Fallback policy | Silent VM fallback is forbidden; unsupported, deopt, helper fault, COM/native fault, and real JIT execution are distinct results. | Workset and harness design |

## Review Gate Results

| Gate | Result | Notes |
|---|---|---|
| VM truth review | Pass for implementation entry | VM/bytecode remains the executable oracle; slot, error, cleanup, BSTR, SAFEARRAY, ByRef, object, COM, and native surfaces are named in the semantic contract. |
| Package layering review | Pass for implementation entry | VM and JIT share the executable semantic package boundary; direct bytecode-to-CLIF and parallel typed-JIT reconstruction remain out of bounds. |
| COM/native review | Pass for implementation entry | COM and native interop are in the first design slice through shared descriptors and helper ABI entries; they are not fallback-only paths. |
| Cranelift review | Pass for implementation entry | Cranelift is constrained behind `ProcLoweringIr`, conservative memory flags, explicit helper imports, verifier gates, and diagnostic CLIF artifacts. |
| Fresh-eyes review | Pass for implementation entry | No Variant-universal JIT value model, no silent fallback, no public API behavior change, and no JIT execution claim were introduced. |

## Tracer-Bullet Readiness

`Ready` here means the fixture and design intent are present for later work. It
does not mean executable JIT work may consume missing package facts. Executable
tracer work remains blocked until the VM strengthening workset records the
package evidence named for that tracer.

| Tracer | Entry status | Residual before tracer closure |
|---|---|---|
| TB01 Primitive typed scalar loop | Fixture/design ready; VM package primitive slot evidence present; executable work still blocked on canonical carrier/layout and verifier evidence | Needs canonical primitive carrier layout facts, operator/coercion descriptor ids, `ProcLoweringIr` verifier output, CLIF verifier output, and VM/JIT projected snapshot equality after code exists. |
| TB02 UDT struct field/copy | Fixture/design ready; VM package UDT descriptor and selected lifecycle evidence present; executable work still blocked on offset/layout and cleanup/deopt evidence | Needs field-offset/layout evidence, copy-independence evidence, explicit cleanup/deopt materialization, and VM/JIT projected field snapshot equality. |
| TB03 Error routing | Fixture/design ready; executable work blocked on error-frame package evidence | Needs failing helper evidence, `Err` state equality, and resume target equality. |
| TB04 BSTR lifetime | Fixture/design ready; executable work blocked on cleanup/lifetime package evidence | Needs allocation/release counters and branch/failure/deopt cleanup evidence. |
| TB05 SAFEARRAY | VM seed exists for store, index, For Each, and bounds metadata; executable work blocked on array package evidence | Runtime bounds-error fixture/evidence is still required before TB05 can close. |
| TB06 Late-bound COM | Hosted VM seed exists; executable work blocked on COM descriptor evidence | Future JIT evidence still needs HRESULT, EXCEPINFO, ArgErr, named/default member handling, and object identity comparison. |
| TB07 Early-bound COM | Hosted VM seed exists; executable work blocked on typelib/COM descriptor evidence | Future JIT evidence still needs descriptor digest and dispatch/vtable parity evidence. |
| TB08 Native Declare | Hosted Windows native VM seed exists; executable work blocked on native descriptor evidence | Current VM seed covers scalar, BSTR pointer, SAFEARRAY buffer pointer, Variant cell pointer, and scalar ByRef writeback; general Automation `Variant`/`SAFEARRAY` declared-parameter ABI support remains a future tracer-closure gap. |
| TB09 Exported callable | Internal VM projection seed exists; executable work blocked on export descriptor evidence | External inbound/outbound ABI projection evidence is still required before TB09 can close. |

## Implementation-Entry Checklist

- Workset, research review, executable semantic package draft, implementation
  design, semantic contract/fact pack, helper ABI catalog, `ProcLoweringIr`,
  differential harness, validation matrices, fixture set, and VM seed runner are
  cross-linked.
- The first support-scaffolding implementation cut has a bounded path:
  support query -> `ProcLoweringIr` data model -> verifier -> helper manifest ->
  harness unavailable rows -> Cranelift module setup. TB01/TB02 execution
  starts only after their VM/package evidence gates pass.
- `oxvba-jit` remains a disabled public boundary and still reports not
  implemented.
- `./scripts/run-jit-v2-tracer-fixtures.ps1` is the planning-stage guard for
  VM-ready fixture drift across CLI and hosted seed paths.

## Non-Goals Preserved

- No Cranelift dependency activation.
- No JIT execution.
- No silent fallback to VM execution.
- No new public API behavior.
- No replacement value model for declared VBA carriers; `Variant` remains exact
  where declared or required at COM/native boundaries, but the JIT plan is not
  Variant-focused.
