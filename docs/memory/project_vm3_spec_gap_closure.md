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
- Verification target:
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
- Added `crates/oxvba-differential/tests/array_bounds_unallocated_vm3.rs`,
  which proves `LBound` and `UBound` on both never-allocated and erased dynamic
  arrays raise run-time error 13, while allocated dynamic and fixed arrays still
  return their declared bounds.
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
