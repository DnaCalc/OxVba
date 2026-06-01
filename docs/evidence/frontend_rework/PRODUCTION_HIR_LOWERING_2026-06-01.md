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
descriptor with both arguments. Fresh-eyes correction in that slice also preserves the no-`Call`
invocation-syntax descriptor, parenthesized statement-level `force_byval` arguments, and expression
call return copyout metadata when the broader call-site descriptor fixture routes through HIR.

Follow-up continuation adds a backend expression-statement discard shape for value-producing
expressions with observable effects, so statement-form member calls such as `obj.Method 1, 2` now
lower through HIR and emit the existing late-bound dispatch invoke bytecode while discarding the
result.

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

Follow-up continuation accepts module-level `Event` declarations on the HIR production route when
paired with the already-supported `RaiseEvent` statement. The event declaration is currently a
symbol/fact declaration with no direct bytecode; declared-event signature validation, named event
arguments, WithEvents handler matching, Implements coupling, and project event binding remain
broader event/COM work.

Follow-up continuation accepts the existing single-source `Implements IFoo` directive shape on the
HIR production route as a directive with no direct bytecode. Project/class Implements validation,
interface member matching, imported-interface handling, and Implements/event coupling remain
project/front-end semantic work outside this single-source route slice.

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

Follow-up continuation adds the first `With` route slice: read-side dot-prefixed member expressions
inside `With obj ... End With` are bound to the active With receiver and lower through the existing
late-bound member read path. With member assignment targets remain fallback-eligible because member
write/property Let/Set semantics are still broader FE-7/FE-8 work.

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

## Enum Continuation

The latest FE-8.5 slice removes the basic enum constant residual without weakening the production
guard:

- `EnumBlock` is no longer rejected by the HIR production syntax gate.
- The front-end symbol model declares enum members as module-scope constant symbols so procedure
  bodies can resolve `Safe` in `x = Safe + 1` through the same HIR name path used by module
  constants.
- HIR production lowering parses enum member values with the same simple explicit/incrementing
  integer semantics as the legacy resolver, substitutes enum member references as `IntConst`, and
  projects `BoundEnumDescriptor` metadata into the lowered module.
- The production route audit includes a `Public Enum Mode ... Safe ...` fixture and classifies it
  as `HirProduction`.
- A legacy bytecode assertion was relaxed from a specific `AddConstI32` peephole to equivalent add
  bytecode because the workset does not require byte-identical output from the new front-end.

Remaining production residuals are still real semantic work, not marker-only gaps:

- `DeclareStmt`: native declarations require external declaration descriptors, ABI policy,
  argument/return marshalling, and call diagnostics on the HIR path.
- `TypeBlock`: UDT declarations require descriptor/layout/field alias/lifetime projection.
- `NewExpr`: object construction requires project class handles, imported/COM construction rules,
  `As New` lazy construction interactions, and assignment/writeback behavior.

## Declare Continuation

The latest FE-8.5 slice removes the basic declared external call residual:

- `DeclareStmt` is no longer rejected by the HIR production syntax gate.
- The front-end symbol model now extracts the declared procedure name after `Function` / `Sub`
  instead of incorrectly treating `PtrSafe` as the procedure symbol in flat declare syntax.
- HIR production lowering reuses the existing external declaration parser/descriptors, seeds the
  lowered `BoundModule.external_declarations`, and includes external procedure signatures for
  typechecking.
- Calls such as `y = HostPing(3)` lower as ordinary HIR `ProcCall` expressions and emit the
  existing `IntrinsicInvokeSymbolHost` bytecode plus `ExternalCallDescriptor` metadata.
- Unsupported declaration shapes, including missing `PtrSafe`, return HIR `Unsupported` so the
  default compiler path can keep them on the tracked fallback/diagnostic surface.

Remaining production residuals after this slice:

- `TypeBlock`: UDT declarations require descriptor/layout/field alias/lifetime projection, and UDT
  member syntax must not be confused with late-bound object member dispatch.
- `NewExpr`: object construction requires project class handles, imported/COM construction rules,
  `As New` lazy construction interactions, and assignment/writeback behavior.

## UDT Layout Continuation

The FE-8.5 UDT slices remove the basic UDT declaration/layout and simple field-alias residuals:

