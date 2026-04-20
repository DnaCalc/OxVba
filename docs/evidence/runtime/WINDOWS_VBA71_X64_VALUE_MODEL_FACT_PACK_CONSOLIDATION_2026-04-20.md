# Windows VBA 7.1 x64 Value Model Fact Pack Consolidation

Date: 2026-04-20
Owner: Codex
Status: published
Workset: `WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md`
Bead: `bd-t8rr.2.5` / `vmm-b4`

## Scope

This note consolidates the Windows VBA 7.1 x64 fact pack into one migration
input for the upcoming value-model rewrite.

Source family notes:

- [WINDOWS_VBA71_X64_BSTR_AND_STRING_POINTER_FACT_PACK_2026-04-20.md](/C:/Work/DnaCalc/OxVba/docs/evidence/runtime/WINDOWS_VBA71_X64_BSTR_AND_STRING_POINTER_FACT_PACK_2026-04-20.md)
- [WINDOWS_VBA71_X64_VARIANT_AND_SAFEARRAY_FACT_PACK_2026-04-20.md](/C:/Work/DnaCalc/OxVba/docs/evidence/runtime/WINDOWS_VBA71_X64_VARIANT_AND_SAFEARRAY_FACT_PACK_2026-04-20.md)
- [WINDOWS_VBA71_X64_INTERFACE_EVENT_LAYOUT_FACT_PACK_2026-04-20.md](/C:/Work/DnaCalc/OxVba/docs/evidence/runtime/WINDOWS_VBA71_X64_INTERFACE_EVENT_LAYOUT_FACT_PACK_2026-04-20.md)

## Consolidated Migration Inputs

### `FM-STR`: Strings / `BSTR`

Migration input:

1. canonical migrated string storage should align to the documented `BSTR`
   substrate:
   - 4-byte byte-length prefix
   - UTF-16 payload
   - terminating `WCHAR(0)`
2. `StrPtr(s)` must continue to denote the character payload pointer
3. `VarPtr(s As String)` must continue to denote a container cell whose contents
   identify the `BSTR` value, not collapse to `StrPtr(s)`
4. embedded-null-aware length semantics must remain explicit
5. writable native wide-string target lanes must continue to synchronize back
   into the owning OxVba string variable for supported calls

Current old-code truth:

- canonical string carrier is still `BStr(String)`
- Windows-looking `BSTR` truth is projected today at helper and COM seams

### `FM-VAR`: Variants / `VARIANT`

Migration input:

1. the migrated canonical carrier should preserve the Windows `VARIANT`
   fixed-header shape and x64 size posture
2. strings, arrays, and object/interface payloads must become first-class
   canonical cases instead of boundary-only projections
3. decimal handling must preserve the distinct outer-union overlay posture
4. `VARIANT_BOOL`, byref legality, and `VT_*` tagging must remain boundary-true
5. `VarPtr(v As Variant)` must continue to expose a real container shape rather
   than a payload pointer alias

Current old-code truth:

- current runtime `Variant` is already a 16-byte compat slot
- but it still rejects canonical string, array, object, and binding-handle
  shapes

### `FM-ARR`: Arrays / `SAFEARRAY`

Migration input:

1. the migrated array carrier must preserve explicit:
   - rank
   - lower bounds
   - element size/type implications
   - column-major ordering
2. typed-array and multidimensional result lanes already proven in the checked-in
   old baseline must remain correct
3. descriptor-level flags and cleanup obligations matter for string/interface
   element cases and cannot remain implicit forever

Current old-code truth:

- current runtime `SafeArray` is semantic rather than a raw native descriptor
- current semantic ordering already matches Windows/VBA column-major rules

### `FM-OBJ`: Interface identity / object transport

Migration input:

1. canonical object/interface identity must remain stable under `IUnknown`
   semantics
2. the value migration must not break the ability to compare object identity
   through stable anchors
3. `IDispatch`-driven Automation transport remains a required first-class lane
4. `ObjPtr` must preserve stable object identity semantics for supported object
   categories

Current old-code truth:

- object transport is already identity-aware but still constrained around
  `IDispatch`-centric rebinding and bounded unsupported cases

### `FM-EVT`: Event transport

Migration input:

1. dispatch-style connection-point callbacks (`COM-EVT-A`) remain the required
   event compatibility lane
2. connection-point subscription state must survive representation migration
3. callback payload shapes and instance routing must remain stable
4. event-path exception propagation must keep rich `EXCEPINFO` details intact

Current old-code truth:

- native connection-point subscription/unsubscription infrastructure already
  exists
- required dispatch-style event lane is already evidenced
- source-interface event transport remains explicitly narrow/tiered

