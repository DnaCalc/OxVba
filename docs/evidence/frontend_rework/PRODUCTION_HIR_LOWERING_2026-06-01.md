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

Follow-up FE-8.5.f route work narrows that residual: optional parameters with simple explicit
defaults now remain eligible for the default HIR path and preserve optional/default signature
metadata for otherwise completed single-source inputs. Richer default expressions and broader
optional missing-state call-entry behavior remain outside the lightweight default route.
Additional property-declaration work teaches HIR lowering to derive `Property Get`/`Property Let`
procedure kinds from the HIR property record, preserve getter return-slot metadata, and bind the
getter self-assignment name to that return slot. Same-module zero-argument `Property Get` reads now
also lower through HIR as procedure calls, and simple same-module `Property Let`/`Property Set`
writes lower as synthetic property assignment calls. Simple non-indexed property declarations now
remain eligible for the default HIR route. Indexed property invocation and broader default-member
semantics remain open.

Further FE-8.5.f route work moves the simple `ParamArray` declaration and positional packed-call
shape onto the default HIR route. HIR lowering preserves the `ParamArray` signature role and
call-site `ParamArrayPack` descriptor, and the existing named ParamArray-target diagnostic still
fires after the route flip. This does not claim broad intrinsic coverage inside ParamArray callees,
richer default expressions, or the broader optional/missing-state call-entry matrix.

Follow-up ParamArray callee work resolves and lowers the built-in array-bound intrinsics
`LBound`/`UBound` through HIR. The focused regression covers `UBound(items)` inside a ParamArray
callee and verifies that HIR emits the existing `IntrinsicUBoundArray` bytecode. Broader callee
intrinsics remain tracked residual work.

The next bounded ParamArray callee slice extends that same HIR intrinsic bridge to the
legacy-recognized one-argument introspection intrinsics `IsArray`, `VarType`, `TypeName`,
`IsNumeric`, `IsDate`, `IsObject`, `IsEmpty`, `IsNull`, and `IsError`. The focused regression writes
each result to a distinct ByRef parameter so optimizer dead-store elimination cannot hide missing
bytecode, and verifies the existing `IntrinsicIsArrayTag`, `IntrinsicVarType`,
`IntrinsicTypeNameTag`, `IntrinsicIsNumeric`, `IntrinsicIsDateTag`, `IntrinsicIsObjectTag`,
`IntrinsicIsEmpty`, `IntrinsicIsNull`, and `IntrinsicIsError` instructions. At this point it was
still not blanket intrinsic closure; multi-argument and host-sensitive callee intrinsics plus
broader optional/missing-state entry behavior remained open.

Follow-up deterministic intrinsic work adds the string/search subset `Len`, `Left`, `Right`, `Mid`,
`InStr`, `InStrRev`, `Replace`, and `StrComp` to the shared HIR built-in allowlist. The focused
regression writes each result to a separate ByRef parameter and verifies the existing string/search
bytecode instructions. This covers a representative multi-argument intrinsic family without
claiming array-producing, date/time/math/financial, pointer, host-sensitive, or optional-entry
closure.

