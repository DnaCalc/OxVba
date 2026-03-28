# VBP Subset and Project Artifact Strategy Discussion v1

Status: `design-draft`
Date: 2026-03-05
Scope: pragmatic `.vbp` subset support plus executable artifact strategy for OxVba

## 1. Why this now

OxVba already has a deterministic internal project model (`ProjectManifest`/`ProjectGraph`) and multi-module/reference execution paths, but it lacks a direct VB6-era project file ingest path (`.vbp`) and an explicit on-disk compiled project artifact.

This document defines:

1. the `.vbp` subset we can support with current runtime/compiler capabilities,
2. the short-path implementation approach,
3. lateral options for compiled artifact design and runner UX.

## 2. Current capability baseline (what exists today)

Grounded in current code:

- Internal project model: `ProjectManifest`, `ModuleUnit`, `ProjectReference`, `ReferencedProjectManifest`.
- Module kinds available in model: `Procedural`, `Class`, `Document`, `Form`, `Extension`.
- Reference kinds available in model: `Project`, `TypeLibrary`, `HostInjected`.
- Host execution path supports direct project execution: `Engine::execute_project_with_snapshot_phased(...)`.
- CLI currently runs single-source files (`oxvba run <file>`), not project containers.
- Project storage import/export parity (MS-OVBA aligned) is still planned/deferred.

Implication: we can immediately add a deterministic `.vbp -> ProjectManifest` bridge for a constrained subset without waiting for full OVBA storage parity.

Direct single-file execution (`oxvba run <file.bas>`) is also a first-class OxVba hosting lane. That direct-file lane should support top-level executable statements without requiring a `.vbp` or `.basproj`, while still sharing the same compiler/runtime substrate as project execution.

## 3. Proposed `.vbp` subset for immediate support (VBP-S0)

Goal: load real loose-file projects that map cleanly onto existing execution semantics.

### 3.1 Supported keys (initial)

- `Type=`
  - accepted: `Exe`, `OleDll`, `Control`.
  - mapped to `ProjectKind`:
    - `Exe` -> `Source`
    - `OleDll`/`Control` -> `Library`
- `Startup=`
  - accepted as explicit startup/entrypoint metadata.
- `Module=<logicalName>; <path>`
  - load as `ModuleKind::Procedural`.
- `Class=<logicalName>; <path>`
  - load as `ModuleKind::Class`.
- `Reference=`
  - `*\G...` is accepted for ordered type-library references.
  - `*\A...` is accepted for ordered project references when the referenced artifact path ends in `.vbp` or `.basproj`.
- `Name=` (or equivalent project name metadata)
  - used as `ProjectManifest.project_name` when valid.

### 3.2 Explicitly deferred keys (initial)

- `Form=...`, `UserControl=...`, `PropertyPage=...`, designer sidecars (`.frx`/`.ctx`):
  - parsed as known-but-unsupported with stable diagnostics.
- Build/IDE metadata (`MajorVer`, `MinorVer`, `RevisionVer`, debug flags, optimization flags, icon/version resources):
  - accepted as ignored metadata or rejected depending on strictness mode.
- COM registration and binary compatibility directives:
  - deferred to later COM/server milestones.

### 3.3 Path and determinism policy

- Paths resolved relative to `.vbp` directory only.
- No globbing, no environment-variable expansion in VBP-S0.
- UTF-8 + ANSI fallback policy must be explicit and deterministic.

## 4. Proposed runner UX

### 4.1 New command family

- `oxvba run-project <path.vbp> [--jit] [runner policy flags...]`
- `oxvba compile-project <path.vbp> --out <artifact>`
- `oxvba run-artifact <artifact> [--jit|--no-jit]`
- `oxvba build-wrapper-exe <path.vbp> --out <exe>`
- `oxvba build-wrapper-dll <path.vbp> --out <dll> --com-sta`

### 4.2 Execution modes

- Loose mode: parse `.vbp`, load source files, build `ProjectManifest`, compile+run.
- Compiled mode: load precompiled artifact and run directly, with optional JIT stage.

## 5. Compiled artifact brainstorming (lateral pass)

Question: what should "compiled" mean at this stage?

### Option A: Bytecode bundle (`.oxvbapkg`)

- Contents:
  - canonicalized project manifest snapshot,
  - normalized source hash set,
  - emitted bytecode,
  - host export table,
  - build metadata (tool version/profile/policy fingerprint).
- Pros:
  - smallest scope jump from current runtime,
  - fast startup,
  - deterministic replay.
- Cons:
  - ties to bytecode schema evolution.

### Option B: Source+cache package

- Contents:
  - source files + normalized manifest + optional bytecode cache.
- Pros:
  - robust across bytecode version changes,
  - easier audit/debug.
- Cons:
  - slower cold starts.

### Option C: Native cache layer (per host profile)

- Artifact references profile-locked machine code cache (or generated-on-first-run cache).
- Pros:
  - highest hot-path speed potential.
- Cons:
  - cache invalidation complexity,
  - portability constraints.

### Option D: Wrapper EXE (runtime + embedded project artifact)

- One executable embedding:
  - OxVba runtime slice,
  - project artifact payload (`A0` or source+cache),
  - bootstrap policy defaults.
- Pros:
  - distribution simplicity with no external project files,
  - clear \"app\" deployment shape for VB6-style project execution.
- Cons:
  - larger binaries,
  - update/versioning policy needs explicit runtime/artifact compatibility gates.

### Option E: Wrapper DLL (in-process COM server, STA-only)

