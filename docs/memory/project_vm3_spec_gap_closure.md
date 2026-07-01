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
  Current-green tests cover the legal baseline shapes; ignored tests encode the
  oracle-backed expected failures for `bd-4ktq.9.2` through `bd-4ktq.9.6`.
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