Follow-up numeric/math intrinsic work adds `Abs`, `Int`, `Fix`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`,
`Log`, `Exp`, `Atn`, and `Tan` to the same HIR built-in allowlist and verifies their existing
numeric intrinsic bytecode. Fresh-eyes note: the initial regression used `Abs(-7)` and exposed that
general unary expressions were still rejected by HIR production expression building; the accepted
test uses `Abs(7)` so the intrinsic slice did not silently bundle unary-expression parity.

Follow-up FE-4/FE-8 expression work fills that unary-expression gap directly. HIR building now
constructs `HirExprKind::Unary` for unary minus and `Not`, treats unary plus as transparent, and the
production lowering regression verifies `NegSlot` and `BoolNot` bytecode from ordinary assignment
expressions.

Follow-up deterministic date/time intrinsic work adds `Year`, `Month`, `Day`, `Weekday`,
`MonthName`, `DateValue`, `TimeValue`, `DateSerial`, `TimeSerial`, `DateAdd`, and `DateDiff` to the
HIR built-in allowlist and verifies the existing date/time bytecode variants. This intentionally
excludes host current-time intrinsics such as `Date()`, `Time()`, `Now()`, and `Timer()`.

Follow-up deterministic conversion/formatting intrinsic work adds `CStr`, `Str`, `Val`, `CDate`,
`Hex`, and `Oct` to the same HIR built-in allowlist and verifies their existing bytecode variants.
This remains a scalar expression subset; array-producing, collection, financial, pointer, dispatch,
and host-sensitive intrinsic groups remain separate FE-8.5/FE-8 retirement work.

Follow-up string transform/format intrinsic work adds `LCase`, `UCase`, `Trim`, `LTrim`, `RTrim`,
`Space`, `String`, `Chr`, `Asc`, `StrReverse`, `StrConv`, `Format`, `Split`, and `Join` to HIR
built-in resolution and verifies the existing transformation/formatting bytecode variants. This
continues the deterministic intrinsic migration without claiming collection, financial, pointer,
dispatch, or host-sensitive closure.

Follow-up collection intrinsic work adds `CollectionAdd`, `CollectionItem`, `CollectionRemove`, and
`CollectionCount` to HIR built-in resolution and verifies the existing collection bytecode variants.
This covers the deterministic collection helper family but does not claim general VBA `Collection`
object/member syntax or default-member semantics.

Follow-up financial intrinsic work adds `FV`, `PV`, `Pmt`, `NPV`, `IRR`, `MIRR`, `Rate`, and `NPer`
to HIR built-in resolution and verifies the existing financial bytecode variants. The regression
uses the minimal accepted arity for each intrinsic; richer optional argument combinations remain
covered by the shared emitter/runtime paths and broader call-entry optional-state work.

Follow-up pointer-helper proof adds focused production-HIR coverage for `StrPtr`, `VarPtr`, and
`ObjPtr`. No allowlist change was needed: these names were already symbol-declared and lowered as
typed `StructuralIntrinsic` variants. The regression verifies the existing pointer bytecode
variants through that route.

Follow-up array-literal intrinsic work adds `Array(...)` to HIR built-in resolution and verifies
the existing `IntrinsicArrayLiteral` bytecode through production HIR. This proves the literal helper
shape only; it does not close the separate array storage/indexing/`ReDim` parity lane.

Follow-up RNG intrinsic work adds the VM-stateful deterministic `Rnd`/`Randomize` family to HIR
built-in resolution and verifies no-seed and seeded forms through the existing
`IntrinsicRndDigits` and `IntrinsicRandomizeDigits` bytecode. This deliberately excludes
host-sensitive current-time, file, shell/environment, dialog, dispatch, and COM callback
intrinsics.

Follow-up `TypeOf ... Is ...` work stops treating the RHS type expression as a value lookup in HIR.
The HIR builder now records a dedicated `TypeOfIs` expression with object expression plus type-name
text, HIR lowering targets the existing `typeofis` intrinsic contract, and the route audit includes
a production fixture that emits `IntrinsicTypeOfIs` bytecode.

Follow-up time-locale host intrinsic work adds `Date()`, `Time()`, `Now()`, and `Timer()` to HIR
built-in resolution and verifies the existing host bytecode instructions through production HIR.
This proves routing only; host policy/runtime behavior remains owned by the VM/HAL contract.

Follow-up host utility intrinsic work adds `FreeFile()`/`FreeFile(range)` and `DoEvents()` to HIR
built-in resolution and verifies the existing file-number and event-pump host bytecode through
production HIR. This is still route proof only; HAL policy remains outside the front-end.

Follow-up file-position host intrinsic work adds `EOF(handle)`, `LOF(handle)`, `Seek(handle)`, and
`Loc(handle)` to HIR built-in resolution and verifies the existing file-position host bytecode
through production HIR. File statements such as `Open`, `Close`, `Kill`, `Print`, `Write`, and
`Line Input` remain separate statement-lowering surfaces.

Follow-up dialog host intrinsic work adds `MsgBox(prompt[, style])` and
`InputBox(prompt[, default])` to HIR built-in resolution and verifies the existing UI host bytecode
through production HIR. This is route proof only and does not change the HAL dialog contract.

Follow-up process/environment host intrinsic work adds `Shell(command)`, `Environ(key)`,
`Dir()`, and `Dir(path)` to HIR built-in resolution and verifies the existing process/environment
host bytecode through production HIR. COM object creation remains separate.

Follow-up COM object creation route work adds `CreateObject(progId)` to HIR built-in resolution and
verifies `IntrinsicCreateObjectHost` plus ProgID string preservation through production HIR. This
does not claim dispatch invocation, default-member, or COM binding cleanup. Fresh-eyes correction:
default-route eligibility keeps `DispatchInvoke(...)` sources on the legacy route until HIR
preserves dispatch metadata such as named argument descriptors; simple `CreateObject(...)` remains
eligible.

Follow-up dynamic-dispatch route work replaces that temporary guard: HIR now lowers
`DispatchInvoke`/`__oxvbaearlyinvoke` structural calls through a bound call-argument carrier that
preserves argument names into `IntrinsicDispatchInvokeHost`. The default route now accepts named
`DispatchInvoke(CreateObject(...), ..., value := ..., lhs := ...)` sources and verifies both
bytecode argument-name preservation and the existing named-dispatch assignment-form regression.
This is still a compatibility carrier for explicit dynamic dispatch helpers; it does not claim
full COM/default-member binding cleanup.

The broad compiler-suite run for that route flip exposed three adjacent HIR-default correctness
issues that were fixed in the same slice: declaration annotation symbols such as builtin type names
and procedure return symbols are no longer treated as runtime frame locals by the HIR lowering
contract; fixed-array accesses after a fixed-array `ReDim` now reject static indexes outside the
current bounds instead of falling through to runtime dynamic-array access; and assignment RHS
forms such as `valueOut = widget x` remain rejected for internal-class/default-member
no-parentheses getter reads rather than being accepted by the production route.

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
- production HIR lowering maps dynamic-array resizes to `BoundStmt::ReDimRuntime`, including
  two-dimensional runtime bounds such as `ReDim grid(rows - 1, cols - 1)`;
- explicit static integer lower-bound `To` dimensions such as `ReDim buf(1 To length - 1)` are
  represented as lower/upper HIR dimension pairs and emitted as runtime resize lower-bound
  metadata;
- read-side dynamic-array element access such as `x = buf(1)` lowers from HIR `IndexExpr` on a
  declared dynamic-array local to the existing `__oxvba_array_get` backend intrinsic;
- write-side dynamic-array element access such as `buf(1) = 7` lowers from the same HIR
  `IndexExpr` target shape to `BoundStmt::AssignRuntimeArrayElement` and the backend
  `IntrinsicArraySet` instruction;
- initial fixed-array declarations with static integer bounds such as `Dim a(1 To 2) As Integer`
  now materialize the same `a_0`, `a_1`, ... alias slots used by the legacy fixed-array backend,
  record static fixed-array shape metadata, and resolve static element reads/writes through those
  aliases;
- fixed-array `ReDim` / `ReDim Preserve` with static integer bounds now rematerializes alias
  slots and updates static fixed-array shape metadata before subsequent element references are
  lowered;
- local multidimensional dynamic-array element reads/writes lower to backend array get/set
  intrinsics with two index operands, and local multidimensional fixed-array element reads/writes
  resolve through the legacy-compatible linear alias calculation;
- local `Dim name() As T` declarations contribute array declaration type and runtime
  `ArrayShapeDescriptor` metadata, including element type and the lower-bound policy available to
  the HIR route;
- dynamic-array shape metadata now widens rank from observed runtime `ReDim` bounds, so a
  two-dimensional resize records rank `2` instead of the declaration seed rank; and
- the front-end `ProjectSymbolIndex` now records project field-array descriptors for class and
  procedural module fields, including dynamic fields, multidimensional fixed bounds, and omitted
  lower bounds derived from `Option Base`, and class field-array descriptors now flow into
  `ProjectDynamicObjectRoute` metadata with stable field tokens;
- dynamic class array fields now compile executable `ReDim`, element writes, and element reads
  through the per-instance field token by loading the field array into a generated temporary,
  applying the runtime array intrinsic, and writing the array value back to the field token; and
- the route audit now includes one-dimensional dynamic-array `ReDim buf(length - 1)`,
  two-dimensional dynamic-array `ReDim grid(rows - 1, cols - 1)`, and explicit lower-bound
  `ReDim buf(1 To length - 1)` fixtures, plus read- and write-side dynamic-array element
  fixtures, an initial fixed-array alias fixture, and a fixed-array `ReDim Preserve`
  rematerialization fixture. It also includes local multidimensional dynamic/fixed element
  fixtures.

This is intentionally not full `ReDim` parity. Runtime lower bounds currently match the old
production constraint: the lower side of `To` must be a static integer, while upper bounds may be
expressions. Fixed-array declaration and `ReDim` alias materialization currently require static
integer bounds and static integer element indices. Project/class array field shapes are now
front-end indexed, class shapes are emitted in dynamic-object route metadata, and dynamic class
array-field `ReDim`/element get/set is executable through the current bridge. Fixed
project/class array-field executable semantics and broader project-owned array shapes remain
broader HIR and project-semantics work.

Follow-up default-route correction narrows the earlier `OptionStmt` exclusion: `Option Base 0`,
`Option Base 1`, default-equivalent `Option Compare Binary`, and `Option Compare Text` no longer
disqualify otherwise completed HIR-default sources. HIR lowering now reuses the compiler option
compare collector and emits text comparison bytecode (`StringCompareMode::Text`) for string
comparisons under `Option Compare Text`. Follow-up FE-8.5.e work also preserves the
`Option Explicit` flag in the HIR-bound module and adds production route-audit coverage for an
otherwise completed `Option Explicit` source. The HIR lowerer now reuses the compiler DefType
default table collector, with route-audit coverage for `DefLng A-Z` applying to a local untyped
`Dim`, module-scope scalar `Dim` declarations, and DefType/type-character/explicit-`As` precedence
in parameters and function returns. Known DefType directives now remain eligible for default HIR
production routing for otherwise completed sources; unknown `Def*` directives remain outside that
route.
`Option Compare Database` now routes through HIR/default production for otherwise completed sources;
the current runtime intentionally maps Database compare to binary comparison rather than Access
collation. `Option Private Module` now routes through single-source/default HIR for otherwise
completed sources; project module-kind and reference-visibility enforcement remains in the project
route.
Basic conditional-compilation filtering now also runs before the default HIR route for otherwise
completed single-source inputs, using the resolver's physical-line normalization and existing
`#Const`/`#If`/`#Else`/`#End If` evaluator before HIR parsing. This is route coverage for the
current compiler preprocessor surface, not terminal proof of full VBA preprocessor parity or source
mapping.
Basic single-source module attributes such as `Attribute VB_Name = "Module1"` also route through
the default HIR path as ignored metadata lines when the remaining source is otherwise completed.
Project-owned module attributes and member attributes continue to be enforced by the project route
and remain part of the broader attribute-semantics residual.
Basic typed constant declarators such as `Const CBase As Long = 7` also route through default HIR
and substitute the value into procedure bytecode. This covers the current literal/simple-expression
constant evaluator; typed constant coercion, overflow/type diagnostics, and broader compile-time
expression parity remain open.
Other declaration/compile-time surfaces remain outside the lightweight default route until HIR owns
their semantics, and broader DefType surfaces for visibility-prefixed class/project fields remain
open.

