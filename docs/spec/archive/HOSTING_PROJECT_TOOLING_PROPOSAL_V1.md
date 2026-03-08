# OxVBA Hosting, Project, Packaging, and Tooling Proposal v1

Status: `design-draft`  
Date: 2026-03-07  
Scope owner: OxVBA runtime/host/tooling  
Related docs:
- `docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md`
- `docs/worksets/WORKSET_2026-03-05_VBP_SUBSET_AND_ARTIFACT_PLAN.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
- `docs/spec/HAL_SPEC_WORKING_DRAFT.md`
- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`

## 1. Executive Summary

This proposal defines the next productization stage for OxVBA after COM early-binding ladder closure (`v466`): turning the existing compiler/runtime/host substrate into a coherent multi-surface product with clear contracts for:

1. app-embedded hosting (primary target, Excel-like role),
2. add-in packaging and execution models (document-scoped and process-global),
3. modern CLI and project workflows for broader ecosystem adoption,
4. WebAssembly hosting and security posture,
5. forward-compatible project and artifact formats that are not VB6-first.

This document is both a discussion paper and requirements source material. It proposes concrete defaults, decision points, phased execution, and command/help UX.

## 2. Current Implementation Baseline (As of 2026-03-07)

### 2.1 Core execution and embedding surface

Implemented:
- `oxvba-host::Engine` with source and project execution paths:
  - `execute_source_with_snapshot_phased(...)`
  - `execute_project_with_snapshot_phased(...)`
- VM and JIT execution toggles exist through `HostConfig.enable_jit`.
- Host policy and runtime-profile selection are implemented (HAL profiles + policy presets).
- Root object registration exists:
  - `register_root_object(name, type_name)`
  - `has_root_object(name)`
- Project model and graph exist (`ProjectManifest`, `ProjectGraph`, references, type-library binding records).

Meaning:
- OxVBA already runs as a library in-process and already has a deterministic project-manifest execution model.
- This is enough to build an embedded host contract now, without waiting for storage/parsing expansions.

### 2.2 Host abstraction and platform posture

Implemented:
- HAL profiles: Windows, Linux, macOS, WASM, Null.
- Runtime classes and host policy presets.
- Compile-time vs runtime unsupported-feature handling.
- Deterministic error taxonomy for host capability failures and policy denials.
- Host-sensitive intrinsic preflight in compile-time policy mode.

Meaning:
- The runtime already has a strong policy and safety substrate suitable for embedders and sandboxes.

### 2.3 COM state

Implemented:
- Late-bound COM client lanes and early-binding/type-library ladder through terminal gate `v466` are complete and evidence-backed.
- Type-library reference binding model exists in PMR/host layers.

Known scope boundary:
- Non-Windows COM behavior remains deterministic unsupported by policy and profile contract.

### 2.4 Project/module/reference and host export posture

Implemented:
- Multi-module project compile/run in deterministic subset.
- Reference-order and export-visibility model (including `Option Private Module` host-direct export split).
- Integration suite and oracle/deferred-gate infrastructure.

Known explicit open gaps:
- `WithEvents`, `Implements`, and `RaiseEvent` full runtime semantics are not complete.
- Project-aware compile-time legality/coverage diagnostics are now implemented; remaining divergence focus is runtime event dispatch/ordering and host wiring (`DIV-0003`, `DIV-0004`).

### 2.5 CLI posture

Implemented:
- Current CLI is single-file execution oriented (`oxvba run <file>` + HAL bootstrap flags).

Not yet implemented:
- `run-project`, `compile-project`, `run-artifact`,
- wrapper build commands (`build-wrapper-exe`, `build-wrapper-dll`),
- directory-first project commands,
- language-service CLI.

### 2.6 Storage and packaging posture

Implemented/planned split:
- Internal canonical project model exists.
- `.vbp` subset and artifact lane have a planned workset and draft strategy.

Not yet implemented:
- Direct `.vbp` ingestion in CLI/runtime path.
- Canonical OxVBA project file format.
- Artifact format with compatibility policy and signing/metadata story.
- Full OVBA package/storage parity.

## 3. Product Use Cases and Outcome Targets

### 3.1 UC-A: Embedded app host (primary, Excel-like)

