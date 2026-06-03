# Architecture

## Current Workspace

Workspace crates and current roles:
- `oxvba-syntax`: lexer/parser and syntax-tree infrastructure.
- `oxvba-runtime`: canonical runtime value substrate centered on `Variant`,
  `BStr`, `ObjectRef`, `SAFEARRAY`, and related semantic carriers.
- `oxvba-compiler`: resolve/typecheck/project lowering and bytecode emission.
- `oxvba-vm`: register-slot interpreter over `Variant` values.
- `oxvba-jit`: placeholder crate boundary for a future JIT v2 design; current
  APIs report not implemented and do not fall back to VM execution.
- `oxvba-hal`: host/profile/policy boundary plus shared adapter/bootstrap core.
- `oxvba-com`: live Windows COM bridge crate; owns COM client bridge services,
  COM wire translation, runtime state/metadata, and the compiler-facing COM
  reference facade direction.
- `oxvba-host`: engine orchestration, host policy, project runtime sessions, and
  event dispatch.
- `oxvba-launcher`: standalone launcher for direct VBA script execution.
- `oxvba-cli`: CLI bootstrap/run surface.

## Current Execution Shape

High-level execution path:
- source/project inputs enter through `oxvba-host` or `oxvba-cli`;
- `oxvba-compiler` emits `Bytecode` plus runtime/project metadata, currently
  packaged by `OxBundle` when persistence or wrapper surfaces need a durable
  compiled artifact;
- `oxvba-vm` executes compiled code over register slots using the current
  bytecode and metadata surfaces;
- `oxvba-jit` is disabled pending a new design and must not be used as
  compatibility or performance evidence;
- wrapper EXE/library paths package compiled OxVba artifacts and dispatch
  through the existing runtime lanes rather than emitting direct native code;
- `oxvba-hal` provides profile/policy-governed host services;
- `oxvba-com` translates runtime values to and from COM wire representations
  (`VARIANT`, `BSTR`, `SAFEARRAY`, `IDispatch`, event payload transport).

The current repository does not have a direct native AOT compiler that emits PE
or ELF objects. Native compilation is a planned later lane after the
native-ready rebase worksets establish a coherent value substrate, correctness
corpus, runner schema, and real procedure-lowering IR decision.

## End-State Destination (North Star)

OxVba targets a state-of-the-art VBA compiler with **one compiler-owned
front-end** feeding **one shared executable artifact** consumed by **two runtime
targets**:

```
source
  -> oxvba-syntax lossless CST
  -> binder -> bound HIR + SemanticModel
  -> lowering
  -> bytecode + metadata  (the executable semantic package)
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
   The in-flight front-end HIR migration (tracked under `bd-aprs`; see
   [`FRONTEND_STATE_REPORT_2026-06-03.md`](FRONTEND_STATE_REPORT_2026-06-03.md))
   is part of making this phase correct. The package must be designed JIT-ready
   during this phase so Phase 2 need not reopen it.
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

The current `OxBundle` is the seed of this direction, but it is not yet the
full contract. The target package must preserve the bytecode control stream,
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

Two different things have been called "IR" in this repo; they must not be
conflated:

- The **removed `oxvba-ir` mid-level optimization IR** (`VbaHir`/`VbaMir`/`CfgIr`
  plus the `lower_to_hir` no-op lowering). It was removed during the native-ready
  rebase because it was a sequence-preserving scaffold rather than a semantic
  compiler layer: it did not carry the block, terminator, slot-effect,
  helper-call, diagnostic, or source/bytecode mapping structure needed for native
  compilation. There is still **no** active multi-level (HIR→MIR→CFG)
  *optimization* pipeline of this kind.
- The **active front-end bound HIR** (`oxvba-compiler/src/frontend_hir*.rs`): a
  source-level, resolved, arena-allocated tree with CST back-pointers, plus a
  `SemanticModel` overlay for IDE queries. This is the in-flight replacement for
  the legacy string-rewriting front-end (`project.rs` rewrites + legacy
  `resolve::parse_expr`), tracked under `bd-aprs` (see
  [`FRONTEND_STATE_REPORT_2026-06-03.md`](FRONTEND_STATE_REPORT_2026-06-03.md) and
  the End-State Destination above). For the migrated construct subset the default
  production path is now `source → oxvba-syntax CST → binder → bound HIR →
  lowering → bytecode`, with a legacy fallback for not-yet-migrated constructs.
  This front-end HIR is **not** the removed `oxvba-ir`; it currently lowers (via
  the existing `BoundModule`/`emit` backend) to the same bytecode the VM already
  executes, and is not a separate optimization pipeline.

JIT v2 planning now names the future procedure-lowering IR
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

## Native-Ready Rebase Direction

The current native-ready execution authority is
[`docs/worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`](worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md).

Near-term architectural work is intentionally ordered before direct native AOT:
- rebase docs around implementation truth and archive historical plans;
- keep active APIs free of `RuntimeValue` and fake IR scaffolds;
- make numeric helpers and UDT planning `Variant`-native and descriptor-backed,
  while keeping native UDT ABI materialization as a separate future layer;
- build a correctness corpus that exposes numeric, coercion, error-state, array,
  and UDT skeletons;
- standardize VM/wrapper runner results before comparing native artifacts, with
  JIT rows limited to explicit disabled-placeholder status until JIT v2 lands;
- only then introduce direct native compilation through the executable semantic
  package and a real procedure-lowering IR.

The MACH-1000 material remains useful historical synthesis and vision context,
but it is not the current implementation authority where it conflicts with this
architecture snapshot, the native-ready worksets, or implementation evidence.