## Member Expression Continuation

The twenty-sixth FE-8.5 slice removes the first explicit-receiver dot-member expression residual:

- `MemberExpr` nodes with a normal receiver and dot member name lower into HIR member expressions
  and allocate member symbols that are visible to compiler-owned semantic queries;
- production HIR lowering maps value-side member reads to the existing `BoundExpr::Member` backend
  shape;
- `CallExpr` targets that lower to member expressions preserve positional arguments, so
  `obj.Method(1)` reaches the existing late-bound member-call bytecode path; and
- the route audit now includes a fixture for `x = obj.Value` and `y = obj.Method(1)`.

This is intentionally not full member/property/object parity. Bang access, member-write targets,
`With` dot-shorthand, `New`/object construction, default-member resolution, property Get/Let/Set
selection, project/class binding, early-bound COM binding, ByRef/writeback behavior, and
host-provided member semantics remain FE-7/FE-8 residuals at this point in the history below.

Follow-up continuation adds the first `With` route slice: read-side dot-prefixed member expressions
inside `With obj ... End With` are bound to the active With receiver and lower through the existing
late-bound member read path. With member assignment targets remain fallback-eligible because member
write/property Let/Set semantics are still broader FE-7/FE-8 work.

Follow-up continuation adds the first production HIR member-write route: explicit receiver member
targets and `With` dot-prefixed member targets now lower to late-bound dispatch with an explicit
property Let or property Set hint. This removes the raw fallback for simple `obj.Value = ...`,
`obj!Value = ...`, `Set obj.Ref = ...`, and `.Value = ...` target shapes. It does not close
default-member selection, indexed/named writeback breadth, project/class property resolution,
early-bound COM property put, or full property Let/Set overload validation.

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
- The project compile boundary now uses those HIR construction facts for the accepted direct
  active-project `Set x = New Widget` shape: it reconstructs the generated project-instance helper
  assignment back to a HIR `New` expression, passes the matching `HirNewExpressionBinding` facts to
  HIR lowering, and only falls back when the project shape remains unsupported by HIR.

