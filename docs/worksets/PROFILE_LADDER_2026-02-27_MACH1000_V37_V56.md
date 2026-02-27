# PROFILE_LADDER_2026-02-27_MACH1000_V37_V56.md

## Why This Ladder Exists
`v36` closed the current stabilization gate. The next horizon should not mix unrelated risk classes.

This ladder splits work into three explicit tracks:
1. **Language Core** (parser/binder/runtime semantics).
2. **Intrinsic Runtime ("standard library")**.
3. **Host/Interop Surface** (COM/object-model dependent behavior).

Planning horizon in this document:
- Profiles: `v37` through `v56`
- Total planned steps: **20**
- Formal level target: `F3` throughout (with non-blocking formal failures logged per current policy).

## Microsoft Spec Anchors (Scope Boundary)
- **MS-VBAL** is the primary reference for VBA language semantics and intrinsic function behavior.
  - Main spec root: https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74
  - Example function semantic pages:
    - `Mid`: https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/4a65ee4e-b6b9-45d3-a3f6-576fed4bb227
    - `CreateObject`: https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/a2040e64-6724-4bf6-a496-7ef01ec9af31
    - `TimeSerial`: https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/3f2478d7-8abf-4489-b89e-e56eca57d047
- **MS-OAUT** is the boundary reference for Automation interop (`VARIANT`, `IDispatch`, packing/invoke contracts).
  - Types: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/7b5fa59b-d8f6-4a47-9695-630d3c10363e
  - Invoke mapping: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/18d74e75-f9a7-4407-9fe8-3406679f7dd8

Interpretation rule for this ladder:
- If behavior is defined in MS-VBAL core semantics, treat mismatch as a compatibility bug.
- If behavior is marked implementation-defined (or host-environment dependent), track in divergence/evidence and bind it to host policy.

## Standard Library Split (Execution Design)
Do not treat "VBA standard library" as one monolith. Split by determinism and dependency boundary:

1. `Intrinsic Core` (deterministic, host-independent):
   - conversion, numeric, string transforms, date/time math, array/introspection helpers.
   - Implement in `oxvba-runtime` as pure functions over `Variant`.
2. `Intrinsic Host` (environment dependent):
   - object activation, process launch, filesystem/environment, host callbacks.
   - Implement in `oxvba-host` (or adapter traits in `oxvba-com` + `oxvba-host`), not in pure runtime core.
3. `Boundary Marshal`:
   - canonical COM `VARIANT` in/out at host/interop edges only.
   - Keep core execution representation stable and deterministic; marshal only at boundaries.

Implementation shape for call dispatch:
- Introduce an intrinsic registry keyed by canonical name + arity + optional argument metadata.
- Attach capability flags per intrinsic:
  - `pure`,
  - `host_required`,
  - `implementation_defined`,
  - `windows_only`.

## Pass-Pack Contract
Profiles use the existing pass-pack structure:
`P0..P9` (`Syntax`, `Bind`, `HIR`, `MIR`, `CFG`, `Emit`, `Runtime`, `Conformance`, `Evidence`, `Formal`).

Formal execution policy:
- Kani lanes that are long-running should run through async orchestration:
  - `./scripts/run-formal-kani-async.ps1 -Action Start|Status|Tail|Wait|Stop`
- If Kani cannot complete in-cycle, log structured follow-up in formal backlog and continue per ladder policy.

## 20-Profile Ladder (v37-v56)

### Track A: Language Core Closure (`v37..v44`)

### v37 — `mvp-lang-optional-params-v37` (F3)
Scope:
- Optional parameters in procedure declarations and calls.
- Default value materialization for omitted args.
Formal obligations:
- Determinism proof for call binding with omitted trailing args.
- VM/JIT parity checks for optional-arg resolution.
Gate:
- New optional-param conformance corpus passes on `vm` and `jit`.

### v38 — `mvp-lang-named-args-v38` (F3)
Scope:
- Named argument binding and reordering at call sites.
- Arity/name validation diagnostics.
Formal obligations:
- Name-to-position mapping totality and uniqueness checks.
- Equivalence: named-arg call vs canonical positional call.
Gate:
- Named-arg fixtures green; mismatch cases produce stable error codes.

