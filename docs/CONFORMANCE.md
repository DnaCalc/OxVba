# CONFORMANCE.md

## Purpose
Defines the current conformance loop and matrix gate for the active ladder profile.

## Assets
- `conformance/tests/*.bas` — executable input corpus.
- `conformance/golden/*.csv` — expected outcomes.
- `conformance/divergences/*.bas` — divergence/regression fixtures tracked in evidence docs.
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` — non-blocking oracle foldback register.
- `docs/evidence/conformance/ORACLE_PROBE_SCAFFOLD.md` — reusable probe queue scaffold for deferred oracle capture.

Current corpus includes:
- MVP arithmetic smoke path.
- `Option Explicit` success case.
- `Option Explicit` undeclared-variable failure case.
- Integer subtraction path.
- `If ... Then ... End If` branch behavior.
- `For ... Next` loop behavior (including zero-iteration case).
- Nested `If` inside `For`.
- Relational operator branches (`<>`, `<`, `>=`).
- Boolean condition composition (`Not`, `And`, `Or`).
- `Else` and `ElseIf` branch-chain selection.
- `Do While ... Loop`, `Do ... Loop While`, and `Exit Do`.
- `GoSub`/`Return` intra-procedure flow subset.
- `Select Case` constant dispatch with `Case Else`.
- Named `Sub`/`Function` declarations and `Call` dispatch.
- `ByVal`/`ByRef` parameter passing subset.
- Trailing `Optional` parameter defaults (integer literal subset).
- Named argument call binding (`name := expr`) with ordering validation.
- `Property Get/Let/Set` declaration subset with assignment-form routing to `Let/Set`.
- Intrinsic conversion subset: `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, `Str` (current int-domain semantics).
- String-core intrinsic subset: `Len`, `Left`, `Right`, `Mid`, `InStr`, `LCase`, `UCase` (decimal-string-over-int semantics).
- String-advanced intrinsic subset: `Split`, `Join`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp` (decimal-string-over-int semantics).
- Date/time intrinsic subset: `DateSerial`, `TimeSerial`, `DateValue`, `TimeValue`, `DateAdd`, `DateDiff`.
- Math/financial intrinsic subset: `Abs`, `Int`, `Fix`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`, `Log`, `Exp`, `FV`, `PV`, `PMT` (current deterministic subset semantics).
- Array/introspection intrinsic subset: `Array`, `LBound`, `UBound`, `IsArray`, `VarType`, `TypeName`, `IsNumeric`, `IsDate`, `IsObject`.
- Error-surface subset: `Err.Raise` statement form and `CVErr`.
- Host-sensitive intrinsic subset: `Shell`, `Environ`, `Dir` (deterministic fallback behavior).
- Collection subset model: `CollectionAdd`, `CollectionItem`, `CollectionRemove`, `CollectionCount`.
- Class lifecycle subset: `Class_Initialize` and `Class_Terminate` are invoked around entry execution.
- Dispatch-boundary subset: `CreateObject` and `DispatchInvoke` intrinsic bridge.
- Fixed-size arrays with indexed load/store and bounds errors.
- Dynamic `ReDim` / `ReDim Preserve` (1D literal-bound subset).
- Module-level `Const` and `Enum` declaration usage subset.
- `Type ... End Type` declaration-block parse acceptance baseline.
- `On Error Resume Next` and `Err.Number` subset behavior.
- `On Error GoTo 0` reset behavior and `Resume Next` statement subset.
- `On Error GoTo <label>` handler transfer subset.

## Commands
```powershell
./scripts/run-smoke.ps1
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1
```

## Current policy
At MVP stage, conformance compares:
- execution status (`ok` / `error`)
- slot snapshot output (`SLOTS:` line from CLI)

As runtime semantics mature, this will expand to richer structured outputs (error state and object lifecycle signals).

## Declared Profile Scope (Current Gate)
- Profile id: `mvp-profile-v226` (active host-platform expansion ladder terminal gate)
- Platform: Windows x64
- Backends: `vm`, `jit` (JIT toggle path with VM-equivalent semantics)
- Required matrix cells:
  - `windows/x64/vm`
  - `windows/x64/jit`

Current profile gate is evaluated by `./scripts/run-matrix.ps1`, which writes:
- `docs/evidence/profiles/<version>/matrix_latest.csv`
- `docs/evidence/profiles/<version>/gate_report.md`

Oracle-dependent parity remains deferred and tracked separately:
- register: `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
- scaffold queue: `docs/evidence/conformance/oracle_probe_queue.csv` (generated via `scripts/oracle-probe.ps1`)
