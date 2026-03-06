# WORKSET: VBP Subset Adapter and Compiled Artifact Plan (VBP-S0/A0)

Date: 2026-03-05
Status: planned
Scope: implement `.vbp` subset ingest and first compiled project artifact lane

## 1. Objectives

1. Add deterministic `.vbp` ingest for a constrained, documented subset.
2. Map parsed projects to existing `ProjectManifest`/`ProjectGraph` without semantic drift.
3. Add first compiled artifact format for fast repeat execution.
4. Publish conformance fixtures, diagnostics, and evidence artifacts for the new path.
5. Add wrapper packaging plans for EXE and in-process STA COM DLL on top of shared artifacts.

## 2. Target scope

### 2.1 In-scope (VBP-S0)

- Parse `.vbp` keys:
  - `Type`, `Name`, `Startup`, `Module`, `Class`, `Reference` (restricted).
- Load `.bas`/`.cls` module files.
- Resolve project-relative paths deterministically.
- Produce canonical `ProjectManifest`.
- Execute through existing `Engine::execute_project_with_snapshot_phased` path.

### 2.2 Out-of-scope (explicitly deferred)

- Form/control designer files (`.frm`/`.frx`, `.ctl`/`.ctx`) execution parity.
- Full VB6 build metadata and binary compatibility semantics.
- Full MS-OVBA container parity claims.
- Native-code image generation beyond existing VM/JIT flow (planned later).

Decision note (2026-03-06):

- Native image planning remains deferred until wrapper/artifact parity stabilizes.
- Wrapper work should prioritize two explicit build profiles:
  - `lite` (no JIT),
  - `jit` (VM + JIT).

## 3. Phase plan

### Phase VBP1 - Data model and parser skeleton

Deliverables:

- Add parser module (`oxvba-host` or dedicated crate) with:
  - `VbpProject` model,
  - key/value parser,
  - deterministic path resolver.
- Stable parse diagnostics (`VBP-E-*` codes).

Checks:

- Unit tests for whitespace/quoting/case handling.
- Deterministic parse snapshot tests.

### Phase VBP2 - Manifest bridge

Deliverables:

- Bridge `VbpProject -> ProjectManifest`:
  - `Type` to `ProjectKind` mapping,
  - module/class file loading into `ModuleUnit`,
  - restricted `Reference` mapping to `ProjectReference` + importlib hints.
- Startup validation in subset mode.

Checks:

- Unit tests against known-good fixture matrix.
- Explicit unsupported tests for forms/designer keys.

### Phase VBP3 - CLI integration

Deliverables:

- Add command: `oxvba run-project <path.vbp>`.
- Reuse existing runner bootstrap/policy flags.

Checks:

- CLI parse tests.
- End-to-end smoke fixture run on Windows and Linux (where subset behavior is host-neutral).

### Phase VBP4 - Compiled artifact A0 (bytecode bundle)

Deliverables:

- Add compiled artifact format `A0` (versioned):
  - canonical manifest projection,
  - bytecode payload,
  - source hash set,
  - build/runtime fingerprint.
- Add commands:
  - `oxvba compile-project <path.vbp> --out <artifact>`
  - `oxvba run-artifact <artifact>`

Checks:

- Roundtrip tests (`compile-project -> run-artifact`) match loose-run snapshots.
- Schema version mismatch diagnostic is stable.

### Phase VBP5 - Wrapper EXE packaging

Deliverables:

- Add command: `oxvba build-wrapper-exe <path.vbp> --out <exe>`.
- Wrapper executable embeds:
  - runtime configuration baseline,
  - project artifact payload (`A0`),
  - policy/profile fingerprint metadata.
- Add startup mode flags for wrapper execution (`--jit` on/off policy).
- Provide explicit wrapper build flavors:
  - default `lite` flavor (no JIT),
  - opt-in `jit` flavor.

Checks:

- Wrapper EXE run matches `run-project` and `run-artifact` snapshots for fixture set.
- Fingerprint mismatch behavior is deterministic and test-covered.
- Size budget checks are published for both flavors.

### Phase VBP6 - Wrapper DLL (in-process STA COM server)