### v39 — `mvp-lang-with-block-v39` (F3)
Scope:
- `With ... End With` resolution and member access rewriting.
Formal obligations:
- Reference-target stability across nested `With`.
- No cross-scope alias corruption from rewritten member chains.
Gate:
- `With` conformance suite green including nested forms.

### v40 — `mvp-lang-gosub-return-v40` (F3)
Scope:
- `GoSub` / `Return` intra-procedure control flow.
- Return stack semantics and nesting.
Formal obligations:
- Return-address integrity invariants.
- Bounded stack-depth safety checks.
Gate:
- GoSub fixtures green; malformed return paths fail predictably.

### v41 — `mvp-lang-on-error-goto-label-v41` (F3)
Scope:
- Full `On Error GoTo <label>` transfer semantics.
- `Resume` and label target interplay completion.
Formal obligations:
- Error-state machine refinement against existing v11/v12 model.
- Handler edge soundness and no-invalid-target jumps.
Gate:
- Error label corpus green with explicit `Err` state assertions.

### v42 — `mvp-lang-redim-preserve-v42` (F3)
Scope:
- Dynamic arrays with `ReDim` and `ReDim Preserve` subset.
- Bounds and preserve-copy semantics.
Formal obligations:
- Shape transformation invariants for preserve mode.
- No data loss on valid dimension-preserving operations.
Gate:
- Dynamic array conformance and parity green.

### v43 — `mvp-lang-udt-enum-const-v43` (F3)
Scope:
- `Type ... End Type` (UDT) baseline.
- `Enum` declarations and module-level `Const` semantics.
Formal obligations:
- UDT field layout/access consistency checks.
- Enum/const fold equivalence across optimizer modes.
Gate:
- Type declaration and usage corpus green.

### v44 — `mvp-lang-property-procedures-v44` (F3)
Scope:
- `Property Get/Let/Set` in class/module scope subset.
- Call/assignment routing to property procedures.
Formal obligations:
- Accessor dispatch correctness and mutation visibility checks.
- `Set` vs value assignment semantic separation.
Gate:
- Property procedure fixtures green; runtime snapshots stable.

### Track B: Intrinsic Runtime ("Standard Library") (`v45..v52`)

