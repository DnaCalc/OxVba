# VM3 Spec Gap Closure Memory

## 2026-07-01 - `currency-mul-f64-lossy` (`bd-4ktq.8`)

- Closed the Currency arithmetic value-typing gap by adding an exact scaled
  `i128` lane in `crates/oxvba-eval/src/arith.rs` for Currency `+`, `-`, and
  `*`.
- The lane applies to `Checked(Currency)` typed arithmetic and to Variant
  widening when a Currency operand combines with exact integer-compatible
  operands. Non-exact operands still use the existing coercion path.
- Multiplication divides by the Currency scale (`10_000`) with half-scaled-unit
  ties-to-even rounding, preserves Currency subtype in vm3, and raises Overflow
  (6) at the scaled `i64` boundary.
- Currency-to-Currency and exact integer-compatible Currency coercion now stays
  on the exact path, avoiding f64 re-rounding near the boundary.
- Differential coverage lives in
  `crates/oxvba-differential/tests/currency_arithmetic_vm3.rs`.
- Verification passed:
  - `cargo test -p oxvba-eval currency`
  - `cargo test -p oxvba-differential --test currency_arithmetic_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo clippy --workspace --all-targets` exited 0; warn-level findings
    remained in unrelated crates/tests.
- Live Excel retry note: the first probe produced a VBA compile modal
  (`Expected array`) because helper function `D` was shadowed by local Currency
  variable `d`. UI Automation captured selected token `d` and the line
  `"mul_near=" & d(a * b) & vbLf & _`; the owned PID-scoped dialog/process was
  dismissed and stopped.
- New standing oracle rule recorded in `AGENTS.md`, `docs/CONFORMANCE.md`, and
  `docs/memory/EXCEL_VBA_ORACLE_MODAL_HANDLING.md`: always prepare a
  PID-scoped UI Automation watcher/helper for Excel/VBA compile/runtime modals,
  and treat `Application.Run` macro-availability errors as ambiguous until a
  VBE Debug -> Compile diagnostic is captured.

## 2026-07-01 - Scoping Visibility Fixture Baseline (`bd-4ktq.9.1`)

- Created the fixture-first truth surface for the multi-module scoping batch
  under `bd-4ktq.9`.
- Live Excel/VBA oracle evidence lives in
  `docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/`.
  The runner invokes VBE Debug -> Compile VBAProject through command id `578`,
  captures owned compile modals with UI Automation, and kills only the owned
  Excel PID for each case.
- Oracle matrix:
  - same-module `Private` function: compiles and runs (`7`),
  - cross-module unqualified `Private`: `Sub or Function not defined`,
  - cross-module `Module.PrivateMember`: `Method or data member not found`,
  - duplicate Public unqualified member: `Ambiguous name detected: Dup`,
  - module-name/Public-member collision: `Expected variable or procedure, not module`,
  - valid `VBAProject.Module.Member`: compiles and runs (`13`),
  - wrong project qualifier under `Option Explicit`: `Variable not defined`,
  - `Friend` in a standard module: `Only valid in object module`,
  - `Friend` in a class module: compiles and runs (`19`).
- Added `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`.
  Current-green tests cover the legal baseline shapes and oracle-backed
  expected failures for `bd-4ktq.9.2` through `bd-4ktq.9.6`.
- Verification passed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Private Module Visibility (`bd-4ktq.9.2`)

- Closed `intra-project-private-not-enforced` for same-project standard-module
  leakage.
- `ProjectProvider::MemberEntry` now carries scanner-owned `Visibility`.
  Project-level unqualified lookup only publishes `Public` members, and
  `Module.Member` / `Project.Module.Member` qualified lookup uses a public-only
  owner-member resolver. The existing all-member owner resolver remains for
  typed member paths so class/internal member mechanics are not broadly
  rewritten in this bead.
- Same-module `Private` access remains valid through the source scope chain,
  which is consulted before provider lookup.
- Flipped on the oracle-backed scoping fixture assertions for:
  - cross-module unqualified `Private` -> rejected,
  - cross-module `Module.PrivateMember` -> rejected.
- Verification passed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Duplicate Public Ambiguity (`bd-4ktq.9.3`)

- Closed `ambiguous-name-not-detected` for unqualified same-project public
  lookup.
- `ProjectProvider` now tracks the owning module symbol for each public
  candidate. If a bare active-project name has public candidates from multiple
  modules, ordered provider lookup stops before lower-priority providers such as
  the VBA library can win.
- The binder maps unresolved bare-expression/call-target contexts with such a
  provider ambiguity to `BindError::AmbiguousName`, whose stable diagnostic code
  is `BIND-E-AMBIGUOUS-NAME` and whose text matches the live-oracle shape
  `ambiguous name detected: <name>`.
- Qualified `Module.Member` lookup remains valid for each public module owner.
- Flipped on the oracle-backed vm3 fixture for duplicate public `Dup()` across
  `Alpha` and `Beta`, and added a resolver regression where duplicate active
  public `Len` declarations block fallback to the VBA library `Len`.
- Fresh-eyes review found and removed a stale host-test anchor that now matches
  zero tests from the current PMR rows; the old anchor remains only in
  historical oracle capture output.
- Verification completed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Module Name / Public Member Collision (`bd-4ktq.9.4`)

- Closed `module-name-public-member-collision` by making bare module namespace
  use fail deliberately with a VBA-shaped binder diagnostic instead of falling
  through to incidental array/place errors or a colliding public member.
- Added `BindError::ExpectedVariableOrProcedureNotModule` with stable diagnostic
  code `BIND-E-EXPECTED-VARIABLE-OR-PROCEDURE-NOT-MODULE` and message shape
  `expected variable or procedure, not module: <name>`.
- `bind_call_route`, `bind_ident`, and `bind_index_or_call` now reject
  `SymbolKind::Module` bindings when used as values/callees. `Module.Member`
  qualification remains handled by the namespace-qualified member path.
- Flipped on the oracle-backed scoping fixture for module `Clash` plus public
  function `Other.Clash()`, asserting the module diagnostic shape.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Project Qualifier Validation (`bd-4ktq.9.5`)

- Closed `project-qualifier-ignored` for active-project qualified lookup.
- `ProjectProvider` now carries the folded active project name and only resolves
  three-part `Project.Module.Member` names when the project segment matches.
  A wrong active-project qualifier returns no binding instead of resolving as if
  the segment were absent.
- `ResolutionEnvironment::is_project_name` exposes active and referenced
  project names. The binder treats such names as namespace qualifiers, allowing
  valid active-project `VBAProject.Module.Member` expressions to reach qualified
  resolution.
- Referenced-project surface resolution already validated its project segment;
  this bead preserved that path.
- Flipped on the oracle-backed scoping fixtures for valid
  `VBAProject.Lib.Pub()` and invalid `WrongProject.Lib.Pub()`.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Friend on Standard Modules (`bd-4ktq.9.6`)

- Closed `friend-on-standard-module`.
- The scanner now carries the module kind into declaration scanning and rejects
  module-level `Friend` procedures/properties in `ModuleKind::Procedural`
  modules with `SymbolModelError::FriendNotValidInStandardModule`.
- The stable diagnostic code is `SYM-E-FRIEND-ONLY-VALID-IN-OBJECT-MODULE`,
  matching the live-oracle rule that `Friend` is only valid in object modules.
- Class-module `Friend` remains valid and distinct from `Public`/`Private`.
- Flipped on the oracle-backed vm3 fixture for standard-module
  `Friend Sub Helper`; the class Friend baseline remains active and green.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Multi-Module Scoping Visibility Batch (`bd-4ktq.9`)

- Closed the scoping/visibility batch after all six child beads landed:
  oracle truth surface, Private visibility, duplicate Public ambiguity,
  module-name/member collision diagnostics, project qualifier validation, and
  standard-module Friend rejection.
- The active vm3 scoping fixture now covers 11 live-oracle-backed cases with no
  ignored tests.
- Terminal batch verification:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Scoping Follow-up Cross-Project Fixture Surface (`bd-4ktq.36.1`)

- Created the follow-up scoping/visibility oracle and fixture surface for
  `bd-4ktq.36`.
- Live Excel/VBA 7.1 evidence lives in
  `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`,
  captured by `scripts/run-vm3-scoping-followup-oracle.ps1`.
- The oracle runner follows the modal-safe Excel/VBA guidance: it invokes VBE
  Debug -> Compile VBAProject through command id `578`, captures compile
  dialogs with PID-scoped UI Automation, captures selected token/line via UIA
  and the VBIDE selection fallback where exposed, dismisses only owned dialogs,
  and cleans up only the owned Excel PID.
- Oracle matrix:
  - active project with two modules plus referenced `LibProj.RefTools`:
    compiles and runs (`42`),
  - referenced module-qualified call `RefTools.RefValue()`: compiles and runs
    (`42` with the local helper),
  - referenced project-qualified call `LibProj.RefTools.RefValue()`: compiles
    and runs (`30`),
  - Public Const/Public variable collision: `Ambiguous name detected:
    SharedName`,
  - Public Type/Public Enum collision: `Ambiguous name detected: Payload`,
  - referenced `Option Private Module` export: `Sub or Function not defined`,
  - referenced project precedence plus explicit later-project qualifier:
    compiles and runs (`102`),
  - active-project WithEvents source/handler baseline: compiles and runs (`23`).
- Added `oxvba_differential::run_project_closure` so tests can execute
  leaf-first project-reference closures through the same vm3 closure path used
  by the host.
- Extended `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` with
  active project-reference fixtures for the green baseline, module-qualified
  reference calls, Public Const/variable ambiguity, referenced Option Private
  hiding, reference precedence/project qualifier behavior, and a synthetic
  referenced-project WithEvents source route.
- At this evidence point the Public UDT/Public Enum collision fixture remained
  the follow-on target for `bd-4ktq.36.3`; that subset is now closed by the
  later `bd-4ktq.36.3` entry below.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Public Const / Variable Collision Diagnostics (`bd-4ktq.36.2`)

- Closed the follow-up Public Const/Public module variable collision subset for
  PMR-VIS-002 and PMR-NAME-001.
- Live Excel evidence for the unqualified row is in
  `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`:
  `SCOPING-CONST-VAR-COLLISION` rejects with `Ambiguous name detected:
  SharedName`, selected token `SharedName`, selected line
  `RunProbe = SharedName`.
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` now has active
  rows proving:
  - unqualified `SharedName` across `Public Const SharedName` and
    `Public SharedName As Long` is rejected as ambiguous,
  - `Alpha.SharedName` / `Beta.SharedName` remain deterministic,
  - `VBAProject.Alpha.SharedName` / `VBAProject.Beta.SharedName` remain
    deterministic.
- PMR clause docs and the fixture matrix now point to those vm3 differential
  rows while keeping broader PMR-NAME-001 status partial because UDT/Enum and
  other declaration-space edges remain open.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Public UDT / Enum Naming Conflicts (`bd-4ktq.36.3`)

- Closed the follow-up Public UDT/Public Enum cross-module naming conflict
  subset for PMR-NAME-002.
- Live Excel evidence for the unqualified row is in
  `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`:
  `SCOPING-UDT-ENUM-COLLISION` rejects `Dim Value As Payload` with `Ambiguous
  name detected: Payload`.
- `oxvba-symbol` now builds an alias-aware type-name index for each project
  closure: UDT fields and enum type names are available through bare,
  `Module.Type`, and `Project.Module.Type` spellings, while ambiguous
  unqualified public type-space owners are tracked per project and rejected as a
  symbol diagnostic before lowering.
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` now has active
  rows proving:
  - unqualified `Payload` across `Public Type Payload` and `Public Enum Payload`
    is rejected as ambiguous,
  - `Types.Payload` remains a valid UDT type reference,
  - `VBAProject.Types.Payload` remains a valid UDT type reference.
- PMR clause docs and the fixture matrix now point to those vm3 differential
  rows while keeping PMR-NAME-002 partial for remaining project/module/library
  namespace conflict edges.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`

## 2026-07-01 - Option Private Cross-Project Visibility (`bd-4ktq.36.4`)

- Closed the follow-up referenced-project `Option Private Module` visibility
  subset for PMR-VIS-004.
- Live Excel evidence for the external hidden-module row is in
  `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`:
  `SCOPING-OPTION-PRIVATE-XREF` rejects `HiddenValue()` with `Sub or Function
  not defined`, selected token `HiddenValue`.
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` now has active
  rows proving:
  - a referenced `Option Private Module` public procedure is not callable
    unqualified from an external project,
  - `LibProj.HiddenTools.HiddenValue()` is also not externally callable,
  - the same `Option Private Module` procedure remains callable inside its
    defining project,
  - a non-private public module in the same referenced project remains visible
    through both unqualified and project-qualified calls.
- No production resolver change was needed in this cycle; the export-surface
  synthesis already omits `Option Private Module` types from referenced-project
  surfaces while preserving same-project source scope resolution.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`

## 2026-07-01 - Referenced-Project Precedence And Qualifiers (`bd-4ktq.36.5`)

- Closed the follow-up referenced-project precedence and qualifier subset for
  PMR-NAME-003 / PMR-REF-001.
- Live Excel evidence for the ordered-reference row is in
  `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`:
  `SCOPING-XREF-PRECEDENCE` returns `102`, proving the first reference wins for
  unqualified `Pick()` while `LibB.PickTools.Pick()` reaches the later
  reference explicitly.
- `SurfaceProvider` now reports ambiguous unqualified global names within one
  referenced project. That stops lookup before a later reference or the VBA
  library can be selected, matching the active-project provider contract.
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` now has active
  rows proving:
  - active project members shadow referenced project members while explicit
    project qualifiers still reach the reference,
  - reference order selects the first project for unqualified duplicate members
    while explicit project qualifiers disambiguate a later reference,
  - wrong referenced-project qualifiers reject,
  - duplicate global names inside one referenced project are ambiguous,
  - an ambiguous first reference blocks fallback to a later reference.