Goal:
- OxVBA is embedded as a library in another process.
- Source and project artifacts are provided by host-managed storage:
  - host IDE/store, or
  - application file embedding project data.
- Host controls policy, object injection, event routing, diagnostics, lifecycle.

Outcome target:
- A complete host contract that is stable enough for DNA Calc and other embedders.

### 3.2 UC-B: Add-in authoring and runtime (host ecosystem)

Goal:
- Author and ship add-ins independent of documents.
- Support both runtime shapes:
  - add-in-local runtime payload (self-contained),
  - shared language-host process/add-in that loads many projects.

Outcome target:
- Deterministic function registration and invocation contracts.
- Clear scope model: workbook/document scope vs process-global registration.

### 3.3 UC-C: Excel interop pathfinder (parallel validation lane)

Goal:
- Build an XLL-based integration path to run OxVBA add-in projects in Excel for early validation.
- Not parity-identical to native VBA call paths, but high value for compatibility signal and adoption experiments.

Outcome target:
- One working prototype lane that can register functions, invoke macros, and bind Application bridge.

### 3.4 UC-D: General developer/runtime tooling

Goal:
- Provide modern command-line/project workflows usable from many environments (Rust/.NET/Python/Go/etc.).
- Support quick script-like execution and project-grade build/run.

Outcome target:
- Coherent command surface, directory-first defaults, and predictable build outputs.

### 3.5 UC-E: WebAssembly hosting

Goal:
- Support controlled WASM runtime classes with explicit host loading model and capability boundaries.

Outcome target:
- Security-first contract that clearly separates allowed host bridges from unsupported capabilities.

## 4. Key Questions and Recommended Decisions

### 4.1 Canonical project format baseline

Decision proposal: **Adopt a native `oxvba.toml` project format as the canonical format.**

Rationale:
- Avoids inheriting VB6-era constraints as the primary model.
- Better fit for modern directory-first workflows and package/build metadata.
- `.vbp` should be an adapter/import source, not the canonical core.

Decision impact:
- Canonical model remains `ProjectManifest` in memory.
- Parsers/adapters:
  - `oxvba.toml` -> `ProjectManifest` (primary),
  - `.vbp` subset -> `ProjectManifest` (compat adapter),
  - future OVBA extractor -> `ProjectManifest` (container adapter).

### 4.2 Entry point model and top-level code

Decision proposal: **Support both explicit entry procedures and optional top-level modules via a defined extension.**

Rules:
- Default rule remains VBA-compatible explicit entry (`Sub Main` or configured entrypoint).
- Optional extension: top-level statements in files marked as script modules (not generic `.bas` by default).
- `Option` statements and declarations must remain syntactically and semantically valid before top-level executable statements.

Rationale:
- Preserves compatibility expectations while enabling modern script UX.

### 4.3 Add-in scope model

Decision proposal: **First-class scope flag with two modes:**
- `document` scope: exports visible only to owning document/project context.
- `process` scope: exports registered globally in host process (Excel add-in style).

Rationale:
- Covers workbook vs add-in semantics explicitly.
- Prevents accidental leakage from document workflows.

### 4.4 Host object and event hookup

Decision proposal: **Split host event hookup into two layers:**
1. Host bridge contract (object identity, event dispatch API, subscription lifecycle),
2. VBA semantic layer (`WithEvents`/`RaiseEvent`/`Implements` full class graph).

Current state note:
- Layer (1) can be standardized now.
- Layer (2) needs dedicated closure work; current diagnostics confirm it is not done.

### 4.5 Wrapper strategy

Decision proposal:
- Keep shared artifact core (`A0` first).
- Build wrappers as projections over the same artifact.
- Default wrapper flavor should be `lite` (VM-only), with explicit `jit` flavor opt-in.

Rationale:
- Consistent semantics across run modes.
- Better size baseline for distribution.

### 4.6 XLL integration strategy

Decision proposal: **Prototype both shapes, decide after evidence:**
- Shape X1: one self-contained XLL per add-in project.
- Shape X2: one shared OxVBA language-host XLL that loads multiple projects.

Recommendation:
- Implement X1 first for simpler packaging and debugging.
- Implement X2 next if operational gains are clear.

### 4.7 WASM model

