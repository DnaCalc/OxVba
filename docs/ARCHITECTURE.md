# Architecture

## Current Workspace

Workspace crates and current roles:
- `oxvba-diagnostics`: shared structured diagnostic model used by compiler,
  project loading, host/CLI, runtime, HAL, and COM boundaries.
- `oxvba-syntax`: lossless lexer/parser and green/red syntax-tree
  infrastructure (CST).
- `oxvba-runtime`: canonical runtime value substrate centered on `Variant`,
  `BStr`, `ObjectRef`, `SAFEARRAY`, and related semantic carriers.
- `oxvba-symbol`: uniform symbol model — providers (project modules, VBA base
  library, COM type libraries, `Declare` descriptors, host primitives,
  referenced-project surfaces), source-agnostic resolution, and the intrinsic
  catalog.
- `oxvba-bind`: the binder. Lowers resolved CST to Core IR (`CoreProgram`):
  procedure bodies, places, coercions, call binding, imports/exports.
- `oxvba-bundle`: Core IR definitions, the primitive instruction set, and
  `linearize` — producing the `Bundle`, the executable bytecode + descriptor
  package both runtimes consume.
- `oxvba-vm2`: the interpreting VM; executes `Bundle`s (including multi-bundle
  cross-project linking via `Vm::link`) and is the permanent reference
  runtime.
- `oxvba-lib`: native bodies of the VBA base library (strings, math, dates,
  conversion, financial, file I/O dispatch).
- `oxvba-jit`: placeholder crate boundary for the future Cranelift JIT;
  current APIs report not implemented and do not fall back to VM execution.
- `oxvba-hal`: host/profile/policy boundary plus shared adapter/bootstrap core.
- `oxvba-com`: live Windows COM bridge crate; owns COM client bridge services,
  COM wire translation (`VARIANT`, `BSTR`, `SAFEARRAY`, `IDispatch`), typelib
  loading, and runtime state/metadata.
- `oxvba-host`: engine orchestration — bind/linearize/execute pipeline, host
  policy, snapshots, package-backed runtime sessions, error routing.
- `oxvba-build`: clean wrapper build orchestration. The current
  `WrappedComServer` slice validates `.basproj` target shape, emits a versioned
  `.oxb` bundle package, projects deterministic COM descriptors from the export
  surface, writes IDL/shim-source artifacts, compiles a generated type library,
  and compiles a bounded Windows in-process COM DLL with per-user class/typelib
  registration and late-bound `IDispatch` dispatch over package-backed runtime
  sessions.
- `oxvba-project`: `.basproj`/`.vbp` project formats, manifests, and
  reference-closure loading.
- `oxvba-cli`: CLI bootstrap/run/build surface.

## Current Execution Shape

The clean pipeline is the sole execution path:

```
source/project (oxvba-project, oxvba-cli, oxvba-host)
  -> oxvba-syntax lossless CST          (parsed once, shared)
  -> oxvba-symbol resolution environment
  -> oxvba-bind                          binder -> Core IR (CoreProgram)
  -> oxvba-bundle::linearize             -> Bundle (bytecode + descriptors)
  -> oxvba-vm2                           interprets; one Bundle per project,
                                         cross-project dispatch via Vm::link
```

- `oxvba-lib` provides base-library natives invoked from the VM;
- `oxvba-hal` provides profile/policy-governed host services (filesystem,
  console, dynamic linking for `Declare`, COM adapter, events);
- `oxvba-com` translates runtime values to and from COM wire representations;
- `oxvba-jit` is a stub pending the JIT v2 design and must not be used as
  compatibility or performance evidence.

The authoritative front-end and package contract is
[`docs/spec/OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md`](spec/OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md).

The current repository does not have a direct native AOT compiler that emits PE
or ELF objects. Native compilation is a planned later lane, after the Cranelift
JIT consumes the same package.

## End-State Destination (North Star)

OxVba targets a state-of-the-art VBA compiler with **one compiler-owned
front-end** feeding **one shared executable artifact** consumed by **two runtime
targets**:

