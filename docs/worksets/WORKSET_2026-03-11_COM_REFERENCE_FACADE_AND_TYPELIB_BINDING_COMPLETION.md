# Workset: COM Reference Facade and Typelib Binding Completion

Date: 2026-03-11  
Status: planned  
Primary ladder mapping: `v487..v496`, `v517..v523`  
Secondary ladder mapping: `v524..v526`, `v527..v533`

## 1. Objective

Make COM type-library imports look like proper referenced external libraries to the compiler/binder instead of a separate ad hoc COM-only symbol domain.

This workset turns the design rule into implementation:
1. typelib-backed imports become synthetic reference/project metadata,
2. binder/typecheck/lowering consume that metadata,
3. early-bound and metadata-backed late-bound behaviors derive from the same authoritative imported-member model.

## 2. Why this workset exists

Current problems:
1. early-bound COM is still partly a rewrite-oriented subset rather than a fully coherent imported-library model,
2. COM metadata is not yet fully treated as compiler-visible reference structure,
3. default-member/member-kind/invoke-kind coverage remains split across transitional paths,
4. the architectural COM cleanup needs a compile-time destination, not only a runtime one.

This workset is the compiler/binder-facing cleanup complement to the unified dynamic-object protocol work.

## 3. Target architecture

### 3.1 Synthetic reference facade

Typelib-backed imports should project into compiler-visible structures that behave like referenced libraries where VBA semantics allow:
1. imported namespaces and types participate in normal reference precedence,
2. imported classes/interfaces/enums/constants are represented as reference-owned symbols,
3. member metadata is available as binder-visible data rather than only runtime lookup material,
4. default-member, invoke-kind, optional/named parameter, and event metadata are attached to those imported symbols.

### 3.2 Lowering rule

Lowering should increasingly use imported metadata rather than hardcoded token tables:
1. early-bound type/member resolution follows imported-library metadata,
2. diagnostics for unsupported or mismatched metadata become compile-time and deterministic,
3. metadata-backed late-bound default-member/member-shape improvements can reuse the same source of truth.

### 3.3 Boundary rule

`oxvba-com` should own:
1. typelib ingestion,
2. metadata normalization,
3. projection into synthetic reference-facade structures.

Compiler and binder should consume that projected metadata, not raw COM wire details.

## 4. Scope

### In scope

1. Synthetic reference-facade model for typelib-backed imports.
2. Compiler/binder integration for imported COM symbols.
3. Typelib metadata normalization needed for member/default-property/invoke-kind coverage.
4. Lowering and diagnostics cleanup so imported COM members behave like referenced-library members where applicable.

### Out of scope

1. Full COM runtime invoke parity by itself.
2. Full server/export publication parity.
3. All Office oracle lanes for imported COM behavior.

## 5. Deliverables

1. Explicit synthetic reference-facade representation and ownership model.
2. `oxvba-com` metadata projection support for that facade.
3. Binder integration for imported COM symbols and precedence rules.
4. Lowering updates that reduce hardcoded member token tables and rely on metadata.
5. Deterministic diagnostics for unresolved, ambiguous, unsupported, and mismatched imported members.
6. Tests for:
   - imported type resolution,
   - reference precedence,
   - default-member metadata,
   - invoke-kind/member-kind mapping,
   - early-bound lowering parity for the supported scope.

## 6. Execution phases

### Phase A. Facade model lock

Deliverables:
1. define the projected metadata model,
2. define how it attaches to PMR/ProjectGraph/reference binding state,
3. define the compiler-visible symbol kinds for imported COM constructs.

Acceptance:
1. imported COM metadata is treated as compiler-facing reference structure, not only runtime lookup data.

### Phase B. Metadata normalization and projection

Deliverables:
1. expand normalized typelib metadata where required,
2. project it into the synthetic facade from `oxvba-com`,
3. keep deterministic identity and cache behavior.

Acceptance:
1. imported COM members/types are available through one authoritative metadata path.

### Phase C. Binder/lowering integration

Deliverables:
1. consume the synthetic facade in binder/typecheck,
2. replace or shrink hardcoded member-token assumptions,
3. use metadata for member kind/default member/invoke kind/arity/optional rules in the supported scope.

Acceptance:
1. early-bound COM lowering is materially more metadata-driven and reference-like.

### Phase D. Integration cleanup

Deliverables:
1. align late-bound metadata-backed default-member/member-shape improvements with the same source of truth,
2. update worklists/specs/blockers to reflect the reduced transitional surface,
3. ensure companion runtime worksets consume the same authoritative metadata.

Acceptance:
1. `IP-05` and parts of `IP-03`/`IP-02` have a coherent compile-time metadata home.

## 7. Verification

Core verification:

```powershell
cargo test -p oxvba-compiler -p oxvba-host -p oxvba-com --quiet
cargo test -p oxvba-host --test com_early_project_end_to_end -- --test-threads=1 --nocapture
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
```

Targeted expectations:
1. imported type/member resolution remains deterministic,
2. reference precedence remains stable,
3. early-bound controlled lanes still pass,
4. diagnostics stay machine-stable.

## 8. Exit criteria

This workset is complete when:
1. typelib-backed imports project as a coherent synthetic reference facade,
2. compiler/binder/lowering consume that facade as the authoritative imported-library model,
3. hardcoded subset token tables are materially reduced in favor of metadata-driven lowering for the supported scope,
4. early-bound and metadata-backed late-bound follow-on work can rely on one authoritative imported-member metadata source,
5. docs and worklists reflect that COM imports now behave like referenced-library metadata rather than an ad hoc special case.

## 9. Related documents

- `docs/spec/COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md`
- `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`
- `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- `CURRENT_BLOCKERS.md`