- PMR clause docs and the fixture matrix now point to those vm3 differential
  rows while keeping broader PMR-NAME-003 / PMR-REF-001 status partial for
  remaining library/type-space and broader reference-boundary edges.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`

## 2026-07-01 - WithEvents Source Visibility And Handler Binding (`bd-4ktq.36.6`)

- Closed the follow-up `WithEvents` source visibility and handler-prefix binding
  subset for PMR-CLS-001 / PMR-CLS-002.
- Live Excel evidence for the active source/handler baseline is in
  `docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/`:
  `SCOPING-WITHEVENTS-ACTIVE` returns `23`.
- `oxvba-symbol` now rejects module-level `WithEvents` declarations in
  procedural modules with `SYM-E-WITHEVENTS-ONLY-VALID-IN-OBJECT-MODULE`,
  instead of letting the allocator treat the declaration as an ordinary global.
- `oxvba-bind` now distinguishes a known event source with no matching handler
  from an unknown or inaccessible `WithEvents` source type, reporting
  `BIND-E-UNRESOLVED-NAME` for the latter instead of silently emitting no event
  routes.
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` now has active
  rows proving:
  - active-project `WithEvents src As Clock` routes `Clock.Tick` to
    `src_Tick`,
  - referenced-project `WithEvents src As LibProj.Clock` routes through the
    referenced export surface,
  - a mismatched handler prefix does not route,
  - procedural-module `WithEvents` declarations reject with a deterministic
    diagnostic,
  - a declaration-only `WithEvents src As LibProj.Clock` rejects when `Clock` is
    not exported,
  - non-exposed referenced event source classes are not visible across the
    project boundary.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`

## 2026-07-01 - Scoping Follow-up Terminal Reconciliation (`bd-4ktq.36.7`)

- Reconciled the follow-up scoping/visibility batch after delivery beads
  `bd-4ktq.36.1` through `bd-4ktq.36.6` closed.
- `crates/oxvba-differential/tests/scoping_visibility_vm3.rs` now has 34 active
  fixtures and no ignored follow-on rows for the scoped batch.
- Updated the inventory critique addendum so the old "no multi-module /
  multi-project fixtures" residual risk is explicitly superseded by the closed
  follow-up batch.
- Confirmed PMR partial statuses remain intentionally broader than the scoped
  batch where they still cover unclosed project/module/library, reference, or
  event-graph semantics.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `git diff --check`
  - `br dep cycles --json`

## 2026-07-01 - Val Radix Prefix Strings (`bd-4ktq.7`)

- Closed the `Val("&H...")`/`Val("&O...")` radix-prefix gap.
- `Val` now checks for a leading VBA radix token before falling back to its
  existing decimal-prefix scanner, reusing the runtime width/sign helpers so
  `&HFFFFFFFF` and `&O37777777777` evaluate to `-1`.
- `CInt`/`CLng`/`CDbl` and `IsNumeric` keep full-token parsing; `Val` uses the
  leading-token form and can skip VBA-ignored spaces inside the radix token.
- The remaining wide-literal carrier issue is still tracked separately as
  `integer-literal-surfaces-as-long`.
- Verification target:
  - `cargo test -p oxvba-lib val_parses_vba_radix_prefixes`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind numeric_conversion_intrinsics`
  - `cargo test -p oxvba-differential --test hex_oct_literal_sign_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Call Argument Oracle Truth Surface (`bd-4ktq.10.1`)

- Created the `bd-4ktq.10` call/argument fixture and oracle truth surface.
- Live Excel/VBA evidence:
  `docs/evidence/conformance/vm3_call_argument_oracle_20260701T1040Z/`.
- The oracle runner uses VBE Debug -> Compile (command ID 578), PID-scoped UI
  Automation modal capture/dismissal, VBIDE selected token/line fallback, and
  PID-scoped Excel cleanup.
- Oracle outcomes:
  - bare statement ByRef mutates caller: `105`
  - ByVal parameter does not mutate caller: `5`
  - statement-form parenthesized ByRef arg is forced ByVal: `5`
  - `Call Inc(x)` keeps ByRef writeback: `105`
  - ByRef type mismatch compile error on selected line `TakeLong x`, token `x`
  - extra arg compile error on selected line `TakeOne 1, 2`
  - missing required arg compile error on selected line `TakeTwo 1`
  - optional default and ParamArray legal baselines return `12` and `6`
- Added vm3 fixture `call_argument_binding_vm3.rs`: 5 active baselines green and
  4 ignored follow-on assertions for `bd-4ktq.10.2` through `bd-4ktq.10.4`.
- Verification target:
  - PowerShell parser check for `scripts/run-vm3-call-argument-oracle.ps1`
  - `scripts/run-vm3-call-argument-oracle.ps1 -RunId vm3_call_argument_oracle_20260701T1040Z`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Statement Call Parentheses Force ByVal (`bd-4ktq.10.2`)

- Closed `statement-call-paren-not-byval`.
- Parser change: implicit statement-call callee parsing now leaves
  whitespace-separated terminal parentheses (`Inc (x)`) for the bare argument
  list instead of swallowing them as a callee `IndexExpr`.
- Compatibility guard: attached no-space forms (`DispatchInvoke(...)`) still
  parse as attached `IndexExpr` callees, and indexed receivers that continue
  into member access (`obj(1).Inc (x)`) keep the receiver index in the callee
  while splitting only the terminal parenthesized argument.
- Binder/vm3 effect: the existing parenthesized-argument path now sees
  `ParenExpr` for `Inc (r)` and constructs a ByVal argument, so the caller stays
  unchanged while bare `Inc r` and `Call Inc(r)` keep ByRef writeback behavior.
- Golden snapshot audit: one intended line changed in
  `vmr04_byref_expression_forms.bas`; `Touch (seed)` now leaves
  `forcedByValObserved = 10` instead of the previous wrong `11`.
- Verification target:
  - `cargo test -p oxvba-syntax`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - ByRef Type Mismatch Rejection (`bd-4ktq.10.3`)

- Closed `byref-type-mismatch-accepted`.
- Project-procedure `bind_one_arg` now checks the declared type of an aliased
  ByRef l-value against the declared parameter type before emitting
  `CoreArg::ByRef`.
- Mismatches return stable binder diagnostic
  `BIND-E-BYREF-TYPE-MISMATCH` with message
  `ByRef argument type mismatch: expected <type>, got <type>`.
- Parenthesized/non-aliased arguments still pass through a ByVal temporary and
  are coerced to the parameter type, matching the VBA rule that parentheses
  force evaluation/coercion instead of caller-slot writeback.
- Boundary kept explicit: exact Variant l-values remain valid for `ByRef
  Variant`, scalar typed l-values such as `Long` are rejected for `ByRef
  Variant`, and array l-values remain accepted for `ByRef Variant` to preserve
  the ChibiPDF/dynamic-array idiom.
- Golden snapshot audit: one intended line changed in
  `conformance/tests/byref_typed_mismatch_error.bas`, from the previous wrong
  `ok[2]` to `err(bind: ByRefTypeMismatch { expected: "Long", actual:
  "Integer" })`; older conformance oracle/golden CSV evidence already classified
  that fixture as an error.
- Verification target:
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Procedure Arity Validation (`bd-4ktq.10.4`)

- Closed `no-call-arity-validation`.
- Project-procedure `bind_proc_args` now rejects extra positional arguments
  when the callee has no `ParamArray`, instead of binding and later dropping
  the tail.
- Missing required fixed parameters now bind-error immediately as
  `ArgumentNotOptional { parameter: "<name>" }`.
- New stable diagnostics:
  `BIND-E-WRONG-NUMBER-OF-ARGUMENTS` for the VBA-compatible
  `Wrong number of arguments or invalid property assignment` shape, and
  `BIND-E-ARGUMENT-NOT-OPTIONAL` for omitted required parameters.
- Optional/default parameters and `ParamArray` calls remain accepted and
  covered by regression tests.
- Project property put/set calls now use a property-specific argument binder
  that reserves the trailing RHS value parameter, for indexed and unindexed
  assignment routes. That rejects `Prop(index, extra) = value` instead of
  silently overwriting the extra supplied argument with the RHS, and prevents
  `Prop = value` from satisfying an earlier required index parameter with the
  RHS.
- Activated the oracle-backed vm3 arity rejection tests; the call-argument
  fixture now has 9 active tests and 0 ignored tests.
- Golden snapshot audit: `vmr04_diag_missing_required.bas` moved from wrong
  `ok[]` to `err(bind: ArgumentNotOptional { parameter: "target" })`, and
  `vmr04_diag_too_many_args.bas` moved from wrong `ok[2]` to
  `err(bind: WrongNumberOfArgumentsOrInvalidPropertyAssignment)`.
- Verification target:
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Call Argument Binding Batch (`bd-4ktq.10`)

- Closed the vm3 call argument binding batch after all four child beads landed:
  oracle truth surface, statement-call parentheses forcing ByVal, ByRef type
  mismatch rejection, and procedure/property arity validation.
- The active oracle-backed vm3 call-argument fixture now has 9 active tests and
  0 ignored tests, covering legal ByRef/ByVal/optional/ParamArray baselines and
  compile-time rejection for parenthesized statement ByVal, ByRef type mismatch,
  extra arguments, and missing required arguments.
- Tier-2 inventory rows now mark `statement-call-paren-not-byval`,
  `byref-type-mismatch-accepted`, and `no-call-arity-validation` done.
- Batch verification:
  - `cargo test -p oxvba-syntax`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Integer Literal Carrier Truth Surface (`bd-4ktq.11.1`)

- Created the truth surface for `integer-literal-surfaces-as-long`.
- Added the modal-safe Excel/VBA oracle runner
  `scripts/run-vm3-integer-literal-oracle.ps1`.
- Captured live Excel/VBA 7.1 evidence in
  `docs/evidence/conformance/vm3_integer_literal_oracle_20260701T1200Z/`.
  The runner makes VBE visible, invokes Debug -> Compile VBAProject using
  command ID 578, captures compile dialogs and selected code lines with
  PID-scoped UI Automation, dismisses only owned dialogs, and cleans up only
  the owned Excel PID.
- Oracle observations:
  - unsuffixed decimal `7` and `32767` are `2:Integer`;
  - `32768`, `2147483647`, and `7&` are `3:Long`;
  - unsuffixed decimal `2147483648` is `5:Double`;
  - `7%` is `2:Integer`, and `7^` is `20:LongLong`;
  - `&HFFFF` and `&O177777` are `2:Integer:-1`;
  - `&H10000`, `&HFFFF&`, `&O200000`, and `&O177777&` are Long-width rows;
  - `&HFFFFFFFFFFFFFFFF^` is `20:LongLong:-1`;
  - unsuffixed `&H100000000` and `&O40000000000` produce compile-time syntax
    errors.
- Added `crates/oxvba-differential/tests/integer_literal_carrier_vm3.rs` with
  active current-green Long-width baselines and ignored oracle-backed
  follow-on assertions for `bd-4ktq.11.2`.
- Next implementation bead needs to thread typed literal carrier information
  through `CoreConst`/`OxConst`/elaboration/vm3 constant loading, and also
  decide the parser/radix diagnostic shape for unsuffixed radix literals beyond
  Long width.
- Verification target:
  - `cargo test -p oxvba-differential --test integer_literal_carrier_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Integer Literal Carrier Preservation (`bd-4ktq.11.2`)

- Implemented the direct literal carrier path for
  `integer-literal-surfaces-as-long`.
- Added `CoreConst::I16` and `OxConst::I16`, then threaded it through binder
  literal inference, radix parsing, symbol const/default handling, conditional
  compilation truthiness, OxIR lowering/type inference, and vm3 `const_variant`.
- Decimal integer literals now use the VBA-visible carrier:
  Integer for signed 16-bit unsuffixed/`%`, Long for Long-width/`&`,
  LongLong for `^`, and Double for unsuffixed decimal beyond Long.
- Radix literals now keep the width carrier from
  `parse_vba_radix_with_width`: Integer/Long for legal unsuffixed widths,
  explicit `^` for LongLong, and rejection for unsuffixed radix beyond Long
  width.
- Activated all rows in
  `crates/oxvba-differential/tests/integer_literal_carrier_vm3.rs`; all 8 pass.
- Updated `hex_oct_literal_sign_vm3.rs` so sign coverage now expects
  Integer-tagged variants for Integer-width radix literals.
- Preserved nearby compatibility behavior that used small numeric values as
  object identities / `Nothing` sentinels by teaching vm3 object helpers about
  `Integer` in addition to `Long`.
- Golden snapshot was re-blessed and audited: the drift is the expected broad
  `Raw { tag: 3 }` to `Raw { tag: 2 }` retagging for Variant/default outputs
  fed by small integer literals; the suspicious `TypeOf 5 Is 5` error drift was
  fixed before blessing.
- Follow-up completed in `bd-4ktq.11.3`: folded constants and optional/default
  metadata paths now have explicit carrier coverage.
- Verification target:
  - `cargo test -p oxvba-runtime vba_radix`
  - `cargo test -p oxvba-bundle core_const_tests`
  - `cargo test -p oxvba-eval collection`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-oxir`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --test integer_literal_carrier_vm3`
  - `cargo test -p oxvba-differential --test hex_oct_literal_sign_vm3`
  - `cargo test -p oxvba-differential --test numeric_suffix_literals_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Integer Literal Const/Default Audit (`bd-4ktq.11.3`)

- Closed the folded constant/default side of
  `integer-literal-surfaces-as-long`.
- Added `DefaultValue::I16` so scanned signature metadata can preserve an
  Integer-width optional default instead of converting it to `I32`/Long before
  exported surface construction.
- Updated surface default conversion so referenced-project calls receive
  `CoreConst::I16` for optional `Variant = 7` defaults.
- Added differential coverage proving:
  - untyped `Const K = 7` and `Const K = &HFFFF` surface as Integer,
  - declared `Const K As Long = 7` remains Long,
  - `Enum` members remain Long as VBA specifies,
  - active-project optional `Variant = 7` defaults preserve Integer.
- Added a cross-project binding regression proving referenced-project optional
  `Variant = 7` metadata also preserves Integer (`VarType` returns `2`, not
  `3`).
- Golden snapshot did not drift after the audit changes.
- Verification target:
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --test integer_literal_carrier_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - String `$` Alias Null Error (`bd-s7cr`)

- Closed the residual `$`-suffix half of `null-not-propagated-string-fns`.
- Kept the existing unsuffixed string-function behavior in `oxvba-lib`:
  value-returning Variant forms still propagate a `Null` argument to `Null`.
- Added source-visible synthetic `VBA` bundle alias exports for string-typed
  forms such as `Left$`, `Right$`, `Mid$`, `UCase$`, `Trim$`, `Chr$`,
  `ChrW$`, `Space$`, `String$`, and `Format$`.
- Updated the VBA library provider to preserve the alias member name in the
  cross-bundle import instead of canonicalizing every alias to the primary
  unsuffixed member.
- Updated vm3 library-import resolution so a call imported through a `$` alias
  raises run-time error 94 (`Invalid use of Null`) when any argument is `Null`,
  before entering the shared unsuffixed native body.
- Added regression coverage:
  - unsuffixed string functions still return `Null` for `Null`,
  - `$` aliases raise error 94 for `Null`,
  - non-`Null` `$` alias calls still execute normally,
  - `Left$` resolves as `VBA.Strings.Left$`, and alias exports target the same
    native body as their primary member.
- Verification target:
  - `cargo test -p oxvba-differential --test null_string_fns_vm3`
  - `cargo test -p oxvba-bundle vba_library`
  - `cargo test -p oxvba-symbol unrelated_class_property_does_not_shadow_vba_left_intrinsic`
  - `cargo test -p oxvba-bind string_functions_left_mid_ucase`

## 2026-07-01 - Numeric Null Coercion Error 94 (`bd-yd6d`)

- Closed `coerce-null-numeric-no-94` for vm3 scalar numeric/date coercions.
- Changed `arith::coerce_numeric` so `Null` raises run-time error 94
  (`Invalid use of Null`) instead of returning a `Null` carrier when a
  declared scalar assignment or explicit OxIR coercion asks for a concrete
  numeric/date target.
- Added `LibError::invalid_use_of_null` and taught `oxvba-lib::as_f64` to
  preserve error 94 for native explicit conversions that route through the
  shared numeric helper (`CBool`, `CByte`, `CInt`, `CLng`, `CLngLng`,
  `CLngPtr`, `CSng`, `CDbl`, `CCur`, `CDate`).
- Left the separate fixed-length-string `Null` behavior/gap untouched; this
  bead is limited to numeric/date scalar coercion.
- Verification target:
  - `cargo test -p oxvba-differential --test null_coercion_vm3`
  - `cargo test -p oxvba-eval`
  - `cargo test -p oxvba-lib`

## 2026-07-01 - String Numeric Character Modulo (`bd-4ktq.12`)

- Closed `string-charcode-mod256` for vm3.
- Changed `String(number, numericCharacter)` so a numeric character argument
  greater than 255 is folded with `Mod 256` before repetition, matching the
  documented VBA rule.
- Kept string character arguments on the existing first-character path, so
  `String(4, "321")` still returns `"3333"` rather than parsing the string as a
  numeric code.
- Added `crates/oxvba-differential/tests/string_repeat_charcode_vm3.rs` to pin
  both the numeric-wrap behavior and the string-argument behavior.
- Verification target:
  - `cargo test -p oxvba-differential --test string_repeat_charcode_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind string_functions_left_mid_ucase`

## 2026-07-01 - Trim Family Space-Only Stripping (`bd-4ktq.13`)

- Closed `trim-strips-all-whitespace` for vm3.
- Replaced Rust Unicode whitespace trimming in `pure::trim` with VBA-style
  U+0020 space trimming for `Trim`, `LTrim`, and `RTrim`.
- Added `crates/oxvba-differential/tests/trim_space_only_vm3.rs`, which strips
  outer spaces while preserving `Chr(9)` tabs for all three Trim-family entry
  points.
- Verification target:
  - `cargo test -p oxvba-differential --test trim_space_only_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind string_functions_left_mid_ucase`
  - `cargo test -p oxvba-differential --test null_string_fns_vm3`

## 2026-07-01 - Hex/Oct Negative Width (`bd-4ktq.14`)

- Closed `hex-oct-negative-width` for vm3.
- Reworked `Hex`/`Oct` formatting through `integer_width_radix`, preserving
  negative fixed-width subtype lanes (Boolean/Integer/Long/LongLong and the
  extended unsigned/signed integer carriers) before formatting two's-complement
  digits.
- Added `crates/oxvba-differential/tests/hex_oct_negative_width_vm3.rs` for
  `CInt(-1)`, `CLng(-1)`, and `CLngLng(-1)` across both `Hex` and `Oct`, plus
  positive unpadded controls.
- Verification target:
  - `cargo test -p oxvba-differential --test hex_oct_negative_width_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-differential --test hex_oct_literal_sign_vm3`
  - `cargo test -p oxvba-bind numeric_conversion_intrinsics_accept_vba_radix_strings`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Sqr/Log/Exp Domain Errors (`bd-4ktq.15`)

- Closed `sqr-log-exp-nan-no-error` for vm3.
- Split `Sqr`, `Log`, and `Exp` out of the raw `math1` f64 wrapper so invalid
  VBA domains surface as run-time errors instead of ordinary IEEE payloads:
  `Sqr` of a negative value and `Log` of a non-positive value now raise error
  5, while overflowing `Exp` raises error 6.
- Left `Sin`, `Cos`, `Atn`, and `Tan` on the existing shared `math1` path; this
  bead only covers the observed `Sqr`/`Log`/`Exp` gap.
- Added `crates/oxvba-differential/tests/math_domain_errors_vm3.rs` for invalid
  domains, overflow, and valid-value controls.
- Verification target:
  - `cargo test -p oxvba-differential --test math_domain_errors_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind math_datetime_conversion_functions_route_through_vba_bundle`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Round Negative Decimal Places (`bd-4ktq.16`)

- Closed `round-negative-digits-clamped` for vm3.
- Changed `Round(number, numdecimalplaces)` to preserve negative
  `numdecimalplaces` instead of clamping them to zero, so VBA-style rounding to
  tens/hundreds/etc. works.
- Kept the existing banker's rounding path; the only change is that the scale
  factor may now be `10^-n`.
- Added `crates/oxvba-differential/tests/round_negative_digits_vm3.rs` for
  `Round(19, -1)`, negative-place half-even controls, and default/positive
  digit controls.
- Verification target:
  - `cargo test -p oxvba-differential --test round_negative_digits_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind math_datetime_conversion_functions_route_through_vba_bundle`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Weekday First Day (`bd-4ktq.17`)

- Closed `weekday-ignores-firstdayofweek` for vm3.
- Changed the native `Weekday` path so its optional `firstdayofweek` argument
  rotates the returned 1-based day number relative to the requested first day;
  the default and explicit Sunday-first behavior remain unchanged.
- Treats `0` (`vbUseSystemDayOfWeek`) as Sunday for deterministic vm3 behavior,
  matching the existing `WeekdayName` handling.
- Added `crates/oxvba-differential/tests/weekday_firstday_vm3.rs` for default
  Sunday-first, explicit Sunday/`0`, and Monday-first controls.
- Verification target:
  - `cargo test -p oxvba-differential --test weekday_firstday_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind math_datetime_conversion_functions_route_through_vba_bundle`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Array VarType/TypeName Elements (`bd-4ktq.18`)

- Closed `vartype-typename-array-element` for vm3.
- Changed `VarType` and `TypeName` for array Variants to read the
  `SafeArray::element_vartype()` metadata instead of treating every array as a
  `Variant()` SAFEARRAY.
- Typed arrays now report `vbArray + element VARTYPE` and `<Element>()`, e.g.
  `Dim a() As Integer` reports 8194 / `Integer()`. `Array(...)` remains a
  Variant array (8204 / `Variant()`).
- Added `crates/oxvba-differential/tests/array_introspection_vm3.rs` for typed
  `Integer`, `Long`, and `String` arrays plus the `Array(...)` control.
- Verification target:
  - `cargo test -p oxvba-differential --test array_introspection_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind math_datetime_conversion_functions_route_through_vba_bundle`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Dynamic Array Bounds Error Proof (`bd-4ktq.19`)

- Closed stale inventory row `lbound-ubound-unallocated-error-13` for vm3 with
  regression evidence; no runtime code change was needed.
- Added `crates/oxvba-differential/tests/array_bounds_unallocated_vm3.rs` for
  the then-assumed unallocated-array bounds error. This was superseded by bead
  `bd-4ktq.53`: live Excel/VBA evidence shows never-allocated and erased
  dynamic arrays raise run-time error 9, not 13, while allocated dynamic and
  fixed arrays still return their declared bounds.
- Verification target:
  - `cargo test -p oxvba-differential --test array_bounds_unallocated_vm3`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Collection Missing Key Error Proof (`bd-4ktq.20`)

- Closed stale inventory row `collection-keynotfound-error-9-not-5` for vm3
  with regression evidence; no runtime code change was needed.
- Added `crates/oxvba-differential/tests/collection_missing_key_vm3.rs`, which
  proves missing keyed `Collection.Item`, default-member access, and `Remove`
  all raise run-time error 9.
- Verification target:
  - `cargo test -p oxvba-differential --test collection_missing_key_vm3`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - For Each Scalar Source Error (`bd-4ktq.21`)

- Closed `foreach-scalar-non-object-empty` for vm3.
- Changed `OxInst::ForEachInit` so non-array, non-object sources raise run-time
  error 13 instead of snapshotting an empty iterator and silently skipping the
  loop body.
- Kept the existing array, built-in `Collection`, project-instance, and foreign
  COM object paths unchanged; the separate `foreach-com-failure-swallowed` gap
  remains open.
- Added `crates/oxvba-differential/tests/foreach_scalar_source_vm3.rs` for
  statically scalar and Variant-held scalar sources, plus array and
  `Collection` controls.
- Verification target:
  - `cargo test -p oxvba-differential --test foreach_scalar_source_vm3`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Stop Statement Headless No-Op (`bd-4ktq.22`)

- Closed `stop-statement-fails-to-bind` for vm3.
- Changed the statement binder so a bare `Stop` call-statement shape lowers to
  no executable Core IR in headless vm3, rather than resolving as a user
  procedure call and failing bind.
- Rejected argument-bearing `Stop` forms in that special case instead of
  silently ignoring malformed arguments.
- Added `crates/oxvba-differential/tests/stop_statement_vm3.rs`, which proves
  execution continues after `Stop` and `Err.Number` remains unchanged.
- Verification target:
  - `cargo test -p oxvba-differential --test stop_statement_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Fixed-Length String Scalar Defaults (`bd-4ktq.23`)

