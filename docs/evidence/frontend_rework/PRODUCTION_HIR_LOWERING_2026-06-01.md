# Production HIR Lowering Evidence

Date: 2026-06-01
Bead: `bd-aprs.9.5`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Outcome

Added `crates/oxvba-compiler/src/frontend_hir_lowering.rs`, a scoped production HIR lowering path.
For the currently supported HIR surface, source is parsed and bound into typed HIR, lowered from HIR
facts into the current bound module shape, then passed through the existing typecheck, optimizer, and
bytecode/metadata emitter. This means the frontend-v2 syntax bridge and the ordinary lightweight
single-source compile path now try real HIR production lowering before falling back to tracked
legacy residuals.

The initial production scope is intentionally narrow and explicit:

- procedure declarations,
- local and parameter frame slots with declared scalar/object types,
- explicit `ByVal` / `ByRef` parameter mechanism projection for lowered procedures,
- `Dim` metadata line projection,
- implicit/explicit `Let` and `Set` assignments,
- simple block and single-line `If ... Then ... Else ...` statements, including `ElseIf` branches,
- `Do While` / `Do Until` loops with front-check or post-check conditions,
- `While` / `Wend` loops,
- simple `For <var> = <start> To <end> [Step <step>] ... Next` range loops,
- simple `For Each <var> In <iterable> ... Next` loops through the iterable backend path,
- `Exit Do`, `Exit For`, and `Exit Sub` statements through HIR statement nodes,
- `On Error Resume Next`, `On Error GoTo 0`, `On Error GoTo label`, `Resume Next`, bare
  `Resume`, and `Resume label` statements,
- identifier/numeric labels and `GoTo` statements,
- `GoSub` and `Return` statements,
- `Erase` statements for named variables,
- simple `Select Case` statements with single integer-value `Case` clauses, integer `Case A To B`
  ranges, comma-separated integer value clauses, integer `Case Is` clauses, and optional
  `Case Else`,
- literals, names, unary expressions, and binary arithmetic/comparison/logical expressions, and
- typed structural `Null`/`Nothing` literals,
- same-module procedure call statements whose targets bind to procedure symbols and whose arguments
  lower through the supported expression surface.

Unsupported constructs are rejected from the HIR production path before lowering and continue through
the tracked fallback path. This prevents silent partial lowering for member/index/new expressions,
unsupported control flow, error handling, `ReDim`, `With`, events, declarations, and other surfaces
not yet implemented in HIR production lowering.

## Reopened Continuation

The second FE-8.5 slice removes the procedure-call syntax residual that the route audit exposed
after the hidden CST bridge fallback was removed:

- `CallStmt` lowers into a HIR expression statement instead of falling through recursive statement
  collection.
- `CallExpr` and parser-shaped `IndexExpr` call forms lower into `HirExprKind::Call`.
- HIR production lowering emits `BoundStmt::Call` for same-module procedure targets.
- Lowered procedure parameters now preserve explicit `ByVal` / `ByRef` source mechanisms, so call
  descriptors no longer report `ByVal` parameters as omitted/default `ByRef`.

The call/coercion seed row no longer exposes a bytecode or call-descriptor bug after this slice.
`conformance/tests/call_coercion_mixed_variant_to_long.bas` now matches diagnostics, bytecode, call
site metadata, and coercion descriptors. The only remaining delta is source-map metadata for the
second procedure: HIR reports the actual `Sub Use` line after the blank line, while the legacy
projection maps the procedure one line early. The diff classifier now records that as a documented
metadata improvement instead of a bug.

Follow-up continuation on the call slice preserves bare argument lists for same-module statement
calls without parentheses, so `Use 1, 2` now lowers through `HirCall` and produces a call-site
descriptor with both arguments. Statement-form member calls such as `obj.Method 1, 2` remain
fallback-eligible because the current backend has no expression-statement discard form for
receiver-based `BoundExpr::Member` calls.

