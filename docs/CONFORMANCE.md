# CONFORMANCE.md

## Purpose
Defines the current conformance loop and matrix gate for the active ladder profile.

## Conformance principle: diagnostic & error-behaviour parity

OxVBA targets compatibility with the VBA **compiler**, not only the VBA runtime. A
divergence in *error behaviour* is a conformance bug, on equal footing with a divergence
in computed results:

- Where the VBA compiler raises a **compile-time** error (e.g. "Argument not optional",
  "Expected Function or variable", "Sub or Function not defined"), OxVBA must raise an
  equivalent diagnostic at the **same program point** — not silently accept the code,
  return `Empty`, or downgrade it to a later/run-time failure.
- Where VBA raises a **run-time** error, OxVBA must raise an equivalent run-time error at
  the same point.
- **Silent divergence** — no error where VBA errors, or an error where VBA accepts — is the
  highest-severity class of conformance bug.

Rationale: OxVBA is meant to be the language-service / compiler substrate for VBA tooling
(language server, linting, refactoring, the `.basproj` toolchain). A compiler that *runs*
VBA correctly but disagrees with VBA about **what is an error** cannot back a faithful
language server. The error matrix (text, program point, compile-vs-run classification) is
therefore part of the golden conformance surface; capture VBA's diagnostic via the
Excel/VBA oracle when the target is uncertain.

## Assets
- `conformance/tests/*.bas` — executable input corpus.
- `conformance/golden/*.csv` — expected outcomes.
- `conformance/divergences/*.bas` — divergence/regression fixtures tracked in evidence docs.
- `conformance/integration/catalog.psv` — tracked multi-project integration suite catalog (active + deferred cases).
- `conformance/integration/projects/*` — project/module/reference integration fixtures.
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv` — non-blocking oracle foldback register.
- `docs/evidence/conformance/ORACLE_PROBE_SCAFFOLD.md` — reusable probe queue scaffold for deferred oracle capture.
- `docs/evidence/conformance/PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md` — executable PMR project-model fixture mapping for workset `P9`.
- `docs/evidence/conformance/PMR_PROJECT_MODEL_ORACLE_TEMPLATES_V1.md` — Excel oracle templates for `CCT-037..CCT-041`.
- `scripts/run-pmr-project-model-oracle.ps1` — executable Excel oracle runner for PMR topics `CCT-037..CCT-041`.
- `scripts/excel-dialog-guardian.ps1` — hidden UI Automation watcher used by PMR oracle runs to auto-handle Excel macro/add-in trust dialogs.

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
- PMR project-model deterministic fixture matrix (manifest/project graph diagnostics, qualification paths, visibility/export gating, reference-order shadowing).

## Commands
```powershell
./scripts/run-smoke.ps1
./scripts/run-conformance.ps1
./scripts/run-project-integration-suite.ps1
./scripts/run-project-integration-suite.ps1 -CasePattern INTP-005
./scripts/run-com-early-conformance.ps1 -IncludeFormalLane
./scripts/run-com-early-conformance.ps1 -IncludeFormalLane -NoArtifacts
./scripts/run-com-early-perf.ps1 -Iterations 3
./scripts/run-com-early-perf.ps1 -Iterations 3 -NoArtifacts
./scripts/run-matrix.ps1
./scripts/run-matrix.ps1 -NoArtifacts
./scripts/run-pmr-project-model-oracle.ps1
./scripts/run-pmr-project-model-oracle.ps1 -DisableDialogGuardian
```

PMR oracle runner note:
- Default behavior starts a hidden UI Automation watcher (`scripts/excel-dialog-guardian.ps1`) bound to the active Excel PID to auto-handle macro/add-in trust dialogs that can otherwise block unattended runs.
- Guardian telemetry is written into each run folder as `excel_dialog_guardian.log`.

Excel/VBA oracle modal-handling rule:
- For any real Excel/VBA oracle run that can compile or execute injected VBA,
  follow Govert's Excel/VBA agentic coding guide and Jun 27, 2026 follow-up:
  `https://gist.github.com/govert/2d3946830c35c74806df3f32b597eb72`.
- Always have a UI Automation helper ready before running VBA. If Excel is
  unresponsive after a second or two, inspect UIA windows scoped to the owned
  Excel/VBE process, capture dialog text, highlighted VBE token, and full
  selected code line, then dismiss only the owned modal.
- Do not use `Application.Run` as a compile check. For compile-error oracle
  work, make the VBE visible, invoke Debug -> Compile VBAProject, and read the
  resulting modal with UIA.
- Treat "Cannot run the macro ... may not be available" as ambiguous: it can
  mean macros disabled, target macro missing, or a compile failure anywhere in
  the project/module. If VBOM access is available, macros are enabled, and the
  macro exists, investigate as a compile failure.
- Error location may point to a reference rather than the defect. Check the
  called declaration and intrinsic-name shadowing traps (`Fix`, `Date`, `Time`,
  `Name`, `Error`, `Left`, `Right`, `Len`, `Val`, `Format`, ...).
- Keep cleanup PID-scoped and never blanket-dismiss user dialogs.

## Current policy
At MVP stage, conformance compares:
- execution status (`ok` / `error`)
- retained semantic value snapshot output (`VALUES:` line from CLI)

As runtime semantics mature, this will expand to richer structured outputs (error state and object lifecycle signals).

Project integration lane:
- Uses `cargo test -p oxvba-host --test project_integration_suite` over `conformance/integration/catalog.psv`.
- `expect_compat_slots` is legacy evidence debt, not an accepted current
  conformance oracle. New rows must use retained value assertions or add a
  delivery bead to remove the remaining compatibility projection at that
  boundary.
- Supports increasing complexity levels (`L1..L6`), `active-limit` expected-failure cases, and deferred/planned entries linked to ODG/CCT tracking.

Late-bound `IDispatch` V0.2 matrix:
- Supported and unsupported rows are recorded in
  [V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md).
- Controlled COM VM/host evidence for those rows is recorded in historical
  [V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md).
- Unsupported rows must remain explicit; architecture prose does not count as
  implementation evidence.

Date-string parsing V0.2 policy:
- Accepted grammar, invariant locale policy, and unsupported ambiguity rows are
  recorded in
  [V02_DATE_STRING_GRAMMAR_POLICY_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_DATE_STRING_GRAMMAR_POLICY_2026-04-27.md).
- V0.2 date-string support is deterministic and does not claim host-locale
  parsing parity.

## Declared Profile Scope (Current Gate)
- Profile id: `mvp-profile-v620` (VBA 7.1 + Windows Office COM compliance ladder terminal gate)
- Platform: Windows x64
- Backends: `vm` is the vm3 typed-OxIR interpreter (the product runtime and the
  cell required by this gate). `jit` is a real Cranelift backend (not disabled)
  whose parity with vm3 is exercised by the `oxvba-differential` harness rather
  than this profile gate.
- Required matrix cells:
  - `windows/x64/vm`

Current profile gate is evaluated by `./scripts/run-matrix.ps1`, which writes:
- `docs/evidence/profiles/<version>/matrix_latest.csv`
- `docs/evidence/profiles/<version>/gate_report.md`

For no-artifact validation runs, use `-NoArtifacts` to redirect outputs to `temp/no-artifacts/...` and avoid mutating tracked `LATEST` evidence.

Oracle-dependent parity remains deferred and tracked separately:
- register: `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
  - structured fields: `foldback_required`, `foldback_steps`, `close_condition`
- scaffold queue: `docs/evidence/conformance/oracle_probe_queue.csv` (generated via `scripts/oracle-probe.ps1`)