- One DLL embedding runtime + project artifact and exposing COM class factories.
- Model:
  - `DllGetClassObject`, `DllCanUnloadNow`, optional `DllRegisterServer`/`DllUnregisterServer` helper lane.
  - STA-only apartment contract for all exposed objects.
  - Late-bound baseline (`IDispatch`) first; early-bound interfaces later.
- Pros:
  - natural fit for host integration scenarios expecting COM automation servers.
  - reuses existing COM + ProjectGraph investments.
- Cons:
  - COM lifecycle/refcount/reentrancy correctness work is non-trivial,
  - needs strict threading and message-pump policy contract.

### Option F: Native code image (non-LLVM-first)

- Ahead-of-time native emission path using non-LLVM strategy first (for example Cranelift object emission, or bytecode-to-C transpile + system compiler lane).
- Pros:
  - potential startup and steady-state wins for stable workloads.
- Cons:
  - highest complexity and portability burden at this stage,
  - larger semantic parity surface to verify.

### Size findings (measured, 2026-03-06, Windows x64 release)

Measured on local `x86_64-pc-windows-msvc` toolchain:

- `oxvba-cli.exe`: `5.52 MiB` (`5,785,088` bytes)
- runtime+JIT probe executable (VM + Cranelift JIT, no CLI/project parsing): `4.93 MiB` (`5,171,712` bytes)
- VM-only probe executable (no JIT): `0.44 MiB` (`465,920` bytes)

Interpretation:

- Cranelift/JIT contributes most of the wrapper footprint.
- A lightweight wrapper profile should include a no-JIT mode as a first-class packaging target.
- \"runtime + JIT\" is still meaningfully smaller than full CLI, but not by an order of magnitude.

## 6. Thin de Bono-style lateral probes

- White (facts): current engine already compiles/runs multi-module manifests; storage ingest is the missing edge.
- Red (risk intuition): `.vbp` compatibility claims can drift if parser leniency is undocumented.
- Black (caution): forms/designer surface can explode scope quickly; keep hard diagnostics at first.
- Yellow (value): `.vbp` subset unlocks immediate migration/testing with real legacy projects.
- Green (creative): treat `.vbp` as an adapter layer into a canonical OxVba manifest, then target multiple packaging shapes (`artifact`, `exe-wrapper`, `dll-wrapper`) without changing core semantics.
- Blue (process): versioned subset contracts (`VBP-S0`, `VBP-S1`) with explicit conformance fixtures and drift guards.

## 6.1 COM wrapper DLL shape (first-pass contract)

- Deployment target: Windows only (initial).
- Threading: STA-only, explicit failure for non-STA activation paths.
- Execution model:
  - project payload loaded once per COM server lifetime,
  - per-object dispatch routed through OxVba runtime boundary,
  - deterministic mapping from COM error (`HRESULT`) to OxVba diagnostics and vice versa.
- Interface tiers:
  - Tier 1: `IDispatch` only (late-bound methods/properties/events subset).
  - Tier 2: optional dual/early-bound interfaces after typelib/importlib maturity.
- Safety boundaries:
  - policy presets must be pinned at wrapper build time (with optional runtime overrides under strict allowlist),
  - unsupported host-sensitive operations follow existing `UnsupportedFeatureMode` contract.

## 7. Recommended path

1. Land VBP-S0 adapter and diagnostics first.
2. Land minimal compiled artifact as bytecode bundle (Option A) with strict schema versioning.
3. Land wrapper EXE packaging on top of artifact A0 (no new semantics).
4. Land wrapper DLL STA-only baseline with `IDispatch` surface only.
5. Keep source+cache fallback for forward compatibility.
6. Defer designer/form and full build metadata until PMR/OVBA extraction depth improves.
7. Defer native code image until artifact and wrapper parity is stable.
8. Add explicit wrapper profiles:
   - `wrapper-lite` (VM only, no JIT; minimum footprint),
   - `wrapper-jit` (VM + JIT; higher performance, larger footprint).

## 8. Contract and conformance posture

- Every accepted `.vbp` key must map to one of:
  - executable semantic effect,
  - explicitly ignored with recorded rationale,
  - deterministic unsupported diagnostic.
- Add clause IDs for VBP subset behavior in a dedicated catalog before broad compatibility claims.
- Keep MS-OVBA parity claims deferred until source extraction gap closes.

## 9. Open decisions

1. Should `Type=Exe` startup resolution in VBP-S0 follow the same OxVba ladder as `.basproj` and direct-file execution: explicit `Startup`, else unique top-level mainline, else unique `Sub Main`?
2. Should unknown keys fail by default (`strict`) or warn (`compat`) in first release?
3. Should compiled artifacts be profile-bound by default (safer determinism) or profile-portable (easier distribution)?
4. Should `.vbp` parsing live in `oxvba-host` (adapter layer) or a dedicated `oxvba-projectio` crate?
5. Wrapper EXE strategy: static runtime embedding vs runtime sidecar requirement?
6. Wrapper DLL COM registration: registry-free first, or dual lane (registry + registration-free)?
7. Should STA enforcement occur at activation time only, or on every method entry guard as well?
8. Native image lane: Cranelift object emission first, or postpone fully until after wrapper convergence?
9. Should `wrapper-lite` be the default for `build-wrapper-exe`, with `--jit` as an opt-in build flavor?

## 10. Near-term success criteria

- Can run a real loose-file project via `.vbp` for the supported key set.
- Can emit/load a versioned compiled artifact and reproduce snapshots deterministically.
- Can package and run the same payload via wrapper EXE without semantic drift.
- Can activate a wrapper DLL as in-process STA COM server and invoke through deterministic `IDispatch` subset.
- Unsupported surfaces fail with stable, cataloged diagnostics.