Decision proposal:
- Keep deterministic default classes (`wasi`, `browser-sandbox`) with capability-denied fallbacks for unsupported domains.
- Provide explicit host API shims for approved capabilities only (time, diagnostics, optional UI virtualization).

Rationale:
- Aligns with existing HAL policy model and sandbox posture.

## 5. Requirements (Draft Normative)

### 5.1 Embedded host contract requirements

1. OxVBA MUST provide a stable in-process API to load, validate, compile, execute, and unload project units.
2. Host MUST be able to supply project source/artifact bytes from host-controlled storage.
3. Host MUST be able to set runtime profile and host policy before execution.
4. Host SHOULD be able to update policy per-run for sandbox tightening.
5. Engine MUST expose deterministic diagnostics with phase classification (compile-time/runtime).

### 5.2 Object injection and eventing requirements

1. Engine MUST support host root object registration with stable identity keys.
2. Host bridge MUST support object lookup by configured well-known names (for example `Application`).
3. Event bridge MUST support subscribe/unsubscribe semantics with deterministic lifecycle.
4. Full `WithEvents`/`RaiseEvent`/`Implements` semantics MUST be completed before claiming Excel-class event parity.
5. Until then, diagnostics for those surfaces MUST remain stable and documented.

### 5.3 Project and storage requirements

1. Canonical project format MUST not depend on VB6 `.vbp`.
2. `.vbp` support SHOULD be implemented as explicit subset adapter (`VBP-S*` versions).
3. OVBA/document-container import MUST be modeled as separate adapter layer.
4. Directory-first discovery SHOULD be available with explicit include/exclude controls.
5. Project metadata MUST include scope (`document`, `addin`, `library`, `application`) and entrypoint policy.

### 5.4 Artifact and packaging requirements

1. Artifact `A0` MUST be versioned and include compatibility metadata:
   - schema version,
   - tool/runtime fingerprint,
   - source hash set,
   - policy profile hints.
2. Wrappers (`exe`, `dll`, later other targets) MUST consume canonical artifact contracts.
3. Wrapper build MUST support at least two flavors:
   - `lite` (VM-only),
   - `jit` (VM+JIT).
4. Runtime MUST fail deterministically on incompatible artifact schema/tooling combinations.

### 5.5 CLI and UX requirements

1. CLI MUST support both single-file and project workflows.
2. CLI SHOULD support directory-default project behavior (`.` as project root).
3. CLI MUST provide stable machine-readable outputs for automation (`--format json` on key commands).
4. CLI MUST clearly separate build, run, pack, and host-inspection operations.
5. CLI help text MUST include host policy/profile flags and security implications.

### 5.6 Build integration requirements

1. Project configuration MUST support external native dependency declarations (DLL/COM/typelib inputs).
2. Build pipeline SHOULD support prebuild hooks and generated artifacts import with deterministic hashing.
3. Typelib/importlib reference metadata MUST be representable without `.vbp`-specific coupling.
4. Cross-toolchain integration MUST avoid hidden mutable global state.

### 5.7 Language service requirements

1. OxVBA MUST define a host-facing language services contract:
   - parse/bind diagnostics,
   - symbol index,
   - completion/signature/reference capabilities,
   - project graph aware resolution.
2. Service results SHOULD include source-map and clause IDs where available.
3. IDE integrations MUST be able to run against host-managed project stores, not only filesystem paths.

### 5.8 WASM and security requirements

1. WASM runtime classes MUST remain capability-explicit and policy-driven.
2. Unsupported host-sensitive features MUST fail deterministically.
3. Browser sandbox mode MUST avoid implicit access to process/env/filesystem/COM surfaces.
4. Host bridge APIs in WASM MUST be explicit, versioned, and permission-scoped.

## 6. Proposed Project and Artifact Model

### 6.1 Canonical project file: `oxvba.toml` (draft)

