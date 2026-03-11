# OxVBA Hosting, Project, Packaging, and Tooling Proposal v2

Status: `design-draft`
Date: 2026-03-07
Scope owner: OxVBA runtime/host/tooling
Canonical path: `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`
Supersedes: `docs/spec/archive/HOSTING_PROJECT_TOOLING_PROPOSAL_V1.md`

Related docs:
- `docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md`
- `docs/worksets/WORKSET_2026-03-05_VBP_SUBSET_AND_ARTIFACT_PLAN.md`
- `docs/worksets/WORKSET_2026-03-07_EVENTS_STORY_COMPLETION.md`
- `docs/worksets/WORKSET_2026-03-08_EVENTS_RUNTIME_HOST_PROJECT_HAL_SPLIT.md`
- `docs/worksets/WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`
- `docs/spec/HAL_SPEC_WORKING_DRAFT.md`
- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`
- `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
- `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current Implementation Baseline](#2-current-implementation-baseline)
3. [Product Use Cases in Depth](#3-product-use-cases-in-depth)
   - 3.1 [UC-A: App-Embedded Hosting (PRIMARY)](#31-uc-a-app-embedded-hosting-primary)
   - 3.1.7 [Normative Integration Split: Host Project vs HAL vs COM](#317-normative-integration-split-host-project-vs-hal-vs-com)
   - 3.2 [UC-B: Add-in Authoring Outside Documents](#32-uc-b-add-in-authoring-outside-documents)
   - 3.3 [UC-C: General Runtime/Framework Tooling](#33-uc-c-general-runtimeframework-tooling)
   - 3.4 [UC-D: Top-Level Code Extension](#34-uc-d-top-level-code-extension)
   - 3.5 [UC-E: WebAssembly Hosting](#35-uc-e-webassembly-hosting)
4. [Cross-Cutting Design](#4-cross-cutting-design)
   - 4.1 [Project File Format: `oxvba.toml`](#41-project-file-format-oxvbatoml)
   - 4.2 [Directory-as-Project Convention](#42-directory-as-project-convention)
   - 4.3 [`.vbp` Adapter](#43-vbp-adapter)
   - 4.4 [Artifact Format: `*.oxvbapkg` (A0)](#44-artifact-format-oxvbapkg-a0)
   - 4.5 [Build Targets: EXE and DLL](#45-build-targets-exe-and-dll)
   - 4.6 [Build Integration with External Systems](#46-build-integration-with-external-systems)
   - 4.7 [Event Model Closure](#47-event-model-closure)
   - 4.8 [Language Services](#48-language-services)
5. [Design Decision Register](#5-design-decision-register)
6. [Phased Execution Plan](#6-phased-execution-plan)
7. [Immediate Next Worksets](#7-immediate-next-worksets)

---

## 1. Executive Summary

OxVBA has reached a significant substrate maturity point. The COM early-binding/type-library ladder (`v466`) is closing. The core compiler, VM, JIT, and HAL policy infrastructure are stable. The project model (`ProjectManifest`/`ProjectGraph`) supports multi-module, multi-reference project execution. Host policy presets, runtime profile bootstrap, and platform abstraction across Windows/Linux/macOS/WASM are implemented and evidence-backed.

This document defines the next productization stage: turning that substrate into a coherent multi-surface product. It covers five product surfaces:

1. **App-embedded hosting** (primary) — OxVBA as a library in another process, like VBA in Excel.
2. **Add-in authoring** — VBA add-ins shipped outside documents, with XLL integration for Excel.
3. **General runtime/framework tooling** — modern CLI and project workflows for broad ecosystem adoption.
4. **Top-level code extension** — script-like VBA execution without explicit `Sub Main`.
5. **WebAssembly hosting** — controlled WASM execution with sandbox-first security.

This document is both a discussion paper and requirements source material. It proposes concrete defaults, enumerates decision points, defines phased execution, and includes example CLI help text and code snippets for the tools we will provide.

---

## 2. Current Implementation Baseline

This section grounds the reader in what exists today in the codebase, not what is planned.

### 2.1 Core execution and embedding surface

The `Engine` struct in `crates/oxvba-host/src/engine.rs` is the primary embedding entry point:

```rust
pub struct Engine {
    config: HostConfig,
    jit: JitEngine,
    root_objects: HashMap<String, String>,
    runtime_profile: RuntimeProfileId,
    host_services: Arc<dyn HostServices>,
}
```

Public API surface:

| Method | Purpose |
|--------|---------|
| `Engine::new(config)` | Create engine with `HostConfig` (JIT toggle, root object name) |
| `set_runtime_profile(profile)` | Set runtime profile (e.g., `WindowsHeadless`, `LinuxStdio`) |
| `set_host_policy(policy)` | Set full host policy |
| `set_host_policy_preset(preset)` | Set policy by preset name |
| `set_unsupported_feature_mode(mode)` | Configure unsupported feature handling |
| `register_root_object(name, type_name)` | Register a host-provided root object by name |
| `has_root_object(name)` | Check if root object is registered |
| `execute_source_with_snapshot_phased(source)` | Compile and run single source, return slot values + phase diagnostics |
| `execute_project_with_snapshot_phased(manifest)` | Compile and run a full project manifest |

The engine already runs as a library in-process with deterministic project-manifest execution. JIT compilation (Cranelift-based) is toggled via `HostConfig.enable_jit`.

### 2.2 Project model and graph

**`ProjectManifest`** (`crates/oxvba-compiler/src/project.rs`):

```rust
pub struct ProjectManifest {
    pub project_name: String,
    pub project_kind: ProjectKind,          // Source | Host | Library
    pub modules: Vec<ModuleUnit>,
    pub references: Vec<ProjectReference>,
    pub reference_projects: Vec<ReferencedProjectManifest>,
    pub conditional_constants: BTreeMap<String, i32>,
}

pub struct ModuleUnit {
    pub module_name: String,
    pub module_kind: ModuleKind,            // Procedural | Class | Document | Form | Extension
    pub attributes: ModuleAttributes,       // VB_Name, VB_Exposed, Option Private, etc.
    pub source: String,
}

