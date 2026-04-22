# Discretionary Decisions

Status: finalized

## Retained Decisions

1. Canonical runtime object identity uses `ObjectRef`
   - decision:
     the migrated runtime carries object identity through `ObjectRef`, backed by
     an `IUnknown`-implementing object base, rather than a standalone integer
     token
   - evidence basis:
     `INTERFACE_IDENTITY_AND_RETAINED_WRAPPER_DECISIONS_2026-04-21.md`
   - compatibility rationale:
     this aligns runtime identity and lifetime with the intended COM/VBA
     direction without turning the whole runtime into raw interface pointers
   - revisit trigger:
     only if later evidence shows the typed `ObjectRef` layer itself blocks a
     required observable VBA/COM behavior
2. Native COM identity remains retained-wrapper state anchored on `IUnknown`
   - decision:
     native COM pointer truth stays in retained bridge state, with `IUnknown`
     as the canonical external identity anchor
   - evidence basis:
     interface/event fact pack plus `vmm-f2` implementation landing
   - compatibility rationale:
     this preserves Windows boundary truth while keeping the canonical runtime
     portable and semantic-first
   - revisit trigger:
     only if a later boundary lane proves the retained-wrapper split cannot
     express the required observable COM behavior
3. Canonical runtime `Variant` is an owned semantic carrier over `VariantCore`
   - decision:
     keep the Windows-shaped 16-byte `VariantCore` for wire/layout truth, but
     let the public runtime `Variant` own side data for strings, arrays,
     objects, and binding handles
   - evidence basis:
     `variant.rs`, `windows_variant.rs`, and the post-`ObjectRef` layout delta
     artifacts
   - compatibility rationale:
     this preserves honest boundary projection while letting the runtime own the
     real semantic payloads it now needs
   - revisit trigger:
     revisit only if later profiling shows part of the owned side-data shape can
     be reduced without changing observable behavior
4. Carrier growth is accepted where it buys honest boundary behavior
   - decision:
     accept the current observed growth in `Variant`, `ObjectIdentityCarrier`,
     and `ComCallbackPayload`
   - evidence basis:
     `vmf2-mem-identity-smoke/comparison/layout_metrics.csv`
   - compatibility rationale:
     this migration was explicitly allowed to trade memory for Windows/VBA/COM
     boundary fidelity
   - revisit trigger:
     performance/memory tuning after correctness remains green
5. Pointer helpers project real boundary shapes, not raw internal storage
   - decision:
     keep the contract that `StrPtr`, `VarPtr`, and `ObjPtr` expose honest
     boundary cells/pointers even when the canonical runtime storage is not
     itself identical to VBA native memory layout
   - evidence basis:
     pointer-helper contract, ABI cell reconciliation, and ABI/layout matrix
   - compatibility rationale:
     this matches how the migration is scoped: observable boundary truth first,
     not undocumented internal layout imitation for its own sake
   - revisit trigger:
     only if broader native UDT/layout parity is later taken in-scope
6. `VarPtr(Variant)` now supports object and array container materialization
   - decision:
     keep the new `VT_UNKNOWN` and `VT_ARRAY | VT_VARIANT` materialization lanes
     rather than preserving the old explicit rejection
   - evidence basis:
     `POINTER_HELPER_ABI_CELL_RECONCILIATION_2026-04-22.md` and
     `ABI_LAYOUT_MATRIX_2026-04-22.md`
   - compatibility rationale:
     the old rejection was a bounded old-OxVba limitation, not the desired
     Windows/VBA-compatible end state
   - revisit trigger:
     only if later evidence shows one of these lanes needs tighter scope
7. Current canonical string-perf artifact selection
   - decision:
     keep `vmd6-perf-check` as the canonical string-perf input
   - evidence basis:
     completed paired one-iteration run across all five workloads
   - compatibility rationale:
     a complete bounded artifact is more trustworthy than a larger partial one
   - revisit trigger:
     replace it once a stable multi-iteration paired run exists
8. Current canonical Variant-perf artifact selection
   - decision:
     keep `vme5-perf-check` as the canonical Variant-perf input
   - evidence basis:
     completed paired one-iteration Variant boundary workload run
   - compatibility rationale:
     same bounded-complete-artifact rule as the string lane
   - revisit trigger:
     replace it once a stable multi-iteration paired run exists
9. `string_slice_ops_dollar.bas` remains classified as a repo-wide old/new bug
   - decision:
     keep treating that fixture as a pre-existing OxVba correctness bug rather
     than a migration delta
   - evidence basis:
     it fails on both the fixed baseline and migrated `HEAD`
   - compatibility rationale:
     this matches the agreed authority hierarchy: VBA/spec truth remains above
     both OxVba versions
   - revisit trigger:
     rerun and remove the exception once the slice semantics are fixed
