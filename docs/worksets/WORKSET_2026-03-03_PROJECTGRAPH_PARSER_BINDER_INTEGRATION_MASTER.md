# WORKSET: ProjectGraph Parser+Binder Integration (Master)

Date: 2026-03-03  
Status: completed  
Primary scope: compiler/parser/binder integration for full Project/Module/Reference execution model.

Execution result: `P0..P10` completed in deterministic subset with explicit deferred-oracle gates for parity topics.
Rollup evidence: `docs/evidence/language/PMR_PROJECTGRAPH_P0_P10_ROLLUP_2026-03-03.md`.
Fixture matrix: `docs/evidence/conformance/PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md`.
Oracle templates: `docs/evidence/conformance/PMR_PROJECT_MODEL_ORACLE_TEMPLATES_V1.md`.

## 1. Objective

Implement a deterministic, testable ProjectGraph pipeline that moves OxVba from single-source compilation to true project/module/reference compilation, aligned with PMR specs and clause catalogs.

This workset is the execution plan for:

- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.md`

## 2. Inputs and Tracking Anchors

Normative/extracted sources and local tracking:

- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`
- `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv` (`CCT-037..CCT-045`)
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` (`ODG-035..ODG-043`)
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/`
- `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/`

## 3. Scope

In scope:

- Module-header retention and module-kind metadata in parse pipeline.
- New multi-module/project compile entrypoint.
- Binder integration with ProjectGraph symbol spaces and reference precedence.
- Deterministic PMR diagnostics for project/module/reference name/visibility errors.
- Cross-module and cross-project resolution in current executable subset.
- Host-facing export registry surface for public procedural members (requirement `MODPROJ-039`).

Out of scope for this workset:

- Full forms runtime behavior.
- Full document-module event wiring.
- Full COM ABI parity and rich external automation parity.
- Full MS-OVBA ingest/emit parity (remains blocked by extraction depth).

## 4. Workstreams

1. `WS-A`: Parser and module artifact model.
2. `WS-B`: Project manifest and compile API.
3. `WS-C`: Binder symbol spaces and resolution engine.
4. `WS-D`: Diagnostics and deterministic error contract.
5. `WS-E`: Host integration and callable export registry.
6. `WS-F`: Conformance, oracle scaffolding, and evidence sync.

## 5. Ordered Work List

## Phase P0: Baseline Lock

1. Confirm PMR clause baseline and status vocabulary in CSV/MD are synchronized.
2. Freeze starting gap snapshot in `MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`.
3. Add this workset to profile execution references before implementation starts.

Exit:

- Baseline files are parseable and status taxonomy clean under meta checks.

## Phase P1: Parser Artifacts (WS-A)

4. Introduce parser-level `ModuleUnit` artifact with:
   - module text,
   - module header attributes (`VB_Name`, `VB_PredeclaredId`, `VB_GlobalNamespace`, `VB_Creatable`, `VB_Exposed`),
   - module directives (`Option Private Module`, `Option Compare`, `Option Base`),
   - declared module kind.
5. Add deterministic header parsing/retention tests for standard and class modules.
6. Add diagnostics for malformed header/attribute lines and header-name mismatches.

Exit:

- Parser can retain module headers without dropping existing language behavior.

## Phase P2: Project Manifest Model (WS-B)

7. Define compiler-facing `ProjectManifest` model:
   - project identity/kind,
   - ordered module list,
   - ordered references,
   - conditional constants.
8. Add `compile_project(manifest)` entrypoint while preserving existing `compile(source)` API.
9. Ensure deterministic manifest normalization rules (identifier canonicalization, order stability).

Exit:

- Single-source path remains intact; project path compiles empty/minimal manifest deterministically.

## Phase P3: Binder ProjectGraph Core (WS-C)

10. Build binder-stage `ProjectBindContext`:
    - project symbol index,
    - module public/private symbol tables,
    - reference index with precedence ordering.
11. Implement module-name uniqueness, reference-target uniqueness, and project-name legality checks at bind start.
12. Integrate module-level declaration collision behavior into project-aware symbol spaces.

Exit:

- Binder rejects basic PMR identity violations with stable PMR diagnostic codes.

## Phase P4: Name Resolution and Qualification (WS-C)

13. Implement unqualified resolution precedence:
    - local module,
    - project scope,
    - referenced projects by declared order.
14. Implement module-qualified and project-qualified resolution paths.
15. Implement ambiguity and missing-name diagnostics:
    - qualification required,
    - ambiguous symbol owner set,
    - not found.

Exit:

- Cross-module call/reference subset compiles and executes with deterministic resolution outcomes.

## Phase P5: Visibility Model (WS-C/WS-D)

16. Enforce `Option Private Module` export restrictions across project boundaries.
17. Enforce public name collision qualification rules (`PMR-VIS-002/003`) in project context.
18. Add explicit diagnostics for visibility denial from referencing projects.