This is still not blanket FE-8.5 closure. Broader HIR production lowering remains open for language
surfaces outside this simple same-module call subset, especially optional/default arguments,
ParamArray, member/index dispatch, control flow, and project/class paths owned jointly with FE-7.

## Control-Flow Continuation

The third FE-8.5 slice removes the simplest control-flow route residual:

- multiline `IfStmt` nodes lower into `HirStmtKind::If` with CST-backed condition, then-body, and
  else-body fields;
- production HIR lowering converts that HIR statement into `BoundStmt::IfCond`;
- HIR production bytecode emission now reaches `Instruction::JumpIfZero` for the simple
  `If x = 0 Then ... End If` fixture; and
- the route audit classifies the simple If fixture as `HirProduction`.

This is intentionally not full control-flow closure. Bare `Do` loops, `Exit Do`, richer
`Select Case`, labels, `GoTo`/`GoSub`, and error-control constructs remain tracked FE-8.5 residuals
until each has HIR shape, lowering tests, bytecode/metadata parity or documented improvement
classification, and route-audit coverage.

## Loop Continuation

The fourth FE-8.5 slice removes the simplest loop route residual:

- front-checked `DoStmt` nodes with a leading `While` condition lower into `HirStmtKind::DoWhile`;
- HIR consumers for lowering-contract facts, semantic-model indexing, and type hooks now walk loop
  bodies instead of ignoring nested statements;
- production HIR lowering converts front-checked `DoWhile` HIR into `BoundStmt::DoWhile`; and
- HIR production bytecode emission now reaches loop branch and backedge bytecode for the
  `Do While x < 3 ... Loop` fixture.

The sixth FE-8.5 slice widens the loop coverage:

- `Do Until` and post-check `Loop While`/`Loop Until` forms use the same HIR loop node with
  explicit `post_check` and `until` flags;
- production lowering maps `Until` by inverting the lowered condition before emitting the existing
  `BoundStmt::DoWhile` backend shape; and
- route-audit fixtures for `Do Until` and post-check loops now classify as `HirProduction`.

Bare `Do` loops and `Exit Do` remain out of scope for this slice because the current backend shape
requires a condition and exit-stack semantics need direct coverage.

The seventh FE-8.5 slice maps `While ... Wend` into the same front-checked conditional loop HIR
shape used by `Do While`; the route audit now classifies the `While/Wend` fixture as
`HirProduction`.

The eighth FE-8.5 slice adds simple range `For` loops:

- `ForStmt` nodes lower into `HirStmtKind::ForRange` when they are not `For Each`;
- the loop counter resolves through the HIR symbol model instead of string-only lowering;
- omitted `Step` lowers to the existing backend default of `1`; and
- the route audit now classifies the simple `For i = 1 To 3 ... Next` fixture as `HirProduction`.

## Select Continuation

The fifth FE-8.5 slice removes the simplest `Select Case` route residual:

- the syntax parser now creates expression nodes for `Select Case <expr>` selectors and simple
  `Case <expr>` values instead of leaving the header as unparsed line text;
- simple `Select Case` statements lower into `HirStmtKind::SelectCase`;
- production HIR lowering converts single integer-value case clauses into `BoundCaseClause::Value`;
  and
- HIR production bytecode emission now reaches branch bytecode for the
  `Select Case x / Case 1` fixture.

The ninth FE-8.5 slice widens `Select Case` clause support:

- `Case A To B` headers now parse both range endpoints as expression nodes;
- HIR represents case clauses as typed value/range variants instead of plain expression lists; and
- production lowering emits `BoundCaseClause::Range` for integer ranges.

The tenth FE-8.5 slice adds comma-separated value clauses:

- `Case 1, 2` headers now parse each comma-separated value as an expression node;
- HIR stores them as multiple `HirCaseClause::Value` entries in one arm; and
- production lowering emits the existing aggregate case-match bytecode path for multi-value arms.