```toml
[project]
name = "FinanceTools"
kind = "addin"                # application | library | document | addin
scope = "process"             # process | document
entry = "MainModule.Main"     # optional for library/addin
language_version = "vba7.1"

[layout]
auto_discover = true
include = ["src/**/*.bas", "src/**/*.cls"]
exclude = ["**/*.generated.bas"]

[host]
default_root_object = "Application"
runtime_profile = "windows-headless"
policy_preset = "deterministic-runtime"

[references]
project = [{ name = "CoreLib", path = "../CoreLib/oxvba.toml" }]
typelib = [
  { importlib = "Scripting", libid = "{420B2830-E718-11CF-893D-00A0C9054228}", major = 1, minor = 0, lcid = 0 }
]
native = [
  { kind = "declare-lib", name = "host", path = "build/hostbridge.dll", symbols = "build/hostbridge.symbols.json" }
]

[build]
default_target = "artifact"
flavor = "lite"               # lite | jit
out_dir = "dist"
deterministic = true
```

### 6.2 Artifact file: `*.oxvbapkg` (`A0`)

Required sections:
- `manifest_snapshot`,
- `bytecode_payload`,
- `source_hashes`,
- `toolchain_fingerprint`,
- `policy_fingerprint`,
- optional wrapper metadata projection block.

### 6.3 Adapter model

- `.vbp` import path:
  - `oxvba import-vbp legacy.vbp --out oxvba.toml`
- OVBA/document import path (future):
  - `oxvba import-ovba workbook.xlsx --project VBAProject --out oxvba.toml`

## 7. Proposed CLI Surface and Help Text

### 7.1 Command map

- `oxvba run <file.bas>`
- `oxvba run-project [path-or-dir]`
- `oxvba build [path-or-dir]`
- `oxvba pack [path-or-dir] --out <artifact>`
- `oxvba run-artifact <artifact>`
- `oxvba build-wrapper-exe [path-or-dir] --out <exe>`
- `oxvba build-wrapper-dll [path-or-dir] --out <dll> --com-sta`
- `oxvba import-vbp <file.vbp> --out <oxvba.toml>`
- `oxvba ls-exports [path-or-dir]`
- `oxvba ls-diagnostics [path-or-dir] --format json`
- `oxvba host-check [path-or-dir]` (reports required capabilities and policy gates)

### 7.2 Help text sketch

```text
oxvba run-project [PATH]
Run an OxVBA project from oxvba.toml (or discover in PATH).

Usage:
  oxvba run-project [PATH] [options]

Options:
  --entry <Module.Proc>      Override configured entry point
  --profile <id>             Runtime profile (windows-headless, linux-stdio, ...)
  --policy <preset>          Host policy preset (strict-ci, deterministic-runtime, ...)
  --jit                      Enable JIT for this run
  --no-jit                   Force VM-only execution
  --dump-bootstrap           Emit resolved runtime/policy fingerprint
  --format <text|json>       Output format
```

```text
oxvba build [PATH]
Compile project and emit configured build output.

Outputs:
  artifact (default), wrapper-exe, wrapper-dll

Options:
  --target <artifact|exe|dll>
  --flavor <lite|jit>
  --out <path>
  --deterministic
  --format <text|json>
```

### 7.3 Example flows

```powershell
# 1) quick script lane
oxvba run .\scratch\hello.bas --profile windows-headless --policy deterministic-runtime

# 2) directory-first project run
oxvba run-project . --jit

# 3) build canonical artifact
oxvba pack . --out .\dist\finance.oxvbapkg --flavor lite

# 4) run built artifact
oxvba run-artifact .\dist\finance.oxvbapkg --profile windows-headless

# 5) build process-global add-in wrapper
oxvba build-wrapper-dll . --out .\dist\FinanceAddin.dll --com-sta --flavor lite
```

## 8. Embedded Host Contract v1 (Proposed)

### 8.1 Host responsibilities

Host provides:
- project store (source/artifact retrieval),
- object model bridge (root objects + object identity),
- event pump integration,
- policy selection and enforcement context,
- diagnostics sink and telemetry integration.

### 8.2 OxVBA responsibilities

OxVBA provides:
- deterministic compilation/execution pipeline,
- policy-aware host capability gating,
- stable diagnostics and error codes,
- project graph and reference resolution,
- export inventory for host registration workflows.

### 8.3 Minimal API shape sketch (Rust)