### v45 — `mvp-stdlib-conversion-core-v45` (F3)
Scope:
- Core conversion functions: `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, `Str`.
Formal obligations:
- Conversion-table slice parity against MS-VBAL definitions.
- Roundtrip sanity checks on in-scope value domains.
Gate:
- Conversion conformance matrix green with coercion parity tests.

### v46 — `mvp-stdlib-string-core-v46` (F3)
Scope:
- `Left`, `Right`, `Mid`, `Len`, `InStr`, `LCase`, `UCase`.
Formal obligations:
- Substring boundary and index-law checks.
- Equivalence of shared implementation paths (`Mid` statement/function subsets where applicable).
Gate:
- String core corpus green; error cases reproducible.

### v47 — `mvp-stdlib-string-advanced-v47` (F3)
Scope:
- `Split`, `Join`, `Replace`, `Trim/LTrim/RTrim`, `StrComp`.
- Option-compare interaction where supported.
Formal obligations:
- Tokenization/replace invariants on delimiter edge cases.
- Compare-mode determinism checks.
Gate:
- Advanced string corpus green with option-compare fixtures.

### v48 — `mvp-stdlib-date-time-core-v48` (F3)
Scope:
- `DateSerial`, `TimeSerial`, `DateValue`, `TimeValue`, `DateAdd`, `DateDiff`.
Formal obligations:
- Calendar boundary and normalization law checks.
- Monotonicity/roundtrip properties for selected units.
Gate:
- Date/time function corpus green and deterministic across backends.

### v49 — `mvp-stdlib-math-financial-core-v49` (F3)
Scope:
- Math primitives (`Abs`, `Int`, `Fix`, `Sgn`, `Round`, `Sqr`, trig/log subset).
- Financial core (`FV`, `PV`, `PMT`, selected rate assumptions).
Formal obligations:
- Numeric error envelope assertions for floating-point-sensitive intrinsics.
- Function identity checks on safe domains.
Gate:
- Math/financial corpus green with tolerance policy documented.

### v50 — `mvp-stdlib-array-variant-introspection-v50` (F3)
Scope:
- `Array`, `LBound`, `UBound`, `IsArray`, `VarType`, `TypeName`, `IsNumeric`, `IsDate`, `IsObject`.
Formal obligations:
- Introspection consistency laws (`VarType`/`TypeName` coherence).
- Bound-function correctness over fixed and dynamic arrays.
Gate:
- Introspection corpus green with variant-type snapshots.

### v51 — `mvp-stdlib-error-surface-v51` (F3)
Scope:
- `Err.Raise` subset, `CVErr`, error object interaction shaping.
- Integration with existing `On Error` model.
Formal obligations:
- Raised-error propagation invariants under each error mode.
- Consistency of `Err.Number` and conversion to/from `CVErr`.
Gate:
- Error intrinsic corpus green; no regressions in v11/v12 semantics.

### v52 — `mvp-stdlib-host-sensitive-v52` (F3)
Scope:
- Host/environment sensitive intrinsics: `Shell`, `Environ`, `Dir` subset.
- Explicit implementation-defined behavior records where spec permits latitude.
Formal obligations:
- Capability-flag enforcement checks (`host_required`, `implementation_defined`).
- Deterministic fallback/diagnostic behavior when host capability is absent.
Gate:
- Host-sensitive corpus green under declared host capability profile.

### Track C: Host/Interop + Consolidation (`v53..v56`)

### v53 — `mvp-object-collection-core-v53` (F3)
Scope:
- `Collection` object baseline (`Add`, `Item`, `Remove`, `Count`) in core engine scope.
Formal obligations:
- Index/key resolution invariants and duplicate-key rejection.
- Mutation/order preservation checks.
Gate:
- Collection conformance corpus green.

### v54 — `mvp-class-lifecycle-v54` (F3)
Scope:
- `Class_Initialize` / `Class_Terminate` invocation semantics.
- Deterministic lifetime behavior for in-scope class subset.
Formal obligations:
- Lifecycle handler ordering/trigger invariants.
- Refcount + lifecycle consistency checks.
Gate:
- Class lifecycle fixtures green with observable event traces.

### v55 — `mvp-com-dispatch-boundary-v55` (F3)
Scope:
- Interop dispatch boundary hardening (`CreateObject`/late-bound subset wiring).
- `IDispatch::Invoke` argument/result packing validation in supported cases.
Formal obligations:
- Marshal conformance checks aligned to MS-OAUT invoke contracts.
- VM/JIT equivalence for supported late-bound call subset.
Gate:
- Boundary corpus green; unsupported paths classified with deterministic diagnostics.

### v56 — `mvp-language-stdlib-consolidation-gate-v56` (F3)
Scope:
- Consolidation gate across language core + intrinsic runtime + selected interop.
- Performance shaping on newly enabled hot paths.
Formal obligations:
- Manifest completeness for `v37..v56` obligations.
- End-to-end parity checks (`vm` vs `jit`, opt on/off) on expanded corpus.
Gate:
- Required matrix cells green,
- formal lane current (WSL async Kani path exercised and evidenced),
- benchmark guardrails met,
- no uncategorized coverage gaps for declared `v56` scope.

## Execution Pattern For This Ladder
Per profile cycle:
1. Implement pass-pack deltas (`P0..P6`).
2. Expand conformance corpus and expected snapshots (`P7`).
3. Update coverage/divergence/evidence artifacts (`P8`).
4. Register and run formal obligations (`P9`), using async WSL Kani for long lanes.
5. Run:
   - `./scripts/run-matrix.ps1 -ProfileScope <profile>`
   - `./scripts/run-formal.ps1 -ProfileScope <profile>` (and async strict run where applicable)
   - `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal`
6. Commit profile status/workset/evidence updates.

## Success Criteria At Ladder End (`v56`)
1. Language coverage index shows closure of all currently `planned`/`partial` core-language rows targeted by `v37..v44`.
2. Intrinsic registry and capability model are in place, with covered function families from `v45..v52`.
3. Host/interop behavior is split from pure runtime intrinsics and documented via capability flags/evidence.
4. Formal reports explicitly distinguish local vs WSL Kani execution, and async long-run evidence is reproducible.
5. Consolidated gate report for `v56` is green with benchmark + parity guardrails.