The eleventh FE-8.5 slice adds `Case Is` clauses:

- the syntax parser now recognizes `Case Is <op> <expr>` headers;
- HIR stores them as `HirCaseClause::Is` with a normalized comparison operator; and
- production lowering emits `BoundCaseClause::Is` for integer comparison values.

This is not full `Select Case` closure. Non-integer case expressions, mixed range lists, and richer
clause parsing remain outside this narrow route-audited subset.

The twelfth FE-8.5 slice adds simple `For Each` lowering:

- `For Each` statements lower into `HirStmtKind::ForEach`;
- the loop variable resolves through the HIR symbol model;
- the iterable expression lowers through normal HIR expression lowering; and
- production lowering emits the existing `IntrinsicForEachInit` / `IntrinsicForEachNext` bytecode
  path.

## Production Entry-Point Continuation

The thirteenth FE-8.5/FE-9.1 slice wires completed HIR lowering into the ordinary lightweight
single-source compile path:

- `compile()` / `compile_with_runtime_metadata()` now try
  `frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir` before entering the legacy
  resolver for eligible non-class, non-forced-object-local sources;
- unsupported HIR shapes still fall through to the tracked legacy residual route;
- HIR compile/type errors remain compile errors instead of being hidden by fallback; and
- the diff harness now uses `compile_with_runtime_metadata_legacy` for the old baseline, so
  differential evidence does not accidentally compare the new route against itself.

Fresh-eyes correction in the same slice: the first default-route attempt was too broad. HIR could
parse some sources whose semantics are not yet fully represented, including DefType defaults,
optional/default parameters, function return types, and project-rewritten modules. The lightweight
default gate now excludes those surfaces and leaves them on the tracked residual path.

## Function Return-Type Continuation

The twenty-second FE-8.5/FE-9.1 slice removes the simple function-declaration residual from the
lightweight HIR route:

- `oxvba-syntax` now keeps declaration type suffixes (`Function Alpha%() As ...`) inside function
  declaration nodes instead of treating the suffix as leftover line text;
- typed HIR hooks record declared function/property return types on the procedure symbol and project
  them into the HIR declaration;
- HIR production lowering uses that return type for the function return slot, procedure signature,
  and runtime metadata instead of defaulting every function to `Variant`;
- HIR production now performs the same basic object-assignment intent checks for the scoped route,
  so `Function alpha%() As Object: alpha = 1` fails instead of compiling through a partial HIR
  return slot; and
- the ordinary lightweight `compile()` / `compile_with_runtime_metadata()` eligibility guard now
  allows simple functions after that return-slot projection.

DefType defaults, optional/default/ParamArray parameters, properties, project rewrites, and
class/object-local compatibility contexts remain tracked residuals until their HIR facts and route
proofs are complete.

## RaiseEvent Continuation

The twenty-third FE-8.5 slice removes the basic `RaiseEvent` statement residual:

- `RaiseEventStmt` nodes lower into typed HIR with a normalized event name;
- `oxvba-syntax` now preserves positional event arguments as normal argument-list expressions;
- production HIR lowering maps the node to the existing backend `BoundStmt::RaiseEvent` form; and
- the production route audit now includes a `RaiseEvent Tick(1)` fixture.

Fresh-eyes correction after this slice: the first `RaiseEvent Tick(1)` route preserved and emitted
arguments, but the HIR `RaiseEvent` node was still treated as a leaf by two front-end fact
consumers. The semantic model now indexes event argument expressions, and the lowering-contract
collector now includes structural facts from event arguments. Focused regressions cover
`RaiseEvent Tick(n)` symbol queries and `RaiseEvent Tick(Null)` lowering-contract intrinsics.

Named event arguments and full project event binding remain broader event/COM work outside this
narrow route slice.

## Const Continuation

The twenty-fourth FE-8.5 slice removes the simple literal `Const` residual:

- procedure and module `ConstStmt` declarations are accepted as HIR declarations without producing
  runtime frame slots;