Exit:

- Visibility behavior is deterministic and covered by fixture matrix.

## Phase P6: Class-Related Project Bind Semantics (WS-C/WS-D)

19. Replace current hard-gating of `WithEvents`/`Implements`/`RaiseEvent` where straightforward static legality can be enforced via module kind.
20. Keep remaining unsupported semantic depths under explicit PMR diagnostic gates with stable codes.
21. Bind class metadata from headers into project graph:
    - default-instance flags,
    - class exposure attributes.

Exit:

- Class/project static legality checks move from coarse gate to precise gate where feasible.

## Phase P7: Host Export Registry (WS-E)

22. Add host-facing export descriptor model for public procedural modules:
    - exported procedure name,
    - owning module,
    - procedure kind (`Function`/`Sub`),
    - visibility/export eligibility flags.
23. Apply export eligibility rules including `Option Private Module`.
24. Add host API to enumerate callable exports from compiled project graph.
25. Add deterministic invocation hook contract (name->call target mapping), without Excel-specific runtime claims yet.

Exit:

- Requirement `MODPROJ-039` has executable host-side surface and fixture evidence.

## Phase P8: Reference-Aware Bind Paths (WS-C/WS-E)

26. Integrate reference-order precedence into cross-project bind selection in executable subset.
27. Keep OAUT-rich lanes partial where unsupported, with explicit PMR/HAL diagnostic boundaries.
28. Add deterministic failure mapping for unresolved/bad reference descriptors.

Exit:

- Reference precedence works in local synthetic project corpus; interop breadth remains correctly staged.

## Phase P9: Conformance and Evidence (WS-F)

29. Add conformance fixtures:
    - duplicate module names,
    - cross-module public call resolution,
    - project-qualified/module-qualified resolution,
    - visibility with and without `Option Private Module`,
    - reference order shadowing,
    - host export enumeration eligibility.
30. Add property-style determinism tests for identical manifest inputs.
31. Add PMR coverage evidence rollup file for this workset execution.
32. Update:
    - `COVERAGE_INDEX.csv`,
    - `MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`,
    - PMR clause catalog statuses and anchors.

Exit:

- All new behavior has fixture evidence and clause mapping.

## Phase P10: Oracle and Deferred Gates (WS-F)

33. Prepare Excel oracle probe templates for `CCT-037`, `CCT-038`, `CCT-039`, `CCT-040`, `CCT-041`.
34. Add explicit foldback notes for new host-export probes linked to `MODPROJ-039`.
35. Keep unresolved host/interop items open in deferred gates with concrete unblock steps.

Exit:

- No accidental parity claim beyond supported evidence tier.

## 6. Gate Criteria

`G1 Parser`: module headers retained and validated under tests.  
`G2 Manifest`: `compile_project` stable and deterministic.  
`G3 Binder`: project/module/reference identity checks live with PMR diagnostics.  
`G4 Resolution`: qualification and ambiguity diagnostics correct in multi-module corpus.  
`G5 Visibility`: `Option Private Module` project-boundary behavior test-covered.  
`G6 Host exports`: public procedural export registry enumerates deterministic callable set.  
`G7 Conformance`: PMR fixture matrix green; catalogs/statuses synchronized.  
`G8 Integrity`: docs-check + meta-check + full cargo test green.

## 7. Suggested Test Command Bundle

```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/docs-check.ps1
./scripts/check-hal-clause-drift.ps1
./scripts/meta-check.ps1
```

## 8. Deliverables

Code:

- parser/module artifact and project compile API updates (`oxvba-syntax`, `oxvba-compiler`).
- project-aware binder resolution and diagnostics (`oxvba-compiler`).
- export registry and host-facing query hooks (`oxvba-host`).

Evidence/docs:

- PMR conformance fixtures in `conformance/tests/`.
- PMR evidence rollup document for this execution.
- updated PMR clause/status catalogs and requirement rows.

## 9. Risks and Controls

Risk: hidden precedence regressions across existing single-source path.  
Control: dual-path tests (`compile` and `compile_project`) on common corpus.

Risk: over-claiming class/com/reference compatibility.  
Control: strict status tiers (`implemented-verified`, `implemented-partial`, `specified-pending`) and deferred-oracle gate discipline.

Risk: host-export behavior conflated with Excel-specific behavior too early.  
Control: define host-neutral export contract now; treat Excel parity as oracle lane.

## 10. Definition of Done

Done means:

1. ProjectGraph parser+binder flow exists end-to-end for deterministic executable subset.
2. PMR clause statuses reflect real executable evidence and pass taxonomy guard tests.
3. Host-facing public-procedure export registry exists and is test-covered.
4. Remaining ambiguity/parity items are explicitly deferred with oracle gate entries, not implicit.
