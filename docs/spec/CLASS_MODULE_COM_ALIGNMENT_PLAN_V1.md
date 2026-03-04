# Class Module COM Alignment Plan v1

Status: `working-draft`
Date: 2026-03-03
Scope: Class-module semantics aligned to COM-compatible behavior without blocking on full COM ABI implementation.

## 1. Decision

OxVba should co-design class modules with COM behavior now, but in two layers:

1. Language/runtime semantic contract (must be COM-congruent).
2. Platform COM bridge mechanics (Windows-focused, staged, partly deferred).

This preserves compatibility while avoiding premature lock-in to one adapter shape.

## 2. Near-Term Planned Steps (Do Now)

## Step A1: Lock semantic object model contracts

Define and stabilize class-runtime invariants independent of ABI:

- object identity stability within runtime handles,
- reference-lifetime transitions and deterministic `Class_Initialize`/`Class_Terminate` boundaries,
- `Nothing` assignment and nullability semantics,
- `Property Get/Let/Set` dispatch classification and default-member surface,
- `Implements` and `WithEvents` legality/coverage requirements.

Deliverables:

- PMR class/object clauses upgraded with explicit pre/postconditions.
- Diagnostic taxonomy additions for class/reference lifecycle errors.

## Step A2: Add executable non-interop conformance lane

Add fixtures that validate class semantics without external COM dependencies:

- lifecycle ordering (`Initialize`/`Terminate`) under scope exit and errors,
- `Property Get/Let/Set` ordering and side-effect behavior,
- `Implements` member-coverage diagnostics,
- `WithEvents` declaration legality and handler-prefix routing.

Deliverables:

- PMR conformance lane updates and fixture matrix.
- Evidence artifacts under `docs/evidence/language/` and PMR conformance reports.

## Step A3: Define COM-boundary contract for class semantics

Specify host-boundary obligations that keep class semantics COM-compatible:

- dispatch name lookup contract (`GetIDsOfNames` case-insensitive mapping),
- invocation argument/result contract (`Invoke` packaging rules subset),
- object-handle identity projection and deterministic failure mapping.

Deliverables:

- PMR/HAL cross-reference updates for class-to-dispatch boundaries.
- Explicit statement of which guarantees are semantic vs adapter-specific.

## Step A4: Integrate class model into Project/Module graph design

Ensure project/module/reference model carries enough metadata for class behavior:

- class instancing metadata (`VB_PredeclaredId`, `VB_Exposed`),
- event/member prefix metadata for `WithEvents`/`Implements`,
- reference precedence inputs used by class/member bind paths.

Deliverables:

- Project graph schema updates in PMR specs.
- Binder/runtime integration notes with deterministic failure classes.

## Step A5: Gate claims with compatibility tiers

Use a strict claim model:

- `implemented-verified`: executable local checks pass and spec clauses are linked.
- `implemented-partial`: deterministic subset only, explicit exclusions listed.
- `specified-pending`: formal contract exists, implementation still open.

Deliverables:

- Clause status updates and gate references in conformance docs.
- No "COM-compatible" claim beyond covered tier.

## 3. Deferred Aspects (Explicit)

The following are intentionally deferred and tracked as interop/HAL-adjacent work:

1. Real Windows COM ABI surfacing (`IUnknown`/`IDispatch` vtable plumbing).
2. COM apartment/threading model integration and callback marshaling behavior.
3. Registry/type-library driven activation matrix beyond deterministic subset.
4. Full `IDispatch::Invoke` out-parameter parity (`VarResult`/`ExcepInfo`/`ArgErr`) across broad call shapes.
5. Cross-platform COM emulation (non-Windows remains explicit unsupported in v1).
6. Rich external automation library parity and full marshaling breadth.

Tracking anchors:

- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`:
  - `ODG-023` (class lifecycle ordering),
  - `ODG-030`, `ODG-031` (COM/typelib interop),
  - `ODG-038`, `ODG-039` (Implements/WithEvents),
  - `ODG-041` (reference/type-library resolution).
- `docs/spec/HAL_COM_BRIDGE_SCOPE_V1.md`
- `docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`

## 4. HAL Interaction Plan

PMR and HAL boundaries stay explicit:

- PMR owns language semantics, diagnostics, and binder/runtime classification.
- HAL owns host capability exposure, policy gating, and host error projection.
- COM capability unsupported profiles must fail deterministically without semantic ambiguity.

Immediate HAL follow-up points:

- verify class-related failure codes map cleanly through host diagnostics,
- verify compile-time vs runtime gating remains deterministic by profile/policy mode,
- keep class semantics executable in non-COM profiles for language-only paths.

## 5. Promotion Criteria

This plan is considered "phase complete" when:

1. A1-A5 deliverables are mapped to clause IDs and tests.
2. Deferred items remain explicitly listed (no silent scope creep).
3. PMR + HAL docs agree on boundary responsibilities for class/COM behavior.

## 6. Current Execution Snapshot (A1-A5)

Status as of 2026-03-03:

- A1: completed in spec + code scaffold (`ProjectGraph`, PMR error codes, class semantic contract text).
- A2: completed for non-interop executable subset (class lifecycle/property tests + explicit class-project diagnostics for deferred features).
- A3: completed as contract boundary documentation (semantic vs adapter split in PMR/HAL docs).
- A4: completed as host-side model scaffold (`ProjectGraph`/`ProjectNode`/references/module metadata and invariants).
- A5: completed via tiered clause status updates and executable tier validation (`formal_pmr_a5_claim_tiers_have_stable_status_values`).