pub struct ProjectReference {
    pub referenced_project_name: String,
    pub reference_kind: ReferenceKind,      // Project | TypeLibrary | HostInjected
}
```

**`ProjectGraph`** (`crates/oxvba-host/src/project.rs`) extends this with multi-project graphs, reference binding state (`Unbound`/`Bound`/`Failed`), type-library catalog entries, and public symbol resolution (local-first, then references).

`HostProcedureExport` records the project/module/procedure triples that a project exposes to the host for registration.

### 2.3 HAL profiles and host policy

Five HAL profiles are implemented:

| Profile | Runtime Classes |
|---------|----------------|
| Windows | `HostNative`, `WindowsGui`, `WindowsHeadless` |
| Linux   | `HostNative`, `LinuxStdio`, `LinuxHeadless` |
| macOS   | `HostNative`, `MacOsGui`, `MacOsHeadless` |
| WASM    | `WasmWasiLocal`, `WasmBrowserSandbox` |
| Null    | `NullFloor` (testing) |

The bootstrap resolver (`resolve_runner_bootstrap`) implements a priority chain: CLI flags > environment variables > config file > defaults. Four policy presets govern capability gating:

- `strict-ci` — all capabilities blocked, deterministic mode on, fail on unsupported.
- `deterministic-runtime` — selective capability allowance, runtime unsupported handling.
- `deterministic-compile-time` — compile-time rejection of unsupported features.
- `interactive-dev` — most features allowed for development.

Eight capability domains are tracked: `UiInteraction`, `EventPump`, `FileSystemIo`, `ProcessEnv`, `ComActivationDispatch`, `TimeLocale`, `DynamicLinking`, `DiagnosticsTelemetry`.

### 2.4 COM state

Late-bound COM client lanes and early-binding/type-library support through the terminal gate `v466` are complete and evidence-backed. Type-library reference binding records exist in the PMR/host layers. Non-Windows COM behavior remains deterministic-unsupported by policy and profile contract.

### 2.5 Event model status

**Compiler/binder (EVT1/EVT2 — completed 2026-03-07):**

- Removed deterministic gate diagnostics for `WithEvents`, `Implements`, and `RaiseEvent` from single-module resolve/compile paths.
- Added project-aware event diagnostics:
  - canonical source: `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`
  - generated list: `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`
- `WithEvents` module-kind legality, `Implements` interface existence + member coverage, `RaiseEvent` class-only + declared-event enforcement are all validated at compile time.

**Runtime (EVT3+ — baseline started 2026-03-08):**

- `compile_project(...)` now lowers `RaiseEvent` into deterministic handler call paths for known `WithEvents` bindings in the executable subset.
- Compiled projects now emit deterministic event dispatch bindings (`source project/module/event -> lowered handler symbol`) for host/runtime hydration.
- Host runtime owns a deterministic non-COM event dispatcher map with subscribe/unsubscribe/dispatch lookup API.
- Remaining runtime parity work: full sink-instance subscription graph parity, full callback argument-shape parity, and COM event adapter completion.
- `DIV-0003` baseline mismatch is closed; `DIV-0004` remains open for full sink-instance graph/subscription parity.

### 2.6 CLI posture

The current CLI supports a single command:

```
oxvba run <file.bas> [options]
```

Options: `--dump-slots`, `--dump-values`, `--dump-bootstrap`, `--jit`, `--config <path>`, `--profile <id>`, `--policy <preset>`, `--runtime-class <class>`, `--allow-interaction`, `--allow-process-spawn`, `--allow-filesystem-mutation`, `--allow-dynamic-link`, `--allow-com-activation`, `--deterministic-mode`, `--ui-virtualization`, `--unsupported-mode`, `--wasm-runtime-class`.

No project-level commands exist.

### 2.7 What does NOT exist yet

- `.vbp` parser or adapter
- `oxvba.toml` project file format
- Compiled artifact format or packaging
- Wrapper EXE/DLL build commands
- Project-level CLI commands (`run-project`, `build`, `pack`, etc.)
- Full runtime event semantics (subscription graph, dispatch, host bridge)
- Language services (diagnostics API, symbol index, completion)
- XLL integration or Excel shim
- Top-level code support
- Directory-as-project discovery

---

## 3. Product Use Cases in Depth

### 3.1 UC-A: App-Embedded Hosting (PRIMARY)

#### 3.1.1 Motivation and scope

The primary use case for OxVBA is the app-embedded role: OxVBA runs as a library inside another process, exactly as VBA runs inside Excel, Access, or Word. The host application:

- manages the VBA project store (either through a host-managed IDE/editor or by embedding projects inside an application file format),
- controls the runtime policy, security boundaries, and capability grants,
- injects root objects (like `Application`) that VBA code navigates to interact with the host,
- routes events between host objects and VBA event handlers,
- consumes diagnostics and telemetry from the engine.

This is the model that the DNA Calc ecosystem targets. OxVBA must provide a stable, comprehensive host contract that embedded hosts can rely on without being coupled to OxVBA internals.

#### 3.1.2 Embedded host contract v1

**Host responsibilities:**

1. **Project store** — provide project source or compiled artifacts from host-controlled storage.
2. **Object model bridge** — provide root objects with stable identity, expose properties/methods/events.
3. **Event pump integration** — pump messages/events when VBA calls `DoEvents` or when the host raises events.
4. **Policy selection** — choose runtime profile, policy preset, and capability grants.
5. **Diagnostics sink** — consume compile-time and runtime diagnostics from the engine.

**OxVBA responsibilities:**

1. **Deterministic compilation/execution** — identical inputs produce identical outputs.
2. **Policy-aware capability gating** — refuse operations the host has not granted.
3. **Stable diagnostics** — error codes and messages with phase classification.
4. **Project graph and reference resolution** — multi-module, multi-reference, multi-project.
5. **Export inventory** — enumerate public procedures for host registration workflows.

**Proposed host bridge trait:**

```rust
/// Host-facing bridge contract for embedded OxVBA hosting.
pub trait OxvbaHostBridge {
    /// Load a project manifest from host-controlled storage.
    fn load_project(&self, id: &str) -> Result<ProjectManifest, HostError>;

    /// Load a compiled artifact from host-controlled storage.
    fn load_artifact(&self, id: &str) -> Result<Vec<u8>, HostError>;

    /// Resolve a root object by well-known name (e.g., "Application").
    /// Returns a token the engine uses for subsequent object operations.
    fn resolve_root_object(&self, name: &str) -> Result<HostObjectToken, HostError>;

    /// Subscribe to an event on a host-provided object.
    fn subscribe_event(
        &self,
        object: HostObjectToken,
        event_name: &str,
        handler: EventHandlerBinding,
    ) -> Result<SubscriptionId, HostError>;

    /// Unsubscribe from a previously subscribed event.
    fn unsubscribe_event(&self, subscription: SubscriptionId) -> Result<(), HostError>;

    /// Release a previously resolved host object token.
    fn release_object(&self, object: HostObjectToken) -> Result<(), HostError>;

    /// Invoke a method on a host-provided object.
    fn invoke_method(
        &self,
        object: HostObjectToken,
        method: &str,
        args: &[Variant],
    ) -> Result<Variant, HostError>;

    /// Get a property value from a host-provided object.
    fn get_property(
        &self,
        object: HostObjectToken,
        property: &str,
    ) -> Result<Variant, HostError>;

    /// Set a property value on a host-provided object.
    fn set_property(
        &self,
        object: HostObjectToken,
        property: &str,
        value: Variant,
    ) -> Result<(), HostError>;

    /// Receive a diagnostic from the engine.
    fn emit_diagnostic(&self, diagnostic: EngineDiagnostic);
}
```

**Contract lock (2026-03-09):**

1. The host bridge keeps a single `Variant` value boundary.
2. Object-valued property/method results cross the boundary as object-capable `Variant` values that carry host object identity.
3. The bridge does not add special-case APIs for collection/default-member behavior.
   - Host Project + runtime semantics remain the authority for deciding when VBA syntax implies default-member access.
   - The bridge exposes ordinary property/method operations only.
4. Host-to-engine event ingress is explicit and engine-owned:

```rust
impl Engine {
    pub fn dispatch_host_event(
        &mut self,
        subscription: SubscriptionId,
        args: &[Variant],
    ) -> Result<(), HostError>;
}
```

5. The bridge owns host object resolution/invocation/subscription/release, while the engine owns VBA semantic dispatch and event lifecycle behavior.

**Lifecycle sequence:**

```
1. Host creates Engine with HostConfig
2. Host configures runtime profile and host policy
3. Host registers root objects (Application, etc.)
4. Host loads project(s) from store -> ProjectManifest
5. Engine compiles project(s) -> CompiledProject
6. Engine validates exports, builds export inventory
7. Host registers exported functions (if applicable)
8. Engine executes entry point (Sub Main or configured entry)
9. During execution:
   - VBA accesses host objects -> bridge method/property calls
   - Host raises events -> engine dispatches to WithEvents handlers
   - VBA raises events -> engine dispatches to subscribers
   - Errors route through deterministic error model
10. Host requests shutdown -> engine runs Class_Terminate, releases objects
```

#### 3.1.3 Host object hookup and event routing

Host object event routing is a two-layer design:

**Layer 1: Host bridge contract (standardize now)**

The host declares which objects support which events. When the engine encounters a `WithEvents` declaration, it calls `subscribe_event` on the bridge. When the host wants to raise an event (e.g., a button was clicked), it invokes the engine's event dispatch entry point with the subscription ID and event arguments.

Example flow — a worksheet-like `Change` event:

```
Host side:                          Engine side:
                                    Dim WithEvents ws As Worksheet
                                    Set ws = Application.ActiveSheet
  <- subscribe_event(ws_token,      ->
     "Change", handler_binding)
  ...
  [user edits cell]
  -> dispatch_event(sub_id,         ->
     "Change", args)                   ws_Change(Target As Range)
                                       ' handler runs
  <- handler returns                <-
```

**Layer 2: VBA semantic layer (EVT3+ phases)**

The runtime subscription graph, handler dispatch ordering, reassignment behavior under `Set ws = Nothing` and `Set ws = other`, and `Class_Terminate` cleanup are VBA-semantic concerns that the engine handles internally. These are being closed through the events workset (WORKSET_2026-03-07), phases EVT3-EVT8.

**VBA code example:**

```vba
' In a class module
Private WithEvents btn As Button

