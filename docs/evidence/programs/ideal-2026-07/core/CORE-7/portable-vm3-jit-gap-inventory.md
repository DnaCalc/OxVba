# CORE-7 Portable VM3/JIT Gap Inventory

Date: 2026-08-18
Bead: `bd-59co.2.9.1` (inventory for the portable-basics tranche)
Status: investigation evidence. This file does not close `CORE-JIT-LOWERING` or `OXIR-JIT-DISPOSITION`.

## Scope

This inventory covers portable language/runtime behavior that VM3 already executes
and that the JIT must match before any Windows COM, Declare, pointer, session-cache
or packaging work. Real VBA remains the compatibility target; VM3 is the current
reference interpreter for this tranche.

Out of this tranche:

- `ComCallEarly`, `ComCallLate`, typelib/COM serving, connection points
- `Declare Lib` / native callbacks / `Ptr` / `AddressOf` Windows ABI
- persistent JIT package sessions, product cache, native DLL/EXE output
- the later CORE-7 architecture rewrite (typed primary entries, inspectable
  lowering plan, sealed `VerifiedOxImage` admission, versioned helper catalog)

Those remain accepted CORE-7/CORE-8/WIN scope under `bd-59co.2.9.9` and the
Windows workset. They are not required for the portable-basics pause gate.

## Current implementation shape

- `oxvba-jit` is a real Cranelift backend in one 35,454-line `src/lib.rs`.
- Public entry is still the universal dynamic ABI
  `unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32`.
- There is no VM fallback. Unsupported OxIR is a hard decline.
- VM3 implements the full portable OxInst vocabulary, including COM/native ops.
- Focused `cargo test -p oxvba-jit` and selected Linux-safe differentials are
  green. That proves a large implemented subset, not portable VM3 parity.

The Linux-safe scope snapshot currently records:

| fixture | JIT |
|---|---|
| scalar/checked_long_loop | compiled |
| coercion/variant_string_long | compiled |
| arrays/dynamic_long_loop | compiled |
| records/nested_udt_arrays | compiled |
| strings/mid_mutation_boundary | compiled |
| error/resume_next_div_zero | compiled |
| unsupported/native_declare | declined (`native/COM calls start in M4-9`) |
| project_object/call_by_name_method | compiled |
| project_object/class_field_aggregates | compiled |
| project_reference/bundle_only_class_aggregates | compiled |

Compile/decline status is not structural parity.

## Confirmed portable gaps

### Hard declines that VM3 already executes

1. Whole-image admission rejects any program with a nonempty `external_calls` or
   `com_interfaces` table, even when the executed path never lowers those ops.
   A module that merely *declares* a unused `Declare` therefore cannot JIT.
2. `OxInst::ErlGet` is not lowered and falls through to
   `instruction not lowered in M4-4`.
3. `OxInst::SetLineNumber` is a no-op. JIT `Erl` cannot seat the active line.
4. `OxInst::ErrFieldSet` is not lowered.
5. `OxInst::ArrayAppend` is not lowered.
6. `OxInst::Ptr` is not lowered (Windows/native; out of this tranche).
7. `OxInst::ComCallEarly` is an explicit decline (Windows; out of this tranche).
8. M4-era subset declines remain in ordinary portable shapes:
   omitted Optional arguments except ByVal Variant; ParamArray except ByVal
   Variant / dynamic Variant-array; several checked arith/coerce/compare lanes;
   `NewExtern` for imported VBA/COM library classes other than the Collection
   special case; `TypeOfIs` only for statically typed active-project classes.

### Semantic mismatch risks on already-admitted code

- Error/Resume control exists (`SetErrorHandler`, `ClearErr`, `ErrFieldGet`),
  but line/Erl seating does not, so Resume/Erl fixtures can compile and still
  diverge from VM3.
- Recursion uses the native stack with `MAX_JIT_FRAMES = 50_000`; there is no
  proven VBA error-28 path before process-stack exhaustion.
- Differential comparison of objects/arrays/records is still tag-only
  (`Canon::Opaque`) in the shared harness. Status/tag equality can hide
  structural drift.
- Helper registration is not the versioned catalog required by `RUNTIME-ABI-001`.
  Portable basics may keep the current helpers if VM3 observables match; the
  catalog rewrite stays on `bd-59co.2.9.9`.

## Required first delivery path

1. `bd-59co.2.9.2` — fail-closed structural VM3/JIT harness for the portable
   basics corpus.
2. `bd-59co.2.9.3` — line/Erl/Err write/resume seating.
3. `bd-59co.2.9.4` — remaining portable OxIR declines that VM3 executes,
   including unused-Declare whole-image rejection.
4. `bd-59co.2.9.5` — calls, ByRef, Optional, ParamArray.
5. `bd-59co.2.9.6` — arrays, records, strings, project objects.
6. `bd-59co.2.9.7` — portable VBA library routes.
7. `bd-59co.2.9.8` — pause gate: every VM3-executed portable-basics fixture
   matches JIT. Then stop for the Windows/COM/packaging discussion.
8. `bd-59co.2.9.9` — remaining CORE-7 architecture after that pause.

No row in this inventory is `implemented` or `closed`. Status remains
`in-progress` until `bd-59co.2.9.8` has matching evidence.