- HIR production lowering substitutes simple integer, Boolean, and string literal constants at use
  sites;
- comma-separated literal declarators in the same `Const` statement are accepted, including string
  literals containing commas; and
- the production route audit now includes a module-level multi-literal constant fixture.

Expression-valued constants, typed constants beyond the current literal subset, conditional
compilation constants, and enum constants remain broader constant/compile-time evaluation work.

## ReDim Continuation

The twenty-fifth FE-8.5 slice removes the first runtime `ReDim` residual:

- `oxvba-syntax` now preserves `ReDim [Preserve] name(expr)` bound expressions in the CST instead
  of treating the whole statement tail as opaque text;
- typed HIR carries a `ReDim` statement node with normalized target name, preserve flag, and bound
  expression ids;
- production HIR lowering maps one-dimensional dynamic-array resizes to `BoundStmt::ReDimRuntime`;
- local `Dim name() As T` declarations contribute array declaration type and runtime
  `ArrayShapeDescriptor` metadata, including element type and the lower-bound policy available to
  the HIR route; and
- the route audit now includes a dynamic-array `ReDim buf(length - 1)` fixture.

This is intentionally not full `ReDim` parity. Lower-bound forms such as `1 To n`,
multi-dimensional resizes, fixed-array alias materialization, project/class array fields, and
array element read/write migration remain broader HIR and project-semantics work. The ordinary
lightweight default route still excludes `OptionStmt`, so `Option Base` sources remain outside this
default-routed ReDim subset.

## Member Expression Continuation

The twenty-sixth FE-8.5 slice removes the first explicit-receiver dot-member expression residual:

- `MemberExpr` nodes with a normal receiver and dot member name lower into HIR member expressions
  and allocate member symbols that are visible to compiler-owned semantic queries;
- production HIR lowering maps value-side member reads to the existing `BoundExpr::Member` backend
  shape;
- `CallExpr` targets that lower to member expressions preserve positional arguments, so
  `obj.Method(1)` reaches the existing late-bound member-call bytecode path; and
- the route audit now includes a fixture for `x = obj.Value` and `y = obj.Method(1)`.

This is intentionally not full member/property/object parity. Bang access, member assignment
targets, `With` dot-shorthand, `New`/object construction, default-member resolution, property
Get/Let/Set selection, project/class binding, early-bound COM binding, ByRef/writeback behavior,
and host-provided member semantics remain FE-7/FE-8 residuals.

## ElseIf Continuation

The fourteenth FE-8.5 slice widens block-If coverage:

- `ElseIfClause` nodes lower into nested `HirStmtKind::If` statements in the outer else branch;
- `Else` blocks remain the terminal nested else body;
- production HIR lowering emits multiple conditional branch sites for the nested branch tree; and
- the production route audit now includes `If/Else` and `If/ElseIf/Else` fixtures as
  `HirProduction`.

## Exit Continuation

The fifteenth FE-8.5 slice removes the basic exit-statement residual:

- `ExitStmt` nodes lower into typed HIR statement variants for `Exit Do`, `Exit For`, and
  `Exit Sub`/`Exit Function`/`Exit Property`;
- production HIR lowering maps those variants to the existing `BoundStmt::ExitDo`,
  `BoundStmt::ExitFor`, and `BoundStmt::ExitProcedure` backend forms;
- HIR SemanticModel, type-hook, and lowering-contract walks treat exit statements as leaf
  statements; and
- the production route audit now includes `Exit Do`, `Exit For`, and `Exit Sub` fixtures as
  `HirProduction`.

## Single-Line If Continuation

The sixteenth FE-8.5 slice closes a silent-partial-lowering hazard:

- the syntax parser now builds `Block` and optional `ElseClause` children for single-line
  `If ... Then ... Else ...` statements instead of consuming the rest of the line as unstructured
  text;
- HIR collection consumes those inline blocks through the same `HirStmtKind::If` shape used by
  multiline If;