Private Sub Class_Initialize()
    Set btn = Application.WorkPanel.Controls("btnCalculate")
End Sub

Private Sub btn_Click()
    Dim result As Double
    result = CDbl(Application.WorkPanel.Controls("txtInput").Value)
    Application.WorkPanel.Controls("lblOutput").Caption = "Result: " & CStr(result * 2)
End Sub
```

This code exercises: root object navigation (`Application`), child object access (`WorkPanel.Controls`), `WithEvents` subscription, event handler dispatch, and property get/set on host objects.

#### 3.1.4 Document scope vs process scope

When a workbook contains VBA, its public procedures are scoped to that document's context. When the same workbook is converted to an add-in:

- **Public functions become process-global**: registered for all documents in the host process.
- **Editing is blocked**: the project is read-only at runtime.
- **Function registration metadata** may include category tags, volatility flags, and argument descriptions for host integration (e.g., Excel's Function Wizard).

The scope model is represented as a first-class project attribute:

| Scope | Visibility | Editing | Use case |
|-------|-----------|---------|----------|
| `document` | exports visible only in owning document/project context | allowed | normal workbook VBA |
| `process` | exports registered globally in host process | typically blocked | add-ins |

**Collision policy for process-global registration:**

When multiple add-ins export procedures with the same name, the host must apply a deterministic collision policy. Recommended default: `fail` (reject the conflicting registration with a diagnostic). Alternatives: `shadow` (last-registered wins), `namespace-prefix` (prefix with project name).

#### 3.1.5 DNA VbCalc: full-exercise hosting pathfinder

> **Note on presentation format:** This section uses two tiers to distinguish what is normative for the OxVBA engine from what is application-level design:
> - **`[HOST-REQ]`** — what we require from the OxVBA hosting and interface contract. These are engine requirements.
> - **`[APP-IDEA]`** — initial ideas for making DNA VbCalc an interactive and useful environment. These are application-level choices, not engine requirements, and will be refined further.

**Purpose and philosophy**

DNA VbCalc is a purpose-built pathfinder host application designed to put us "in harm's way" for every aspect of the kind of hosting that Excel does to the VBA runtime. It is not a minimal stub — it is a full-exercise embedded host that validates every interaction surface between a host application and the OxVBA engine. It also serves as a useful interactive runner for trying out VBA code.

**Repository boundary note (2026-03-09):**

DNA VbCalc is expected to live in a separate future repository.

This OxVba repo carries:
1. the host/tooling contract,
2. the bridge semantics,
3. the preparatory baseline note:
   - `docs/DNAVBCALC_HOST_SHELL_BASELINE_PREPARATION_2026-03-09.md`

The actual DNA VbCalc implementation plan should be created in that future repository, not added to OxVba workset execution as if it were an in-repo implementation track.

The richer DNA VbCalc application ideas are intentionally moved into a separate preparation doc set so they do not interfere with OxVba workset planning:
1. `docs/DNAVBCALC_PREPARATION_INDEX_2026-03-09.md`
2. `docs/DNAVBCALC_HOST_SHELL_BASELINE_PREPARATION_2026-03-09.md`
3. `docs/DNAVBCALC_APPLICATION_IDEAS_PREPARATION_2026-03-09.md`

**Baseline lock (2026-03-09):**

For the future separate DNA VbCalc repository, the first baseline host shell is:
1. Tauri desktop shell,
2. Rust backend,
3. web UI frontend,
4. `oxvba.toml` project open path at startup and via UI,
5. debug/immediate-style shell as the first user-facing interaction surface,
6. full reset + recompile on reload,
7. non-COM host-bridge path first.

This baseline is intentionally debug-centric and does not require a first-pass visual designer or rich control hierarchy.

**Normative host-contract implications** `[HOST-REQ]`

The DNA VbCalc pathfinder remains valuable here only insofar as it validates the host/tooling contract. The important in-repo requirements are:
1. host-managed project load from non-filesystem-controlled storage paths when needed,
2. root object injection and object model navigation through the hosting bridge,
3. explicit event subscription and host-to-engine event ingress,
4. diagnostics/error routing through the host contract,
5. deterministic reset/reload behavior for v1,
6. language services against host-managed source stores.

#### 3.1.6 Language services contract

Embedded hosts that provide a VBA IDE or editor need language services from the engine:

**Required capabilities:**

| Service | Description |
|---------|-------------|
| Parse diagnostics | Syntax errors with source locations |
| Bind diagnostics | Name resolution failures, type mismatches |
| Symbol index | All symbols in project with kind, type, scope, location |
| Completion | Context-aware completion lists at cursor position |
| Signature help | Parameter info for procedure calls |
| Go-to-definition | Navigate to symbol declaration |
| Find references | All usage sites for a symbol |
| Hover info | Type and documentation for symbol at position |

**Key constraint:** Language services MUST work against host-managed project stores, not only filesystem paths. The host provides source text to the engine; the engine returns service results with source-map positions. This is essential for hosts where VBA source lives inside a container format (like `.vbcalc` or an Office document).

**Transport decision:**

- **Option A (recommended first):** Direct Rust API — the engine exposes service methods that the host calls in-process. Lowest latency, simplest integration for Rust-based hosts.
- **Option B (follow-up):** LSP wrapper — an LSP server wrapping the Rust API for editor integration (VS Code, etc.). Higher compatibility with external editors, but adds IPC overhead.

Recommendation: implement direct Rust API first (for DNA VbCalc and other in-process hosts), then add LSP wrapper for broader editor ecosystem.

#### 3.1.7 Normative integration split: Host Project vs HAL vs COM

To avoid over-coupling the language model to COM, OxVBA adopts a three-plane contract:

1. **Host Project semantic plane (language-level, cross-platform)**
   - Defines host-visible symbols/types/events available to user projects.
   - Defines compile-time shape and name binding for host entities (including event signature/prefix rules).
   - Is the canonical semantic contract for both COM and non-COM hosts.

2. **HAL service plane (runtime capabilities, cross-platform)**
   - Hosts MUST provide the full HAL service suite contract (subject to selected runtime profile/policy):
     `FileSystemIo`, `TimeLocale`, `ProcessEnv`, `UiInteraction`, `EventPump`, `DiagnosticsTelemetry`, and related capability gates.
   - Host Project semantics do not replace HAL provisioning; they complement it.
   - Policy presets and capability denials remain enforced through HAL regardless of host object model style.

3. **Transport adapter plane (platform-specific)**
   - COM is a Windows transport adapter lane for object/event delivery (`IDispatch`, connection points, typelib binding).
   - Non-COM hosts use equivalent bridge transports while preserving the same Host Project semantic contract.
   - DNA VbCalc pathfinder is explicitly required to validate this contract cross-platform without COM dependency.
   - Object-valued returns and event ingress MUST still respect the host-bridge contract above (`Variant` value boundary + explicit `dispatch_host_event(...)`), even when COM is the underlying transport.

**Normative consequence:**
- Semantic compatibility claims for host-object/event behavior are anchored to the Host Project + runtime event engine.
- COM parity claims are scoped to adapter parity, not semantic ownership of the event model.
- Runtime event execution parity (`WithEvents` reassignment ordering, `RaiseEvent` dispatch lifecycle) remains tracked in EVT3-EVT8 and `DIV-0004`.

---

### 3.2 UC-B: Add-in Authoring Outside Documents

#### 3.2.1 Motivation

Beyond document-embedded VBA, a key scenario is authoring VBA add-ins that ship independently — not embedded in a workbook or document. These add-ins extend the host application with new functions, macros, and tools.

Two distribution models exist in the ecosystem:

- **Per-add-in runtime**: each add-in ships with its own OxVBA runtime payload. Like Excel-DNA for .NET add-ins.
- **Shared language host**: one OxVBA host process/add-in loads and manages many VBA projects. Like PyXLL or xlOil for Python in Excel.

#### 3.2.2 Model B1: per-project self-contained wrapper

Each VBA add-in project compiles into a self-contained XLL (for Excel) or DLL that embeds:

- the OxVBA runtime (lite or JIT flavor),
- the compiled project artifact,
- bootstrap and policy configuration.

The wrapper handles function registration, Application object bridging, and lifecycle management. The host application loads the XLL/DLL through its standard add-in mechanism.

**Advantages:** simple packaging, independent versioning, isolated failures.
**Disadvantages:** larger total footprint when many add-ins are loaded, duplicated runtime instances.

**Comparison with Excel-DNA:** Excel-DNA compiles .NET code into self-contained XLLs with an embedded .NET runtime. The OxVBA model is architecturally similar — an OxVBA runtime core embedded in each XLL.

#### 3.2.3 Model B2: shared language-host add-in

A single "OxVBA Language Host" add-in loads into the host process and manages multiple VBA projects:

- One runtime instance serves all loaded VBA projects.
- Projects are loaded/unloaded dynamically.
- Function registration is centralized through the language host.

**Advantages:** shared runtime footprint, centralized management, easier updates.
**Disadvantages:** shared failure domain, version compatibility across projects.

**Comparison with PyXLL / xlOil:** Both load a single Python runtime into Excel and host multiple Python-based add-in projects through a configuration file. The OxVBA B2 model follows this pattern.

#### 3.2.4 XLL-to-VBA shim mechanics

For Excel integration, the XLL shim performs these steps:

1. **`xlAutoOpen`**: called by Excel when the XLL loads.
   - Initialize OxVBA engine with host policy.
   - Load compiled project artifact.
   - Scan export inventory for public `Function` and `Sub` procedures.
   - Register each exported function with Excel via `xlfRegister`:
     - function name, argument types, category, help text.
   - Bridge the `Application` object from Excel to the VBA runtime.

2. **UDF invocation**: when Excel calls a registered function:
   - Excel passes arguments through the XLL C API.
   - The shim marshals arguments into VBA `Variant` values.
   - The engine executes the target function.
   - The shim marshals the return value back to Excel.

3. **`xlAutoClose`**: called when the XLL unloads.
   - Engine shutdown, object cleanup, unregistration.

**Explicit caveat:** The XLL UDF call path is structurally different from how native VBA UDFs are invoked by Excel. Native VBA functions are called through the VBA runtime's internal dispatch; XLL functions go through the C API shim. This means:

- Calling conventions differ (XLL uses `XLOPER`/`XLOPER12`; VBA uses `Variant`/`SAFEARRAY`).
- Error handling paths differ.
- Reentrancy rules differ.

This lane is for compatibility and ecosystem integration signal, not claim-equivalent execution semantics with native VBA.

#### 3.2.5 Recommended prototype sequence

1. **B1 first** — implement per-project self-contained XLL wrapper for simplest packaging and debugging.
2. **Collect data** — measure runtime footprint, function call overhead, and operational complexity.
3. **B2 follow-up** — implement shared language-host XLL if operational gains justify the complexity.

#### 3.2.6 Example commands and help text

```
oxvba build-wrapper-dll [PATH] --out <dll> [options]

