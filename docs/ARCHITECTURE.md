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
- `oxvba-compiler` emits `Bytecode` directly;
- `oxvba-vm` executes compiled code over `Variant` register slots;
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
corpus, runner schema, and real native-facing IR decision.

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

The active compiler path is source/project analysis to `Bytecode`; it does not
flow through a semantic HIR/MIR/CFG pipeline. The previous `oxvba-ir`
HIR/MIR/CFG crate and `lower_to_hir` no-op lowering were removed during the
native-ready rebase because they were sequence-preserving scaffolds rather than
semantic compiler layers. They did not carry the block, terminator, slot-effect,
helper-call, diagnostic, or source/bytecode mapping structure needed for native
compilation.

A future native-facing IR may be introduced only when it is a real contract,
provisionally named `NativeProcIr`, with:
- basic blocks and typed terminators;
- explicit slot/value effects;
- structured helper/runtime calls;
- error-state and control-flow semantics;
- diagnostics and source/bytecode mapping;
- lowering evidence from current bytecode or compiler semantic state.

Until that exists, bytecode plus VM behavior is the executable truth.

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
- only then introduce direct native compilation and a real native-facing IR or
  direct bytecode-to-native path.

The MACH-1000 material remains useful historical synthesis and vision context,
but it is not the current implementation authority where it conflicts with this
architecture snapshot, the native-ready worksets, or implementation evidence.