Remaining production residuals after this slice:

- `Dim As New`, `Class_Initialize`, construction source maps, WithEvents construction interaction
  beyond the existing compatibility workaround, and imported/COM construction still need direct-HIR
  integration under `bd-aprs.9.7` and `bd-aprs.8.8`.

## Project Construction Compile-Entry Continuation

The latest FE-8.5 construction slice adds the missing compile entry point for those binding facts:

- `compile_source_with_runtime_metadata_via_hir_with_new_bindings(...)` accepts
  `HirNewExpressionBinding` facts, lowers `New <Class>` through typed HIR, then runs the normal
  typecheck, optimizer, and bytecode/metadata emission path.
- The focused test proves the emitted bytecode includes the existing project-object reference load
  path when the constructed object remains live.
- Follow-up `bd-aprs.9.6` work now calls this entry point from `compile_project(...)` for the
  accepted direct active-project `Set x = New Widget` construction shape.

## Project Boundary HIR Route Continuation

The latest FE-8.5/FE-9.1 project slice routes a narrow production project shape through the
HIR-capable metadata compiler:

- `compile_project(...)` now calls the HIR-capable compile wrapper for single active
  procedural-module projects with no reference projects. Unsupported HIR shapes still fall back
  inside that wrapper.
- Multi-module projects, class/document modules, forced-object-local shapes, and reference-project
  shapes still call the legacy project backend directly. A first attempt to route broader projects
  exposed module-qualified metadata drift, so the eligibility was narrowed before commit.