- Closed `fixed-string-scalar-init-empty` for vm3.
- Changed the binder's scalar default helper so fixed-length string defaults
  pass through `types::coerce_store(..., FixedString(N))`, reusing the existing
  padding/truncation path used by ordinary assignments.
- Local and module-level `Dim/Public s As String * N` slots now initialize as
  `N` spaces instead of an empty BSTR.
- Added `crates/oxvba-differential/tests/fixed_string_default_vm3.rs` for
  local and module-level defaults plus assignment padding/truncation controls.
- Verification target:
  - `cargo test -p oxvba-differential --test fixed_string_default_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Negative Constant Array Lower Bounds (`bd-4ktq.24`)

- Closed `redim-negative-lower-rejected` for vm3.
- Extended the binder's `fold_const_i32` helper to fold unary
  `CoreUnOp::Negate` over integer constants with checked negation.
- Dynamic `ReDim a(-N To M)` and fixed `Dim a(-N To M)` now bind and execute.
- Added `crates/oxvba-differential/tests/redim_negative_lower_vm3.rs` for
  dynamic and fixed arrays, checking `LBound`/`UBound` and negative-index
  element access.
- Verification target:
  - `cargo test -p oxvba-differential --test redim_negative_lower_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Runtime `ReDim` Lower Bounds (`bd-4ktq.34`)

- Closed `redim-nonconstant-lower-rejected` for vm3.
- `CoreBound.lower` and `OxInst::ArrayRedim.lower_bounds` now carry runtime
  values/operands, matching the existing runtime upper-bound path, instead of
  forcing explicit `lo To hi` lower bounds through a binder-only constant `i32`.
- vm3 now coerces both lower and upper bound operands at resize time, preserving
  the existing subscript/out-of-memory guards and `ReDim Preserve` lower-bound
  compatibility checks.
- Extended `crates/oxvba-differential/tests/redim_negative_lower_vm3.rs` so
  `ReDim a(n To n + 4)` executes and reports the expected `LBound`/`UBound`,
  while a single-bound `ReDim a(3)` still honors `Option Base 1`.
- Verification target:
  - `cargo test -p oxvba-differential --test redim_negative_lower_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-oxir`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `Resume 0` Elaboration (`bd-4ktq.25`)

- Closed `resume-0-fails-elaboration` for vm3.
- Updated the binder's `ResumeStmt` lowering so a label reference token `0`
  maps to `ErrorOp::Resume`, matching bare `Resume`, instead of creating a
  synthetic label `0` that fails OxIR elaboration.
- Added `crates/oxvba-differential/tests/resume_zero_vm3.rs` for `Resume 0`
  re-entering the faulting statement and a bare `Resume` control.
- Verification target:
  - `cargo test -p oxvba-differential --test resume_zero_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Bare `End` Statement (`bd-4ktq.26`)

- Closed `end-statement-misparsed` for vm3.
- Added a syntax `EndStmt` for bare `End` and made executable block parsing treat
  `KwEnd` as a block terminator only when followed on the same statement by a
  closure keyword such as `Sub`, `If`, `Select`, or `With`.
- Added `CoreStmt::End`, bound bare `End` to it, and lowered it to the existing
  OxIR/vm3 `Halt` terminator.
- Added `crates/oxvba-differential/tests/end_statement_vm3.rs` covering direct
  halt, halt from a callee before caller continuation, and bare `End` before a
  later `If` keyword.
- Verification target:
  - `cargo test -p oxvba-differential --test end_statement_vm3`
  - `cargo test -p oxvba-syntax`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `CDec` Conversion (`bd-4ktq.27`)

- Closed `cdec-absent` for vm3.
- Added `NativeImplId::CDec` to the VBA Conversion module export surface, symbol
  catalog, and oxvba-lib dispatch.
- Implemented `CDec` as a Variant Decimal subtype using the existing `Decimal96`
  carrier, including exact common numeric-string parsing, exponent text, Currency
  scaling, finite numeric conversion, Null error 94, type mismatch 13, and Decimal
  overflow 6.
- Added `crates/oxvba-differential/tests/cdec_conversion_vm3.rs` for
  VarType/TypeName/CStr/CDbl observability, high-precision string input, exponent
  input, raw Decimal payload for `CDec(10)`, and error numbers.
- Extended `null_coercion_vm3` and re-blessed the intended golden drift in
  `conformance/tests/conversion_extended_scalar_subset.bas` from unresolved
  `CDec` to successful Decimal subtype output.
- Verification target:
  - `cargo test -p oxvba-differential --test cdec_conversion_vm3`
  - `cargo test -p oxvba-differential --test null_coercion_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Default Runtime Error Messages (`bd-4ktq.28`)

- Closed `sparse-default-error-message` for vm3.
- Captured live Excel/VBA 7.1 default `Err.Description` strings for the common
  runtime codes in
  `docs/evidence/conformance/vm3_default_error_message_oracle_20260701T1410Z/`
  using VBE Debug -> Compile VBAProject (`ID=578`) before running the probe and
  PID-scoped UI Automation modal handling.
- Expanded `crates/oxvba-vm3/src/lib.rs` `default_error_message` beyond the
  previous handful of codes to cover common core/file/object/Automation errors
  that vm3 already surfaces, while preserving the generic
  `Application-defined or object-defined error` fallback for unmapped custom
  codes.
- Added `crates/oxvba-differential/tests/default_error_messages_vm3.rs` to
  exercise both `Error n` and omitted-description `Err.Raise n` against the
  captured table, plus an unmapped custom-code fallback control.
- Verification target:
  - `cargo test -p oxvba-differential --test default_error_messages_vm3`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `Val` Complete Prefix Parsing (`bd-4ktq.29`)

- Closed `val-incomplete-parse` for vm3.
- Captured live Excel/VBA 7.1 `Val` behavior in
  `docs/evidence/conformance/vm3_val_oracle_20260701T1420Z/` using VBE
  Debug -> Compile VBAProject (`ID=578`) before running the probe and
  PID-scoped UI Automation modal handling.
- Updated `oxvba-lib` `Val` to strip ASCII spaces/tabs/newlines before parsing,
  preserve the already-fixed `&H`/`&O` radix-prefix path, and parse the longest
  complete decimal token rather than letting an incomplete later continuation
  make the whole prefix parse as zero.
- Covered `12-3`, `1.2.3`, incomplete `1e`/`1e+`, complete `E`/`D` exponents,
  `.5`/`-.5`, punctuation stops, whitespace stripping, and radix controls in
  `crates/oxvba-differential/tests/val_incomplete_parse_vm3.rs` plus an
  oxvba-lib unit test.
- Verification target:
  - `cargo test -p oxvba-differential --test val_incomplete_parse_vm3`
  - `cargo test -p oxvba-lib val_`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `Mid` Start Position Error (`bd-4ktq.30`)

- Closed `mid-start-less-than-1-clamped` for vm3.
- Captured live Excel/VBA 7.1 `Mid` function and statement behavior in
  `docs/evidence/conformance/vm3_mid_start_oracle_20260701T1425Z/` using VBE
  Debug -> Compile VBAProject (`ID=578`) before running the probe and
  PID-scoped UI Automation modal handling.
- Updated `oxvba-lib` `mid` and `mid_stmt` to share a one-based start validator
  that raises runtime error 5 when `start < 1`, instead of clamping zero to the
  first character. Positive starts, overlarge function starts, and valid
  statement replacement are preserved.
- Added `crates/oxvba-differential/tests/mid_start_vm3.rs` plus an oxvba-lib
  unit test for function and statement forms.
- Verification target:
  - `cargo test -p oxvba-differential --test mid_start_vm3`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - COM `For Each` Enumeration Failures (`bd-4ktq.31`)

- Closed `foreach-com-failure-swallowed` for vm3.
- Updated the foreign COM object arm of `OxInst::ForEachInit` to propagate
  `ComHal::enumerate_object` failures through `Fault::from_hal` instead of
  silently treating every failure as an empty enumerator.
- Updated HAL comments so the `enumerate_object` contract no longer claims the
  VM converts adapter errors into empty iteration.
- Added `foreach_over_foreign_object_surfaces_enumeration_failure`, a focused
  vm3 unit regression with a custom host that returns a foreign-looking
  `ObjectRef` from `CreateObject` and fails enumeration with host error 438.
  The test proves the loop body does not run and `Err.Number` is populated.
- Verification target:
  - `cargo test -p oxvba-vm3 foreach_over_foreign_object_surfaces_enumeration_failure`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --test foreach_scalar_source_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `Err` Property Writes (`bd-4ktq.32`)

- Closed `err-properties-not-writable` for vm3.
- Captured live Excel/VBA 7.1 behavior in
  `docs/evidence/conformance/vm3_err_property_writes_oracle_20260701T1442Z/`
  using VBE Debug -> Compile VBAProject (`ID=578`) before running the probe and
  PID-scoped UI Automation modal handling.
- Added `ErrorOp::SetErrField` / `OxInst::ErrFieldSet`, with binder support for
  `Err.Number`, `Err.Description`, and `Err.Source` assignments and a
  read-only rejection for `Err.LastDllError`.
- Updated vm3 `ErrState` with an inheritable-field bit: raised errors and
  `Err.Description`/`Err.Source` writes make omitted `Err.Raise`
  Source/Description inherit; `Err.Clear` resets it; `Err.Number` writes only
  update the numeric property.
- Added `crates/oxvba-differential/tests/err_property_writes_vm3.rs` and an
  OxIR elaboration regression for `ErrFieldSet`.
- Verification target:
  - `cargo test -p oxvba-differential --test err_property_writes_vm3`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-oxir`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Native Local Clock Time Facet (`bd-4ktq.33`)

- Closed `now-date-time-utc-not-local` for vm3's native HAL path.
- `StandardHostServices` now converts native time through the host local civil
  clock for `Date`, `Time`, `Now`, and `Timer` instead of deriving the day and
  time-of-day directly from UTC seconds.
- The deterministic lane remains unchanged at its existing stable date/time
  constants.
- `FileDateTime` now uses the same local `SystemTime` -> VBA `Date` serial
  conversion. The first focused host test exposed the old UTC helper as a
  two-hour drift from fresh-file `Now` on the local machine, so this companion
  fix keeps filesystem timestamps aligned with VBA's local wall-clock model.
- Verification target:
  - `cargo check -p oxvba-hal`
  - `cargo test -p oxvba-hal time_`
  - `cargo test -p oxvba-host --test filesystem_statements`

## 2026-07-01 - `Nothing` Distinct From `Empty` (`bd-4ktq.35`)

- Closed `nothing-represented-as-empty` for vm3.
- Captured live Excel/VBA 7.1 `Nothing`/`Empty` observables in
  `docs/evidence/conformance/vm3_nothing_oracle_20260701T151239Z/` using VBE
  Debug -> Compile VBAProject (`ID=578`) with PID-scoped UI Automation modal
  handling. The assignment follow-up used `On Error Resume Next` to pin that
  `v = Nothing` raises 91 and leaves a Variant `Empty`, while
  `Set v = Nothing` succeeds and stores a null object Variant.
- Added `Variant::nothing()` and made vm3 constants materialize `Nothing` as
  `VT_OBJECT` with a null object pointer instead of `Empty`.
- Updated `TypeName` to report null object Variants as `Nothing`, while ordinary
  non-null objects still use the vm3 object-name path where available.
- Extended OxIR assignment lowering and vm3 `ValidateAssignment` so object-valued
  `Let` into a Variant catches the null-object `Nothing` value as runtime error
  91, while `Set` assignments of `Nothing` remain valid.
- Made null-object value-context numeric coercion, widening arithmetic, and mixed
  comparison raise runtime error 91 instead of treating the object as `Empty` or
  falling through to type mismatch.
- Added `crates/oxvba-differential/tests/nothing_vs_empty_vm3.rs` covering
  literal introspection, object variables set/unset to Nothing, the Empty
  baseline, `Set Variant = Nothing`, and `Let Variant = Nothing` under
  `On Error Resume Next`, plus unset object numeric assignment, arithmetic, and
  comparison error 91.
- Verification target:
  - `cargo test -p oxvba-runtime`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-eval`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-oxir`
  - `cargo test -p oxvba-vm3`
  - `cargo test -p oxvba-differential --test nothing_vs_empty_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Form Modality Constants (`bd-4ktq.37.1`)

- Closed `vbmodal-vbmodeless-absent` for vm3.
- Added `vbModeless = 0` and `vbModal = 1` to the shared VBA-library constant
  provider so they fold exactly like other `vb*` value constants.
- Added symbol-provider payload assertions and executable vm3 coverage in
  `crates/oxvba-differential/tests/library_constants_vm3.rs`.