```
source
  -> oxvba-syntax lossless CST
  -> binder (oxvba-bind) -> Core IR
  -> linearize (oxvba-bundle)
  -> bytecode + metadata  (the executable semantic package: Bundle)
        |-- interpreting VM   reference oracle; runs anywhere, incl. browser (WASM) and desktop (Tauri)
        \-- Cranelift JIT     optimizing fast path, lowering from the same package
```

Binding properties of the destination:
- **One front-end, no source surgery.** VBA source enters exactly one pipeline;
  the production compiler and the language service answer every
  symbol/type/diagnostic question from the same HIR/SemanticModel facts. No
  production path performs source-text rewriting or substring parsing.
- **One shared package.** The bytecode-plus-metadata package is the single
  source of truth (IL-style, in the CLR/JVM/Wasm sense). Any fact a JIT tracer
  needs must live in the package and be visible to VM execution — no
  source-to-JIT or side-channel reconstruction. There is no separate front-end
  "lowering IR" between HIR and bytecode beyond what the package already is; the
  only motivated lowering IR is the JIT's consumer-side `ProcLoweringIr`.
- **VM is the permanent reference.** The interpreting VM stays the correctness
  oracle even after the JIT lands; the JIT is a performance fast path validated
  against the VM, never a replacement for it.

The destination is reached in **two strictly ordered phases**:

1. **Phase 1 — full correctness on the VM.** The entire imaginable feature and
   deployment matrix runs correctly through the interpreting VM:
   - all COM scenarios (early/late-bound client and COM-server hosting);
   - native interop (`Declare`, pointer helpers);
   - execution in the browser (WASM) and on the desktop (Tauri);
   - all build targets — `Bundle`, `WrapperExe`, `WrapperLibrary`, and
     `WrappedComServer` (`BuildTarget` in `oxvba-project`); native-image
     `NativeExe`/`NativeDll` are a later evolution.
   The package must be designed JIT-ready during this phase so Phase 2 need
   not reopen it.
2. **Phase 2 — Cranelift JIT.** Only after Phase 1, build the Cranelift-based
   JIT on the same bytecode + metadata, with deep optimization, while the VM
   remains the stable reference. JIT activation is gated on Phase-1 correctness,
   not on a schedule.

## Next Execution-Layer Evolution

The next architectural evolution is a complete executable semantic package:
an IL-style bytecode-plus-metadata boundary that both the VM and JIT consume.
The working draft is
[`docs/spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md).
The declared type model for that package is
[`docs/spec/VBA_TYPE_SYSTEM_V1.md`](spec/VBA_TYPE_SYSTEM_V1.md). The companion
expression/call model is
[`docs/spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md).

The current `Bundle` (`oxvba-bundle`) implements the core of this direction,
but it is not yet the full contract. The target package must preserve the
bytecode control stream,
declared type/slot metadata, procedure/project metadata, UDT descriptors,
array descriptors, COM/native descriptors, error/source maps, helper ABI
requirements, host capability requirements, carrier/layout versioning,
expression classification, Let/Set coercion, operator semantics, property
accessor grouping, Optional/ParamArray binding, and ByRef/ByVal call-site
descriptors.

This package is the semantic input to execution engines:
- the VM interprets it and remains the reference executable truth;
- JIT v2 plans and lowers from it into `ProcLoweringIr`;
- wrappers and future native lanes use the same descriptors and capability
  facts rather than reconstructing semantics from side channels.

Direct source-to-JIT or parallel typed-JIT reconstruction is not an accepted
layering path. If a JIT tracer needs a semantic fact, that fact belongs in the
package first and must also be visible to VM execution or VM evidence.

## Current Value Truth

`Variant` is the canonical execution and snapshot carrier for VM/host
coordination. `RuntimeValue` has been removed from active Rust source; any
remaining mentions are historical docs/evidence or recovery notes, not active
runtime architecture.

Important boundaries:
- retained `Variant` values are the semantic runtime substrate;
- `BStr` exposes a Windows-style owned UTF-16 core view (`OwnedBStrCore`) for
  boundary projection;