Build an in-process COM DLL or XLL wrapper for a VBA project.

Options:
  --out <path>              Output DLL path (required)
  --com-sta                 Build as COM STA in-process server
  --xll                     Build as Excel XLL add-in
  --flavor <lite|jit>       Runtime flavor (default: lite)
  --scope <document|process> Export scope (default: document)
  --format <text|json>      Output format
```

---

### 3.3 UC-C: General Runtime/Framework Tooling

#### 3.3.1 Motivation

OxVBA should provide modern developer-friendly CLI tools for compiling and running VBA code — usable from any development environment (Rust, .NET, Python, Go, etc.). The tools should support both quick script-like execution and project-grade build/run workflows.

#### 3.3.2 CLI comparison with other runtimes

| Operation | `oxvba` | `dotnet` | `cargo` | `go` | `deno` | `python` |
|-----------|---------|----------|---------|------|--------|----------|
| Run file | `run <file>` | `dotnet script <f>` | — | `go run <f>` | `deno run <f>` | `python <f>` |
| Run project | `run-project [dir]` | `dotnet run` | `cargo run` | `go run .` | `deno task run` | — |
| Init project | `init [dir]` | `dotnet new` | `cargo init` | `go mod init` | `deno init` | — |
| Build | `build [dir]` | `dotnet build` | `cargo build` | `go build` | — | — |
| Pack artifact | `pack [dir]` | `dotnet pack` | `cargo package` | — | — | — |
| Run artifact | `run-artifact <pkg>` | `dotnet <dll>` | — | `./<binary>` | — | — |
| List exports | `ls-exports [dir]` | — | — | — | — | — |
| Import legacy | `import-vbp <vbp>` | — | — | — | — | — |
| Host check | `host-check [dir]` | — | — | — | — | — |

**Design principles drawn from the comparison:**
- `run` for single files (like `go run`, `deno run`, `python`).
- `run-project` for directory/project execution (like `dotnet run`, `cargo run`).
- `build` for compilation (universal pattern).
- `init` for project scaffolding (like `cargo init`, `dotnet new`).
- Explicit separation between source operations and artifact operations.

#### 3.3.3 Full command map with help text

```
oxvba run <file.bas> [options]

Run a single VBA source file directly.

Usage:
  oxvba run <file.bas> [options]

Options:
  --top-level               Treat file as top-level code (no Sub Main required)
  --profile <id>            Runtime profile (windows-headless, linux-stdio, ...)
  --policy <preset>         Host policy preset (strict-ci, deterministic-runtime, ...)
  --jit                     Enable JIT compilation
  --no-jit                  Force VM-only execution
  --dump-slots              Output execution slot values
  --dump-values             Output semantic runtime values
  --dump-bootstrap          Emit resolved runtime/policy fingerprint
  --format <text|json>      Output format

Examples:
  oxvba run hello.bas
  oxvba run script.bas --top-level --profile windows-headless
  oxvba run benchmark.bas --jit --dump-slots
  oxvba run benchmark.bas --jit --dump-values
```

```
oxvba run-project [PATH] [options]

Run an OxVBA project from oxvba.toml or .vbp file.
If PATH is a directory, looks for oxvba.toml in that directory.
If PATH is omitted, uses the current directory.

Usage:
  oxvba run-project [PATH] [options]

Options:
  --entry <Module.Proc>     Override configured entry point
  --profile <id>            Runtime profile
  --policy <preset>         Host policy preset
  --jit                     Enable JIT for this run
  --no-jit                  Force VM-only execution
  --dump-bootstrap          Emit resolved runtime/policy fingerprint
  --format <text|json>      Output format

Examples:
  oxvba run-project .
  oxvba run-project ./my-project --jit
  oxvba run-project legacy.vbp --entry Module1.Main
```

```
oxvba init [PATH] [options]

Initialize a new OxVBA project with oxvba.toml and directory structure.

Usage:
  oxvba init [PATH] [options]

Options:
  --name <name>             Project name (default: directory name)
  --kind <kind>             Project kind: application, library, addin (default: application)
  --scope <scope>           Export scope: document, process (default: document)

Examples:
  oxvba init .
  oxvba init ./my-addin --kind addin --scope process
```

```
oxvba build [PATH] [options]

Compile a project and emit the configured build output.

Usage:
  oxvba build [PATH] [options]

Options:
  --target <target>         Build target: artifact, exe, dll (default: artifact)
  --flavor <lite|jit>       Runtime flavor for wrapper targets (default: lite)
  --out <path>              Output path
  --deterministic           Enable deterministic build mode
  --format <text|json>      Output format

Examples:
  oxvba build . --target artifact --out dist/myproject.oxvbapkg
  oxvba build . --target exe --flavor lite --out dist/myapp.exe
  oxvba build . --target dll --flavor lite --out dist/mylib.dll --com-sta
```

```
oxvba pack [PATH] --out <artifact> [options]

Compile a project into a versioned artifact package.

Usage:
  oxvba pack [PATH] --out <artifact> [options]

Options:
  --out <path>              Output artifact path (required)
  --flavor <lite|jit>       Compilation flavor (default: lite)
  --deterministic           Enable deterministic serialization
  --format <text|json>      Output format