Deliverables:

- Add command: `oxvba build-wrapper-dll <path.vbp> --out <dll> --com-sta`.
- Implement first COM server shell:
  - `DllGetClassObject`
  - `DllCanUnloadNow`
  - STA-only activation guard
  - late-bound `IDispatch` subset adapter into project runtime.
- Define deterministic `HRESULT`/diagnostic mapping table for wrapper boundary.

Checks:

- In-proc COM smoke tests through controlled harness.
- Non-STA activation path fails with stable error.
- Repeated activation/invocation lifecycle tests remain deterministic.

### Phase VBP7 - Conformance + evidence + docs

Deliverables:

- Add conformance fixtures under `conformance/tests/project_vbp_subset/...`.
- Add conformance matrix doc for VBP-S0 clauses.
- Add deferred topics entries for unsupported VB6/OVBA surfaces.
- Add wrapper-specific conformance fixtures for EXE and DLL lanes.

Checks:

- `meta-check` integration lane for VBP subset (non-HAL, deterministic).
- Evidence artifacts generated in `docs/evidence/conformance/project_vbp_subset/`.
- Evidence artifacts generated in:
  - `docs/evidence/conformance/project_vbp_wrapper_exe/`
  - `docs/evidence/conformance/project_vbp_wrapper_dll/`

## 4. Diagnostics policy

- New diagnostic namespace:
  - `VBP-E-PARSE-*`
  - `VBP-E-UNSUPPORTED-*`
  - `VBP-E-PATH-*`
  - `VBP-E-MAP-*`
- Unknown/unsupported keys policy:
  - default strict mode in CI/profile gates,
  - optional compat mode later (explicitly tracked as implementation-defined).

## 5. Conformance seeds (initial)

1. single-module executable project (`Module + Startup`).
2. multi-module qualification collision case.
3. class module present but no deferred class-event features.
4. typelib reference mapping with deterministic importlib hint.
5. unsupported `Form=` key returns stable diagnostic.
6. path traversal/absolute-path policy rejection case.

## 6. Artifact A0 design constraints

- Deterministic serialization order.
- Explicit schema id/version.
- Toolchain fingerprint capture (`oxvba` version + runtime profile/policy hash).
- Optional `--no-bytecode-cache` mode for debug parity.
- Wrapper consumers (`exe`/`dll`) must consume the same canonical artifact contract.

### 6.1 Wrapper footprint baseline (measured, 2026-03-06)

- `oxvba-cli.exe`: `5.52 MiB` (`5,785,088` bytes)
- runtime+JIT probe executable: `4.93 MiB` (`5,171,712` bytes)
- VM-only probe executable: `0.44 MiB` (`465,920` bytes)

Guidance:

- treat `lite` (no-JIT) wrapper as baseline footprint target,
- treat `jit` wrapper as explicit performance-oriented variant.

## 7. Risks and mitigations

- Risk: accidental broad VB6 compatibility claims.
  - Mitigation: explicit `VBP-S0` label + unsupported diagnostics + clause catalog.
- Risk: parser permissiveness drift.
  - Mitigation: parser snapshot tests + fixture lockfile.
- Risk: artifact format churn.
  - Mitigation: versioned schema + compatibility matrix in docs.
- Risk: COM wrapper lifecycle/reentrancy defects.
  - Mitigation: STA-only first, deterministic harness, strict lifecycle tests before broad claims.
- Risk: wrapper size growth hides deployment cost.
  - Mitigation: keep flavor-specific size budgets and fail conformance when budgets regress unexpectedly.

## 8. Exit criteria

1. `run-project` works for VBP-S0 fixture set with deterministic results.
2. `compile-project`/`run-artifact` parity checks pass for fixture set.
3. `build-wrapper-exe` output runs with parity against loose/artifact lanes.
4. `build-wrapper-dll` output activates as in-process STA COM server with deterministic late-bound subset behavior.
5. Unsupported keys fail with stable diagnostics.
6. VBP subset and wrapper conformance/deferred topics are published and linked from docs index.
7. Wrapper flavor size baselines (`lite`/`jit`) are captured in evidence and tracked for regression.
