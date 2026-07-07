# M4-8 Class Support Terminal Residual Handoff - 2026-07-07

Scope: terminal reconciliation for the accepted M4-8 project-class JIT subset. This evidence
does not claim COM activation, COM connection points, COM server/export readiness, AOT vtables, or
live Windows COM parity.

## Supported M4-8 Subset

The VM3/JIT project-class subset is green for:

- active-project and referenced-project class construction;
- lazy `As New` locals and class fields;
- `VB_PredeclaredId` default instances, reset, replacement, and referenced-project predeclared
  singletons;
- object identity, `Is`, `Is Nothing`, `TypeOf ... Is`, and `TypeName`;
- descriptor-backed method, `Property Get`, `Property Let`, `Property Set`, default-member, and
  object-valued property dispatch;
- named/positional argument mapping, optional parameters, `ParamArray`, and ByRef aliasing in the
  covered project member paths;
- `Class_Initialize` and `Class_Terminate` timing, initializer-failure cleanup/retry, release
  drains, cascades, error suppression, and fault/Resume Next timing;
- project `RaiseEvent` and `WithEvents` fan-out, owner reassignment/detach, owner teardown, and
  handler-fault routing.

## Evidence Chain

- `docs/evidence/class_metadata_package_contract_audit_20260707.md`
- `docs/evidence/class_binder_lowering_residual_suite_audit_20260707.md`
- `docs/evidence/class_vm3_jit_parity_sweep_20260707.md`
- `crates/oxvba-differential/tests/jit_project_objects.rs`
- `crates/oxvba-differential/tests/raiseevent_fanout_vm3.rs`
- `crates/oxvba-differential/tests/class_lifecycle_vm3.rs`
- `crates/oxvba-jit/src/lib.rs` focused JIT unsupported-diagnostic tests

## Explicit Residuals

These are outside M4-8 and remain tracked by later beads:

- `bd-h4oh.11` / M4-9: JIT COM late/early calls, imported COM object dispatch, `Declare`,
  pointer helpers, `GetObject`, ByRef COM writeback, and live `com_matrix` JIT legs.
- `bd-h4oh.12` / M4-10: COM `WithEvents` connection-point transport and event-pump delivery into
  compiled handlers.
- `bd-h4oh.15.1` under `bd-h4oh.15` / M4-13: class COM-export descriptor readiness handoff,
  generated COM vtable metadata, AOT/export packaging, and gated Windows export smoke evidence.
- `bd-h4oh.16` / M4-14: final AOT export packaging for COM server and generic DLL scenarios.

## Unsupported Diagnostic Boundaries

The JIT keeps deterministic unsupported diagnostics for out-of-scope class/COM shapes:

- `NewExtern` / predeclared external imports resolve referenced-project classes, while imported
  `VBA`/COM library coclasses report that imported VBA/COM library classes remain unsupported and
  referenced-project classes are supported.
- `As New` currently accepts active-project class bindings; external class and COM class bindings
  decline explicitly.
- `ComCallEarly` and unresolved dynamic COM/object dispatch remain unsupported with COM/object
  dispatch wording instead of falling through to a generic "instruction not lowered" boundary.

No additional in-scope M4-8 project-class delivery bead is required from the terminal sweep.