- The focused regression uses an inline statement sequence that the legacy-only metadata path cannot
  parse, proving this project boundary now reaches HIR for completed constructs.

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
remaining construction residuals are narrower: `Dim As New`, `Class_Initialize`, construction
source maps, broader WithEvents construction, and imported/COM construction still need direct-HIR
integration after the accepted direct `Set x = New Widget` route.

## Direct Project Construction HIR Continuation

The `bd-aprs.9.6` slice removes the direct active-project `Set x = New <Class>` helper-source
compile residual:

- `compile_project(...)` now derives a HIR construction source from the module-aware lowered source
  by replacing only generated `Set <var> = __oxvba_project_instance(handle)` assignment carriers
  with `Set <var> = New <constructor-type>` when a matching `HirNewExpressionBinding` fact exists.
- The HIR compile entry point consumes those facts and emits the typed project-object reference
  bytecode; unsupported project shapes still fall back without silently claiming HIR ownership.
- The compiled project's public `rewritten_source` now shows the HIR construction source for the
  accepted direct construction fixture, so route evidence no longer reports helper-source
  compilation for `Set obj = New Widget`.
- Focused regressions cover the reconstruction helper, reject non-assignment helper-call rewrites,
  and prove `compile_project(...)` emits `LoadProjectObjectRef` while preserving dynamic object
  route metadata for the constructed `Widget`.

Remaining construction residuals after this slice are explicitly not closed: `As New`,
`Class_Initialize`, source-map/lifetime metadata, imported/COM construction, and WithEvents
construction beyond the existing temporary workaround remain owned by `bd-aprs.9.7` /
`bd-aprs.8.8`.

## As New Construction HIR Continuation

The `bd-aprs.9.7` continuation moves accepted active-project `Dim x As New <Class>` off the
project-instance helper-source compile artifact on the HIR construction route:

- Active-project construction bindings now include both explicit `Set x = New <Class>` and
  `Dim x As New <Class>` source kinds when deriving `HirNewExpressionBinding` facts.
- The baseline rewritten source remains fallback-compatible for unsupported project shapes, while
  the HIR construction candidate derives a separate source: eager `As New` helper carriers are
  removed, and guarded first-use/after-`Nothing` `If x Is Nothing Then Set x = New <T>` sites are
  inserted before accepted dereference lines.