### `FM-LAY`: Layout-sensitive native / UDT boundary

Migration input:

1. layout-sensitive native and UDT truth must be recorded now, but broad closure
   still belongs to the later ABI/layout epic
2. value migration must avoid silently claiming layout parity beyond the current
   evidence
3. pointer-string, byref, and mixed COM/native output contracts remain explicit
   boundary concerns until later beads close them

Current old-code truth:

- non-boundary UDT/runtime subset exists
- broad native boundary closure is still explicitly deferred

## Initial Discretionary Decision Register

### `DDR-001`: Internal null-versus-empty string distinction

Question:

- should canonical migrated string state preserve an internal distinction
  between null `BSTR` and empty `BSTR`, or normalize internally while preserving
  boundary truth?

Evidence basis:

- `BSTR` docs treat null and empty similarly at many API surfaces
- MS-OAUT wire rules still distinguish transmitted null from transmitted empty

Current stance:

- unresolved; preserve boundary truth at minimum

Revisit trigger:

- first implementation of canonical owned string carrier
- any failing Excel/VBA oracle that distinguishes null versus empty beyond the
  boundary

### `DDR-002`: Canonical object/interface anchor model

Question:

- should the migrated object carrier store a directly retained native interface
  anchor, or a wrapper model that can still re-derive a stable `IUnknown`
  identity?

Evidence basis:

- COM identity is defined through `IUnknown`
- current OxVba behavior already requires stable object identity and `ObjPtr`
  truth without promising raw arbitrary pointer exposure

Current stance:

- unresolved; stable `IUnknown` identity is mandatory, representation strategy
  still open

Revisit trigger:

- start of `vmm-f1` / `vmm-f2`

### `DDR-003`: Canonical `SAFEARRAY` ownership level

Question:

- should canonical array state become a real native `SAFEARRAY` descriptor, or a
  descriptor-backed semantic wrapper that still owns native-compatible metadata?

Evidence basis:

- documented `SAFEARRAY` layout/flags are real and behavior-bearing
- current OxVba semantic wrapper already preserves ordering and bounds, but not
  descriptor flags or native header state

Current stance:

- unresolved; semantic wrapper is acceptable only if it preserves the required
  native truths without forcing broad re-projection costs

Revisit trigger:

- start of `vmm-e3` / `vmm-g1`
- first perf/memory comparison that shows descriptor overhead is materially
  relevant

### `DDR-004`: Source-interface events beyond the current narrow lane

Question:

- should the migration attempt to widen source-interface event transport
  (`COM-EVT-B`) or keep it explicitly tiered/deferred while stabilizing the
  required dispatch-style lane?

Evidence basis:

- repo event resolutions make `COM-EVT-A` required and `COM-EVT-B` tiered
- current source-interface support is intentionally narrow and special-cased

Current stance:

- keep deferred/tiered during value migration unless a dependency is discovered

Revisit trigger:

- `vmm-f4` rollout
- evidence that a source-interface event case blocks canonical interface/object
  migration correctness

### `DDR-005`: Layout-sensitive closure scope during migration

Question:

- how much UDT/native-layout closure belongs inside the value-model migration,
  versus being left to the later ABI/layout lane?

Evidence basis:

- current HAL/native docs explicitly keep pointer-string, byref, and mixed
  COM/native output contracts as boundary topics
- current workset already assigns broader closure to epic G

Current stance:

- migration should capture facts and preserve existing behavior, but not absorb
  the entire native ABI/layout closure program

Revisit trigger:

- `vmm-g0` rollout
- any migration change that would otherwise break current pointer-helper or
  declared-native correctness lanes

### `DDR-006`: `BSTR` allocator-observation interpretation

Question:

- how should Windows allocator caching behavior for `BSTR` blocks be treated in
  performance and memory reporting?

Evidence basis:

- Microsoft documents that Automation may cache freed `BSTR` allocations

Current stance:

- logical lifetime and benchmark timings are primary; raw allocator observation
  must be interpreted carefully

Revisit trigger:

- start of paired perf/memory harness work in epic C

## Migration Readiness Output

The fact pack is now usable as the direct input to the next epics:

1. Epic C can build the old/new matrix and evidence harness using the explicit
   compatibility requirements recorded across the three family notes.
2. Epic D can implement the string carrier against `FM-STR`.
3. Epic E can implement the canonical variant/value carrier against `FM-VAR`
   and `FM-ARR`.
4. Epic F can implement interface/event transport against `FM-OBJ` and
   `FM-EVT`.
5. Epic G can close the remaining ABI/layout-sensitive rows against `FM-LAY`
   without confusing them with already-settled value-model facts.