- production HIR lowering emits conditional branch bytecode for the single-line fixture; and
- the production route audit now includes a single-line If fixture as `HirProduction`.

## Basic Error-Control Continuation

The seventeenth FE-8.5 slice removes the basic non-label error-control residual:

- `OnErrorStmt` nodes lower into typed HIR variants for `On Error Resume Next` and
  `On Error GoTo 0`;
- `ResumeStmt` nodes lower into typed HIR variants for `Resume Next` and bare `Resume`;
- production HIR lowering maps those variants to the existing backend error-control statements; and
- label-targeted `On Error GoTo label` / `Resume label` is sequenced after labels are represented
  by the syntax/HIR route.

## Label and GoTo Continuation

The eighteenth FE-8.5 slice adds the first label-targeted control-flow route:

- `oxvba-syntax` now parses identifier and numeric labels as `LabelStmt` nodes instead of treating
  them as unstructured statement text;
- HIR lowers labels and `GoTo` targets into typed statement variants, normalizing numeric labels to
  the existing backend `__line_N` key shape;
- production HIR lowering maps those variants to `BoundStmt::Label` and `BoundStmt::GoTo`; and
- `On Error GoTo label` and `Resume label` remain residuals until their label target behavior is
  audited separately.

## GoSub Continuation

The nineteenth FE-8.5 slice widens label-targeted control flow:

- `GoSubStmt` nodes lower through the same HIR label normalization used by `GoTo`;
- `ReturnStmt` nodes lower into a typed HIR leaf statement; and
- production HIR lowering maps them to the existing `BoundStmt::GoSub` and `BoundStmt::Return`
  backend forms.

## Label Error-Control Continuation

The twentieth FE-8.5 slice completes the basic label-targeted error-control route:

- `On Error GoTo label` lowers into `HirStmtKind::OnErrorGotoLabel`;
- `Resume label` lowers into `HirStmtKind::ResumeLabel`;
- both use the same identifier/numeric label normalization as `GoTo`; and
- production HIR lowering maps them to the existing backend error-control label forms.

## Erase Continuation

The twenty-first FE-8.5 slice adds the simplest array/reset statement route:

- `EraseStmt` nodes lower into a typed HIR leaf with a normalized target name;
- production HIR lowering maps that leaf to the existing `BoundStmt::Erase` backend form; and
- richer array-shape validation remains tied to the broader `ReDim`/array descriptor residual.

## Checks

- `cargo test -p oxvba-compiler frontend_hir_lowering --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler frontend_diff_v2_smoke_matches_legacy_for_supported_assignment --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_uses_hir_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
- `cargo test -p oxvba-compiler --quiet`
- `cargo check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `git diff --check`

## Fresh-Eyes Review

- This bead does not remove the fallback bridge; FE-9 default-route and audit beads must decide which
  construct families are flipped and which residuals remain tracked.
- Call-site descriptors, object/member bindings, and writebacks remain out of the current HIR
  production scope beyond the simple same-module call route above. Broader argument binding,
  optional/default, ParamArray, member dispatch, and writeback semantics remain open FE-8.5/FE-7
  delivery work.
- The first attempt let HIR production lowering silently ignore call statements. The production guard
  now rejects unsupported syntax kinds up front, and call statements are covered by direct HIR
  lowering tests so scoped HIR lowering is not allowed to compile a partial program.
- The simple assignment parity check initially exposed metadata drift in assignment intent and
  declaration line numbers; HIR lowering now preserves implicit assignment intent and projects local
  declaration source lines into procedure metadata.
- A corpus bookkeeping error briefly attached the FE-8.5 call/coercion rationale to
  `examples/basic/arithmetic.bas`. The corpus test now asserts that the single bug row is
  gone and that `conformance_call_coercion_mixed_variant_to_long` is an intentional metadata
  improvement, so equivalent arithmetic cannot mask a call/coercion residual.