- `compile_project(...)` reconstructs generated explicit `Set x = __oxvba_project_instance(handle)`
  carriers back to `Set x = New <constructor-type>` and supplies one
  `HirNewExpressionBinding` fact for each generated/reconstructed `New` occurrence.
- Source-class `WithEvents` assignments from `New <Class>` now use the same construction candidate:
  the generated temporary assignment is restored to `Set __oxvba_withevents_new_instance_N = New T`,
  the runtime `__oxvba_withevents_set(...)` side effect receives that temporary rather than an inline
  `New`, and the compiled artifact no longer depends on the project-instance carrier for the
  accepted fixture.
- HIR parameter lowering now reuses the existing procedure-signature parser for parameter metadata,
  preserving generated optional/default guard-wrapper parameters needed by the WithEvents route.
- HIR symbol binding now declares the typed structural-intrinsic prelude from
  `StructuralIntrinsic`, and HIR lowering maps those call targets back to typed structural
  intrinsic calls. This lets generated field-storage helper calls in `Class_Initialize` bodies stay
  on the HIR route instead of failing name binding.
- The focused `As New` regression covers a field-mutating `Class_Initialize` body, verifies that
  the compiled artifact no longer contains `__oxvba_project_instance(...)`, checks
  `LoadProjectObjectRef` bytecode, confirms dynamic route metadata still retains the initializer
  member, and checks the original module source line remains mapped.
- The reset regression proves `Set x = Nothing` followed by another accepted dereference produces a
  second guarded `New` site and a second `LoadProjectObjectRef`, so the accepted HIR route no
  longer models `As New` as declaration-time-only construction. It also covers the accepted
  active-project lifetime/source-map slice: private `Class_Terminate` is retained in the HIR
  construction source, the dynamic object route retains termination metadata, and both the
  first-use and after-`Nothing` user dereference lines remain mapped.

This closes the scoped `bd-aprs.9.7` active-project construction lane. Imported/reference/COM
activation remains owned by `bd-aprs.8.8`; unsupported project shapes remain compatibility fallback
until classified by the broad route audit; broader event semantics remain under FE-7/FE-9 coverage.

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
- This does not close full member writes: default-member/property selection, project/class and
  early-bound COM property-put resolution, indexed/named writeback breadth, and property Let/Set
  overload validation remain tracked residual work.

## Checks

- `cargo test -p oxvba-compiler hir_production_lowering_accepts_bang_member_access --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_expression_const_statement --quiet`
- `cargo test -p oxvba-host --test source_member_call_statements --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_fields_are_per_instance_storage --quiet`
- `cargo test -p oxvba-host pure_oxvba_class_distinct_new_instances_have_separate_state --quiet`
- `cargo test -p oxvba-compiler compile_project_lowers_withevents_new_source_class_expression --quiet`
- `cargo test -p oxvba-compiler project_hir_construction_source_restores_new_expression_from_binding_facts --quiet`
- `cargo test -p oxvba-compiler compile_project_consumes_hir_new_bindings_for_active_project_set_new --quiet`
- `cargo test -p oxvba-compiler compile_project_consumes_hir_new_bindings_for_as_new_and_initializer --quiet`
- `cargo test -p oxvba-compiler compile_project_lazily_reconstructs_as_new_after_set_nothing --quiet`
- `cargo test -p oxvba-compiler compile_project_lowers_withevents_new_source_class_expression --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_preserves_optional_parameter_defaults --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_preserves_param_array_metadata_and_call_pack --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_ubound_on_param_array --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_param_array_through_hir --quiet`
- `cargo test -p oxvba-compiler compile_paramarray --quiet`
- `cargo test -p oxvba-compiler hir_lowering_lowers_structural_intrinsic_call_targets --quiet`
- `cargo test -p oxvba-compiler compile_project_uses_hir_capable_boundary_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler compile_project_rewrites_module_qualified_calls_for_unique_names --quiet`
- `cargo test -p oxvba-compiler withevents --quiet`
- `cargo test -p oxvba-compiler hir_compile_binds_new_expression_to_project_instance_bytecode --quiet`
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
  optional/default breadth, intrinsic-backed ParamArray callee bodies, member dispatch, and
  writeback semantics remain open FE-8.5/FE-7 delivery work.
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
