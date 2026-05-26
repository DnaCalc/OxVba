# JIT v2 VM Native Capability Review

Status: `planning-review-complete`
Date: 2026-05-26
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)
VM seed test:
[`../../crates/oxvba-host/tests/jit_v2_tracer_vm_seed.rs`](../../crates/oxvba-host/tests/jit_v2_tracer_vm_seed.rs)

## Purpose

Classify TB01-TB09 VM seed capability, including whether the hosted/native
tracers are test/harness setup issues or real current VM/native limitations.
The result is executable: the planning-stage seed runner now validates every
tracer bullet through the VM.

## Current VM Seed Classification

| Tracer | VM runnable now | Mechanism | Classification |
|---|---|---|---|
| TB01 primitive typed scalar | Yes | CLI VM fixture covers declared `Long`, `Double`, and `Boolean` locals through a loop and scalar result projection. | VM capability present for the seed. Future JIT evidence must prove primitive carriers, not Variant-only boxing. |
| TB02 UDT struct field/copy | Yes | CLI VM fixture covers UDT declaration, field assignment, whole-UDT copy, field update, and field arithmetic. | VM capability present for the seed. Future JIT evidence must prove descriptor-backed UDT layout and copy/deopt materialization. |
| TB03 error routing | Yes | CLI VM fixture covers `On Error Resume Next`, declared primitive inputs, a failing division, and `Err.Number`. | VM capability present for the seed. |
| TB04 BSTR lifetime | Yes | CLI VM fixture covers declared `String` assignment, concat, and `Len`. | VM capability present for the seed. Future closure still needs cleanup counters/evidence. |
| TB05 SAFEARRAY | Yes | CLI VM fixture covers a typed `Long` array for stores, index reads, and `For Each`, plus `Array(...)`, `LBound`, and `UBound` metadata. | VM capability present for the seed. Runtime out-of-bounds error evidence remains a later tracer-closure fixture. |
| TB06 late-bound COM | Yes, hosted | `oxvba-host` VM seed uses `HostPolicy::interactive_dev()` and the controlled `OxVba.TestDispatch` COM fixture compiled into the Rust test binary. | Test harness gap, not a VM limitation. The standalone CLI cannot rely on this test-only COM fixture or external ProgID registration. |
| TB07 early-bound COM | Yes, hosted project | `oxvba-host` VM seed wraps the `.bas` source in a `ProjectManifest` with an `OxVba` type-library reference. | Test harness gap, not a VM limitation. The standalone `.bas` file lacks the reference metadata required for typed imported COM binding. |
| TB08 native Declare | Yes, hosted Windows native | `oxvba-host` VM seed runs Windows `Declare PtrSafe` calls through the descriptor-backed dynamic-link lane. | Mixed. The original fixture had a `PtrSafe` bug and used an unsupported custom host symbol. The VM supports current native scalar/string/pointer/ByRef-buffer paths, but not a general Automation `Variant`/`SAFEARRAY` Declare ABI. |
| TB09 exported callable | Yes | CLI VM fixture proves internal callable invocation, `ByRef` writeback, and return projection. | VM capability present for the seed. External exported ABI ingress/egress remains future evidence. |

## VM To Native Truth Now

The VM already lowers Declare calls to `Instruction::IntrinsicInvokeSymbolHost`
with `ExternalCallDescriptor` metadata. At runtime the VM calls
`HostServices::dynlink().invoke_descriptor_variants(...)`, applies returned
writebacks, and routes HAL errors through VM error handling.

The current Windows host-backed native lane supports:

- scalar numeric arguments and returns;
- `String`/BSTR access through `StrPtr(...)` or supported string marshaling;
- byte-array/SAFEARRAY payload access through `VarPtr(array(0))`;
- `Variant` cell pointer exposure through `VarPtr(variantSlot)`;
- scalar `ByRef` writeback for the implemented native storage types;
- deterministic policy denial when dynamic linking is not allowed.

The current VM/native lane does not yet provide:

- a general native `Declare` ABI for passing `Variant` as an Automation
  `VARIANT*`/`VARIANT`;
- a general native `Declare` ABI for passing `SAFEARRAY*` as a first-class
  declared parameter;
- a custom `"host" / "jit_trace_declare"` multi-shape symbol. The existing
  deterministic `"host"` lane only covers the bounded `ping`/`double` probes.

## Test Consequence

`scripts/run-jit-v2-tracer-fixtures.ps1` now has two VM seed paths:

- CLI VM seeds for host-independent fixtures with stable `expected_vm_values`
  rows.
- Hosted VM seeds through `cargo test -p oxvba-host --test
  jit_v2_tracer_vm_seed` for COM/type-library/native fixtures that need
  controlled host setup or non-constant pointer observations.

This is the right split for JIT v2 planning: the VM behavior is executable
today, and the remaining TB08 Automation-carrier breadth is explicitly a future
native ABI design/implementation gap rather than an accidental missing test.