Examples:
  oxvba pack . --out dist/finance.oxvbapkg
  oxvba pack . --out dist/finance.oxvbapkg --flavor jit --deterministic
```

```
oxvba run-artifact <artifact> [options]

Run a previously compiled OxVBA artifact package.

Usage:
  oxvba run-artifact <artifact> [options]

Options:
  --profile <id>            Runtime profile
  --policy <preset>         Host policy preset
  --jit                     Enable JIT for this run
  --no-jit                  Force VM-only execution
  --format <text|json>      Output format

Examples:
  oxvba run-artifact dist/finance.oxvbapkg --profile windows-headless
```

```
oxvba import-vbp <file.vbp> [options]

Import a VB6 .vbp project file into oxvba.toml format.

Usage:
  oxvba import-vbp <file.vbp> [options]

Options:
  --out <path>              Output oxvba.toml path (default: ./oxvba.toml)
  --strict                  Fail on unknown keys (default)
  --compat                  Warn on unknown keys instead of failing
  --format <text|json>      Output format

Examples:
  oxvba import-vbp legacy/Project1.vbp --out ./oxvba.toml
  oxvba import-vbp legacy/Project1.vbp --compat
```

```
oxvba ls-exports [PATH] [options]

List all public procedures exported by a project.

Usage:
  oxvba ls-exports [PATH] [options]

Options:
  --format <text|json>      Output format (default: text)

Output columns: Module, Procedure, Kind (Sub/Function), Scope (document/process)

Examples:
  oxvba ls-exports .
  oxvba ls-exports . --format json
```

```
oxvba ls-diagnostics [PATH] [options]

Compile a project and list all diagnostics without executing.

Usage:
  oxvba ls-diagnostics [PATH] [options]

Options:
  --phase <compile|all>     Filter by phase (default: all)
  --format <text|json>      Output format (default: text)

Examples:
  oxvba ls-diagnostics . --format json
```

```
oxvba host-check [PATH] [options]

Report the host capabilities and policy gates required by a project.

Usage:
  oxvba host-check [PATH] [options]

Options:
  --profile <id>            Check against specific runtime profile
  --policy <preset>         Check against specific policy preset
  --format <text|json>      Output format

Examples:
  oxvba host-check . --profile windows-headless --policy strict-ci
```

#### 3.3.4 Example workflow sessions

**1. Quick script execution:**
```powershell
$ cat hello.bas
Sub Main()
    Debug.Print "Hello from OxVBA"
End Sub

$ oxvba run hello.bas --profile windows-headless
Hello from OxVBA
```

**2. Top-level script (extension):**
```powershell
$ cat calc.bas
Option Explicit
Dim x As Double
x = 3.14159
Debug.Print "Pi squared = " & CStr(x * x)

$ oxvba run calc.bas --top-level
Pi squared = 9.8696...
```

**3. Directory-first project run:**
```powershell
$ ls my-project/
oxvba.toml  src/Main.bas  src/Utils.bas  src/MathLib.cls

$ oxvba run-project my-project/ --jit
[project output]
```

**4. Artifact build-and-run:**
```powershell
$ oxvba pack . --out dist/finance.oxvbapkg --deterministic
oxvba: packed finance.oxvbapkg (3 modules, schema v1, fingerprint abc123)

$ oxvba run-artifact dist/finance.oxvbapkg --profile windows-headless
[output identical to run-project]
```

**5. Legacy import:**
```powershell
$ oxvba import-vbp legacy/FinanceCalc.vbp --out ./oxvba.toml
oxvba: imported 5 modules, 2 references
oxvba: 1 unsupported key ignored: Form=frmMain; frmMain.frm (VBP-E-UNSUPPORTED-FORM)

$ oxvba run-project .
[project runs with supported subset]
```

**6. Host capability check:**
```powershell
$ oxvba host-check . --profile wasm-browser-sandbox --policy strict-ci
Required capabilities:
  - ComActivationDispatch: DENIED (policy: strict-ci)
  - FileSystemIo: DENIED (profile: wasm-browser-sandbox)
  - DynamicLinking: DENIED (profile: wasm-browser-sandbox)

Result: 3 capability denials. Project will fail at runtime on denied operations.
```

#### 3.3.5 Programmatic embedding sketches

For environments where the CLI is insufficient, OxVBA will expose a C-compatible API (`liboxvba`) that can be consumed by other languages. These sketches are forward-looking and not an immediate deliverable.

**Rust (direct crate dependency):**
```rust
use oxvba_host::{Engine, HostConfig};

let engine = Engine::new(HostConfig::default());
let result = engine.execute_source_with_snapshot("Sub Main()\nEnd Sub");
```

**C API surface (proposed):**
```c
// liboxvba.h
typedef struct OxvbaEngine OxvbaEngine;
OxvbaEngine* oxvba_engine_new(void);
int oxvba_engine_execute_source(OxvbaEngine* engine, const char* source);
void oxvba_engine_free(OxvbaEngine* engine);
```

**Python (ctypes):**
```python
import ctypes
lib = ctypes.CDLL("liboxvba.so")
engine = lib.oxvba_engine_new()
lib.oxvba_engine_execute_source(engine, b"Sub Main()\nEnd Sub")
lib.oxvba_engine_free(engine)
```

**.NET (P/Invoke):**
```csharp
[DllImport("oxvba")]
static extern IntPtr oxvba_engine_new();

[DllImport("oxvba")]
static extern int oxvba_engine_execute_source(IntPtr engine, string source);
```

**Go (cgo):**
```go
// #cgo LDFLAGS: -loxvba
// #include "liboxvba.h"
import "C"

engine := C.oxvba_engine_new()
C.oxvba_engine_execute_source(engine, C.CString("Sub Main()\nEnd Sub"))
C.oxvba_engine_free(engine)
```

These embeddings all consume the same C API. On Windows, COM Automation interop is also a natural integration path — a compiled OxVBA wrapper DLL is directly consumable from any COM-aware language.

---

### 3.4 UC-D: Top-Level Code Extension

#### 3.4.1 VBA spec context

Standard VBA requires all executable code to live inside procedures (`Sub`, `Function`, `Property`). Module-level scope permits only declarations (`Dim`, `Const`, `Type`, `Enum`, `Declare`, `Option` statements).

Top-level code is an OxVBA extension to the VBA 7 spec. It enables script-like execution:

```powershell
$ oxvba run script.bas --top-level
```

This is explicitly not standard VBA behavior. It is an opt-in extension that makes OxVBA useful for quick scripting and one-off execution without requiring boilerplate `Sub Main() / End Sub` wrappers.

#### 3.4.2 Design options

| Approach | Mechanism | Pros | Cons |
|----------|-----------|------|------|
| **A: File marker** | `'!oxvba:top-level` comment at file start | Self-documenting files | Non-standard syntax; grep-unfriendly |
| **B: Command/project mode** | `--top-level` flag or `top_level = true` in `oxvba.toml` | No source modification; clean VBA | Requires flag on every invocation |
| **C: File extension** | `.oxvba` files treated as top-level | Automatic via convention | New extension; breaks existing tooling |

**Recommendation: Option B (command/project mode).**

Rationale:
- Source files remain valid VBA syntax (module-level declarations + executable statements are syntactically parseable).
- No non-standard comments or file extensions needed.
- The mode is explicit — no ambiguity about whether a file is top-level.
- In `oxvba.toml`, configured as `[extensions] top_level = true`.

#### 3.4.3 Semantic rules for top-level code

1. `Option Explicit`, `Option Compare`, `Option Base` MUST precede all executable statements.
2. `Dim`, `Const`, `Type`, `Enum` declarations are valid at module level and MUST precede their first use when `Option Explicit` is active.
3. Executable statements (`Debug.Print`, assignments, control flow, procedure calls) appear after declarations.
4. The compiler implicitly wraps executable statements in an anonymous `Sub Main()` for execution.
5. Module-level scope rules apply: variables declared at module level are accessible throughout.
6. Procedures (`Sub`, `Function`) may be defined in the same file and called from top-level code.

#### 3.4.4 Example