- This bead deliberately does not broaden any form `Show` or UI modality runtime
  claim; it only closes the missing constant surface.
- Verification target:
  - `rustfmt --edition 2024 --check crates/oxvba-symbol/src/providers/vba_library.rs crates/oxvba-symbol/src/tests.rs crates/oxvba-differential/tests/library_constants_vm3.rs`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms`
  - `cargo test -p oxvba-differential --test library_constants_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `Partition` Function (`bd-4ktq.37.2`)

- Closed `partition-absent` for vm3.
- Used Microsoft VBA/.NET documentation plus a direct
  `Microsoft.VisualBasic.Interaction.Partition` probe to pin the range-label
  truth table: bounds are right-justified to the width of `Stop + 1`, out-of-
  range labels blank the missing side, decimal inputs use banker's rounding,
  any `Null` input returns `Null`, and invalid `Start`/`Stop`/`Interval`
  raises runtime error 5.
- Added `Partition` to the native library `Interaction` surface, catalog
  metadata, bundle member export, and oxvba-lib dispatch. `Partition` remains a
  deterministic pure computation even though it lives in the same VBA typelib
  module as host-sensitive Interaction functions.
- Added `pure::partition` plus vm3 differential coverage for in-range,
  below-start, above-stop, `Null`, and invalid-interval behavior.
- Verification target:
  - `rustfmt --edition 2024 --check crates/oxvba-bundle/src/native.rs crates/oxvba-bundle/src/vba_library.rs crates/oxvba-symbol/src/catalog.rs crates/oxvba-lib/src/lib.rs crates/oxvba-lib/src/pure.rs crates/oxvba-differential/tests/partition_vm3.rs`
  - `cargo test -p oxvba-bundle vba_library`
  - `cargo test -p oxvba-symbol catalog`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms`
  - `cargo test -p oxvba-lib partition`
  - `cargo test -p oxvba-differential --test partition_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - FormatNumber Family (`bd-4ktq.37.3`)

- Closed `format-number-family-absent` for vm3.
- Used official Microsoft documentation for the argument shape and tri-state
  semantics, plus a local `Microsoft.VisualBasic.Strings` probe to confirm the
  regional nature of defaults and representative explicit overrides.
- Added `FormatNumber`, `FormatCurrency`, `FormatPercent`, and `FormatDateTime`
  as ordinary migrated VBA-library `Strings` members with catalog metadata,
  bundle exports, and `oxvba-lib` dispatch.
- Implemented a deterministic formatting boundary in `format.rs`: decimal `.`,
  grouping `,`, currency `$`, and the existing date masks. Omitted or `-1`
  decimal places normalize to `2`; `vbUseDefault` tri-state options normalize to
  leading digit on, grouping on, parentheses off. This closes the absent API
  surface without claiming full host regional-settings emulation.
- Added oxvba-lib unit tests and `format_number_family_vm3` coverage for all
  four functions, explicit tri-state overrides, named date/time constants, and
  invalid option runtime error 5.
- Verification target:
  - `rustfmt --edition 2024 --check crates/oxvba-bundle/src/native.rs crates/oxvba-symbol/src/catalog.rs crates/oxvba-lib/src/lib.rs crates/oxvba-lib/src/format.rs crates/oxvba-lib/src/pure.rs crates/oxvba-differential/tests/format_number_family_vm3.rs`
  - `cargo test -p oxvba-bundle vba_library`
  - `cargo test -p oxvba-symbol catalog`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms`
  - `cargo test -p oxvba-lib format_`
  - `cargo test -p oxvba-differential --test format_number_family_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - `Command` and `Error` Functions (`bd-4ktq.37.4`)

- Closed `command-absent` and `error-function-unsupported` for vm3.
- Added `Command`/`Command$` to the VBA `Interaction` library surface, catalog,
  bundle exports, and oxvba-lib host dispatch. The runtime now calls a
  `ProcessEnv` HAL facet: deterministic headless mode returns an empty string,
  native host mode can expose process arguments, and unsupported adapters report
  the process-env capability boundary explicitly.
- Added `Error`/`Error$` to the VBA `Information` library surface and moved the
  vm3 default-error-message table into `oxvba-runtime` so `Error(number)`, the
  legacy `Error n` statement, and omitted-description `Err.Raise n` share the
  same text source. `Error(0)` returns an empty string, unmapped positive codes
  return the generic application/object message, and negative codes raise error
  5.
- vm3 intercepts zero-argument `Error()`/`Error$()` so expression-form calls
  read the current `Err.Description`, matching the stateful language behavior
  while keeping `Error(number)` in the pure library path.
- Extended the parser's contextual-name keyword set so `Error(...)` can parse as
  a function expression without disturbing `On Error ...` or legacy `Error n`
  statement parsing.
- Verification target:
  - `cargo test -p oxvba-syntax error_keyword_can_be_function_expression`
  - `cargo test -p oxvba-bundle vba_library`
  - `cargo test -p oxvba-symbol catalog`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms`
  - `cargo test -p oxvba-lib error_text_returns_default_messages_and_fallbacks`
  - `cargo test -p oxvba-hal process_env`
  - `cargo test -p oxvba-differential --test command_error_vm3`
  - `cargo test -p oxvba-differential --test default_error_messages_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Settings Function Family (`bd-4ktq.37.5`)

- Closed `getsetting-family-absent` for vm3.
- Used official Microsoft VBA documentation for the four-member family:
  `SaveSetting appname, section, key, setting`, `GetSetting(appname,
  section, key, [default])`, `GetAllSettings(appname, section)`, and
  `DeleteSetting appname, section, [key]`.
- Added `GetSetting`, `GetAllSettings`, `SaveSetting`, and `DeleteSetting` to
  the VBA `Interaction` library surface, catalog metadata, bundle exports, and
  oxvba-lib host dispatch.
- Implemented a deterministic `ProcessEnv` HAL settings map in the standard
  host. Values are scoped to one host instance/run, names compare
  case-insensitively, original key casing is preserved for `GetAllSettings`,
  and no real registry/HKCU persistence is claimed. Missing `GetSetting`
  returns the supplied default or `""`; missing `GetAllSettings` returns
  `Empty`; missing `DeleteSetting` targets surface runtime error 5.
- `GetAllSettings` returns a 0-based two-dimensional BSTR SAFEARRAY shaped as
  rows of key/value pairs (`[0..n-1, 0..1]`), matching the documented array
  contract within the deterministic host boundary.
- Verification target:
  - `cargo test -p oxvba-bundle vba_library`
  - `cargo test -p oxvba-symbol catalog`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms`
  - `cargo test -p oxvba-hal settings_state_round_trips_defaults_arrays_and_delete_faults`
  - `cargo test -p oxvba-lib setting`
  - `cargo test -p oxvba-differential --test settings_family_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Library Absent-Surface Batch Reconciliation (`bd-4ktq.37.6`)

- Reconciled the `bd-4ktq.37` batch after all delivery children closed:
  `vbmodal-vbmodeless-absent`, `partition-absent`,
  `format-number-family-absent`, `command-absent`,
  `error-function-unsupported`, and `getsetting-family-absent`.
- Confirmed `docs/VM3_VBA_SPEC_GAP_INVENTORY.md` marks each scoped row done
  with coverage and residual-boundary language. `Command` and `GetSetting`
  family behavior remains explicitly host-policy/deterministic rather than
  overclaiming Access command-line or persistent registry semantics.
- Added a reconciliation note to `docs/BUILTIN_LIBRARY_NATIVE_VS_VBA_SPLIT.md`
  so future built-in-body split work treats this batch as existing native
  behavior/refactor baseline, not absent API surface.
- No new delivery bead was required by fresh-eyes review; remaining inventory
  rows such as `AddressOf` native callbacks, `LeftB`/`RightB`,
  SendKeys/AppActivate, and Erl stay outside this batch and remain owned by the
  broader `bd-4ktq` inventory.
- Verification target:
  - `cargo test -p oxvba-bundle vba_library`
  - `cargo test -p oxvba-symbol catalog`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms`
  - `cargo test -p oxvba-differential --test library_constants_vm3`
  - `cargo test -p oxvba-differential --test partition_vm3`
  - `cargo test -p oxvba-differential --test format_number_family_vm3`
  - `cargo test -p oxvba-differential --test command_error_vm3`
  - `cargo test -p oxvba-differential --test default_error_messages_vm3`
  - `cargo test -p oxvba-differential --test settings_family_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `br dep cycles --json`

## 2026-07-01 - Scoping PMR Residual Audit (`bd-4ktq.38.1`)

- Produced the row-level PMR residual map for the vm3 multi-module scoping and
  visibility surface:
  `docs/evidence/language/PMR_SCOPING_VISIBILITY_RESIDUAL_MAP_2026-07-01.md`.
- The map reconciles closed scoping batches `bd-4ktq.9` and `bd-4ktq.36` with
  older PMR rows for project references/qualifiers, public namespace
  collisions, `Option Private Module`, and `WithEvents` source visibility.
- Current result: no untracked delivery lane was exposed by the audit. The
  remaining accepted reconciliation work is represented by child beads
  `bd-4ktq.38.2` through `bd-4ktq.38.6`.
- Verification target:
  - docs fresh-eyes read-through of the residual map against
    `MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`,
    `PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md`,
    `DEFERRED_ORACLE_GATES.csv`, and the vm3 inventory
  - `git diff --check`
  - `br dep cycles --json`

## 2026-07-01 - Project Reference / Qualifier PMR Reconciliation (`bd-4ktq.38.2`)

- Reconciled the PMR project-reference and qualifier rows against the closed
  scoping evidence.
- Updated `MODPROJ-005` and `MODPROJ-016` in
  `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv` to point to
  the scoped vm3 referenced-project and qualifier fixtures:
  unqualified referenced public calls, module-qualified referenced calls,
  project-qualified referenced calls, active-project shadowing,
  first-reference precedence, wrong-project rejection, duplicate referenced
  globals, and ambiguous-reference fallback blocking.
- Kept both rows `partial` because external type-library, broken-reference,
  library/type-space, and broader reference-boundary edges remain outside this
  vm3 scoping subset under existing PMR/COM residual lanes such as `ODG-041`.
- Verification target:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-bind --test cross_project cross_project`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`

## 2026-07-01 - Public Namespace Collision PMR Reconciliation (`bd-4ktq.38.3`)

- Reconciled the PMR public namespace collision rows against the closed
  scoping evidence from `bd-4ktq.9` and `bd-4ktq.36`.
- Updated `MODPROJ-018` and `MODPROJ-019` to point at live vm3 and symbol
  fixtures for duplicate Public procedure ambiguity, module-name/public-member
  collisions, Public Const/Public variable ambiguity, legal module/project
  qualified access, and duplicate-member ambiguity before VBA-library fallback.
- Refreshed `PMR-VIS-002`, `PMR-VIS-003`, `PMR-NAME-001`, and the current
  qualified-name anchors to remove stale pre-refactor `oxvba-compiler` /
  `public_symbol_collisions_require_qualification` references from the active
  PMR scoping truth rows.
- Kept the affected rows `partial` where the row scope reaches broader
  project/module/library namespace behavior beyond the vm3 scoping subset.
- Fresh-eyes review checked the new row anchors against live test functions,
  verified stale public-collision and qualified-name anchors are gone from the
  active PMR scoping rows, and confirmed the affected rows still use
  subset-safe `partial` wording.
- Verification completed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`

## 2026-07-01 - Option Private PMR Reconciliation (`bd-4ktq.38.4`)

- Reconciled the PMR `Option Private Module` reference and host-boundary rows
  against the closed scoping evidence from `bd-4ktq.36.4`.
- Updated `MODPROJ-017`, `MODPROJ-039`, and `PMR-VIS-001` to point at live vm3
  and symbol fixtures for referenced-project hiding, project-qualified hiding,
  same-project access, normal public referenced-module access, and the current
  project export-surface boundary.
- Kept the host-direct invocation distinction tied to historical CCT-038 oracle
  evidence because the old host-export unit anchors are no longer live test
  names in the current crate graph.
- Kept the affected rows `partial` where the row scope reaches broader host
  catalog and host/HAL project-public-entity behavior beyond the vm3 scoping
  subset.
- Fresh-eyes review checked the new anchors against live vm3 and symbol tests,
  verified the touched Option Private rows no longer cite the old host/export
  test names, and left broader stale compiler-era anchors in non-scoping PMR
  rows for the terminal reconciliation pass.
- Verification completed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-host -- --list`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`

## 2026-07-01 - WithEvents PMR Reconciliation (`bd-4ktq.38.5`)

- Reconciled the PMR `WithEvents` source-visibility and handler-prefix rows
  against the closed vm3 scoping evidence from `bd-4ktq.36.6`.
- Updated `MODPROJ-022` and `PMR-CLS-001` to point at live symbol/vm3 fixtures
  for procedural-module `WithEvents` rejection:
  `scanner_rejects_withevents_in_standard_modules` and
  `withevents_in_procedural_module_should_be_rejected`.
- Moved `MODPROJ-023` from `planned` to `partial` for the active-project and
  referenced-project source-visibility/handler-prefix subset covered by
  `active_project_withevents_source_routes_to_handler`,
  `referenced_project_withevents_source_routes_to_active_project_handler`,
  `withevents_handler_prefix_mismatch_does_not_route`, and private/non-exposed
  referenced source rejection fixtures.
- Kept full event lifecycle, reassignment ordering, cleanup, and broader COM
  event parity outside this scoping batch under `DIV-0004` and event/COM work.
- Fresh-eyes review checked the new anchors against live test functions,
  verified stale WithEvents compiler-era anchors are gone from the touched PMR
  truth rows, and confirmed `MODPROJ-023` stays subset-safe as `partial`.
- Verification completed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`

## 2026-07-01 - Scoping PMR Terminal Reconciliation (`bd-4ktq.38.6`)

- Ran the terminal reconciliation for the residual PMR scoping/visibility batch
  after child beads `bd-4ktq.38.2` through `bd-4ktq.38.5` closed.
- Added focused current-stack diagnostic coverage for class/event anchors that
  still pointed at the removed `oxvba-compiler` crate:
  `scanner_rejects_implements_in_standard_modules`,
  `raise_event_outside_class_module_is_bind_error`, and
  `raise_event_undeclared_event_is_bind_error`.
- Refreshed active PMR rows and clauses for Implements/RaiseEvent adjacency:
  `MODPROJ-024`, `MODPROJ-025`, `MODPROJ-038`, `PMR-GEN-002`,
  `PMR-CLS-003`, `PMR-CLS-005`, `PMR-CLS-006`, and `PMR-CLS-007`.
- Updated the fixture matrix, conformance command skeleton, class/COM evidence
  note, divergence reproduction commands, residual map, and inventory so they
  agree on the scoped closure and on residual owners (`DIV-0004`, `ODG-041`,
  host/HAL, storage, startup, and broader event/COM work).
- Closed the parent residual batch bead `bd-4ktq.38` after all audit,
  delivery, and terminal reconciliation children were closed.
- Verification completed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-project load_basproj_uses_vb_name_as_semantic_identity_while_preserving_include_path`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review rechecked the active PMR/spec/evidence anchors for stale
  compiler-era references and found no remaining accepted scoping delivery work
  outside the closed child beads.

## 2026-07-01 - For Header Coercion (`bd-4ktq.39.2`)

- Closed the `for-start-step-not-coerced` Tier 4 residual for declared scalar
  vm3 loop counters.
- `bind_for` now resolves the counter's declared type and wraps the `For`
  start, end/limit, and explicit `Step` expressions with the same
  `types::coerce_store` path used by ordinary scalar assignment.
- Added `for_header_coercion_vm3` coverage for string-valued numeric bounds,
  string-valued `Step`, and fractional `Integer` headers that round once before
  loop execution. Existing fixed-integer overflow coverage remains in
  `for_counter_overflow_vm3`.
- Verification completed:
  - `cargo test -p oxvba-differential --test for_header_coercion_vm3`
  - `cargo test -p oxvba-differential --test for_counter_overflow_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo test -p oxvba-bind`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review re-read the binder diff, new vm3 differential tests,
  inventory wording, memory evidence, and bead graph. No unsupported completion
  language, stale residual references, or fixed-counter overflow regressions were
  found.

## 2026-07-01 - Date/Time Residuals (`bd-4ktq.39.3`)

- Closed the live date/time rows scoped to the first-wave residual bead:
  `input-no-date-null-parse`, `datediff-w-day-count`,
  `datediff-datepart-ww-ignore-firstday`, `negative-date-serial-floor`,
  `date-range-not-validated`, `date-string-parser-inconsistent`, and
  `hms-round-crosses-boundary`.
- `DateDiff("w")` now counts matching weekdays between the two dates instead
  of raw days, and `DateDiff("ww", ..., firstdayofweek)` now counts configured
  week-boundary days.
- VBA Date decomposition now uses the whole-number date part for negative
  serials and carries second rounding over to the next displayed date when a
  time rounds past 23:59:59.
- Date constructors/conversions now reject serials outside the Windows VBA Date
  range, while `IsDate` returns False for out-of-range date strings.
- `CDate`/`TimeValue` share stricter time-string parsing, including AM/PM and
  combined date-time strings with trailing AM/PM. Invalid time strings now raise
  Type mismatch instead of becoming midnight.
- `Input #` now parses `Write #` machine-readable date literals and `#NULL#`
  fields back to Date/Null values through the standard HAL filesystem adapter.