- canonical runtime object identity flows through `ObjectRef`, whose base
  object implements a runtime `IUnknown`-style vtable with `AddRef` and
  `Release`;
- `BindingHandle` remains a typed semantic leaf for non-object binding identity;
- raw integer identities remain only where they are explicit control-plane
  tokens or projection/debug compatibility data;
- `ComValue` in `oxvba-com` mirrors the semantic carrier direction rather than
  redefining the runtime around raw COM wire types.

Windows-facing layout truth is projected at helper or boundary seams instead of
falling out of the canonical substrate directly:
- `BSTR` cells for `StrPtr` and `VarPtr(String)` are synthesized in
  pointer-helper logic;
- `VARIANT` truth for COM calls is translated in `oxvba-com`;
- native COM pointer truth remains retained in `oxvba-com`, while runtime
  object identity is `ObjectRef` rather than a raw COM interface pointer or an
  integer handle.

These are current checked-in differences, not hidden assumptions. Any remaining
compatibility projection must be tracked as a named boundary, not treated as
execution truth.

## Current IR Truth

The single active IR is **Core IR** (`oxvba-bundle::coreir`): a resolved,
source-agnostic tree (`CoreProgram` / `CoreProc` / `CoreStmt` / `CoreValue` /
`CorePlace`) emitted by the binder (`oxvba-bind`) and consumed by `linearize`,
which produces the `Bundle` instruction stream plus descriptors. Every
desugaring is explicit in the binder; the contract is
[`docs/spec/OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md`](spec/OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md).

There is **no** mid-level (HIR→MIR→CFG) *optimization* pipeline. The earlier
`oxvba-ir` scaffold (`VbaHir`/`VbaMir`/`CfgIr`), the legacy string-rewriting
front-end, and the transitional `oxvba-compiler` bound-HIR were all removed
with the legacy stack; see git history.

JIT v2 planning names the future procedure-lowering IR
`ProcLoweringIr`. It may be introduced only as a real contract with:
- basic blocks and typed terminators;
- explicit slot/value effects;
- structured helper/runtime calls;
- error-state and control-flow semantics;
- diagnostics and source/bytecode mapping;
- lowering evidence from the executable semantic package.

Until that exists, bytecode plus current VM behavior is the executable truth.
As the executable semantic package matures, that package becomes the durable
compiled artifact while VM behavior remains the reference execution oracle.

## COM And Host Truth

1. `oxvba-hal` is a real workspace crate and part of the active runtime
   boundary.
2. `oxvba-com` is the live Windows COM client bridge and no longer transitional
   scaffolding.
3. `StandardHostServices` is currently the shared Windows/Linux/macOS adapter
   core.
4. Windows COM support is active and tested; non-Windows COM remains explicitly
   unsupported.
5. Host/runtime event ingress exists in two planes:
   - project/runtime event routing in `oxvba-host`;
   - COM callback transport through HAL/adapter state, including payload-based
     polling support.
6. The current COM blockers are behavioral/parity blockers rather than HAL
   ownership blockers:
   - late-bound `IDispatch` parity still remains below VBA/Excel behavior;
   - richer COM value transport still needs broader object/interface/SAFEARRAY
     coverage;
   - those lanes proceed with `oxvba-com` as the live bridge.

## Current Direction

Phase 1 (full correctness on the VM) is the active phase. Near-term work is
ordered:
- conformance and robustness of the clean pipeline (the `.bas` conformance
  corpus under `conformance/`, the differential oracle against real Office
  VBA, and hardening of the unsafe FFI/COM marshalling core);
- language-surface completion on the existing `Bundle` contract (no ISA growth
  without contract review);
- only then Phase 2: the Cranelift JIT consuming the same package, followed by
  native-image lanes.

Historical worksets, gate apparatus, and the MACH-1000 material under
`docs/archive/` and `docs/worksets/` are synthesis and vision context, not
implementation authority where they conflict with this snapshot or the spec
contracts under `docs/spec/`.