```vba
' file: quickcalc.bas (run with: oxvba run quickcalc.bas --top-level)
Option Explicit

Dim principal As Double
Dim rate As Double
Dim years As Long

principal = 10000
rate = 0.05
years = 10

Dim futureValue As Double
futureValue = CalculateFV(principal, rate, years)

Debug.Print "Future value of " & Format(principal, "$#,##0") & _
            " at " & Format(rate, "0.0%") & _
            " for " & years & " years:"
Debug.Print Format(futureValue, "$#,##0.00")

Function CalculateFV(pv As Double, r As Double, n As Long) As Double
    CalculateFV = pv * (1 + r) ^ n
End Function
```

---

### 3.5 UC-E: WebAssembly Hosting

#### 3.5.1 Comparison with other runtimes

| Aspect | Rust (wasm-bindgen) | Go (TinyGo) | C# (Blazor) | Python (Pyodide) | OxVBA (proposed) |
|--------|-------------------|-------------|-------------|-----------------|------------------|
| **Loader** | wasm-pack + JS glue | TinyGo compiler | .NET WASM runtime | Pyodide bootstrap | Host-provided container |
| **Binary size** | ~100KB-2MB | ~300KB-1MB | ~5-20MB | ~10-20MB | ~0.5MB (lite) |
| **Host bridge** | `wasm-bindgen` auto-gen | Go exports | JS interop | Pyodide API | Explicit bridge trait |
| **Sandbox** | Browser sandbox | Browser sandbox | Browser sandbox | Browser sandbox | Deny-by-default HAL policy |
| **Filesystem** | None / WASI | None / WASI | Virtual FS | Emscripten FS | WASI or denied |
| **COM/native** | N/A | N/A | N/A | N/A | Denied by policy |

#### 3.5.2 OxVBA WASM hosting model

OxVBA compiles to a WASM module that is loaded by a host-provided runtime container. The host container owns capabilities and bridge injection. OxVBA remains a capability-consumer under HAL policy.

Two runtime classes:

- **`wasi`** — WASM + WASI for server-side or local execution. Filesystem, environment, and time are available through WASI.
- **`browser-sandbox`** — WASM in browser. No filesystem, no process, no COM. UI virtualization is required for any interaction.

The OxVBA WASM module exposes:
- `oxvba_init(config)` — initialize engine with serialized configuration.
- `oxvba_execute(source)` — compile and execute VBA source.
- `oxvba_load_project(manifest)` — load a project manifest.
- Host callback imports for bridge methods (property access, method invocation, event dispatch).

#### 3.5.3 Security and sandbox contract

- **Deny by default** for filesystem, process, COM, dynamic linking.
- **Explicit allowlist** for approved host bridges (declared at initialization).
- **Structured diagnostics** for denied operations (deterministic error codes, not silent failures).
- **No implicit privilege escalation** through convenience APIs.
- **Memory isolation** via WASM linear memory — host cannot access engine internals and vice versa without explicit bridge calls.

#### 3.5.4 Host loading example

```javascript
// Browser: loading OxVBA WASM module
const oxvba = await WebAssembly.instantiateStreaming(
    fetch('/oxvba.wasm'),
    {
        env: {
            // Host bridge callbacks
            host_get_property: (objectId, propertyNamePtr) => {
                // ... marshal and return property value
            },
            host_invoke_method: (objectId, methodNamePtr, argsPtr) => {
                // ... marshal and invoke
            },
            host_emit_diagnostic: (level, messagePtr) => {
                console.log(`[OxVBA ${level}] ${readString(messagePtr)}`);
            },
        }
    }
);

// Initialize with browser-sandbox policy
oxvba.exports.oxvba_init(/* config pointer */);

// Execute VBA code
const source = `Sub Main()\n    Debug.Print "Hello from WASM"\nEnd Sub`;
oxvba.exports.oxvba_execute(encodeString(source));
```

---

## 4. Cross-Cutting Design

### 4.1 Project File Format: `oxvba.toml`

#### 4.1.1 Design principles

- **Not VB6 baseline.** The canonical project format is a modern TOML file, not a `.vbp` derivative. `.vbp` support is an adapter/import path.
- **Directory-first compatible.** Works naturally with source files in subdirectories.
- **Covers all use cases.** Application, library, add-in, document projects.
- **Explicit over implicit.** All build, policy, and reference configuration is visible and versionable.

#### 4.1.2 Full annotated schema

```toml
# Required: schema version for forward compatibility
schema_version = 1

[project]
name = "FinanceTools"               # Project name (default: directory name)
kind = "addin"                      # application | library | document | addin
scope = "process"                   # process | document (default: document)
entry = "MainModule.Main"           # Entry point (required for application/addin)
language_version = "vba7.1"         # VBA language version (default: vba7.1)

[layout]
auto_discover = true                # Auto-discover .bas/.cls files in project dir
include = ["src/**/*.bas", "src/**/*.cls"]   # Include globs (when auto_discover=false)
exclude = ["**/*.generated.bas", "**/*.test.bas"]  # Exclude globs

[host]
default_root_object = "Application" # Well-known root object name
runtime_profile = "windows-headless"  # Default runtime profile
policy_preset = "deterministic-runtime"  # Default policy preset

# Project references
[[references.project]]
name = "CoreLib"
path = "../CoreLib/oxvba.toml"

# Type library references
[[references.typelib]]
importlib = "Scripting"
libid = "{420B2830-E718-11CF-893D-00A0C9054228}"
major = 1
minor = 0
lcid = 0

# Native library references (for Declare statements)
[[references.native]]
kind = "declare-lib"
name = "host"
path = "build/hostbridge.dll"
symbols = "build/hostbridge.symbols.json"   # optional symbol metadata

[build]
default_target = "artifact"         # artifact | exe | dll
flavor = "lite"                     # lite (VM-only) | jit (VM+JIT)
out_dir = "dist"                    # Output directory
deterministic = true                # Deterministic build mode

[build.hooks]
prebuild = ["cmake --build build --config Release"]   # Commands to run before build

[extensions]
top_level = false                   # Enable top-level code (OxVBA extension)

[conditional_constants]
DEBUG = 1                           # Conditional compilation constants
VBA7 = 1
```

#### 4.1.3 Schema evolution policy

- `schema_version` is a monotonically increasing integer.
- Parsers MUST reject schemas with a version higher than they support.
- New optional fields are added without incrementing the schema version.
- Removing or changing the semantics of existing fields increments the schema version.

### 4.2 Directory-as-Project Convention

Modern language tools use the containing directory as the project scope. OxVBA adopts this convention:

**With `oxvba.toml` (explicit mode):**
- The directory containing `oxvba.toml` is the project root.
- `[layout]` section controls file discovery.
- Project name, kind, entry point, and references are explicit.

**Without `oxvba.toml` (convention mode):**
- All `.bas` and `.cls` files in the directory are compiled.
- Directory name becomes the project name.
- `ProjectKind::Source` is assumed.
- Entry point lookup: `Sub Main` in any module (error if not found or ambiguous).
- No references, no policy overrides (defaults apply).

**Discovery order for `oxvba run-project [PATH]`:**

1. If PATH is a `.vbp` file: use VBP-S0 adapter.
2. If PATH is a directory containing `oxvba.toml`: use `oxvba.toml`.
3. If PATH is a directory without `oxvba.toml`: use convention mode.
4. If PATH is omitted: use current directory and repeat steps 2-3.

### 4.3 `.vbp` Adapter

The `.vbp` adapter is an import/compatibility layer, not the canonical project format.

**VBP-S0 subset — supported keys:**

| `.vbp` key | OxVBA mapping |
|-----------|---------------|
| `Type=Exe` | `ProjectKind::Source` |
| `Type=OleDll` / `Type=Control` | `ProjectKind::Library` |
| `Name=<name>` | `ProjectManifest.project_name` |
| `Startup="Sub Main"` | Entry point configuration |
| `Module=<name>; <path>` | `ModuleUnit` with `ModuleKind::Procedural` |
| `Class=<name>; <path>` | `ModuleUnit` with `ModuleKind::Class` |
| `Reference=<...>` | `ProjectReference` with `ReferenceKind::TypeLibrary` |

**Deferred keys:** `Form`, `UserControl`, `PropertyPage`, build metadata, COM registration directives — parsed as known-but-unsupported with stable `VBP-E-UNSUPPORTED-*` diagnostics.