- `TypeBlock` is no longer rejected by the HIR production syntax gate.
- HIR production lowering parses simple module-level `Type ... End Type` definitions, recognizes
  local declarations such as `Dim p As Point`, and emits flattened field slots such as `p_x` and
  `p_y` with the declared primitive field types.
- Lowered procedures now carry `BoundUdtDescriptor` data so emitted procedure metadata includes UDT
  type descriptors, instances, and field aliases.
- Simple UDT field reads/writes now lower to flattened aliases, so `p.X = 1` and `y = p.X + 2`
  avoid the object/member dispatch path.
- Same-shape whole-value UDT assignment lowers to the existing field-wise `BoundStmt::UdtAssign`
  copy path.
- The production route audit includes a `Type Point ... p.X = 1 ... y = p.X + 2` fixture and
  classifies it as `HirProduction`.

Remaining production residuals after this slice:

- Nested UDT fields, UDT array fields, fixed-string field storage, richer cross-type assignment
  diagnostics, and broader UDT lifetime/default initialization parity remain open.
- `NewExpr` still requires project class handles, imported/COM construction rules, `As New` lazy
  construction interactions, and assignment/writeback behavior.

## New Expression Shape Continuation

The latest FE-8.5 slice moves `New` from a raw CST syntax guard into the frontend expression model:

- `HirExprKind::New { type_name }` now records the normalized constructor type name, including
  qualified names such as `Foo.Bar`.
- SemanticModel indexes `New` as a leaf expression, so IDE/compiler callers can see the same HIR
  shape instead of losing the construct before semantic indexing.
- HIR production lowering now rejects `New` at the exact missing semantic boundary:
  `New expression '<type>' requires project-aware construction binding`.
- This is intentionally not closure for object construction. The remaining delivery step is a
  project-aware HIR lowering path that binds the constructor type to active-project classes,
  imported/COM activation metadata, generated instance handles, `Class_Initialize`, and `As New`
  lazy construction semantics without relying on `project.rs` source-text rewrites.

Remaining production residuals after this slice:

- `Set obj = New Widget` and `Dim obj As New Widget` still need project-aware construction facts
  and handle allocation on the HIR production path.
- Existing project rewrite behavior remains compatibility/parity scaffolding until that route is
  replaced or quarantined by the construction-lowering continuation.

## New Construction Binding Continuation

The latest FE-8.5 continuation adds the first project-aware construction hook to HIR lowering:

- `HirNewExpressionBinding { type_name, object_handle }` is an explicit lowering input for
  constructor facts known by project binding.
- `lower_typed_hir_to_bound_module_with_new_bindings(...)` preserves the default residual behavior
  when no binding is supplied, but consumes supplied constructor handles in source order by
  normalized type name.
- A bound `New Widget` expression now lowers to typed
  `StructuralIntrinsic::ProjectInstance(IntConst(handle))`, reusing the existing typed project
  instance intrinsic instead of a magic helper name.
- The focused regression proves `Set obj = New Widget` lowers to an object `Set` assignment with a
  `ProjectInstance` structural intrinsic when supplied with handle `7`.

Remaining production residuals after this slice:

- Project compilation still has to generate and pass these `New` construction bindings from the
  active-project class/COM binding pass. Until then, `project.rs` source-text rewrite behavior is
  still present and must remain owned by FE-8.5/FE-9.6.
- `Class_Initialize`, imported/COM activation, `Dim As New` lazy semantics, and source-map
  accounting still need end-to-end integration on the production project route.

## Project Construction Binding Fact Continuation

The latest FE-8.5 project slice connects active-project construction analysis to the HIR binding
payload:

- `ProjectDynamicInstanceBindingDraft` now records the normalized constructor type name separately
  from the resolved project/module route. This matters when the source constructor (`Widget`) and
  module file identity (`WidgetFile`) differ.
- Project lowering materializes `HirNewExpressionBinding` facts from generated dynamic instance
  handles in source order. The active-project `Dim As New` / `Set x = New Widget` route test now
  proves handles `1` and `2` produce corresponding `widget` HIR construction bindings.
- The project compile boundary currently builds those HIR construction facts, but still compiles
  the rewritten backend source. This keeps existing behavior stable while exposing the exact data
  needed for the next rewrite-retirement slice.