- Broad `cargo test -p oxvba-hal` exposed an out-of-scope existing seek
  property failure (`prop_seek_eof_boundary`, `path_token = 1`, `offset = 0`);
  it is tracked as `bd-1a2x` and its generated proptest seed was not committed
  with this bead.
- Verification completed:
  - `cargo test -p oxvba-differential --test date_time_residuals_vm3`
  - `cargo test -p oxvba-runtime vba_date`
  - `cargo test -p oxvba-host --test filesystem_statements write_input_roundtrips_date_and_null_fields -- --exact`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-differential --test date_conversion_vm3`
  - `cargo test -p oxvba-differential --test date_to_string_vm3`
  - `cargo test -p oxvba-differential --test weekday_firstday_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `rustfmt --edition 2024 --check` on the touched date/runtime/HAL/vm3 test
    files; the larger host filesystem test file still has pre-existing
    rustfmt rewrap drift outside this bead's inserted test.
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review re-read the date library/runtime decomposition changes,
  standard HAL `Input #` parser change, host/vm3 tests, inventory wording, bead
  graph, and memory entry. No unsupported completion language or untracked
  accepted date/time residual remained; the only out-of-scope issue found is
  tracked as `bd-1a2x`.

## 2026-07-01 - Numeric/String Coercion Residuals (`bd-4ktq.39.4`)

- Closed the scoped numeric/string residual rows:
  `cstar-null-error-13-not-94`, `empty-plus-numeric-promotes-double`,
  `empty-plus-string-type-mismatch`, `pow-negative-base-fractional-nan`,
  `numeric-string-parse-rust-f64`, `left-right-mid-index-by-char`,
  `sgn-nan-double`, and `strconv-byte-modes-passthrough`.
- Runtime now owns a shared full VBA numeric-string parser used by both
  explicit conversions and implicit arithmetic string coercion. This rejects
  Rust-only `NaN`/`inf` spellings while preserving VBA numeric/radix strings.
- Variant-regime `Empty + numeric` now returns a Double carrier, while
  `Empty + String` raises Type mismatch 13. Ordinary non-empty numeric-string
  arithmetic remains allowed.
- Exponentiation now raises error 5 when `powf` would produce NaN, closing the
  negative-base/fractional-exponent leak. `Sgn` now also rejects a host-supplied
  NaN Double with error 5 instead of returning zero.
- `CStr(Null)` and string store coercion now preserve error 94. Existing
  unsuffixed string-function Null propagation and `$` alias error behavior
  remain covered by the older null-string vm3 tests.
- `Left`/`Right`/`Mid` and statement-form `Mid` now slice/splice UTF-16 code
  units instead of Rust scalar chars, preserving lone surrogate halves.
- `StrConv(..., vbFromUnicode)` returns a typed Byte SAFEARRAY through the shared
  ANSI codec, and `StrConv(byteArray, vbUnicode)` decodes Byte arrays back to
  strings. This follows the documented VBA byte-array conversion shape while
  keeping East Asian width/kana modes as the existing locale boundary.
- Verification completed:
  - `cargo test -p oxvba-differential --test numeric_string_coercion_residuals_vm3`
  - `cargo test -p oxvba-lib sgn_rejects_nan_double`
  - `cargo test -p oxvba-eval`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-differential --test null_coercion_vm3 --test null_string_fns_vm3 --test abs_int_fix_sgn_vm3 --test mid_start_vm3 --test val_incomplete_parse_vm3 --test math_domain_errors_vm3 --test array_introspection_vm3`
  - `cargo test -p oxvba-runtime coerce`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `rustfmt --edition 2024 --check` on touched runtime/eval/lib/vm3 test files
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review re-read the runtime parser move, eval operator changes,
  library string/StrConv changes, vm3 tests, inventory wording, memory evidence,
  and bead graph. The review caught and fixed two issues before closure: `Mid`
  slicing now uses saturating/checked index arithmetic for huge counts, and
  `StrConv(..., vbUnicode)` preserves string-input behavior while adding Byte
  array decoding. No remaining scoped numeric/string residual is open.

## 2026-07-01 - Print # Layout Residuals (`bd-4ktq.39.5`)

- Closed the scoped `print-separators-zones` residuals left after the file-I/O
  data-loss fix: cross-statement print-column continuation after a suppressed
  newline, numeric field sign/trailing padding, and `Spc(n)`/`Tab(n)`/bare
  `Tab` print-clause positioning.
- `Print #` binding now emits both the separator spec and a per-item kind spec,
  so ordinary values and print controls reach the file host without making
  `Spc`/`Tab` general callable intrinsics.
- The standard filesystem adapter now tracks the zero-based formatted-output
  column per handle. `Print #` assembly starts from that persisted column, while
  the verbatim file sink advances or resets it as text and line terminators are
  written.
- Full `oxvba-bind` coverage exposed one stale expectation from the preceding
  Null-coercion closure: `fixed_length_string_store_rejects_null` still expected
  error 13 even though the canonical vm3 row now preserves error 94 for string
  store coercion from `Null`. The test expectation was updated to match that
  closed truth and rerun exactly.
- Verification completed:
  - `cargo test -p oxvba-lib print_record`
  - `cargo test -p oxvba-lib`
  - `cargo test -p oxvba-bind print_hash_binds -- --nocapture`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-bind fixed_length_string_store_rejects_null -- --exact`
  - `cargo test -p oxvba-host --test filesystem_statements print_hash_layout_residuals_match_vba_shape -- --exact`
  - `cargo test -p oxvba-host --test filesystem_statements`
  - `cargo test -p oxvba-hal native_mode_print_line_roundtrips_through_host_file`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `rustfmt --edition 2024 --check` on touched source files
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review re-read the binder control-token lowering, `oxvba-lib`
  column math, standard HAL column bookkeeping, host/regression tests, inventory
  row, and bead graph. The review caught and fixed two issues before closure:
  bare `Tab` now uses its own internal item kind so explicit `Tab(0)` remains an
  invalid-call path, and `Seek #` in Append mode recomputes the formatted-output
  column from EOF rather than the reported seek cursor. No remaining scoped
  `Print #` layout residual is open.

## 2026-07-01 - Width # Output Wrapping (`bd-4ktq.45`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_width_oracle_20260702T0004Z/`.
- Oracle findings:
  - `Width #f, 0` disables wrapping; width values outside `0..=255` raise
    runtime error 5, "Invalid procedure call or argument".
  - Closing and reopening the file resets the width to unwrapped output.
  - `Width #` affects `Print #` but not `Write #`.
  - Ordinary string and numeric fields wrap before the next item if adding that
    item would exceed the active width. A single long field is not split.
  - Cross-statement `Print #` continuation after a trailing semicolon observes
    the persisted file output column and wraps before the next item when needed.
  - Comma print zones and bare `Tab` break to the next line when their next
    14-column zone would exceed the width.
  - `Spc(n)` and explicit `Tab(n)` use modulo-width positioning under an active
    width; for example `Spc(6)` and `Tab(10)` at width 5 yield one and four
    leading spaces respectively.
- Implemented a `print_width_variant` HAL getter beside the existing
  `print_column_variant`, kept `Width #` state per file handle, and enforced the
  VBA `0..=255` range at the standard filesystem boundary with host error code
  5 for out-of-range values.
- `oxvba-lib` now assembles `Print #` records with the active width while
  preserving existing field/control boundaries: value items wrap before
  overflow, comma/bare-Tab zones break rather than padding past the width, and
  `Spc`/explicit `Tab` use the Excel-observed modulo behavior. `Write #` remains
  unchanged.
- Added source-level host tests for the oracle-shaped output and invalid width
  errors, low-level HAL state/range coverage, and `oxvba-lib` unit tests for the
  wrapping primitives.
- Verification completed:
  - `scripts/run-vm3-width-oracle.ps1 -RunId vm3_width_oracle_20260702T0004Z`
  - `cargo check -p oxvba-hal`
  - `cargo check -p oxvba-lib`
  - `cargo check -p oxvba-host`
  - `cargo test -p oxvba-lib print_record --quiet`
  - `cargo test -p oxvba-lib --quiet`
  - `cargo test -p oxvba-host --test filesystem_statements width_hash --quiet`
  - `cargo test -p oxvba-host --test filesystem_statements --quiet`
  - `cargo test -p oxvba-hal native_mode_width_tracks_vba_range_and_state --quiet`
  - `cargo test -p oxvba-hal native_mode_ --quiet`
  - `cargo test -p oxvba-bind random_access_file_statements_bind_and_lower --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `rustfmt --edition 2024 --check crates/oxvba-lib/src/host.rs crates/oxvba-hal/src/traits.rs crates/oxvba-hal/src/adapters/recording.rs crates/oxvba-hal/src/adapters/standard/filesystem.rs`
  - `scripts/check-governance.ps1`
  - `br dep cycles --json`
  - `git diff --check`

## 2026-07-02 - ParamArray Element Caller Aliasing (`bd-4ktq.50`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_call_argument_oracle_bd4ktq50_20260702T0218Z/`.
- Oracle findings:
  - Assigning `xs(0) = 99` inside `Sub Touch(ParamArray xs() As Variant)`
    writes back to caller scalar variables, Variant variables, and array-element
    l-values.
  - Rebinding an object ParamArray element with `Set xs(0) = Nothing` rebinds
    the caller object slot; the probe then raises runtime error 91 when reading
    the caller variable.
  - Mutating an array stored in a Variant ParamArray element (`xs(0)(0) = 99`)
    mutates the caller's Variant-held array payload.
  - This refutes the stale "ByVal isolation" gap wording; the compatibility
    target is caller aliasing for these l-value shapes.
- Implemented ParamArray alias metadata on CoreIR/OxIR `ArrayLiteral`.
  The binder records caller l-value aliases for ParamArray tail arguments while
  preserving forced-ByVal cases such as parenthesized arguments and explicit
  call-site `ByVal`.
- vm3 now tracks ParamArray packs by their resolved storage location, propagates
  alias metadata into callee frames, mirrors ParamArray element writes to caller
  storage, keeps duplicate element aliases in sync, and prunes alias metadata
  when frames are popped/truncated or a whole slot is overwritten.
- OxIR lowering now reuses the existing compound ByRef copy-out path for
  compound ParamArray aliases, so array-element l-values are copied into an
  addressable temp, mirrored during the call, and written back after the call
  only if changed.
- Added active vm3 coverage in
  `crates/oxvba-differential/tests/call_argument_binding_vm3.rs` for scalar,
  Variant, array-element l-value, object rebind, and Variant-held array mutation
  ParamArray caller-aliasing cases.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-call-argument-oracle.ps1 -RunId vm3_call_argument_oracle_bd4ktq50_20260702T0218Z`
  - `cargo check -p oxvba-bundle -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3 --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip paramarray --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1`
  - `rustfmt --edition 2024 --check crates/oxvba-bind/src/call.rs crates/oxvba-bundle/src/coreir.rs crates/oxvba-differential/tests/call_argument_binding_vm3.rs`
  - `git diff --check`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `br dep cycles --json`
- The default formal runner refreshed `docs/evidence/formal/latest_run.*`.
  Remaining Kani skips and unrelated profile/event TODO obligations are
  non-blocking under the current ladder policy and are unchanged by this
  ParamArray bead.
- A broader `rustfmt --check` including the touched OxIR files was attempted
  and still reports pre-existing OxIR-wide formatting drift; no broad formatter
  sweep was mixed into this semantic change.
- Fresh-eyes review caught the initial direct-slot-only implementation and
  extended it to compound l-values using the existing ByRef copy-out mechanism;
  the final implementation removes the legacy copied-element assumption for the
  scoped VBA-observed ParamArray element write-back behavior.

## 2026-07-02 - Fixed-Length String UDT Field Layout (`bd-4ktq.51`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_fixed_string_udt_oracle_bd4ktq51_20260702T0300Z/`.
- Oracle findings:
  - A scalar `String * N` variable still defaults to spaces, but a UDT field
    `Name As String * 5` defaults to five NUL UTF-16 code units; `Len` reports
    5 and `Asc(Mid(field, 1, 1))` reports 0.
  - Assigning a short value to the UDT field pads with spaces; assigning a long
    value truncates to the declared width.
  - `p.Name = Null` raises run-time error 94 (`Invalid use of Null`) and leaves
    the NUL-filled field unchanged.
  - Arrays of UDTs and whole-UDT assignment preserve the same fixed-field
    behavior.
  - `Len` vs `LenB` on `Byte + String * 5 + Integer` reports `8:14`: file
    length counts the fixed string as 5 bytes, while memory layout is packed
    UTF-16 (`1 + 10 + pad + 2`).
- Implemented `ArrayElementType::FixedString` for UDT record-field metadata and
  `VbaRecordFieldKind::FixedString { len }` for inline byte-packed UTF-16
  storage.
- The binder now preserves `String * N` only for UDT record fields (including
  fixed-array field elements) and leaves ordinary array-element fixed-string
  behavior for a separate oracle-backed lane.
- `VbaRecord` defaults fixed-string fields to zeroed inline storage, reads them
  as BSTR strings that may contain embedded NULs, and writes by truncating or
  space-padding UTF-16 code units.
- `Len(record)` and `LenB(record)` now use native record metadata for the scoped
  fixed-string/scalar record shapes; variable-length String and Variant record
  field file lengths remain explicitly unimplemented rather than inferred from
  pointer-sized storage.
- Added active vm3 and runtime coverage in
  `crates/oxvba-differential/tests/fixed_string_default_vm3.rs` and
  `crates/oxvba-runtime/src/vba_record.rs`.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-fixed-string-udt-oracle.ps1 -RunId vm3_fixed_string_udt_oracle_bd4ktq51_20260702T0300Z`
  - `cargo test -p oxvba-differential --test fixed_string_default_vm3 --quiet`
  - `cargo test -p oxvba-runtime vba_record --quiet`
  - `cargo check -p oxvba-runtime -p oxvba-bundle -p oxvba-bind -p oxvba-lib -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-lib --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `br dep cycles --json`
  - `git diff --check`
- The formal runner refreshed `docs/evidence/formal/latest_run.*` in
  non-blocking mode. Kani remains deferred to WSL async, and unrelated
  historical TODO obligations remain non-blocking.

## 2026-07-02 - Compatibility Objective Reinforcement

- User clarified that the goal should always be to match real VBA compile-time
  and run-time behavior.
- Recorded this as durable guidance in `CHARTER.md`, `OPERATIONS.md`, and
  `AGENTS.md`: existing OxVBA behavior, legacy fallbacks, and internal
  convenience paths are not compatibility targets except as explicitly tracked
  temporary gaps on the way to VBA parity.

## 2026-07-02 - Project Class NewEnum For Each (`bd-4ktq.49`)

- Implemented vm3 `For Each` over project-class instances that carry a
  `VB_UserMemId = -4` enumerator member. The scanner now recognizes exact
  `VB_UserMemId` values for both default members (`0`) and NewEnum members
  (`-4`), the binder/OxIR class-method metadata preserves the NewEnum bit, and
  vm3 invokes the marked property/method to obtain the enumeration source.
- Added native `VBA.Collection.[_NewEnum]` support for the internal enumerator
  path used by project-class NewEnum properties, plus parser support for dotted
  bracketed member names such as `items.[_NewEnum]`.
- Changed plain project instances without a NewEnum member from silent empty
  iteration to VBA-style runtime error 438.
- Added regressions:
  - `crates/oxvba-differential/tests/project_class_newenum_vm3.rs`
  - `crates/oxvba-bind/tests/bind_roundtrip.rs::project_newenum_attribute_marks_enumerator_member`
  - `crates/oxvba-bind/tests/bind_roundtrip.rs::project_newenum_attribute_requires_exact_minus_four_memid`
  - `crates/oxvba-syntax::parser::tests::expr_dot_bracketed_member_name`
- Verification completed:
  - `cargo check -p oxvba-syntax -p oxvba-symbol -p oxvba-bind -p oxvba-bundle -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-bind --test bind_roundtrip project_newenum_attribute --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip project_default_member --quiet`
  - `cargo test -p oxvba-differential --test project_class_newenum_vm3 --quiet`
  - `cargo test -p oxvba-differential --test foreach_scalar_source_vm3 --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-bundle vba_bundle_exports_collection_with_native_methods --quiet`
  - `cargo test -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential -p oxvba-symbol -p oxvba-syntax --tests --no-run`
  - `rustfmt --edition 2024 --check` on the locally formatter-clean touched
    source/test files, excluding `crates/oxvba-oxir/src/elaborate/lower.rs`
    and `crates/oxvba-vm3/src/lib.rs` because broad rustfmt on those files
    still includes pre-existing unrelated drift.
  - `scripts/check-governance.ps1`
  - `br dep cycles --json`
  - `git diff --check`
- Fresh-eyes review reverted formatter-only churn in the lowerer/vm3 files,
  rechecked exact `VB_UserMemId = -4` parsing, and confirmed the previous
  silent-empty project-class iteration fallback is gone in favor of runtime 438.