```rust
pub trait OxvbaHostBridge {
    fn load_project(&self, id: &str) -> Result<ProjectManifest, HostError>;
    fn load_artifact(&self, id: &str) -> Result<Vec<u8>, HostError>;
    fn resolve_root_object(&self, name: &str) -> Result<HostObjectToken, HostError>;
    fn subscribe_event(&self, object: HostObjectToken, event: &str) -> Result<SubscriptionId, HostError>;
    fn unsubscribe_event(&self, subscription: SubscriptionId) -> Result<(), HostError>;
    fn emit_diagnostic(&self, diagnostic: EngineDiagnostic);
}
```

### 8.4 "Minimal OxVBA Embedded Host" pathfinder

Proposed project:
- `DNA CellCalc` (pathfinder host application).

Goals:
- exercise complete embedder contract,
- demonstrate source/artifact load paths,
- demonstrate root object hookup and event propagation,
- demonstrate COM and non-COM host capability boundaries,
- provide repeatable harness for interaction debugging.

## 9. Add-in Runtime Model (Document vs Process Scope)

### 9.1 Scope contract

- `scope=document`:
  - exports resolved in document/application instance context,
  - no process-global registration side effects.
- `scope=process`:
  - exports published globally in host process registry.

### 9.2 Editing policy

Add-in runtime may require read-only policy:
- `edit_mode=blocked` for packaged/deployed add-ins,
- optional `edit_mode=host-managed` for dev lanes.

### 9.3 Function registration behavior

For process-global add-ins:
- host export inventory drives registration.
- registration metadata may include category, volatility, argument docs.
- collisions require deterministic conflict policy (`fail`, `shadow`, `namespace-prefix`).

## 10. Excel XLL Prototype Plan

### 10.1 Purpose

Use Excel as a high-value interoperability lab:
- validate function registration semantics,
- validate host object bridge patterns,
- generate differential observations.

### 10.2 Prototype tracks

Track X1: per-project XLL wrapper
- easiest packaging and debug story,
- each add-in ships runtime payload.

Track X2: shared language-host XLL
- one infrastructure add-in loads many OxVBA projects,
- operationally efficient at scale.

### 10.3 Recommended sequence

1. X1 baseline first.
2. Collect performance/operability data.
3. Implement X2 if data supports consolidation.

### 10.4 Explicit caveat

XLL UDF invocation path is not identical to native VBA runtime invocation.  
This lane is for compatibility and ecosystem integration signal, not claim-equivalent execution semantics.

## 11. WithEvents, Implements, RaiseEvent Closure Plan

### 11.1 Direct answer to current question

Current status:
- Host root object hookup substrate exists.
- Full `WithEvents`/`Implements`/`RaiseEvent` class-event semantics are not complete.
- Current behavior is deterministic diagnostics (intentional gates), not parity behavior.

### 11.2 Required closure steps

1. Class graph metadata completion in PMR/compiler.
2. Event declaration and handler binding model.
3. Runtime subscription/dispatch semantics and ordering.
4. Conformance and oracle foldback for event ordering and interface coverage.
5. Remove temporary PMR diagnostic gates for completed semantics.

## 12. WASM Strategy and Security Posture

### 12.1 Runtime classes

Keep explicit classes:
- `wasi`,
- `browser-sandbox`.

### 12.2 Loader and host ownership

Decision proposal:
- OxVBA WASM module is loaded by a host-provided runtime container.
- Host container owns capabilities and bridge injection.
- OxVBA remains capability-consumer under HAL policy.

### 12.3 Security model

- Deny by default for filesystem/process/COM/dynamic-link.
- Explicit allowlist for approved host bridges.
- Structured diagnostic and telemetry output for denied operations.
- No implicit privilege escalation through convenience APIs.

### 12.4 Comparison with modern language runtimes

Convergent patterns to follow:
- profile-based capability declaration,
- explicit host bridge contracts,
- deterministic failure for unavailable host features,
- clear separation between compile/run package and host execution sandbox.

## 13. Integration with External Build Systems

### 13.1 Native library and typelib pipeline

Required scenario:
- project builds native `.dll`/COM server + typelib externally,
- OxVBA project references resulting artifacts.

Proposed model:
- `native` and `typelib` reference blocks in `oxvba.toml`,
- deterministic build hooks:
  - `prebuild` command,
  - artifact hash capture,
  - reference resolution to generated outputs.

### 13.2 Example