Remaining production residuals after this slice:

- Project compilation must consume the HIR construction facts when compiling the module source, so
  `Set x = New Widget` no longer needs to become `Set x = __oxvba_project_instance(handle)`.
- `Dim As New`, `Class_Initialize`, source maps, and imported/COM construction still need the same
  direct-HIR integration.

## Project Construction Downstream Regression Continuation

Two downstream object-construction regressions were narrowed while the direct HIR project compile
route remains open:

- Source-class public field reads now use the class field-token route for value-side
  `obj.PublicField` reads. The exact `c.Total` regression is covered by
  `source_member_call_statements`, which now proves statement-form member calls preserve
  per-instance public field state.
- The concrete WithEvents `New` rewrite failure is fixed for direct active-project source-class
  construction: `Set <WithEventsField> = New <Class>` now skips the plain Set-New expansion and
  lowers through a WithEvents-aware temporary project instance before
  `__oxvba_withevents_set(...)`. This prevents raw `New <Class>` text from reaching the legacy
  expression parser on that path while preserving `Class_Initialize` identity.

These fixes do not close FE-8.5 object construction. They reduce downstream breakage while the
main production residual remains: project compile must consume `HirNewExpressionBinding` directly
instead of compiling rewritten `__oxvba_project_instance(...)` source text.

## Const Expression Continuation

The latest FE-8.5 slice widens the HIR `Const` route from literal-only substitution to simple
constant expressions:

- `Const` production eligibility now accepts simple expression trees composed from literal values,
  parentheses, unary minus, arithmetic operators, and string concatenation.
- `collect_const_values(...)` records those values as bound expression trees, so uses such as
  `x = CBase` for `Const CBase = 1 + 2` lower through HIR and produce expression bytecode without
  allocating a runtime local slot for `CBase`.
- Later declarators in the same `Const` statement can reference earlier declarators, for example
  `Const CBase = 1 + 2, CTotal = CBase + 1`; those references are substituted as expression trees,
  not runtime variable reads.
- This is intentionally still a bounded subset. Constant expressions that require broader
  module/procedure-scoped name evaluation beyond same-statement declarators and the already handled
  enum/literal route remain future FE-8.5 work.

## Bang Member Read Continuation

The latest FE-8.5 slice removes the read-side bang member residual:

- HIR member-name extraction now accepts `!` as a member selector for expressions such as
  `obj!Value`, matching the existing syntax bridge/backend representation.
- HIR production lowering emits the same late-bound dispatch shape used for dot member reads.
- This does not close member writes: `obj!Value = ...`, `obj.Value = ...`, `With` shorthand writes,
  property Let/Set selection, and writeback semantics remain tracked residual work.

## Checks

- `cargo test -p oxvba-compiler hir_production_lowering_accepts_bang_member_access --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_expression_const_statement --quiet`
- `cargo test -p oxvba-host --test source_member_call_statements --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_fields_are_per_instance_storage --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_distinct_new_instances_have_separate_state --quiet`
- `cargo test -p oxvba-compiler compile_project_lowers_withevents_new_source_class_expression --quiet`
- `cargo test -p oxvba-compiler withevents --quiet`
- `cargo test -p oxvba-compiler frontend_hir_lowering --quiet`
- `cargo test -p oxvba-compiler frontend_hir --quiet`
- `cargo test -p oxvba-compiler new_expression --quiet`
- `cargo test -p oxvba-compiler hir_lowering_binds_new_expression --quiet`
- `cargo test -p oxvba-compiler expand_bound_source_line_uses_frontend_class_route_for_active_project_new --quiet`
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_enum_member_constants --quiet`
- `cargo test -p oxvba-compiler compile_enum_member_usage_is_supported --quiet`
- `cargo test -p oxvba-compiler declared_external_call --quiet`
- `cargo test -p oxvba-compiler declare_without_ptrsafe --quiet`
- `cargo test -p oxvba-compiler udt_layout --quiet`
- `cargo test -p oxvba-compiler udt_field_read_write --quiet`
- `cargo test -p oxvba-compiler compile_udt_whole_assignment_emits_field_copy_slots --quiet`
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