- Known check caveat: broad rustfmt checks still surface the pre-existing
  repo-wide formatting drift tracked in `bd-4ktq.58`, so this bead used the
  targeted rustfmt lane above for files that do not pull in unrelated module
  drift.
- Fresh-eyes review re-read the oracle table, wrapping code, HAL range/state
  changes, host tests, inventory wording, and bead acceptance text. The review
  caught one omission before closure: close/reopen reset behavior was in the
  bead acceptance text but not yet proved, so the oracle, host regression, and
  docs were expanded to cover it. No remaining scoped Width # issue is open.

## 2026-07-02 - Headless Interaction And Shell Policy Residuals (`bd-4ktq.46`)

- Captured live Excel/VBA 7.1 behavior for `Shell` with VBE Debug -> Compile
  and PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_headless_interaction_oracle_20260702T0015Z/`.
- Oracle finding: `Shell("cmd /c ping -n 3 ...", vbHide)` returns before the
  delayed child process exits, yields a positive task id, and the returned
  Variant subtype is `Double` (`VarType=5`); the captured elapsed time was about
  0.016 seconds.
- Native host-backed `Shell` no longer waits for the child process. It returns
  the spawned process id immediately as a Variant/Double, so the product target
  remains real VBA compile/runtime behavior rather than legacy OxVBA behavior.
- Non-native deterministic process policy remains a host boundary token for
  environments that cannot or must not spawn processes; it is not documented as
  Excel/VBA parity.
- `Debug.Assert` in headless vm3 evaluates its condition expression, including
  side effects, and does not print or break. A false assertion in Excel/VBE is
  an IDE debugger break-state boundary, not a headless runtime modal, so OxVBA
  does not fake that UI state.
- Added source-level host and HAL regressions for async native `Shell` timing
  and the Variant/Double return shape, plus a `Debug.Assert` side-effect
  regression.
- Verification completed:
  - `scripts/run-vm3-headless-interaction-oracle.ps1 -RunId vm3_headless_interaction_oracle_20260702T0015Z`
  - `cargo check -p oxvba-hal`
  - `cargo check -p oxvba-host`
  - `cargo test -p oxvba-host --test debug_and_console_print --quiet`
  - `cargo test -p oxvba-host --test process_statements --quiet`
  - `cargo test -p oxvba-hal native_mode_ --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `rustfmt --edition 2024 --check --config skip_children=true crates/oxvba-hal/src/adapters/standard/mod.rs crates/oxvba-hal/src/adapters/standard/process.rs crates/oxvba-host/tests/process_statements.rs crates/oxvba-host/tests/debug_and_console_print.rs crates/oxvba-hal/src/conformance.rs`
  - PowerShell parser check for
    `scripts/run-vm3-headless-interaction-oracle.ps1`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Known check caveat: broad rustfmt over the `standard` module still surfaces
  pre-existing drift in sibling `com.rs`; this bead used `skip_children=true`
  plus targeted touched-file checks to avoid mixing unrelated formatting into a
  runtime behavior change.
- Fresh-eyes review checked the native Shell implementation, async host/HAL
  tests, Debug.Assert side-effect test, Excel oracle harness/evidence, and
  inventory wording against the rule that VBA compile/runtime behavior is the
  compatibility target. The review caught one important issue before closure:
  native `Shell` had been made asynchronous but still returned a `Long`; the
  Excel oracle shows `VarType=5`, so the native path and regressions were
  corrected to return a Variant/Double task id.

## 2026-07-02 - Conditional Compilation And Library Import Guard (`bd-4ktq.47`)

- Replaced the unconditional Windows x64 conditional-compilation table with an
  explicit target model carrying host, pointer width, and VBA7 facts.
- Host source and manifest execution now carry a conditional-compilation
  target alongside project `DefineConstants` before binding. Explicit project
  `DefineConstants` still win, so project policy can override defaults
  deliberately.
- Referenced projects use the same target predefines without inheriting
  active-project custom `DefineConstants`, preserving project boundary behavior
  while avoiding hardwired Windows x64 constants.
- Mac target selection is covered by host-level execution tests; Windows target
  behavior remains the active VBA 7.1 Windows default.
- The vm3 `resolve_library_import` helper no longer keeps the dead non-`VBA`
  fallback. Cross-project imports are resolved by `call_extern` before that
  helper is reached, and a VBA-library export without a native body is now
  reported as malformed library metadata rather than a cross-project link gap.
- Verification completed:
  - `cargo check -p oxvba-symbol -p oxvba-host -p oxvba-bind -p oxvba-differential -p oxvba-project -p oxvba-vm3`
  - `cargo test -p oxvba-symbol cond_comp --quiet`
  - `cargo test -p oxvba-host --test conditional_compilation --quiet`
  - `cargo test -p oxvba-bind --test feature_coverage conditional_compilation --quiet`
  - `cargo test -p oxvba-bind --test cross_project conditional_compilation --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-bind --tests --no-run`
  - `cargo test -p oxvba-host --tests --no-run`
  - `cargo test -p oxvba-differential --tests --no-run`
  - `cargo test -p oxvba-symbol --tests --no-run`
  - `rustfmt --edition 2024 --check crates/oxvba-symbol/src/cond_comp.rs crates/oxvba-symbol/src/provider.rs crates/oxvba-symbol/src/manifest.rs crates/oxvba-host/tests/conditional_compilation.rs crates/oxvba-project/src/closure.rs`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
  - Fresh-eyes review rechecked the target-vs-DefineConstants split, referenced
    project constant isolation, the vm3 import helper guard, manifest-field
    repair sites, docs, and staged scope.

## 2026-07-02 - Object Default-Member Indexed Get/Set (`bd-4ktq.48`)

- Replaced the untyped-object `obj(index)` lowering shortcut that guessed a
  literal `"Item"` member with an explicit CoreIR/OxIR default-member dispatch
  selector.
- vm3 now dispatches that selector through project-class `VB_UserMemId = 0`
  metadata, built-in `Collection.Item`, or COM `DISPID_VALUE` as appropriate.
- `Variant`-held object values that reach the array-index fallback now invoke the
  object's default member instead of reporting unsupported object indexing.
  Scalar RHS values route as `PropertyLet`; object RHS values route as
  `PropertySet`.
- Project-class default-member calls share the existing project dispatch frame
  runner and now reorder named arguments by callee parameter name for the covered
  indexed property shape.
- Verification completed:
  - `cargo check -p oxvba-bundle -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-host -p oxvba-differential`
  - `cargo test -p oxvba-bind --test bind_roundtrip default_member --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip late_bound_object_index --quiet`
  - `cargo test -p oxvba-oxir late_bound_com_call_lowers_to_com_call_late --quiet`
  - `cargo test -p oxvba-vm3 --test cross_program --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --test default_member_index_vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-host --test com_matrix_properties --quiet`
    (compiled; live COM cases ignored in this environment)
  - `cargo test -p oxvba-bind --tests --no-run`
  - `cargo test -p oxvba-oxir --tests --no-run`
  - `cargo test -p oxvba-vm3 --tests --no-run`
  - `cargo test -p oxvba-differential --tests --no-run`
  - `rustfmt --edition 2024 --check crates/oxvba-bundle/src/coreir.rs crates/oxvba-bind/src/call.rs crates/oxvba-bind/src/expr.rs crates/oxvba-bind/tests/bind_roundtrip.rs crates/oxvba-vm3/tests/cross_program.rs crates/oxvba-differential/tests/default_member_index_vm3.rs`
  - Broad `rustfmt --check` over `oxvba-oxir` and `oxvba-vm3/src/lib.rs`
    still reports pre-existing repository formatting drift; this bead kept
    formatting changes scoped and manually inspected the touched OxIR/vm3 hunks.
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review rechecked the selector modeling, literal-`Item` removal,
  Set/Let routing for Variant-held object fallback, named-argument reorder path,
  docs, and staged-scope boundaries.

## 2026-07-01 - Statement Parser/Error-Model Slice (`bd-4ktq.39.6`)

- Closed the focused statement/parser residual subset:
  `next-multivariable-unsupported`, `line-number-labels-no-colon`,
  `sub-in-expression-accepted`, and `exit-do-in-while-accepted`.
- Parser support now treats colonless numeric line labels as label statements
  that can prefix a same-line statement, and `Next j, i` closes the nested
  source `For` statements while rejecting a `Next` name list longer than the
  open loop stack.
- Binder support now rejects a project `Sub` used as a value-producing
  expression with stable diagnostic code
  `BIND-E-EXPECTED-FUNCTION-OR-VARIABLE`. Statement-position calls, including
  module-qualified and member method calls, remain valid.
- The binder now tracks source-level loop kinds so `Exit Do` inside
  `While/Wend` is rejected even though `While` lowers to the same vm3 `DoLoop`
  shape as a real `Do` loop.
- Remaining scoped rows were split into explicit delivery beads rather than
  claimed here:
  `bd-4ktq.40` (`lset-rset-unrecognized`), `bd-4ktq.41`
  (`redim-undeclared-rejected`), `bd-4ktq.42` (`erl-absent` and
  `on-error-undefined-label-malformed`), `bd-4ktq.43`
  (`on-goto-out-of-range-no-5`), `bd-4ktq.44`
  (`err-helpfile-helpcontext-dropped`), `bd-4ktq.45`
  (`width-statement-no-wrap`), `bd-4ktq.46`
  (`debug-assert-no-break` and `shell-blocks-until-exit`), and
  `bd-4ktq.47` (`cc-constants-hardwired-64bit` and
  `resolve-library-import-dead-guard`).
- Golden drift was audited and re-blessed only for
  `conformance/tests/goto_line_number_statement_basic.bas`, which now matches
  the existing oracle/value evidence by returning `5` instead of parse-failing.
- Verification completed:
  - `cargo test -p oxvba-syntax`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `rustfmt --edition 2024 --check` on touched parser/binder Rust files
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Fresh-eyes review re-read the parser `Next` accounting, statement/value call
  split, source loop-kind validation, new tests, inventory row state, split-bead
  descriptions, and the golden line drift. The review caught and fixed two
  issues before closure: PowerShell-mangled follow-up bead descriptions were
  rewritten cleanly, and `Exit Do` validation was tightened so a nested
  `While/Wend` lowered as a `DoLoop` cannot become the runtime break target for
  a source `Exit Do`.

## 2026-07-01 - Object/Array Residual Triage (`bd-4ktq.39.7`)

- Closed this support bead by splitting every scoped accepted object/array or
  lifecycle residual into explicit epic-level delivery beads, rather than
  changing risky object semantics without a focused oracle/test slice.
- Follow-up delivery paths:
  `bd-4ktq.48` (`object-default-member-index-get` /
  `object-default-member-index-set`), `bd-4ktq.49`
  (`foreach-project-class-no-newenum`), `bd-4ktq.50`
  (`paramarray-elements-byval`; later closed as ParamArray caller aliasing),
  `bd-4ktq.51` (`fixed-string-udt-field-layout`; closed 2026-07-02),
  `bd-4ktq.52`
  (`array-byval-accepted-lost` / `array-assign-into-fixed-lhs`),
  `bd-4ktq.53` (`isarray-unallocated-false`), `bd-4ktq.54`
  (`raiseevent-fan-out-order`), `bd-4ktq.55`
  (`predeclared-singleton-no-resurrection`), and `bd-4ktq.56`
  (`class-terminate-not-synchronous` / `dim-as-new-no-resurrection`).
- Inventory rows now point to those beads explicitly, so the first-wave terminal
  reconciliation can reason over an open delivery path instead of prose-only
  residual text.
- Verification completed:
  - `br dep cycles --json`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - inventory/search audit for every scoped residual row
- Fresh-eyes review re-read the new bead descriptions, inventory split markers,
  memory entry, and bead graph. No scoped object/array/lifecycle residual remains
  only as narrative text, and risky lifecycle rows remain delivery-gated on
  focused oracle evidence rather than being closed by support triage.

## 2026-07-01 - Tier 4/5 First-Wave Reconciliation (`bd-4ktq.39.8`)

- Reconciled the first-wave Tier 4/5 correctness batch (`bd-4ktq.39`):
  children `bd-4ktq.39.1` through `bd-4ktq.39.7` are closed, with accepted
  residuals either fixed in focused delivery beads or split into explicit
  epic-level follow-up beads.
- The inventory and memory now reflect the same truth:
  `for-start-step-not-coerced`, date/time rows, numeric/string rows, `Print #`
  layout, and the focused statement/parser rows are marked done with evidence;
  the remaining statement/object/array/headless/frontend rows are linked to
  `bd-4ktq.40` through `bd-4ktq.56`.
- Removed the broad `bd-us4v` dependency on `bd-4ktq`. The remaining
  UDT-record-field array performance issue is correctness-neutral and is now
  unblocked after the scoped correctness residuals were fixed or split.
- Verification completed:
  - `br dep cycles --json`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - bead graph/status review for `bd-4ktq.39`, `bd-4ktq.39.8`, and `bd-us4v`
- Fresh-eyes review checked for unsupported completion language, stale
  `bd-us4v` blocked wording, prose-only residual rows, and missing delivery
  paths. The stale performance-row block text was corrected before closure.

## 2026-07-01 - Dynamic Array Access Performance Closure (`bd-us4v`)

- Closed the remaining vm3 array-loop performance residual after the Tier 4/5
  correctness split unblocked `bd-us4v`.
- Added fused `RecordArrayGet` / `RecordArraySet` lowering and vm3 execution for
  `rec.arr(i)` where `arr` is a UDT fixed-array field, so reads/writes borrow the
  `VbaRecord` payload and touch only one inline element instead of materializing
  the whole fixed-array field through `RecordGet`.
- Added borrowed `VbaRecord`/`Variant` helpers for record array-field bounds and
  element read/write. The fused path is native-`VbaRecord` only; SAFEARRAY-backed
  record bags are not retained as a compatibility route. Indexed scalar UDT fields
  are rejected by the binder as `Expected array`, matching the Excel/VBE compile
  oracle.
- Evidence:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-record-array-field-oracle.ps1 -RunId vm3_record_array_field_oracle_20260701T2135Z`
  - `cargo test -p oxvba-oxir record_array_fields_elaborate_to_fused_ops --quiet`
  - `cargo check -p oxvba-vm3`
  - `cargo test -p oxvba-differential --test record_array_access_vm3 --quiet`
  - `cargo test -p oxvba-bind module_udt_scalar_field_index --quiet`
  - `cargo test -p oxvba-differential --test array_access_perf_vm3 --quiet`
  - `cargo test -p oxvba-differential --test field_array_access_vm3 --quiet`
  - `cargo test -p oxvba-differential --test fixed_array_erase_vm3 --quiet`
  - `cargo test -p oxvba-differential --test array_perf_diagnose -- --ignored --nocapture`
- Final diagnostic rows were flat for module, class-field, and UDT-field arrays:
  module ~12.0 -> 9.5 us/elem, class ~12.3 -> 10.6 us/elem, UDT ~10.4 ->
  8.9 us/elem across N=250,500,1000.
- Fresh-eyes review checked that the change is scoped to element-level access,
  removes the legacy record-bag fallback, matches VBA's `Expected array` compile
  behavior for scalar field indexing, does not change fixed-array erase semantics,
  and updates the handover/inventory away from stale "UDT remains" wording.

## 2026-07-01 - LSet/RSet String Alignment (`bd-4ktq.40`)

- Captured live Excel/VBA 7.1 behavior with PID-scoped VBE/UI Automation modal
  handling in
  `docs/evidence/conformance/vm3_lset_rset_oracle_20260701T215755Z/`.
  Important results:
  - `LSet` fixed/variable strings left-align and right-pad to the target width.
  - `RSet` fixed/variable strings right-align and left-pad to the target width.
  - Variable-length string width is the target's current length: an empty target
    stays empty, and a pre-sized `"....."` target remains length 5.
  - Overlong RHS text truncates to the leftmost target-width UTF-16 units for
    both `LSet` and `RSet`.
  - `Null` RHS raises runtime error 94, and non-string targets produce the real
    compile messages: `LSet allowed only on strings and user-defined types` /
    `RSet allowed only on strings`.
  - Excel accepts UDT `LSet a = b` record copy; that broader record-layout path
    is split to `bd-4ktq.57` rather than being claimed by the string bead.
- Implemented explicit `KwLSet`/`KwRSet` parsing into `LSetStmt`/`RSetStmt`.
  Binder lowers string targets to name-less native statement bodies
  `NativeImplId::LSetStmt` / `RSetStmt`, passing the target's current value plus
  the RHS so vm3 computes dynamic-width alignment. UDT `LSet` is now an explicit
  unsupported split, not a silent fallback.
- Added vm3 coverage in `crates/oxvba-differential/tests/lset_rset_vm3.rs` and
  binder syntax/diagnostic coverage in `oxvba-syntax` / `oxvba-bind`.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-lset-rset-oracle.ps1`
  - `cargo test -p oxvba-syntax lset_rset_statements_are_structured_assignments --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip lset --quiet`
  - `cargo test -p oxvba-bundle library_member_covers_exactly_the_migrated_ids --quiet`
  - `cargo test -p oxvba-differential --test lset_rset_vm3 --quiet`