```toml
[build.hooks]
prebuild = ["cmake --build build --config Release"]

[[references.typelib]]
importlib = "MyNativeLib"
tlb_path = "build/MyNativeLib.tlb"

[[references.native]]
kind = "declare-lib"
name = "mynativelib"
path = "build/MyNativeLib.dll"
```

## 14. Proposed Phases (Next Program Ladder)

### Phase P1: Unified product contract and decision lock

Deliverables:
- lock v1 decisions from this doc,
- publish clause catalog for hosting/project/tooling contract,
- derive executable acceptance tests.

Gate:
- approved design-lock doc + clause table + initial acceptance suite scaffold.

### Phase P2: Canonical project format and directory workflows

Deliverables:
- `oxvba.toml` parser/validator,
- project discovery (`run-project .`),
- include/exclude and entrypoint policy.

Gate:
- deterministic parse/validation + sample project corpus pass.

### Phase P3: VBP-S0 adapter

Deliverables:
- `.vbp -> ProjectManifest` adapter,
- import command and diagnostics.

Gate:
- VBP fixture matrix pass, stable unsupported diagnostics.

### Phase P4: Artifact A0 and run parity

Deliverables:
- `pack` and `run-artifact`,
- schema versioning and compatibility checks.

Gate:
- parity across loose project run and artifact run on fixture suite.

### Phase P5: Embedded host contract and DNA CellCalc pathfinder

Deliverables:
- host bridge API v1,
- minimal embedded host app with instrumentation.

Gate:
- end-to-end scenario pass:
  - load project from host store,
  - inject root object,
  - execute entry and host callbacks.

### Phase P6: Class/event model closure

Deliverables:
- `WithEvents`/`Implements`/`RaiseEvent` execution semantics.

Gate:
- close divergence tickets (`DIV-0003`, `DIV-0004`) or explicitly downgrade parity claim scope if unresolved.

### Phase P7: Wrapper outputs and add-in semantics

Deliverables:
- wrapper EXE and DLL build flows,
- scope-aware export registration semantics.

Gate:
- deterministic registration behavior for document and process scope.

### Phase P8: Excel XLL prototype

Deliverables:
- X1 prototype, optional X2 follow-up.

Gate:
- reproducible interop demo suite and documented caveat matrix.

### Phase P9: WASM host lane hardening

Deliverables:
- WASM bridge contract and conformance suite expansion.

Gate:
- sandbox security checks + deterministic capability-denial behavior.

## 15. Immediate Next Worksets (Recommended)

1. `HOST-CONTRACT-V1`: embedder contract, object/event bridge API, diagnostics sink protocol.
2. `PROJECT-FORMAT-V1`: `oxvba.toml` schema, directory discovery, entrypoint policy.
3. `VBP-S0-EXEC`: begin `WORKSET_2026-03-05_VBP_SUBSET_AND_ARTIFACT_PLAN.md` execution with P2/P3 alignment.
4. `EVENT-MODEL-CLOSURE-TRACK`: dedicated closure ladder for `WithEvents`/`Implements`/`RaiseEvent`.
5. `DNA-CELLCALC-PATHFINDER`: minimal embedded host app and scenario tests.

## 16. Open Decision Register

1. Top-level code extension: file marker vs project-level mode vs explicit command flag.
2. Default command for `oxvba run .`: run project if `oxvba.toml` exists, else script lane?
3. Artifact portability policy: strict profile-locked by default vs profile-portable by default.
4. Process-global registration collision policy default.
5. Wrapper DLL activation model details: registry-free first vs dual lane from day one.
6. XLL architecture: per-project runtime bundle vs shared language-host default.
7. Language service transport: direct Rust API first vs LSP-first external boundary.

## 17. Summary of Recommended Plan

Recommended trajectory:
- keep app-embedded hosting as the primary product shape,
- stabilize project/tooling contracts around a modern canonical format,
- treat `.vbp` as a compatibility adapter,
- close class/event semantics before claiming full Excel-like host parity for evented object models,
- use Excel XLL interop as a high-value test/prototyping lane,
- retain strict policy-driven HAL behavior across native and WASM host environments.

This path maximizes near-term delivery value for DNA Calc while keeping long-term portability and ecosystem integration clean.