**Import command:**

```powershell
$ cat legacy/Project1.vbp
Type=Exe
Startup="Sub Main"
Name="FinanceCalc"
Module=Main; Main.bas
Module=Utils; Utils.bas
Class=Calculator; Calculator.cls

$ oxvba import-vbp legacy/Project1.vbp --out ./oxvba.toml
oxvba: imported 3 modules (2 procedural, 1 class), 0 references
```

The full VBP-S0 implementation plan is in `docs/worksets/WORKSET_2026-03-05_VBP_SUBSET_AND_ARTIFACT_PLAN.md`.

### 4.4 Artifact Format: `*.oxvbapkg` (A0)

The compiled artifact is a versioned, self-describing package containing everything needed to run a project without re-compilation.

**Required sections:**

| Section | Contents |
|---------|----------|
| `manifest_snapshot` | Canonical project manifest projection |
| `bytecode_payload` | Compiled bytecode (rkyv-serialized) |
| `source_hashes` | SHA-256 hash of each source module (for staleness detection) |
| `toolchain_fingerprint` | OxVBA version + build profile that produced the artifact |
| `policy_fingerprint` | Runtime profile and policy preset used during compilation |
| `export_inventory` | Host-visible procedure exports |

**Compatibility rules:**

- Artifact MUST include schema version.
- Runtime MUST reject artifacts with incompatible schema versions deterministically.
- Runtime SHOULD warn on toolchain version mismatches without rejecting.
- Artifacts are profile-locked by default (the policy fingerprint from build time is embedded).

### 4.5 Build Targets: EXE and DLL

**Wrapper EXE:**

Embeds OxVBA runtime + compiled project artifact into a standalone executable.

| Flavor | Contents | Measured size |
|--------|----------|--------------|
| `lite` | VM-only runtime + artifact | ~0.44 MiB + artifact |
| `jit`  | VM + Cranelift JIT + artifact | ~4.93 MiB + artifact |

Requirements for EXE target:
- Project MUST have an entry point: configured `entry` in `oxvba.toml`, `Startup` in `.vbp`, or a unique `Sub Main` found by convention.
- Top-level code files can serve as entry points when the extension is enabled.

**Wrapper DLL (in-process COM server):**

| Aspect | Contract |
|--------|----------|
| Threading | STA-only; non-STA activation fails deterministically |
| Interface tier | `IDispatch` first (late-bound); early-bound interfaces later |
| Exports | `DllGetClassObject`, `DllCanUnloadNow`, optional `DllRegisterServer` |
| Error mapping | Deterministic `HRESULT` ↔ OxVBA diagnostic mapping |
| Activation | Registry-free manifest first; dual-lane (registry + manifest) later |

**Platform portability:**

- EXE wrappers compile for the target platform (Windows `.exe`, Linux ELF, macOS Mach-O).
- DLL wrappers with COM server semantics are Windows-only.
- DLL wrappers without COM (pure C-API export) are cross-platform.

### 4.6 Build Integration with External Systems

**Scenario:** A project builds a native `.dll` or COM server with a type library externally (e.g., using CMake, MSBuild, or a Makefile), then references the resulting artifacts from VBA code.

**Configuration in `oxvba.toml`:**

```toml
[build.hooks]
prebuild = ["cmake --build build --config Release"]

[[references.typelib]]
importlib = "MyNativeLib"
tlb_path = "build/Release/MyNativeLib.tlb"

[[references.native]]
kind = "declare-lib"
name = "mynativelib"
path = "build/Release/MyNativeLib.dll"
```

**Build integration contract:**

1. `prebuild` hooks run before OxVBA compilation, in declared order.
2. Prebuild hook failures abort the build with a deterministic error.
3. Referenced artifacts (`tlb_path`, `path`) are checked for existence after prebuild.
4. Source hash computation includes referenced artifact hashes for staleness detection.
5. No hidden mutable global state — all external dependencies are declared in `oxvba.toml`.

### 4.7 Event Model Closure

**Current state (2026-03-08):**

The compiler/binder event semantics are closed (EVT1/EVT2). `WithEvents`, `Implements`, and `RaiseEvent` have proper project-aware validation with deterministic diagnostics. EVT3 baseline is implemented for the current subset: `RaiseEvent` lowering now dispatches to known `WithEvents` handlers, and compiled projects emit deterministic host-consumable event dispatch bindings.

**Remaining work (EVT3-EVT8):**

| Phase | Scope | Status |
|-------|-------|--------|
| EVT3 | Runtime subscription graph and dispatch semantics | In progress (baseline implemented; deterministic reassignment/clear transition probes executable; full sink-instance graph parity pending) |
| EVT4 | Embedded host event bridge and code-behind routing | In progress (non-COM dispatch mapping baseline implemented) |
| EVT5 | COM-EVT-A: dispatch-style event callbacks (blocking) | In progress (controlled native connection-point callback lifecycle implemented; external oracle evidence pending) |
| EVT6 | COM-EVT-B: non-dispatch event paths (non-blocking deferral allowed) | In progress (controlled source-interface callback lane implemented; external-server parity evidence pending) |
| EVT7 | Conformance, oracle, and formal lanes | Pending |
| EVT8 | Closure gate (close/re-scope remaining event divergences) | In progress (`DIV-0003` closed; `DIV-0004` open) |

**Two-layer design:**

- **Layer 1 (host bridge):** standardize event subscription/dispatch API now. This is the `subscribe_event`/`unsubscribe_event`/`dispatch_event` contract in `OxvbaHostBridge`. Can proceed independently of VBA semantic completion.
- **Layer 2 (VBA semantics):** runtime subscription graph, handler ordering, reassignment behavior, lifecycle integration. Requires EVT3+ phases.

DNA VbCalc is the primary validation target for host-event integration (Layer 1 + Layer 2 working together).

**Proposed diagnostic taxonomy additions:**

Language/binder (implemented):
- canonical list is generated from `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`:
  - `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`

Runtime/host (planned):
- `PMR-E-EVENT-DISPATCH-TARGET-MISSING`, `PMR-E-EVENT-SUBSCRIPTION-STATE-INVALID`

COM bridge (planned):
- `COM-E-EVENT-CONNECTIONPOINT-MISSING`, `COM-E-EVENT-ADVISE-FAILED`
- `COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`, `COM-E-EVENT-PATH-UNSUPPORTED`

### 4.8 Language Services

**Contract shape:**

```rust
pub trait LanguageServiceProvider {
    /// Parse source and return diagnostics.
    fn diagnostics(&self, project: &ProjectManifest) -> Vec<Diagnostic>;

    /// Return all symbols in the project with metadata.
    fn symbols(&self, project: &ProjectManifest) -> Vec<SymbolInfo>;

    /// Return completion candidates at a cursor position.
    fn completions(
        &self,
        project: &ProjectManifest,
        module: &str,
        position: Position,
    ) -> Vec<CompletionItem>;

    /// Return signature help for a call at cursor position.
    fn signature_help(
        &self,
        project: &ProjectManifest,
        module: &str,
        position: Position,
    ) -> Option<SignatureHelp>;

    /// Return the definition location for a symbol at cursor position.
    fn go_to_definition(
        &self,
        project: &ProjectManifest,
        module: &str,
        position: Position,
    ) -> Option<Location>;

    /// Return all references to a symbol at cursor position.
    fn find_references(
        &self,
        project: &ProjectManifest,
        module: &str,
        position: Position,
    ) -> Vec<Location>;
}
```

**Key constraint:** All methods accept `ProjectManifest` — source comes from the host, not the filesystem. This enables hosts like DNA VbCalc to provide language services for VBA code stored inside their container formats.

**Transport strategy:**

1. **Phase 1:** Direct Rust API (for in-process hosts like DNA VbCalc).
2. **Phase 2:** LSP server wrapping the Rust API (for VS Code, other editors).

---

## 5. Design Decision Register