## 2026-07-01 - ReDim Implicit Declaration (`bd-4ktq.41`)

- Captured live Excel/VBA 7.1 behavior with the VBE Debug -> Compile path and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_redim_implicit_oracle_20260701T2238Z/`.
- Oracle findings:
  - `ReDim a(1)` declares an otherwise undeclared dynamic Variant array even
    when `Option Explicit` is present.
  - The resulting array has `VarType(a) = 8204` (`vbArray + vbVariant`);
    a small integer literal stored in an element has `VarType(a(0)) = 2`.
  - An explicit `Dim a() As Long` target stays a Long array
    (`VarType(a) = 8195`, `VarType(a(0)) = 3`).
  - `ReDim Preserve a(1)` does not introduce an undeclared name, with or
    without `Option Explicit`; Excel raises compile-time
    `Variable not defined`.
  - `ReDim` on a scalar `Long` target raises compile-time `Expected array`.
- Implemented a narrow scanner pass that adds implicit local dynamic
  `Variant()` symbols for simple non-`Preserve` `ReDim` targets after explicit
  procedure declarations have been scanned, so later/hoisted `Dim` declarations
  still win and dotted/member targets are not invented.
- Binder validation now rejects scalar simple `ReDim` targets with
  `Expected array`, while preserving Variant and declared dynamic-array targets.
- Added vm3 coverage in
  `crates/oxvba-differential/tests/redim_implicit_vm3.rs` plus binder
  diagnostics in `crates/oxvba-bind/tests/bind_roundtrip.rs`.
- Verification completed:
  - Excel oracle artifacts captured in
    `docs/evidence/conformance/vm3_redim_implicit_oracle_20260701T2238Z/`
    with clean script exit. The older pre-existing Excel process was left
    untouched.
  - `cargo test -p oxvba-bind --test bind_roundtrip redim_ --quiet`
  - `cargo test -p oxvba-differential --test redim_implicit_vm3 --quiet`
  - `cargo test -p oxvba-symbol --quiet`
  - `cargo test -p oxvba-bind --quiet`
  - `cargo test -p oxvba-differential --test redim_negative_lower_vm3 --quiet`
  - `cargo test -p oxvba-differential --test fixed_array_erase_vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`

## 2026-07-01 - Erl And Line-Numbered Error Flow (`bd-4ktq.42`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_erl_line_oracle_20260701T2318Z/`.
- Oracle findings:
  - `Erl` returns a `Long` (`VarType(Erl)=3`) and is 0 before any trapped
    error, even after numeric line labels execute without an error.
  - A trapped error on a numbered statement records that numeric line in
    `Erl` for both `On Error Resume Next` and `On Error GoTo`.
  - An unnumbered fault after a numeric label uses the prior numeric line; a
    colon-only numeric label also becomes the active line for following
    unnumbered statements.
  - If a caller's handler catches a callee fault, `Erl` reports the caller's
    call-site line state, not the callee's source line; the unnumbered caller
    oracle returns `0`.
  - `On Error GoTo MissingHandler` is a compile error with modal text
    `Label not defined`; the VBE selected line is the `On Error GoTo` line.
- Implemented `Erl` as a VM-aware special expression (`CoreValue::Erl` /
  `OxInst::ErlGet`) rather than an ordinary pure library function.
- Numeric label metadata is carried on `CoreProc::label_lines`; OxIR emits
  `SetLineNumber` when a numeric label statement executes, and vm3 copies the
  current frame line into the public `Erl` value only when an error is caught.
- Binder label-reference validation now runs after binding each procedure body,
  so missing `GoTo`/`GoSub`/`On Error GoTo`/`Resume` targets raise the VBA-style
  `Label not defined` bind diagnostic before elaboration.
- Added source-level vm3 regressions in
  `crates/oxvba-bind/tests/bind_roundtrip.rs` plus symbol-provider coverage for
  the `Erl` special-form route.
- Verification completed:
  - `scripts/run-vm3-erl-line-oracle.ps1 -RunId vm3_erl_line_oracle_20260701T2318Z`
  - `cargo test -p oxvba-bind --test bind_roundtrip erl_ --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip label --quiet`
  - `cargo test -p oxvba-vm3 --test cross_program --quiet`
  - `cargo test -p oxvba-symbol library_resolves_constants_intrinsics_structural_and_special_forms --quiet`
  - `cargo test -p oxvba-bind --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `OXVBA_BLESS_GOLDEN=1 cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- `scripts/meta-check.ps1 -Fast -NoArtifacts` was attempted and reached
  `cargo fmt --all --check`, which fails on repo-wide pre-existing formatting
  drift outside this Erl/label parity bead. Tracked separately as
  `bd-4ktq.58`; no broad formatter sweep was mixed into this semantic change.
- Fresh-eyes review checked the binder, Core IR, OxIR, vm3 runtime line state,
  Excel oracle harness, golden drift, and docs. No legacy OxVBA fallback was
  retained for undefined labels; user-visible behavior is the VBA compile/runtime
  behavior captured from Excel.

## 2026-07-01 - Computed On n GoTo/GoSub Selectors (`bd-4ktq.43`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_on_computed_branch_oracle_20260701T2321Z/`.
- Oracle findings:
  - `On 1 GoTo` selects the first target; `On 2 GoSub` calls the second target
    and `Return` resumes after the `On ... GoSub` statement.
  - Selector `0` falls through for both `GoTo` and `GoSub`.
  - Selector values beyond the target list also fall through; they do not raise
    error 5. This corrected the original gap label.
  - Negative selectors raise trappable runtime error 5 for both `GoTo` and
    `GoSub`.
  - Fractional selectors are coerced through VBA Long rounding before branch
    selection: `1.5` selects target 2, and `2.5` rounds to 2 rather than falling
    past a two-target list.
  - Nonnumeric string selectors raise error 13; `Null` selectors raise error 94.
- Implemented the vm3 parity fix in OxIR lowering: after selector coercion to
  `Long`, a single `selector < 0` branch raises normal runtime error 5 through
  the statement fault pad. Zero and out-of-range selectors intentionally keep the
  fallthrough path.
- Added source-level vm3 regressions in
  `crates/oxvba-bind/tests/bind_roundtrip.rs` for computed `GoTo` and `GoSub`
  in-range, zero, negative, out-of-range, fractional, string, `Null`, and GoSub
  return-stack behavior.
- Verification completed:
  - `scripts/run-vm3-on-computed-branch-oracle.ps1 -RunId vm3_on_computed_branch_oracle_20260701T2321Z -CaseId ...`
  - `cargo test -p oxvba-bind --test bind_roundtrip computed_ --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `cargo test -p oxvba-bind --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`

## 2026-07-01 - Err HelpFile And HelpContext (`bd-4ktq.44`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_err_help_oracle_20260701T2332Z/`.
- Oracle findings:
  - Initial `Err.HelpFile` is `""`, `Err.HelpContext` is `0`, and `Err.Clear`
    resets both fields.
  - `Err.HelpFile = ...` and `Err.HelpContext = ...` are writable and readable.
  - `Error 9` populates the VBA help file path
    `C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm`
    and help context `1000009`.
  - `Err.Raise` accepts positional and named `HelpFile`/`HelpContext`.
  - Omitted `Err.Raise` fields inherit from the current `Err` state when it is
    inheritable (after a caught `Err.Raise` or direct Err property writes under
    an active handler). `On Error GoTo ...` clears `Err`, so it is not an
    inheritance-preserving setup step.
  - For generic unmapped errors, the default help context observed from Excel is
    `1000095`; an explicit `HelpFile` with omitted `HelpContext` still inherits
    the context when the current `Err` state is inheritable.
- Implemented `ErrField::HelpFile`/`HelpContext` across Core IR, binder, OxIR,
  and vm3. `Fault` now carries optional help metadata; vm3 defaults missing
  help fields at raise time and stores them in `ErrState`.
- Added source-level vm3 regressions in
  `crates/oxvba-bind/tests/bind_roundtrip.rs` for help reads/writes, `Clear`,
  default `Error` help metadata, explicit/named `Err.Raise` help fields, and
  omitted-field inheritance. Fresh-eyes review added an explicit `Err.Clear`
  then omitted `Err.Raise` case to pin VBA defaulting after inheritance is
  cleared.
- Re-blessed `crates/oxvba-differential/vm3_golden.snap` because existing
  full-surface Err conformance fixtures now bind and run instead of failing on
  unsupported `Err.HelpFile`/`Err.HelpContext`.
- Verification completed:
  - `scripts/run-vm3-err-help-oracle.ps1 -RunId vm3_err_help_oracle_20260701T2332Z`
  - `cargo check -p oxvba-bind`
  - `cargo check -p oxvba-oxir`
  - `cargo check -p oxvba-vm3`
  - `cargo test -p oxvba-bind --test bind_roundtrip err_ --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `cargo test -p oxvba-vm3 err_ --quiet`
  - `OXVBA_BLESS_GOLDEN=1 cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-bind --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `rustfmt --edition 2024 --check crates/oxvba-bind/tests/bind_roundtrip.rs`
  - `scripts/check-governance.ps1`
  - `br dep cycles --json`
  - `git diff --check`

## 2026-07-02 - Array ByVal/Copy And Fixed-Lhs Assignment (`bd-4ktq.52`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_array_copy_assignment_oracle_20260702T025158Z/`.
- Oracle findings:
  - `Private Sub Touch(ByVal a() As Long)` raises compile error
    `Array argument must be ByRef`.
  - A typed array passed to `ByVal v As Variant` is copied before element
    mutation; caller values and bounds remain unchanged.
  - A typed array passed to `ByRef v As Variant` aliases caller storage and
    element mutation writes through.
  - Whole-array assignment between dynamic arrays copies values and bounds, and
    the copy stays independent across later `ReDim Preserve` on the source.
  - Dynamic lhs from fixed rhs is legal and copies values/bounds.
  - Fixed-size array lhs whole assignment from either dynamic or fixed rhs raises
    compile error `Can't assign to array`.
- Implemented parser support for parameter array markers (`a() As T`) so
  signatures carry array type information instead of falling back to Variant.
- Top-level fixed-size array declarators now carry `VarTypeRef::FixedArray`,
  enabling the binder to reject fixed-lhs whole-array assignment at compile time
  while preserving legal fixed-array element access and ReDim/Erase allocation
  behavior.
- Added binder diagnostics `ArrayArgumentMustBeByRef` and `CantAssignToArray`
  with VBA-shaped diagnostic messages. ByRef compatibility canonicalizes
  fixed-array actuals to ordinary array shape, preserving legal fixed-array calls.
- Fresh-eyes review tightened the regression checks to assert the VBA-shaped
  diagnostic text and changed vm3 host bind-failure formatting from Rust debug
  variants to `Display`, so rejected compile shapes surface messages like
  `Array argument must be ByRef` and `Can't assign to array`.
- Added `crates/oxvba-differential/tests/array_copy_assignment_vm3.rs` for the
  oracle-backed dynamic/fixed carriers, ByVal/ByRef Variant array behavior,
  dynamic copy independence, dynamic-from-fixed assignment, and invalid fixed-lhs
  diagnostics. Updated the existing ParamArray baseline to expect a typed Long
  return now that `ParamArray xs() As Variant` is parsed as an array parameter.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-array-copy-assignment-oracle.ps1`
  - `cargo test -p oxvba-syntax typed_accessor_params_from_paramlist --quiet`
  - `cargo test -p oxvba-syntax --quiet`
  - `cargo test -p oxvba-symbol --quiet`
  - `cargo test -p oxvba-bind --quiet`
  - `cargo check -p oxvba-syntax -p oxvba-symbol -p oxvba-bind -p oxvba-differential`
  - `cargo test -p oxvba-differential --test array_copy_assignment_vm3 --quiet`
  - `cargo test -p oxvba-differential --test call_argument_binding_vm3 --quiet`
  - `cargo test -p oxvba-differential --test fixed_array_erase_vm3 --quiet`
  - `cargo test -p oxvba-differential --test redim_implicit_vm3 --quiet`
  - `cargo test -p oxvba-differential --test array_bounds_unallocated_vm3 --quiet`
  - `cargo test -p oxvba-differential --test array_introspection_vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo check -p oxvba-host --quiet`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet`
    (current non-blocking formal evidence: 304 pass, 16 skipped Kani
    obligations, 63 todo; legacy `oxvba-compiler` obligations remain invalid
    because that package is not present, and sampled host obligations pass when
    invoked directly).
- Broader `cargo test -p oxvba-differential --quiet` was also attempted; it
  still fails in the existing `vm3_runs_collection_methods` lib test because the
  snapshot contains `Item(1)=30` as `Integer` rather than the test's expected
  `Long`. This collection subtype expectation is outside the array ByVal/fixed-lhs
  scope and was not mixed into this bead.

## 2026-07-02 - IsArray Unallocated Dynamic Array Parity (`bd-4ktq.53`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_isarray_unallocated_oracle_20260702T040452Z/`.
- Oracle findings:
  - Declared dynamic arrays return `IsArray=True` before `ReDim`, after
    allocation, and after `Erase`.
  - Fixed arrays return `IsArray=True` before and after `Erase`.
  - Empty Variant returns `False`.
  - Variant-held `Array(...)`, allocated dynamic arrays, copied unallocated
    dynamic arrays, and erased Variant arrays return `True`.
  - `LBound`/`UBound` on never-allocated dynamic arrays, erased dynamic arrays,
    and erased Variant-held arrays raise run-time error 9.
  - Typed unallocated `Long()` arrays report `VarType=8195` and
    `TypeName=Long()`, including after `Erase` and when copied into a Variant;
    erased `Array(...)` Variants report `8204`/`Variant()`.
- Implemented vm3 slot initialization for statically array-typed locals/globals
  as a typed null SAFEARRAY array marker, not `Empty`, so array identity and
  element introspection match VBA before allocation and survive copying through
  Variant.
- Changed dynamic-array erasure of an already-array value to return to the same
  typed null array marker. Fixed-size arrays still rebuild/reset their
  SAFEARRAY, and ordinary Empty Variants still erase to Empty.
- The internal marker projects as `VT_ARRAY | element_vartype` with zero
  reserved words in `Variant::to_wire_bytes`, so marker metadata does not leak
  through the COM-compatible wire layout.
- Added `crates/oxvba-differential/tests/isarray_unallocated_vm3.rs` for the
  oracle matrix. Updated `array_bounds_unallocated_vm3` proves the marker does
  not give unallocated arrays fake bounds: `LBound`/`UBound` raise the
  Excel/VBA run-time error 9.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-isarray-unallocated-oracle.ps1`
  - `cargo test -p oxvba-differential --test isarray_unallocated_vm3 --quiet`
  - `cargo test -p oxvba-differential --test array_bounds_unallocated_vm3 --quiet`
  - `cargo test -p oxvba-differential --test array_introspection_vm3 --quiet`
  - `cargo test -p oxvba-differential --test fixed_array_erase_vm3 --quiet`
  - `cargo test -p oxvba-runtime --quiet`
  - `cargo test -p oxvba-lib --quiet`
  - `cargo check -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet`
    (completed non-blocking refresh: 304 pass, 16 skipped Kani obligations,
    63 todo; a first shorter runner timeout was discarded and rerun with a
    longer ceiling).
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `rustfmt --edition 2024 --check crates/oxvba-runtime/src/variant.rs crates/oxvba-lib/src/pure.rs crates/oxvba-differential/tests/array_bounds_unallocated_vm3.rs crates/oxvba-differential/tests/isarray_unallocated_vm3.rs`
  - `git diff --check`
  - `br dep cycles --json`
- Non-blocking formatter note: `cargo fmt --all -- --check` still reports the
  repo-wide rustfmt backlog tracked by the formatter support lane. Direct
  `rustfmt --edition 2024 --check` passed for the touched runtime/lib/test
  files that are formatter-clean. Including `crates/oxvba-vm3/src/lib.rs`
  still reports pre-existing formatter drift outside this bead's edits, so no
  broad vm3 reformat was mixed into this semantic change.
- Verification note: vm3 golden also needed the diagnostic-display refresh from
  the immediately preceding array-copy bead (`Unresolved { ... }`/`Unsupported`
  debug text to user-facing `Display` text). That snapshot refresh is
  diagnostics-only and independent of the `IsArray` slot-marker behavior.

## 2026-07-02 - RaiseEvent Fan-Out Subscription Order (`bd-4ktq.54`)

- Captured live Excel/VBA 7.1 behavior with VBE Debug -> Compile and
  PID-scoped UI Automation modal handling in
  `docs/evidence/conformance/vm3_raiseevent_fanout_oracle_20260702T043855Z/`.
  A failed intermediate harness run proved the modal capture path: the VBE
  selected an illegal helper identifier and reported `Compile error: Syntax
  error`; the helper was renamed and the failed evidence run was discarded.
- Oracle findings:
  - Project-source `RaiseEvent` dispatches `WithEvents` handlers in current
    subscription order.
  - Dispatch order is not declaration order, object creation order, or sink
    identity order.
  - Handler writes to ByRef event parameters are synchronous: each later handler
    sees earlier mutations, and the raiser sees the final value.
  - Rebinding an existing `WithEvents` field, even to the same source, moves that
    subscription to the end.
  - Clearing and rewiring moves the subscription to the end.
  - Reassigning a field to a different source detaches the old source.
- vm3 now stores a monotonic subscription sequence on each live `WithEvents`
  binding. `WithEventsSet` refreshes that sequence for every non-`Nothing`
  assignment after tearing down the old host subscription state. Project-source
  `RaiseEvent` fan-out and the owner-iterator helper sort by this sequence.
- Added `crates/oxvba-differential/tests/raiseevent_fanout_vm3.rs` to pin
  subscription-order fan-out, owner-identity counterexamples, rebinding/clear
  movement, old-source detach, and ByRef writeback order.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-raiseevent-fanout-oracle.ps1`
  - `cargo test -p oxvba-differential --test raiseevent_fanout_vm3 --quiet`
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_project_event_fires_handler --quiet`
  - `cargo check -p oxvba-vm3 -p oxvba-differential`
  - `rustfmt --edition 2024 --check crates/oxvba-differential/tests/raiseevent_fanout_vm3.rs`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `cargo test -p oxvba-host --test package_session_events --quiet`
  - `cargo test -p oxvba-host --test com_matrix_events --quiet` (all 12 tests
    ignored by default)
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet`
    (completed non-blocking refresh: 304 pass, 16 skipped Kani obligations,
    63 todo; first 180-second runner attempt timed out while waiting on cargo
    locks, then completed with a longer timeout)
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
- Non-blocking formatter note: direct `rustfmt --edition 2024 --check
  crates/oxvba-vm3/src/lib.rs` still reports the known vm3 formatter backlog in
  unrelated default-member/error-help-file regions, so no broad vm3 reformat was
  mixed into this semantic change.

## 2026-07-02 - Predeclared Singleton Reset And Resurrection (`bd-4ktq.55`)

- Captured live Excel/VBA 7.1 behavior with an imported `.cls` file carrying
  `Attribute VB_PredeclaredId = True`; the harness used VBE Debug -> Compile and
  PID-scoped UI Automation modal handling:
  `docs/evidence/conformance/vm3_predeclared_singleton_oracle_20260702T080743Z/`.
- Oracle findings:
  - Repeated `ClassName.Member` access reuses the one default instance.
  - Releasing an ordinary local object reference to the default instance does not
    clear or reinitialize the default instance.
  - `Set ClassName = Nothing` is valid VBA and clears the default-instance slot;
    the next `ClassName.Member` access creates a fresh default instance.
  - If no other reference holds the old default, `Class_Terminate` runs before
    the next statement's observable access; if another reference holds it, the
    old instance survives and the new default is separate.
  - `Set ClassName = New ClassName` evaluates and initializes the new object
    before replacing the default slot and releasing the old default.
  - A referenced project's exposed predeclared class resets the owning project's
    default-instance slot.
- vm3 now binds predeclared class names as assignable l-values, lowers them to
  active-project and referenced-project predeclared-slot store instructions, and
  updates the owning `LoadedProgram.predeclared_singletons` cache on Set/Nothing
  or Set/New.
- OxIR statement boundaries now carry a temporary floor so vm3 releases
  statement-local temporaries before running the statement-boundary
  `Class_Terminate` drain. The floor preserves long-lived compound-statement
  temps such as `For` limits/steps, `For Each` iterator state, and `With`
  receivers while still dropping expression receiver temps at VBA statement
  boundaries. Clearing a temp floor also prunes VM auxiliary state keyed by those
  temps (`For Each` iterators and ParamArray alias metadata), so helper maps do
  not keep stale per-statement state alive after the temp slot is released.
- Added `crates/oxvba-differential/tests/predeclared_singleton_vm3.rs` to pin
  persistence, local-reference release, `Set ClassName = Nothing`,
  `Set ClassName = New ClassName`, held-old-reference survival, and referenced
  predeclared-slot reset.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-predeclared-singleton-oracle.ps1`
  - `cargo test -p oxvba-differential --test predeclared_singleton_vm3 --quiet`
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3 --quiet`
  - `cargo test -p oxvba-differential --test for_counter_overflow_vm3 --test for_header_coercion_vm3 --test foreach_scalar_source_vm3 --quiet`
  - `cargo test -p oxvba-differential --test project_class_newenum_vm3 --test compound_place_vm3 --quiet`
  - `cargo test -p oxvba-bind --test cross_project predeclared --quiet`
  - `cargo check -p oxvba-bundle -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `rustfmt --edition 2024 --check crates/oxvba-differential/tests/predeclared_singleton_vm3.rs`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet`
    (completed non-blocking refresh: run_id `20260702T081142Z`,
    obligations=383, failures/todos=63, skipped=16; first 360-second runner
    attempt timed out, then completed with a longer timeout)
- Non-blocking formatter note: direct rustfmt checks on touched legacy OxIR
  files still report pre-existing repo formatter drift outside this bead's
  hunks, so the committed formatter proof is scoped to the new regression test
  and `git diff --check`.

## 2026-07-02 - Class Termination And Dim As New Resurrection (`bd-4ktq.56`)

- Reused the existing class-termination timing oracle
  `docs/evidence/conformance/CLASS_TERMINATE_TIMING_ORACLE_2026-05-31.md`
  for statement-boundary `Class_Terminate` rules, and captured fresh live
  Excel/VBA 7.1 `Dim As New` evidence with VBE Debug -> Compile plus
  PID-scoped UI Automation modal handling:
  `docs/evidence/conformance/vm3_dim_as_new_oracle_20260702T084234Z/`.
- Oracle findings:
  - Local `Dim c As New Counter` and module-level `Private g As New Counter`
    declarations do not instantiate by themselves.
  - First member access instantiates and runs `Class_Initialize`.
  - `c Is Nothing` instantiates the `As New` local and returns `False`.
  - `Set c = Nothing` before any access does not instantiate.
  - `Set c/g = Nothing` after access clears the slot; the next read creates a
    fresh object. When no other reference holds the old object,
    `Class_Terminate` runs before the next statement observes the fresh object.
- The previous pinned entry-frame residual for `Set w = Nothing` was already
  corrected by the statement-temporary lifetime work: vm3 now returns the VBA
  value `101` for `Class_Terminate` before the next statement. The regression was
  renamed to protect that behavior instead of preserving the old `100` result.
- vm3 no longer treats `Dim x As New T` as an eager `Set x = New T`. The binder
  emits an explicit `CoreStmt::AsNew` slot registration for project classes,
  referenced classes such as `VBA.Collection`, and COM coclasses with resolved
  ProgIDs. OxIR carries this as `OxInst::AsNew`.
- The vm3 runtime records resolved `As New` local/global slots, lazily
  instantiates on operand reads when the slot is `Empty`/`Nothing`, writes the
  fresh object back to the slot, and prunes per-frame registrations when frames
  return or unwind. Operand-reading instructions are conservatively considered
  fallible because lazy instantiation can run user `Class_Initialize`.
- Added `crates/oxvba-differential/tests/dim_as_new_vm3.rs` for the oracle
  matrix: local laziness, first access, `Is Nothing`, pre-access `Set Nothing`,
  post-access resurrection, module-level laziness, and module-level resurrection.
- Fresh-eyes review caught one important residual: class-module fields such as
  `Private child As New Counter` need per-object `As New` slot semantics and were
  not covered by this oracle or implementation slice. That residual is not a
  legacy compatibility target; it is split to delivery bead `bd-4ktq.59`. The
  broad inventory row remains `IN-PROGRESS` until that field behavior matches
  real VBA compile/runtime behavior.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-vm3-dim-as-new-oracle.ps1`
  - `cargo check -p oxvba-bundle -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-differential --test dim_as_new_vm3 --quiet`
  - `cargo test -p oxvba-differential vm3_set_nothing_at_main_scope_runs_class_terminate --quiet`
  - `cargo test -p oxvba-bind as_new_auto_instantiates_a_user_class --quiet`
  - `cargo test -p oxvba-differential --test project_class_newenum_vm3 --test predeclared_singleton_vm3 --test raiseevent_fanout_vm3 --quiet`
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3 --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip as_new --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `rustfmt --edition 2024 --check crates/oxvba-differential/tests/dim_as_new_vm3.rs crates/oxvba-bundle/src/coreir.rs crates/oxvba-bind/src/stmt.rs`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet` completed in non-blocking mode with run `20260702T081142Z`: 383 obligations, 63 failures/TODOs, 16 skipped.

## 2026-07-02 - Class-Field Dim As New Per-Instance Resurrection (`bd-4ktq.59`)

- Extended the modal-safe Excel/VBA `Dim As New` oracle harness with a `Host`
  class containing `Private child As New Counter`, then captured field-specific
  evidence in
  `docs/evidence/conformance/vm3_dim_as_new_field_oracle_20260702T0912Z/`.
  The harness again made the VBE visible, invoked Debug -> Compile VBAProject
  via command ID 578, captured/dismissed any modal dialog through UI Automation
  scoped to the owned Excel PID, and performed PID-scoped cleanup.
- Oracle findings for class fields match the local/module slot rules:
  declaration and host construction do not instantiate the child; first member
  access instantiates it; `child Is Nothing` instantiates and returns `False`;
  `Set child = Nothing` before first access does not instantiate; post-access
  `Set child = Nothing` clears the slot and the next read creates a fresh child
  with `Class_Terminate` observed before the next statement; separate host
  instances keep independent child slots (`11/11/12|I;I;`).
- CoreIR/OxIR now carry class-level `as_new_fields` metadata keyed by the stable
  per-class field token and the same `CoreAsNew`/`OxAsNew` activation binding
  used by local/global slots. The binder derives this from class-module `Dim`
  declarators and resolves project, referenced-project, and COM activation
  through the shared `New` ladder before imports are finalized.
- vm3 field reads now consult the owning object's `(bundle_id, route_key,
  field)` metadata. Missing/`Nothing` `As New` fields instantiate in the
  object's owning bundle, store the fresh object back into that instance field,
  and return it. The field-array fallback path uses the same helper so default
  member indexing on an `As New` object field can instantiate before dispatch.
- Added the class-field oracle cases to
  `crates/oxvba-differential/tests/dim_as_new_vm3.rs`; the test now covers 13
  local, module-level, and class-field cases.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -Command "& { & 'scripts\run-vm3-dim-as-new-oracle.ps1' -RunId 'vm3_dim_as_new_field_oracle_20260702T0912Z' -CaseId @('FIELD-DIM-ONLY','FIELD-FIRST-MEMBER','FIELD-IS-NOTHING','FIELD-SET-NOTHING-BEFORE-ACCESS','FIELD-SET-NOTHING-RESURRECT','FIELD-INSTANCE-ISOLATION') }"`
  - `cargo check -p oxvba-bundle -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-differential --test dim_as_new_vm3 --quiet`
  - `cargo test -p oxvba-vm3 --test cross_program --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip as_new --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --test project_class_newenum_vm3 --test predeclared_singleton_vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `rustfmt --edition 2024 --check crates/oxvba-bind/src/expr.rs crates/oxvba-bind/src/ids.rs crates/oxvba-bind/src/lib.rs crates/oxvba-bind/src/stmt.rs crates/oxvba-bundle/src/coreir.rs crates/oxvba-differential/tests/dim_as_new_vm3.rs crates/oxvba-oxir/src/program.rs crates/oxvba-vm3/tests/cross_program.rs`
  - Full `rustfmt --edition 2024 --check` over all touched Rust files still reports pre-existing formatter drift in `crates/oxvba-oxir/src/elaborate/lower.rs`, `crates/oxvba-oxir/src/verify.rs`, and `crates/oxvba-vm3/src/lib.rs`; this bead manually kept its hunks scoped and uses targeted formatter proof plus `git diff --check`.
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet` refreshed formal run `20260702T092738Z` in non-blocking mode: 383 obligations, 125 pass, 242 todo, 16 skipped.

## 2026-07-02 - LSet UDT Record Byte Overlay (`bd-4ktq.57`)

- Extended the modal-safe Excel/VBA LSet/RSet oracle harness with UDT record-copy
  cases and captured fresh evidence in
  `docs/evidence/conformance/vm3_lset_rset_oracle_20260702T_bd57_udt/`.
  The harness made the VBE visible, invoked Debug -> Compile VBAProject through
  command ID 578, used UI Automation scoped to the owned Excel PID to capture
  compile dialogs and selected VBE lines, dismissed only owned dialogs, and used
  PID-scoped cleanup.
- Oracle findings:
  - `LSet targetUdt = sourceUdt` compiles and copies record storage as bytes.
  - Same-layout scalar records return `|xy|:513`.
  - Different UDT types with the same byte size reinterpret storage, e.g.
    `513:3:4`.
  - Shorter sources copy only their byte prefix and leave the target tail intact
    (`120,121,122,122`); longer sources truncate to the target size (`119,120`).
  - Fixed arrays inside UDTs participate in the byte overlay (`1:2:3:4` in the
    real VBA `B(1 To 4)` case).
  - A non-record RHS and UDTs containing variable-length `String` fields both
    compile-error with real VBA `Type mismatch`; `RSet` against a UDT target
    compile-errors with `RSet allowed only on strings`.
- The binder now lowers accepted UDT LSet statements as
  `CoreStmt::LSetRecord`, rejects non-record RHS and byte-unsafe record layouts
  with the VBA compile error text `Type mismatch`, and continues to reject UDT
  `RSet` with the observed VBA compile error text.
- OxIR/vm3 carry `OxInst::RecordLSet`. Runtime execution reads the source record,
  performs `VbaRecord::lset_from`, and writes the mutated target record back to
  the original place. `VbaRecord::lset_from` copies `min(target_size,
  source_size)` bytes using overlap-safe copy semantics so `LSet a = a`, shorter
  source/tail preservation, and longer source/truncation follow the oracle.
- This bead intentionally does not preserve the old unsupported/legacy behavior.
  The only residual found during implementation is a separate pre-existing UDT
  fixed-array projection gap: vm3 currently exposes fixed-array UDT fields as
  zero-based even though real VBA preserves declared lower bounds such as
  `B(1 To 4)`. That parity gap is tracked by delivery bead `bd-vt0r`; bd57 tests
  isolate byte-overlay behavior with `B(0 To 3)` rather than blessing the lower
  bound bug.
- Added focused coverage:
  - `crates/oxvba-differential/tests/lset_rset_vm3.rs` covers UDT byte overlay,
    same-size reinterpretation, shorter/longer source size behavior, fixed-array
    byte overlay, non-record RHS rejection, UDT `RSet` rejection, and
    variable-string UDT rejection.
  - `crates/oxvba-bind/tests/bind_roundtrip.rs` proves UDT LSet lowers through
    `CoreStmt::LSetRecord` and rejects the oracle-backed compile-error cases.
  - `crates/oxvba-runtime/src/vba_record.rs` unit tests pin prefix copy, target
    tail preservation, truncation, same-size reinterpretation, and rejection of
    owning fields such as variable strings and variants.
- Verification completed:
  - `powershell -NoProfile -ExecutionPolicy Bypass -Command "& { .\scripts\run-vm3-lset-rset-oracle.ps1 -RunId 'vm3_lset_rset_oracle_20260702T_bd57_udt' -CaseId @('LSET-UDT-COPY','LSET-UDT-SAME-LAYOUT-SCALAR','LSET-UDT-DIFFERENT-SAME-SIZE','LSET-UDT-SOURCE-SHORTER','LSET-UDT-SOURCE-LONGER','LSET-UDT-FIXED-ARRAY','LSET-UDT-RHS-NONRECORD','RSET-UDT-TARGET','LSET-UDT-VARIABLE-STRING') }"`
  - `cargo check -p oxvba-bundle -p oxvba-bind -p oxvba-oxir -p oxvba-vm3 -p oxvba-differential`
  - `cargo test -p oxvba-runtime lset_record_overlay --quiet`
  - `cargo test -p oxvba-differential --test lset_rset_vm3 --quiet`
  - `cargo test -p oxvba-bind --test bind_roundtrip lset --quiet`
  - `cargo test -p oxvba-oxir --quiet`
  - `cargo test -p oxvba-vm3 --quiet`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot --quiet`
  - `rustfmt --edition 2024 --check crates/oxvba-bind/src/error.rs crates/oxvba-bind/src/stmt.rs crates/oxvba-bind/tests/bind_roundtrip.rs crates/oxvba-bundle/src/coreir.rs crates/oxvba-differential/tests/lset_rset_vm3.rs crates/oxvba-runtime/src/variant.rs crates/oxvba-runtime/src/vba_record.rs`
  - Full touched-file rustfmt still reports pre-existing formatter drift in
    legacy OxIR/vm3 implementation files, so the committed formatter proof is
    scoped to the newly formatted files plus `git diff --check`.
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-governance.ps1`
  - `git diff --check`
  - `br dep cycles --json`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet`
    completed in non-blocking mode with run `20260702T092738Z`: 383
    obligations, 63 failures/TODOs, 16 skipped.