| ID | Question | Options | Recommendation | Status |
|----|----------|---------|---------------|--------|
| D-01 | Top-level code activation mechanism | A: file marker / B: command/project mode / C: file extension | **B: command/project mode** (`--top-level` flag or `[extensions] top_level=true`) | Proposed |
| D-02 | Default behavior for `oxvba run-project .` | A: auto-detect project vs script / B: require oxvba.toml | **Auto-detect:** if `oxvba.toml` exists, use it; else convention mode (all files, find `Sub Main`) | Proposed |
| D-03 | Artifact portability | A: profile-locked / B: profile-portable | **A: profile-locked by default** (safer determinism; portable mode as explicit opt-in) | Proposed |
| D-04 | Process-global registration collision policy | A: fail / B: shadow / C: namespace-prefix | **A: fail by default** (explicit collision error; shadow/prefix as opt-in) | Proposed |
| D-05 | Wrapper DLL COM activation | A: registry-free first / B: dual lane from day one | **A: registry-free first** (simpler deployment; registry lane added later) | Proposed |
| D-06 | XLL architecture default | A: per-project (B1) / B: shared host (B2) | **A: per-project (B1) first** (simpler; B2 follow-up based on data) | Proposed |
| D-07 | Language service transport | A: direct Rust API / B: LSP-first | **A: direct Rust API first** (lowest latency for in-process hosts; LSP wrapper second) | Proposed |
| D-08 | `oxvba.toml` schema version policy | A: semver / B: integer monotonic | **B: integer monotonic** (simpler; no minor/patch ambiguity) | Proposed |
| D-09 | Top-level code `Option` placement | A: before executable only / B: interspersed | **A: before executable only** (matches module-level VBA rules) | Proposed |
| D-10 | WASM default deny scope | A: all HAL capabilities / B: selective | **A: all deny by default** (security-first; explicit allowlist for approved bridges) | Proposed |
| D-11 | EXE entry point requirement | A: strict `Sub Main` / B: auto-detect / C: configurable | **C: configurable** (`entry` in `oxvba.toml`; `Sub Main` fallback if unconfigured) | Proposed |
| D-12 | Unknown `.vbp` keys policy | A: strict (fail) / B: compat (warn) | **A: strict by default** in CI; `--compat` flag for migration workflows | Proposed |
| D-13 | DNA VbCalc persistence format | A: XML-in-ZIP / B: SQLite / C: flat directory | **A: XML-in-ZIP** (Office-inspired simplicity; embedded project support) | Proposed |

---

## 6. Phased Execution Plan

### Phase P1: Design Lock and Contract Catalog

**Deliverables:**
- Lock v2 decisions from this document.
- Publish clause catalog for hosting/project/tooling contract.
- Derive executable acceptance tests from requirements.

**Gate:** Approved design-lock document + clause table + initial acceptance suite scaffold.
**Dependencies:** None.
**Effort:** S

### Phase P2: Canonical Project Format and Directory Workflows

**Deliverables:**
- `oxvba.toml` parser/validator.
- Project discovery (`run-project .` with `oxvba.toml` and convention mode).
- Include/exclude glob evaluation.
- Entry point discovery and validation.
- `init` command for project scaffolding.

**Gate:** Deterministic parse/validation + sample project corpus pass.
**Dependencies:** P1 (design lock).
**Effort:** M

### Phase P3: VBP-S0 Adapter

**Deliverables:**
- `.vbp` parser with VBP-S0 key subset.
- `VbpProject -> ProjectManifest` bridge.
- `import-vbp` command.
- Stable `VBP-E-*` diagnostics for unsupported keys.

**Gate:** VBP fixture matrix pass, stable unsupported diagnostics.
**Dependencies:** P2 (project model must be stable).
**Effort:** M

### Phase P4: Artifact A0 and Run Parity

**Deliverables:**
- `pack` command producing `*.oxvbapkg` artifacts.
- `run-artifact` command consuming artifacts.
- Schema versioning and compatibility checks.
- Source hash staleness detection.

**Gate:** Parity across loose project run and artifact run on fixture suite.
**Dependencies:** P2 (project format), P3 (optional, for legacy input).
**Effort:** M

### Phase P5: Embedded Host Contract and DNA VbCalc Pathfinder

**Deliverables:**
- `OxvbaHostBridge` trait implementation.
- `HostObjectToken` and object model bridge substrate.
- DNA VbCalc pathfinder application:
  - Work panel with controls.
  - Host object model (`Application`, `WorkPanel`, `Controls`).
  - `.vbcalc` persistence format.
  - End-to-end scenario: load project, inject objects, execute, handle events.

**Gate:** End-to-end scenario pass: load project from host store, inject root object, execute entry, handle host callbacks, survive event dispatch cycle.
**Dependencies:** P2, P4 (artifact), P6 (event model — can co-develop).
**Effort:** L

### Phase P6: Event Model Closure (EVT3-EVT8)

**Deliverables:**
- Runtime subscription graph (EVT3).
- Host event bridge and code-behind routing (EVT4).
- COM event bridge: dispatch-style callbacks (EVT5).
- COM event bridge: non-dispatch paths or explicit deferral (EVT6).
- Conformance lanes and oracle probes (EVT7).
- Closure gate: close remaining divergence scope (currently `DIV-0004`) and complete edge-oracle foldback (`ODG-038/039`) (EVT8).

**Gate:** Close divergence tickets or explicitly downgrade parity claim scope.
**Dependencies:** EVT1/EVT2 (already complete). P5 provides validation target.
**Effort:** L

### Phase P7: Wrapper Outputs and Add-in Semantics

**Deliverables:**
- `build-wrapper-exe` command.
- `build-wrapper-dll` command with COM STA surface.
- Scope-aware export registration semantics.
- Lite and JIT wrapper flavors with size budget tracking.
- `ls-exports` and `host-check` commands.

**Gate:** Deterministic registration behavior for document and process scope. Wrapper EXE/DLL run parity with loose/artifact lanes.
**Dependencies:** P4 (artifact format), P5 (host contract).
**Effort:** L

### Phase P8: Excel XLL Prototype

**Deliverables:**
- X1 prototype (per-project self-contained XLL).
- Function registration through `xlAutoOpen`/`xlfRegister`.
- Application object bridge.
- Documented caveat matrix for call-path differences.
- Optional X2 follow-up (shared language-host XLL).

**Gate:** Reproducible interop demo suite + documented caveats.
**Dependencies:** P7 (wrapper DLL substrate).
**Effort:** M-L

### Phase P9: WASM Host Lane Hardening

**Deliverables:**
- WASM bridge contract formalization.
- Conformance suite expansion for WASM profiles.
- Sandbox security verification (capability-denial behavior).
- Browser-sandbox and WASI-local validation.

**Gate:** Sandbox security checks pass + deterministic capability-denial behavior confirmed.
**Dependencies:** P5 (host contract), P6 (events for bridge callbacks).
**Effort:** M

### Dependency Graph

```
P1 ─────► P2 ─────► P3
           │         │
           ▼         │
          P4 ◄───────┘
           │
     ┌─────┼─────┐
     ▼     ▼     ▼
    P5    P6    (parallel possible)
     │     │
     ├─────┤
     ▼     ▼
     P7 ◄──┘
     │
     ├─────► P8
     │
     └─────► P9

Parallelism opportunities:
  - P5 and P6 can co-develop (events pathfinder + events engine)
  - P3 can run in parallel with P4
  - P8 and P9 can run in parallel after P7
```

---

## 7. Immediate Next Worksets

1. **`DESIGN-LOCK-V2`** — Lock decisions from this document, publish clause catalog, derive acceptance test seeds. (P1)

2. **`PROJECT-FORMAT-V1`** — `oxvba.toml` schema, parser, validator, directory discovery, `init` command. (P2)

3. **`VBP-S0-EXEC`** — Execute `WORKSET_2026-03-05_VBP_SUBSET_AND_ARTIFACT_PLAN.md` phases VBP1-VBP3. (P3)

4. **`EVENTS-PARITY-CLOSURE`** — execute `WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md` to drive event runtime semantics from EVR baseline through parity closure (runtime subscription lifecycle, host ingress parity, COM adapter tier closure, divergence/deferred-gate closure). (P6/P5 overlap)

5. **`DNA-VBCALC-PATHFINDER`** — DNA VbCalc application design refinement, object model definition, initial implementation. (P5)

6. **`ARTIFACT-A0`** — Compiled artifact format, `pack` and `run-artifact` commands. (P4)
