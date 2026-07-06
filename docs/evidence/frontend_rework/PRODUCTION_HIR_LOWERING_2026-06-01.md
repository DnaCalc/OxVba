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
class/object-local compatibility contexts were tracked residuals at this point until their HIR facts
and route proofs were complete.

Follow-up FE-8.5.f route work narrows that residual: optional parameters with simple explicit
defaults now remain eligible for the default HIR path and preserve optional/default signature
metadata for otherwise completed single-source inputs. A later optional-parameter continuation also
routes optional parameters without explicit defaults through HIR, preserving the existing
`VariantMissingError448` / declared-type-default descriptor policy and omitted-argument call-site
metadata. The default-route eligibility guard now also checks HIR/parsed signature parameter-name
alignment, and the symbol collector now avoids declaring `As` type-reference tokens as parameters
in typed multi-parameter signatures. Richer default expressions remain outside the lightweight
default route.
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
richer default expressions, or broader optional call-entry combinations beyond the focused omitted
argument/default-state route.

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
broader optional call-entry combinations remained open.

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

Follow-up statement-host route work adds HIR statement nodes for console `Print` and diagnostics
`Debug.Print`, lowering them to the existing console/debug host bytecode. Multi-field `Debug.Print`
payloads are preserved as HIR concatenation expressions before bytecode emission. This is not file
I/O statement closure; `Open`, `Close`, file `Print #`, `Write #`, `Input #`, and `Line Input #`
remain separate statement-lowering surfaces.

Follow-up file-system statement route work adds the first HIR-owned file statement:
`Kill path` now lowers through HIR to the existing file-kill host bytecode. This is still not
file-handle I/O closure; `Open`, `Close`, `Print #`, `Write #`, `Input #`, and `Line Input #`
remain open statement-lowering surfaces.

Follow-up console-input route work adds HIR-owned `Input a[, b...]` statement lowering to the
existing console-input host bytecode, preserving one input operation per target. This does not claim
file input closure; `Input #` and `Line Input #` remain separate file-handle statement surfaces.

Follow-up console line-input route work adds HIR-owned `Line Input target` lowering to the existing
console line-input host bytecode. This remains distinct from file `Line Input #`, which is still an
open file-handle statement surface.

Follow-up file-close route work adds HIR-owned `Close #handle` and `Close` lowering to the existing
file-close host bytecode, including close-all emission. `Open`, file `Print #`, `Write #`, `Input #`,
and file `Line Input #` remain open file-handle statement surfaces.

Follow-up file-print route work adds HIR-owned `Print #handle, data` lowering to the existing
file-print host bytecode for simple literal/name handle and payload expressions. `Open`, `Write #`,
`Input #`, and file `Line Input #` remain open file-handle statement surfaces.

Follow-up file-write route work adds HIR-owned `Write #handle, item[, ...]` lowering to the existing
file-write host bytecode for simple literal/name handle and payload expressions, including
multi-item writes. `Open`, `Input #`, and file `Line Input #` remain open file-handle statement
surfaces.

Follow-up file-input route work adds HIR-owned `Input #handle, target[, ...]` lowering to the
existing file-input host bytecode, including multi-target input. `Open` and file `Line Input #`
remain open file-handle statement surfaces.

Follow-up file-line-input route work adds HIR-owned `Line Input #handle, target` lowering to the
existing file line-input host bytecode. `Open` remains an open file-handle statement surface.

Follow-up file-open route work adds HIR-owned `Open path For mode As #handle` lowering to the
existing file-open host bytecode for simple literal/name path and handle expressions and the
existing `Input`/`Output`/`Append`/`Binary`/`Random` mode set.

Together, the file-handle statement continuations cover the audited simple `Open`, `Close`,
`Print #`, `Write #`, `Input #`, and `Line Input #` route fixtures through HIR production and the
existing host bytecode. Earlier "remain open" notes in this chronological section describe the
state before those follow-up slices, not the final state of the current evidence file. Broader VBA
file I/O options and host behavior remain outside this claim.

Follow-up expression route work adds HIR-owned `Mod`, integer division, and `Like` lowering to the
existing arithmetic/comparison bytecode paths.

Follow-up no-argument call route work lowers resolved `Call <procedure>` statements through HIR as
zero-argument calls, covering `Call Worker` without a parenthesized argument list.

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

Follow-up validation now rejects non-VBA `RaiseEvent` argument-list shapes before lowering:
arguments must be parenthesized when present, zero-argument events must be raised without empty
parentheses, named event arguments reject, omitted event slots reject, and event arity must match
the declared event signature. Full project event binding remains broader event/COM work outside
this narrow route slice.

Follow-up continuation accepts module-level `Event` declarations on the HIR production route when
paired with the already-supported `RaiseEvent` statement. The event declaration is currently a
symbol/fact declaration with no direct bytecode; full WithEvents handler matching, Implements
coupling, and project event binding remain broader event/COM work.

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
- fixed class array fields now compile executable element writes and reads through the same
  per-instance field token and runtime array get/set path without rewriting field declarations as
  executable array reads or emitting resize bytecode; and
- fixed procedural-module array fields now compile executable element writes and reads through the
  same module-state field token and runtime array get/set path without rewriting field declarations
  as executable array reads or emitting resize bytecode; and
- dynamic procedural-module array fields now compile executable `ReDim`, element writes, and
  element reads through the same module-state field token and runtime array resize/get/set path; and
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
front-end indexed, class shapes are emitted in dynamic-object route metadata, dynamic class
array-field `ReDim`/element get/set is executable through the selected module-aware plan, fixed class
array-field element get/set is executable without resize, and fixed procedural-module array-field
element get/set is executable without resize, and dynamic procedural-module array-field
`ReDim`/element get/set is executable through module-state tokens. The old project rewrite bridge is
not production-selected; it remains a `#[cfg(test)]` parity path. Remaining project-owned
array-shape work is native HIR ownership and metadata replacement of compatibility-carrier source
construction, not a known missing fixed/dynamic class/procedural executable get/set route.

2026-06-02 array-field route-proof hardening: representative dynamic class, fixed class, and
dynamic procedural-module array-field routes now assert that the generated compatibility carriers
read and write through deterministic frontend field tokens (`__oxvba_withevents_get/set(owner,
field_token, ...)`) before reaching runtime array get/set/resize bytecode. This prevents generated
compatibility carriers from drifting away from frontend field-route metadata, but it does not remove
the remaining carrier-source construction body or claim native HIR ownership for those field-array
routes. A follow-up selector guard asserts that `compile_project(...)` selects
`ModuleAwareBindPlan`; `RewriteBridge` is test-only parity evidence.

2026-06-03 bead split: `bd-aprs.9.8` is now the local procedure array/indexing/`ReDim` parity
bead. The remaining project/class field-array carrier retirement is explicit `bd-aprs.9.13`,
because native ownership requires field get/mutate/writeback lowering and metadata instead of only
the local array element path or token-backed generated source proof.

2026-06-03 array-field carrier reduction: field-array element assignment now rewrites to the
internal `__oxvba_array_field_set(owner, field_token, index..., value)` intrinsic instead of a
generated `Dim temp() / temp = field / temp(i) = value / field = temp` source block. The intrinsic
emits `IntrinsicWithEventsGet`, `IntrinsicArraySet`, and `IntrinsicWithEventsSet` directly, so the
writeback sequence is bytecode-owned and token-backed. Field-array reads now similarly rewrite to
`__oxvba_array_field_get(owner, field_token, index...)`, which emits `IntrinsicWithEventsGet` plus
`IntrinsicArrayGet` directly instead of nested generated
`__oxvba_array_get(__oxvba_withevents_get(...), ...)` source. This is still partial
`bd-aprs.9.13` progress.

2026-06-03 upper-bound field-array `ReDim` carrier reduction: upper-bound-only field-array
`ReDim`/`ReDim Preserve` now rewrites to internal `__oxvba_array_field_redim*` intrinsics instead
of a generated `Dim temp() / temp = field / ReDim temp(...) / field = temp` block. The intrinsics
emit `IntrinsicWithEventsGet`, runtime array resize with the same Variant element type as the old
untyped temp carrier, and `IntrinsicWithEventsSet`; the optimizer now treats these field-array
resize/writeback intrinsics as observable effects so discard-slot dead-store elimination cannot
drop them.

2026-06-03 explicit lower-bound field-array `ReDim` carrier reduction: explicit literal
lower-bound field-array `ReDim To`/`ReDim Preserve To` forms now rewrite to paired-bound
`__oxvba_array_field_redim*_bounds` intrinsics instead of a generated temp-array block. The paired
form keeps lower bounds as static resize metadata and upper bounds as runtime expression slots, then
emits field get, runtime resize, and field writeback. This matches the existing accepted dynamic
array `ReDim` contract: lower bounds are static integers, while upper bounds may be expressions.
Full closure still requires native HIR-owned project/class field-array lowering and metadata.

2026-06-03 procedural module array-field metadata ownership reduction: `ProjectSymbolIndex` now
exposes frontend-owned module field names, and `ModuleStateBindings` uses those names for procedural
module field-token discovery when the project symbol index is available. This keeps procedural
field-array fixed/dynamic classification tied to frontend project facts instead of depending on the
legacy declaration-line scan as the authoritative name source. Full closure still requires retiring
or quarantining the remaining project rewrite bridge that injects internal field-array intrinsics
before HIR lowering.

Superseding 2026-07-06 field-array closure: the structural residual above no longer exists in the
live crate graph. Static route review finds no `oxvba-compiler` crate, no
`lower_module_source_module_aware`, and no `__oxvba_array_field_*` helper-source carriers under
`crates/`. The current binder resolves dotted field-array `ReDim` targets into `CorePlace::Field`
and emits `CoreStmt::ReDim`; OxIR performs explicit materialize-resize-writeback for compound
`ReDim` targets and fuses `obj.field(i...)` / `obj.field(i...) = value` into
`OxInst::FieldArrayGet` / `OxInst::FieldArraySet`. VM3 executes the fused element operations
through borrowed project-field SAFEARRAY descriptors, preserving O(1) field-array element access.
Evidence for this closure includes:

- `cargo test -p oxvba-oxir class_field_arrays_elaborate_to_fused_ops -- --nocapture`
- `cargo test -p oxvba-oxir record_array_fields_elaborate_to_fused_ops -- --nocapture`
- `cargo test -p oxvba-bind redim_member_array_then_use -- --nocapture`
- `cargo test -p oxvba-differential --test field_array_access_vm3 -- --nocapture`
- `rg -n "__oxvba_array_field_|lower_module_source_module_aware|compile_project_rewrites" crates -g '*.rs'`
  returns no live helper-source/rewrite hits.

This closes `bd-aprs.9.13` for the accepted project/class field-array carrier-retirement surface.
It does not close broader FE-7 property/default-member semantics, reference/imported-COM
activation, or FE-9 final legacy-route retirement.

Follow-up default-route correction narrows the earlier `OptionStmt` exclusion: `Option Base 0`,
`Option Base 1`, default-equivalent `Option Compare Binary`, and `Option Compare Text` no longer
disqualify otherwise completed HIR-default sources. HIR lowering now reuses the compiler option
compare collector and emits text comparison bytecode (`StringCompareMode::Text`) for string
comparisons under `Option Compare Text`. Follow-up FE-8.5.e work also preserves the
`Option Explicit` flag in the HIR-bound module, adds production route-audit coverage for an
otherwise completed `Option Explicit` source, and admits that option to the lightweight default HIR
route once metadata preservation was executable-test visible. The HIR lowerer now reuses the
compiler DefType
default table collector, with route-audit coverage for `DefLng A-Z` applying to a local untyped
`Dim`, module-scope scalar `Dim` declarations, and DefType/type-character/explicit-`As` precedence
in parameters and function returns. Known DefType directives now remain eligible for default HIR
production routing for otherwise completed sources; unknown `Def*` directives remain outside that
route. Later coverage hardening proves visibility-prefixed procedural module scalar fields such as
`Private alpha` and `Public beta%` use the same DefType/type-character precedence through direct
HIR lowering, the lightweight default route, and route-audit classification.
`Option Compare Database` is no longer silently admitted through a Binary comparison
approximation: the scanner now reports `SYM-E-UNSUPPORTED-OPTION-COMPARE-DATABASE`, matching the
Microsoft Learn `Option Compare` documentation that `Database` is Access-only and database
collation dependent. `scanner_rejects_option_compare_database_collation` covers the diagnostic;
`Option Compare Binary`/`Text` remain the supported compare modes. Checks passed for this cleanup:
`cargo test -p oxvba-symbol option_compare --quiet`;
`cargo test -p oxvba-bind option_compare --quiet`;
`cargo test -p oxvba-bind scalar_option_compare_text_boolean_const_expression_executes --quiet`;
`cargo test -p oxvba-symbol --quiet`; `cargo test -p oxvba-bind --quiet`;
`cargo fmt --check -p oxvba-symbol -p oxvba-bind`; `cargo check --workspace`;
`git diff --check`; `./scripts/check-governance.ps1`. `Option Private Module` now routes through
single-source/default HIR for otherwise completed sources, is preserved on the production
`BoundModule`, and feeds compile-derived bundle module facts from the bound front-end surface;
project module-kind and reference-visibility enforcement remains in the project route. Clean
symbol-stack continuation on 2026-06-17 moved the
cross-project export boundary from loader-only metadata to the scanner-owned module facts:
`ModuleScan` now records source `Option Private Module` and export-surface synthesis consults that
fact alongside `ModuleAttributes`. The regression
`source_option_private_module_is_project_private` proves a direct symbol manifest with the source
directive, but no pre-populated manifest flag, hides both the procedural module and its public
constants from the published reference surface. A follow-up source-attribute slice moved
`Attribute VB_Name` to the same scanner-owned boundary: `scan_module` now prefers the source
module-name attribute over loader/manifest fallback names, and
`source_vb_name_attribute_names_exported_module` proves the published surface uses the VBA semantic
module name rather than the storage/manifest fallback when callers build direct symbol manifests.
The same scanner attribute record now carries source `VB_Exposed`, `VB_Creatable`,
`VB_PredeclaredId`, and `VB_GlobalNamespace`; export-surface synthesis uses those source facts for
class publication, creatability, predeclared singletons, and global class-member injection before
falling back to loader metadata. `source_boolean_module_attributes_shape_class_surface` proves a
direct manifest with false loader flags still publishes the class according to its exported source
headers.
Clean symbol-stack DefType continuation on 2026-06-17 moved default-type ownership into
`ModuleScan` signature/type assignment rather than relying on a legacy compiler collector.
The scanner now recognizes source `DefBool`/`DefByte`/`DefInt`/`DefLng`/`DefLngLng`/
`DefLngPtr`/`DefCur`/`DefSng`/`DefDbl`/`DefDate`/`DefStr`/`DefObj`/`DefVar` directives, applies
the documented `As <type>` > type-character > DefType > Variant precedence to variables,
parameters, and Function/Property Get return types, and keeps constants/UDT fields out of the
DefType rule. `scanner_applies_deftype_to_variables_params_and_returns` and
`scanner_honors_type_precedence_over_deftype` prove the symbol facts; the clean-stack
`scalar_deftype_defaults_affect_variables_params_and_returns` regression proves those facts reach
runtime coercion for a numeric DefType route. Follow-up clean-stack type/coercion cleanup adds an
explicit variable-length String store target (`CoerceTarget::String` / `Op::CoerceString`) that
delegates to the runtime `Variant` string coercion helper, so declared `As String` and `DefStr`
variables, `ByVal` parameters, and Function returns store BSTR-backed String Variants instead of
retaining numeric payloads. Because this extends the serialized clean-bundle instruction set,
`BUNDLE_PACKAGE_VERSION` is now `2`, with
`bundle_package_uses_current_version_and_rejects_previous_version` covering the version gate.
`scalar_string_store_coerces_numeric_values` and
`scalar_deftype_string_defaults_affect_variables_params_and_returns` prove the explicit and DefStr
routes through bind -> bundle linearization -> VM execution. Checks passed: `cargo test -p
oxvba-bind scalar_string_store_coerces_numeric_values --quiet`; `cargo test -p oxvba-bind
scalar_deftype_string_defaults_affect_variables_params_and_returns --quiet`; `cargo test -p
oxvba-bind --quiet`; `cargo test -p oxvba-vm2 --quiet`; `cargo test -p oxvba-bundle --quiet`;
`cargo check --workspace`; `cargo fmt --check -p oxvba-bundle -p oxvba-vm2 -p oxvba-bind`; `git
diff --check`; `./scripts/check-governance.ps1`. A later duplicate-range diagnostic pass now
enforces the documented rule that a DefType range cannot include a previously defined letter:
`DefaultTypeTable::set_range` checks the whole range before mutation and reports
`SYM-E-DUPLICATE-DEFTYPE-RANGE` through `SymbolModelError::DuplicateDefTypeLetter`.
`scanner_rejects_duplicate_deftype_letter_ranges` and `scanner_rejects_deftype_after_a_z_range`
cover overlapping subranges and the `A-Z` umbrella case. Checks passed: `cargo test -p
oxvba-symbol deftype --quiet`; `cargo test -p oxvba-symbol --quiet`; `cargo test -p oxvba-bind
--quiet`; `cargo check --workspace`; `cargo fmt --check -p oxvba-symbol`; `git diff --check`;
`./scripts/check-governance.ps1`. Follow-up fixed-length String store hardening keeps the same
runtime coercion authority for `String * N` targets: `Op::CoerceFixedString` now first delegates to
the runtime `Variant` string coercion helper and only then applies fixed-length UTF-16 code-unit
padding/truncation. This preserves numeric-to-string assignment such as `7 -> "7  "` while raising
runtime error 13 for invalid coercions such as `Null -> String * 3`, instead of converting failures
through the lossy display helper into spaces. Regression coverage:
`fixed_length_string_store_coerces_numeric_values` and `fixed_length_string_store_rejects_null`.
Checks passed for the fixed-length follow-up: `cargo test -p oxvba-bind fixed_length_string --quiet`;
`cargo test -p oxvba-bind --quiet`; `cargo test -p oxvba-vm2 --quiet`; `cargo check --workspace`;
`cargo fmt --check -p oxvba-vm2 -p oxvba-bind`; `git diff --check`; `./scripts/check-governance.ps1`.
Follow-up DefDec cleanup makes that boundary explicit in the clean scanner: `DefDec` now reports
`SYM-E-UNSUPPORTED-DEFDEC` because the current type model follows the VBA Decimal policy of
supporting Decimal as a Variant subtype/runtime payload rather than ordinary declared storage.
`scanner_rejects_defdec_declared_decimal_storage` covers the diagnostic. Broader declaration/type
diagnostics remain later work.
The next Decimal cleanup applies the same policy to explicit type references: bare `As Decimal`
now reports `SYM-E-UNSUPPORTED-DECLARED-DECIMAL` during symbol scanning instead of falling through
as an unresolved object type named `decimal`. Qualified/user type references are left to ordinary
object/type resolution. Regression coverage: `scanner_rejects_explicit_declared_decimal_storage`,
`scanner_allows_qualified_decimal_type_reference`, with
`scanner_rejects_defdec_declared_decimal_storage` retained for the DefType directive path. Checks
passed for the explicit-declaration follow-up: `cargo test -p oxvba-symbol decimal --quiet`;
`cargo test -p oxvba-symbol deftype --quiet`; `cargo test -p oxvba-symbol --quiet`;
`cargo test -p oxvba-bind --quiet`; `cargo fmt --check -p oxvba-symbol`;
`cargo check --workspace`; `git diff --check`; `./scripts/check-governance.ps1`.
Basic conditional-compilation filtering now also runs before the default HIR route for otherwise
completed single-source inputs, using the resolver's physical-line normalization and existing
`#Const`/`#If`/`#ElseIf`/`#Else`/`#End If` evaluator before HIR parsing. This is route coverage for the
current compiler preprocessor surface, not terminal proof of full VBA preprocessor parity or source
mapping.
Follow-up project-route work feeds `ProjectManifest::conditional_constants` into the same
preprocessor before project procedure discovery and HIR/project lowering, so active project modules
can select `#If` branches from manifest-supplied constants while source `#Const` directives remain
able to override the initial environment. A host-facade follow-up proves embedded build-workspace
requests that carry a compiler `ProjectManifest` observe the same constants. Later
language-service workspace work threads the manifest constants into the same compiler-owned
preprocessor before semantic snapshots are built for active-project modules, so IDE diagnostics and
symbol facts hide inactive conditional branches consistently with production project compilation.
The `.basproj` loader now also derives the load-time preprocessor target from
`DefaultRuntimeProfile` for validation/rewrite decisions that happen before host execution can
overwrite the symbol manifest target. In particular, `macos-headless` selects the Mac conditional
host during library top-level-mainline validation; the regression
`default_runtime_profile_selects_load_time_conditional_host` proves a Mac-only safe branch is
accepted while the default Windows target still rejects the active top-level `#Else` branch.
This is not a lossless conditional CST yet: semantic snapshot spans are still based on the filtered
analysis source when manifest constants are present, so editor-grade inactive-region/span
preservation remains open. Later bounded evaluator work accepts checked integer `#Const` and `#If`
arithmetic with unary signs, `+`, `-`, `*`, guarded `\`, and `Mod`, composed with comparison/logical
conditional expressions. A follow-up Boolean operator pass adds `Xor`, `Eqv`, and `Imp` to that
preprocessor evaluator and proves the selected branch reaches the default HIR route. Broader
compile-time expression/name parity remains open.
Basic single-source module attributes such as `Attribute VB_Name = "Module1"` also route through
the default HIR path when the remaining source is otherwise completed, and the declared name is
preserved on the production `BoundModule`. Follow-up attribute work also preserves
`VB_PredeclaredId`, `VB_GlobalNamespace`, `VB_Exposed`, and `VB_Creatable` Boolean module attributes
as production `BoundModule` metadata. The semantic effects of those module attributes and member
attributes continue to be enforced by the project route and remain part of the broader
attribute-semantics residual.
Basic typed constant declarators such as `Const CBase As Long = 7` also route through default HIR
and substitute the value into procedure bytecode. Follow-up route coverage proves typed simple
expression declarators and same-statement typed references, for example
`Const CBase As Long = 2 ^ 3 \ 2 Mod 3, CTotal As Long = CBase + 4`. The current typed
`Byte`/`Integer`/`Long` subset now folds exact integer expressions to `IntConst` / `LoadConstI32`
rather than preserving runtime arithmetic bytecode, while still using checked nonnegative
exponentiation, integer division, and `Mod`. True division remains outside that exact fold.
Later focused work carries that constant environment across source-ordered `Const` statements for
the same bounded evaluator, so `Const CTotal As Long = CBase + 4` can reference a prior
`Const CBase As Long = ...`, route through the default HIR path, stay out of runtime local slots,
and still receive range diagnostics.
Later focused diagnostic passes reject explicit `As Byte`, `As Integer`, and `As Long` integer
expressions that overflow their VBA ranges before the unsupported-const fallback can hide them,
including checked exponentiation overflow such as `2 ^ 31` for `Long`. A later FE-8.5.e
continuation extends that HIR-owned diagnostic path to explicit `As LongLong` and `As LongPtr`
integer expressions that overflow the signed 64-bit carrier range, for example
`9223372036854775807 + 1`. Follow-up carrier work adds a signed-64-bit bound expression and
bytecode carrier for explicit `As LongLong` and `As LongPtr` constants, with coverage for both
small values and values that exceed the old i32 literal carrier, plus VM execution coverage for
`Const CTotal As LongLong = 5000000000`.
Follow-up literal-kind work extends the bounded production `Const` substitution evaluator to
simple `Double` literals, including decimal/exponent literals and `#`-suffixed Double literals, with
default-route and VM execution coverage for `Const CTotal As Double = 1.5`. A later exact-carrier
continuation adds declared `Currency` and deterministic `#...#` `Date` literals to the same
production substitution path using `BoundExpr::CurrencyConst(i64)` / `LoadConstCurrency` and
`BoundExpr::DateConst(u64)` / `LoadConstDate`, with default-route and VM execution coverage for
`Const CAmount As Currency = 1.25@` and `Const CStamp As Date = #2026-02-28#`. Later month-name
Date coverage fixes HIR Const declarator splitting so commas inside `#February 28, 2026#` are not
treated as declarator separators; the same statement can still split a following declarator such as
`CNext As Date = CStamp + 1`, with direct HIR, route-audit, and VM execution proof. Follow-up
untyped Date literal work adds `Const CStamp = #2026-02-28#` to the generic literal collector so it
materializes as `DateConst`/`LoadConstDate` instead of an unsupported Const or runtime expression.
The numeric Date literal follow-up accepts unambiguous `month/day/year` forms such as
`#2/28/2026#` in module constants and optional Date defaults, including omitted-argument VM binding;
ambiguous locale-sensitive numeric dates such as `#2/3/2026#` now fail through the canonical
date-literal parser instead of silently choosing a culture-specific order, while deterministic
same month/day values such as `#1/1/2026#` still parse. Coverage:
`date_literals_accept_unambiguous_numeric_orders`,
`date_literals_reject_ambiguous_numeric_orders`, and
`scalar_ambiguous_numeric_date_literal_rejected`. Checks passed for the ambiguous-Date follow-up:
`cargo test -p oxvba-symbol date_literals --quiet`;
`cargo test -p oxvba-bind scalar_numeric_month_day_date_const_carrier_executes --quiet`;
`cargo test -p oxvba-bind scalar_ambiguous_numeric_date_literal_rejected --quiet`;
`cargo test -p oxvba-bind date --quiet`; `cargo test -p oxvba-symbol --quiet`;
`cargo test -p oxvba-bind --quiet`; `cargo fmt --check -p oxvba-symbol -p oxvba-bind`;
`cargo check --workspace`; `git diff --check`; `./scripts/check-governance.ps1`.
The runtime Date coercion follow-up moves that deterministic date parser into `oxvba-runtime`
and reuses it from vm2 `CoerceNumeric(Date)`, so `Dim stamp As Date: stamp = "2026-02-28"`
stores `Variant::from_date_f64(46081.0)` instead of requiring a numeric serial string. Excel 16.0
accepted the same ISO text and displayed `2026/02/28`. Ambiguous numeric date text remains rejected
in OxVba because Excel resolves it through host locale/order (`"2/3/2026"` displayed as
`2026/02/03` in the local probe), and this runtime subset deliberately avoids hidden
locale-sensitive parsing. Coverage: runtime date parser tests,
`scalar_string_date_store_coerces_deterministic_text`,
`scalar_ambiguous_string_date_store_rejected`; checks passed:
`cargo test -p oxvba-runtime --quiet`; `cargo test -p oxvba-symbol --quiet`;
`cargo test -p oxvba-bind --quiet`; `cargo test -p oxvba-vm2 --quiet`;
`cargo fmt --check -p oxvba-runtime -p oxvba-symbol -p oxvba-vm2 -p oxvba-bind`;
`cargo check --workspace`; `git diff --check`; `./scripts/check-governance.ps1`.
A subsequent `Single` carrier slice adds
`BoundExpr::SingleConst(u32)` and
serialized `LoadConstF32`, with bundle format v17 and VM execution coverage for `Const CTotal As
Single = 1.5!`. Later scalar-to-string concat work lets covered typed and untyped `String` constants
fold source-prior scalar constants across `&`, such as `Prefix & CNumber & CFlag` materializing as
`LoadConstString "v7True"`. Later exact-carrier and string-to-declared-scalar follow-ups narrow the
typed constant coercion residual; broader constant-name/expression parity, coercions outside those
covered carrier paths, Date/Currency expression coercion beyond the covered numeric arithmetic and
deterministic string-Date store subset, and full platform `LongPtr` semantics remain open.
Other declaration/compile-time surfaces remain outside the lightweight default route until HIR owns
their semantics, and broader DefType surfaces for class/project field semantics remain open.

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
- Follow-up declaration routing also covers simple `Declare PtrSafe Sub ...` statement calls
  through the same HIR-owned external descriptor and host invocation bytecode path; the route audit
  includes a declared external Sub fixture.
- Follow-up ByRef declaration proof confirms `Declare PtrSafe Sub ... (ByRef x As Long)` calls
  preserve the existing `ExternalCallWritebackKind::ByRefValue` metadata through HIR lowering.
- Follow-up ordinal-alias declaration proof confirms `Declare PtrSafe Function ... Alias "#0007"`
  normalizes to `#7`, keeps `ordinal_alias`, preserves the ordinal selection policy, and remains
  on the audited HIR production route.
- Follow-up typed-signature declaration proof confirms multi-argument external functions with
  non-`Long` parameter and return metadata, such as `String` and `Integer`, preserve
  `DeclareParamType` descriptors through HIR and remain on the audited HIR production route.
- Follow-up type-suffix declaration proof accepts `Declare PtrSafe Function Name& Lib ...`,
  preserves the return suffix as `DeclareParamType::Long` before DefType fallback, and carries it
  into the clean external-call descriptor. Evidence: `structures_declare_function_type_suffix`;
  `declare_function_type_suffix_supplies_return_type`;
  `declare_function_type_suffix_emits_external_return_type`; `cargo test -p oxvba-syntax
  --quiet`; `cargo test -p oxvba-symbol --quiet`; `cargo test -p oxvba-bind --quiet`;
  `cargo fmt --check -p oxvba-syntax -p oxvba-symbol -p oxvba-bind`;
  `cargo check --workspace`; `git diff --check`; `./scripts/check-governance.ps1`.
- Follow-up native declaration proof confirms a `kernel32` `Declare PtrSafe Function` with a
  `LongPtr` parameter preserves native FFI lane selection and `DeclareParamType::LongPtr` metadata
  through HIR on native-FFI targets, with a matching route-audit fixture for the accepted source.
- Unsupported declaration shapes that the shared declaration parser can diagnose, including missing
  `PtrSafe` and invalid ordinal aliases, now return HIR production diagnostics instead of falling
  back to the legacy declaration route solely to report the policy error.
- Bundle fact extraction now has an accepted-declaration route probe proving `Declare PtrSafe`
  module facts, including `LongPtr` usage, come from HIR `BoundModule` construction rather than the
  legacy resolver fallback.
- The retirement-inventory fallback example now uses the explicitly tracked project construction
  residual (`Set obj = New Widget` without project construction bindings) instead of `Xor`, avoiding
  pressure to add a truthiness-only logical `Xor` route where full VBA bitwise/value semantics are
  not yet implemented.

Remaining production residuals after this slice:

- `TypeBlock`: richer executable UDT behavior still requires nested member-chain/indexed-field
  access, lifetime/default initialization parity, and sharper diagnostics; UDT member declaration
  syntax now reaches HIR and no longer falls through expression parsing.
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
- Fixed-length string fields, fixed UDT array fields, and nested UDT field names now survive the
  production syntax/HIR path into emitted UDT metadata, with nested fields expanded into the same
  flattened descriptor/slot shape used by the legacy resolver.
- Simple UDT field reads/writes now lower to flattened aliases, so `p.X = 1` and `y = p.X + 2`
  avoid the object/member dispatch path.
- Nested UDT member-chain reads/writes now resolve through the same flattened aliases, so
  `r.Inner.X = 7` and `y = r.Inner.X + 2` lower to `r_inner_x` rather than late-bound dispatch.
- Fixed UDT array-field reads/writes now reuse the fixed-array alias path for static integer
  indices, so `r.Scores(1) = 7` and `y = r.Scores(2) + 2` lower through scalar element aliases
  such as `r_scores_0` and `r_scores_1`.
- Non-static fixed UDT array-field indices now produce a precise HIR unsupported diagnostic instead
  of being reported as an out-of-bounds static index.
- Dynamic UDT array fields declared as `Scores() As Long` now materialize an array-valued field
  alias such as `r_scores`; indexed reads/writes like `r.Scores(i) = 7` and `y = r.Scores(i)` reuse
  the existing dynamic-array get/set lowering and emit dynamic array shape metadata for the alias.
- Same-shape whole-value UDT assignment lowers to the existing field-wise `BoundStmt::UdtAssign`
  copy path.
- Cross-type whole-value UDT assignment now preserves the legacy unsupported diagnostic instead of
  silently copying same-shaped fields across distinct UDT type names.
- The production route audit includes a `Type Point ... p.X = 1 ... y = p.X + 2` fixture and
  nested/fixed-array/fixed-string descriptor, nested member-chain, fixed UDT array-field index, and
  dynamic UDT array-field index fixtures as `HirProduction`, plus a cross-type whole-value UDT
  assignment fixture classified as a HIR production diagnostic.

Remaining production residuals after this slice:

- `ReDim r.DynamicField(...)` member-target resizing, fixed-field non-static index materialization,
  and broader UDT lifetime/default initialization parity remain open.
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
- `collect_const_values(...)` records those values without allocating runtime local slots for the
  constants. Exact integer expressions such as `Const CBase = 1 + 2` now fold to
  `IntConst`/`LoadConstI32`; non-exact expressions remain bound expression trees.
- Later declarators in the same `Const` statement can reference earlier declarators, for example
  `Const CBase = 1 + 2, CTotal = CBase + 1`; those references now fold to exact integer constants
  for the covered `+`/`-`/`*`/`\`/`Mod`/`^` subset, while true division remains an expression tree
  for broader coercion work.
- Later `Const` statements can reference earlier constants through the same source-ordered
  environment for the covered evaluator subset, including typed `Long` diagnostics and typed
  `LongLong`/`LongPtr` i64 carriers.
- Typed declarators use the same evaluator and route through the default HIR entry point for the
  covered subset, now audited with `Const CBase As Long = 1 + 2, CTotal As Long = CBase + 4`; the
  exact typed `Byte`/`Integer`/`Long` subset materializes folded `IntConst` / `LoadConstI32`
  values while leaving true-division expressions unfurled for broader coercion work.
- A focused follow-up diagnostic rejects explicit `As Long` constants whose integer expression
  exceeds `Long` range, including same-statement reference cases such as
  `Const CBase As Long = 2147483647, CTotal As Long = CBase + 1`.
- A second focused diagnostic pass extends the same integer-expression range checks to explicit
  `As Byte` and `As Integer` constants.
- A third focused diagnostic pass distinguishes unsupported constant expressions from integer
  evaluation overflow and rejects explicit `As LongLong` and `As LongPtr` expressions that exceed
  the signed 64-bit carrier range before unsupported-const fallback can hide them.
- A fourth focused carrier pass adds `BoundExpr::LongLongConst(i64)`,
  `Instruction::LoadConstI64`, and VM execution support, so covered explicit `As LongLong` and
  `As LongPtr` constants use the signed-64-bit carrier even when the value fits in i32.
- A related optional-default carrier pass adds `OptionalDefaultValue::ExplicitI64`, accepts plain
  integral literals outside `i32` range as `LongLongConst`, and routes covered source-prior
  integer expressions such as `Optional ... As LongLong = Big + 7` and
  `Optional ... As LongPtr = Big` through HIR/default metadata and omitted-argument binding. This
  bumps the strict `OxBundle` format to v18 because serialized optional-default metadata gained a
  new enum arm. Full platform-width `LongPtr` semantics remain outside this focused i64 carrier.
- A fifth focused literal-kind pass lets the same bounded evaluator substitute simple typed
  `Double` constants as `BoundExpr::FloatConst`/`LoadConstF64`, including `#`-suffixed Double
  literals and VM execution coverage.
- A sixth focused exact-carrier pass lets declared `Currency` and deterministic `#...#` `Date`
  constants substitute through exact carriers instead of hidden runtime slots:
  `BoundExpr::CurrencyConst(i64)` / `LoadConstCurrency` and `BoundExpr::DateConst(u64)` /
  `LoadConstDate`. The focused proof covers default route metadata equivalence and VM execution for
  `Const CAmount As Currency = 1.25@` and `Const CStamp As Date = #2026-02-28#`.
- A seventh focused exact-carrier pass adds `BoundExpr::SingleConst(u32)` and serialized
  `Instruction::LoadConstF32`, so declared `Single` constants materialize as
  `Variant::from_f32(...)` rather than widened Double values. This bumps strict `OxBundle` format to
  v17 and covers direct HIR, default-route, route-audit, and VM execution paths for
  `Const CTotal As Single = 1.5!`.
- An eighth focused evaluator pass extends declared `Currency` and `Date` module constants from
  literal-only carriers to bounded numeric arithmetic expressions over `+`, `-`, `*`, `/`, `^`,
  unary minus, deterministic `#...#` Date literal operands, and source-prior numeric constants. The
  result still materializes through the exact carrier (`LoadConstCurrency` scaled `i64` or
  `LoadConstDate` f64 bits), with default-route, route-audit, and VM execution coverage for
  `Const CAmount As Currency = 1.25@ * 2@ - 1.0@` and
`Const CStamp As Date = #2026-02-28# + 1`.
- A ninth focused declaration-shape pass applies VBA type-declaration characters on `Const` names
  to the same bounded evaluator. `Const CTotal! = 1.5` now strips the `!` from the symbol name,
  records `Single`, and emits `LoadConstF32`; `Const CAmount@ = 1.25` records `Currency` and emits
  `LoadConstCurrency`. The direct HIR, default-route, route-audit, and VM execution checks prove the
  values materialize as typed carriers rather than widened Double constants.
- A tenth focused declaration-shape pass completes the standard scalar `Const` name
  type-declaration character family for the covered evaluator subset: `%`, `&`, `^`, `!`, `#`, `@`,
  and `$`. The frontend guard and collector now share the same declarator parser for `Const C^ =
  5000000000`, and declared `Double` constants use `FloatConst`/`LoadConstF64` even when the literal
  text is integer-looking (`Const CDouble# = 2`). Coverage proves direct HIR lowering, default-route
  HIR selection, route audit, and VM execution for Integer, Long, LongLong, Single, Double,
  Currency, and String suffix constants.
- An eleventh focused typed-coercion pass extends declared `Single` constants from literal-only
  carriers to the same bounded numeric expression/source-prior evaluator used by the other covered
  exact numeric carriers. `Const CBase As Double = 1.25` followed by `Const CTotal As Single =
  CBase + 0.25` now materializes as `LoadConstF32` / `Variant::from_f32(1.5)` through direct HIR,
  default-route, route-audit, and VM execution paths.
- A companion coverage hardening pass upgrades the representative declared `Double` route from a
  literal-only fixture to the same source-prior expression shape: `Const CBase As Long = 1` followed
  by `Const CTotal As Double = CBase + 0.5`. Direct HIR, default-route, route-audit, and VM
  execution coverage now prove `LoadConstF64` for the expression route, not just the literal route.
- A bounded Boolean typed-constant pass adds `True`/`False`, source-prior Boolean constants, `Not`,
  `And`, `Or`, `Xor`, `Eqv`, `Imp`, and simple comparisons over finite numeric values, Boolean
  equality/inequality, and binary string equality/inequality with `&` concatenation to the declared
  `Boolean` module-constant evaluator. `Const Prefix As String = "re"`, `Const Enabled As Boolean =
  True`, and `Const CFlag As Boolean = Enabled = Not False And 2 > 1 And Prefix & "ady" = "ready"`
  now substitutes as `LoadConstBool true`; `Const CFlag As Boolean = Enabled Xor True`,
  `Enabled Eqv False`, and `Enabled Imp False` substitute as `LoadConstBool false` through direct
  HIR, default-route, route-audit, and VM execution paths.
- Follow-up exact-integer comparison work keeps both-operand integer relational constants on the
  integer lane instead of converting through `f64`. Adjacent `LongLong` values above the IEEE-754
  exact integer range, such as `9007199254740993^` and `9007199254740992^`, now fold correctly for
  `=`, `<>`, `<`, `<=`, `>`, and `>=` through symbol metadata and clean execution.
- Module compare mode now reaches that Boolean constant string-comparison evaluator for the covered
  subset. `Option Compare Text` makes `Const CFlag As Boolean = "a" = "A"` fold to
  `LoadConstBool true`; `Option Compare Database` now reports
  `SYM-E-UNSUPPORTED-OPTION-COMPARE-DATABASE` instead of using a Binary approximation until Access
  collation semantics are implemented.
- A bounded `Like` follow-up extends that same string Boolean-constant lane to the current runtime
  `Like` subset. `Option Compare Text` now lets
  `Const CFlag As Boolean = Prefix & "llo" Like "HELLO"` fold to `LoadConstBool true`, and the
  compile-time evaluator now shares the runtime charlist rules for `?`, `*`, `#`, `[chars]`,
  `[!chars]`, `a-z` ranges, and literal `]` first in a charlist through resolver optional-default
  parsing, HIR/default route selection, route audit, and VM execution.
- A companion `String` typed-constant pass reuses the same bounded string evaluator for declared
  module constants. `Const Prefix As String = "re"` followed by
  `Const CText As String = Prefix & "ady"` now substitutes as `LoadConstString "ready"` through
  direct HIR, default-route, route-audit, and VM execution paths, with no runtime concat bytecode
  needed for the constant itself.
- A bounded scalar-to-string concat pass extends the typed and untyped `String` constant routes to
  already folded scalar constants used as `&` operands. `Const CText As String = Prefix & CNumber &
  CFlag` and the untyped equivalent now substitute as `LoadConstString "v7True"` through direct HIR,
  default-route, route-audit, and VM execution paths.
- A follow-up type-system carrier pass applies declared `Const` types when the symbol layer publishes
  folded constant metadata, so `Single`, `Currency`, `Date`, `String`, Boolean, integer, `LongLong`,
  and `LongPtr` constants no longer rely on later assignment coercion to recover their declared
  carrier. `typed_const_values_preserve_exact_type_system_carriers` proves `CoreConst::F32`,
  `CoreConst::Currency`, `CoreConst::Date`, and folded string concatenation over exact carriers in
  `ResolutionEnvironment::const_value`; `scalar_typed_const_exact_carriers_preserve_variant_values`
  proves assigning those constants to `Variant` preserves `Variant::from_f32`,
  `Variant::from_currency_scaled_i64`, and `Variant::from_date_f64`. The binder constant type helper
  now reports `I64` constants as `LongLong` and recognizes `F32`/`Currency` constants, matching
  their serialized load ops. Checks passed for the carrier-metadata follow-up:
  `cargo test -p oxvba-symbol typed_const_values_preserve_exact_type_system_carriers --quiet`;
  `cargo test -p oxvba-bind scalar_typed_const_exact_carriers_preserve_variant_values --quiet`;
  `cargo test -p oxvba-bind scalar_longlong_const_carrier_executes --quiet`;
  `cargo test -p oxvba-bind scalar_type_char_const_carriers_execute --quiet`;
  `cargo test -p oxvba-symbol --quiet`; `cargo test -p oxvba-bind --quiet`;
  `cargo fmt --check -p oxvba-symbol -p oxvba-bind`; `cargo check --workspace`;
  `git diff --check`; `./scripts/check-governance.ps1`.
- A follow-up string-to-typed-constant coercion pass matches an Excel 16.0 oracle probe for
  declared scalar constants such as `Const CLong As Long = "7"`, `Const CBool As Boolean =
  "False"`, `Const CAmount As Currency = "1.25"`, and `Const CStamp As Date =
  "2026-02-28"`. The symbol constant evaluator now coerces covered string constants into
  declared Boolean, integer, floating, Currency, and Date carriers before publishing
  `ResolutionEnvironment::const_value`; string-to-Date uses the same deterministic date parser as
  `#...#` literals, so ambiguous numeric date text remains unsupported instead of becoming
  locale-sensitive. Evidence: Excel probe returned `7`, `False`, `1.25`, and `2026/02/28` for the
  corresponding VBA constants; `string_typed_const_values_coerce_to_declared_scalar_carriers`;
  `scalar_string_typed_const_values_coerce_to_declared_carriers`; `cargo test -p oxvba-symbol
  --quiet`; `cargo test -p oxvba-bind --quiet`; `cargo fmt --check -p oxvba-symbol
  -p oxvba-bind`; `cargo check --workspace`; `git diff --check`;
  `./scripts/check-governance.ps1`.
- A follow-up untyped `String` expression pass applies that fold before generic binary-expression
  lowering, so `Const Prefix = "re"` followed by `Const CText = Prefix & "ady"` also substitutes as
  `LoadConstString "ready"` through direct HIR, default-route, route-audit, and VM execution paths.
- A follow-up qualified-name pass folds same-project `Module.Const` and `Project.Module.Const`
  references in compile-time constant expressions before binding. The symbol-layer evaluator now
  resolves the module qualifier among siblings of the declaring module's project scope, preserves
  forward-reference `Pending` behavior for cross-module constants, and respects the ordinary binder
  rule that a local/parameter named like the module shadows the qualifier. Excel 16.0 oracle probe:
  `Const X As Long = ModA.K + 1` over `Public Const K As Long = 7` returned `8`. Coverage:
  `module_qualified_const_values_fold_across_modules`;
  `module_qualified_const_initializer_folds`;
  `module_qualified_const_initializer_respects_local_shadowing`;
  `cargo test -p oxvba-symbol --quiet`; `cargo test -p oxvba-bind --quiet`;
  `cargo fmt --check -p oxvba-symbol -p oxvba-bind`; `cargo check --workspace`;
  `git diff --check`; `./scripts/check-governance.ps1`.
- This is intentionally still a bounded subset. Constant expressions that require broader
  procedure-local scoping, conditional-branch source mapping, locale-sensitive string comparison,
  Date/Currency expression coercion beyond the covered numeric arithmetic subset, locale-sensitive
  Date literal breadth, or names beyond source-prior constants, same-project module-qualified
  constants, and the already handled
  enum/literal/type-character route, plus typed constant coercion outside the covered exact and
  string-to-scalar declared carriers and full `LongPtr` platform semantics, remain future FE-8.5
  work.

Follow-up route-audit hardening fixes a hidden gate weakness: the selected production route audit
now asserts `terminal_gate_passed()` directly, so any audited fixture left as a fallback/static
residual fails the test instead of relying only on representative row assertions. This does not
complete FE-9.7's broader corpus/matrix expansion requirement; it only makes the current selected
audit self-consistent.

Follow-up corpus-audit broadening makes the FE-5 seed corpus reusable outside the diff test module
and adds a route audit over source-backed `CompilerUnit` / `ConformanceCase` rows. The current seed
rows `examples/basic/arithmetic.bas`, `conformance/tests/call_coercion_mixed_variant_to_long.bas`,
and the inline statement-separator improvement fixture must classify as HIR production. A narrow
project-entry continuation also promotes the `conformance/integration/projects/INTP-001` seed row:
its single procedural module source must classify as HIR production and compile through
`compile_project(...)`. The Excel oracle source fixture
`conformance/com/office/excel/excel_application_activation_smoke.bas` now participates in the route
audit and must classify as HIR production; live Excel oracle execution remains a higher-layer,
environment-dependent lane outside the compiler route classifier.

The language-service corpus continuation now consumes the same seed corpus for source-backed
`CompilerUnit`, `ConformanceCase`, and the narrow `HostProject` row. Those sources must build
`SemanticSnapshot`s without diagnostics and expose front-end symbols/callables, proving this seed
coverage uses compiler front-end facts rather than a duplicate legacy semantic fallback. Broader
workspace, cross-module, and reference-project IDE route coverage remains FE-9.7 work.

Follow-up host-project corpus expansion widens the HIR-capable project boundary from a single
procedural active module to active projects containing only procedural modules and no references.
The regression fixture uses a two-module project with an inline-statement main procedure and a
module-qualified call, proving the old legacy project boundary would have rejected the source while
the HIR boundary compiles it. The seed route audit now includes `INTP-002` as a multi-module
procedural host-project row. Reference projects, class/document modules, and host/oracle-backed
project semantics remain outside this slice.

Follow-up reference-project expansion makes the project boundary explicit: active procedural-only
projects compile from active source through HIR, while procedural-only projects with procedural
reference projects compile from full lowered source through HIR. The new regression fixture covers a
project-qualified call into `LibMath.MathApi`, and the seed route audit now includes `INTP-003`,
`INTP-004`, and `INTP-019` for procedural reference projects, active-project shadowing of a
referenced procedure, and multiple project references. The same audit now also includes `INTP-016`
as a simple active-project class route with `Dim ... As New` construction and early-bound class
method calls compiling through `ProjectCompileRoute::HirProduction`. The audit also includes
`integration_predeclared_document_project`, proving a project-shaped predeclared `ThisWorkbook`
document reference and synthetic receiver property read (`ThisWorkbook.Path`) compile through
`ProjectCompileRoute::HirProduction`. Imported COM/reference shapes beyond the current seeds,
broader class/document semantics, and host/oracle-backed project semantics remain open.
The audit now includes `integration_imported_typelib_testdispatch` as a source-backed imported COM
route: `Dim obj As New OxVba.TestDispatch` rewrites through imported activation/member metadata and
compiles through `ProjectCompileRoute::HirProduction`. The source-backed route gate now passes; the
selected route audit now has no skipped residual rows after adding the Excel oracle source fixture
as an HIR-production route. Broader uncovered project routes and live host/oracle execution remain
open work outside the selected route gate.
Language-service workspace coverage now loads matching seed-route project manifests: `INTP-003`
proves referenced-project exports surface through workspace symbols with `ProjectReference`
provenance, and `INTP-016` proves active class members surface from the class project workspace
route.
The migrated active/full project HIR boundaries now call strict HIR compilation directly; unsupported
HIR shapes are recorded as `LegacyFallbackAfterHirUnsupported` instead of silently returning through
the legacy fallback wrapper, and the route audit only counts rows with
`ProjectCompileRoute::HirProduction` as HIR production.

## Bang Member Read Continuation

The latest FE-8.5 slice removes the read-side bang member residual:

- HIR member-name extraction now accepts `!` as a member selector for expressions such as
  `obj!Value`, matching the existing syntax bridge/backend representation.
- HIR production lowering emits the same late-bound dispatch shape used for dot member reads.
- This does not close full member writes: default-member/property selection, project/class and
  early-bound COM property-put resolution, indexed/named writeback breadth, and property Let/Set
  overload validation remain tracked residual work.

## Statement-Form DispatchInvoke Continuation

The latest FE-8.5 slice removes the statement-form structural-intrinsic residual exposed by the
Excel oracle broadening attempt:

- HIR production lowering now accepts no-keyword `StructuralIntrinsicCallWithArgs` statement forms
  and lowers them to `BoundStmt::Expr`, so the existing typecheck/emit path can produce
  `IntrinsicDispatchInvokeHost`.
- The regression covers no-keyword statement-form `DispatchInvoke obj, "SetIndexedValue", value :=
  11, lhs := 7` and asserts the named dispatch arguments survive into the emitted host-dispatch
  instruction.
- The production route audit now includes a no-keyword statement-form named `DispatchInvoke`
  fixture in addition to the assignment-form named `DispatchInvoke` fixture.
- `Call DispatchInvoke(...)` remains outside this HIR route so project/imported-COM compatibility
  rewrites can still attach early-bound COM metadata where that route remains load-bearing.
- This closes the compiler-side HIR residual only. Follow-up COM bridge work separately closed
  explicit `DispatchInvoke(sheet, "Range", "A1")` range object access for live Excel by retrying
  strict dynamic-name parameterized properties as `DISPATCH_PROPERTYGET`. Follow-up Excel oracle
  work separately proved named-argument worksheet-add dispatch and null `Cells.Find` results against
  live Excel, and scoped `Range.Value` property-put after the COM dynamic-name bridge honored
  property-put hints. Follow-up direct-DISPID work proved scoped indexed `Range("A1")(1)`
  default-member mutation. Broader Excel mutation lanes remain open.

## Indexed Property Default Route

The latest FE-8.5.c slice narrows the property/default-member residual without claiming indexed
writeback closure:

- Default-route eligibility now allows same-module indexed `Property Get` declarations, because the
  HIR path already lowers `Value(1)` as an argument-preserving `property_get_value` procedure call.
- The regression covers the ordinary default compile entry point, so this is production-route
  migration rather than an opt-in `frontend_v2` only path.
- Follow-up route work now handles same-module indexed `Property Let`/`Property Set` writeback by
  lowering the getter-shaped indexed target into the matching setter procedure call and appending
  the assigned value as the final synthetic property-assignment argument.
- Same-module named indexed `Property Let`/`Property Set` writeback preserves the named index
  argument through HIR and remains on the default route.
- Project/class/COM property writeback, default-member writeback, broader named-argument writeback,
  and overload validation breadth remain residual work.
- Fresh-eyes correction: the first implementation let property/default-member PMR helper traffic
  piggyback on the project construction HIR candidate and broke object-local/default-member rewrite
  lanes. The final route keeps generated `property_*_pmr_*` helpers out of that construction
  candidate while allowing ordinary same-module indexed getter reads and indexed `Property Let`/
  `Property Set` writes through HIR.

## Optional Integer Default Expression Continuation

The latest FE-8.5.f slice narrows the optional-parameter default residual within the existing
`ExplicitI32` descriptor shape:

- Procedure signature parsing now accepts integer constant-expression defaults by reusing the
  resolver expression parser and statically folding only integer-safe shapes.
- This covers decimal, signed, prefixed hex/octal, typed integer suffixes, parentheses, and simple
  integer arithmetic, including checked nonnegative exponentiation, integer division, and `Mod`,
  that can be represented as `OptionalDefaultValue::ExplicitI32`.
- Follow-up route work also allows those integer defaults to reference integer-valued module
  constants, including constants initialized from prefixed hex/octal arithmetic, while keeping the
  same `ExplicitI32` descriptor contract.
- A further focused route proof covers enum-member defaults as integer constants through the same
  HIR production path.
- Follow-up descriptor work adds `OptionalDefaultValue::ExplicitString` and
  `OptionalDefaultValue::ExplicitBool`, records literal `Optional ... As String = "..."` and
  `Optional ... As Boolean = True/False` defaults during procedure signature parsing, preserves
  those defaults through HIR production/default-route metadata, and binds omitted package-VM
  arguments to real string/Boolean `Variant` values.
- Follow-up declared-default work maps omitted `Optional As String`, `Optional As Boolean`, and
  integer optional parameters without explicit defaults to concrete descriptor/runtime defaults
  (`""`, `False`, and `0`) instead of the vague declared-type default marker or integer-only
  entry default.
- Follow-up module-constant proof covers `Optional ... As String = SomeStringConst` and
  `Optional ... As Boolean = SomeBooleanConst` through the same descriptor/runtime route.
- Follow-up Date/Currency carrier work adds explicit optional-default metadata and runtime binding
  for source-backed `Currency` scaled values and `Date` serial values, including declared defaults
  (Currency zero / Date serial zero) and unambiguous module-constant numeric expressions interpreted
  through the parameter's declared type. Later focused slices extend that same carrier route to
  bounded arithmetic numeric constant-expression defaults (`+`, `-`, unary `-`, `*`, and guarded
  `/`) over numeric literals and module constants.
- Follow-up `Single` metadata work adds a `DefaultValue::F32` signature carrier so direct scanner
  metadata for `Optional ... As Single = 1.5!` preserves the f32 bit pattern instead of widening to
  `F64`. This is a symbol signature metadata fix only; omitted-argument execution already flows
  through folded `CoreConst` defaults and remains covered by the package VM. Evidence:
  `optional_single_default_preserves_f32_metadata_carrier`;
  `optional_single_default_is_bound_as_single_for_omitted_arg`; `cargo test -p oxvba-symbol
  --quiet`; `cargo test -p oxvba-bind --quiet`; `cargo fmt --check -p oxvba-symbol
  -p oxvba-bind`; `cargo check --workspace`; `git diff --check`;
  `./scripts/check-governance.ps1`.
- Follow-up Date literal work accepts deterministic `#...#` optional Date defaults and maps them to
  the same Date serial carrier, with resolver, metadata, and VM omitted-argument proofs.
- Follow-up string-to-typed-default coercion matches an Excel 16.0 oracle probe for
  `Optional ... As Long = "7"`, `Optional ... As Boolean = "False"`,
  `Optional ... As Date = "2026-02-28"`, and `Optional ... As Currency = "1.25"`.
  Scanner signature metadata now reuses the declared scalar constant coercion helper, and the
  folded optional-default environment coerces omitted-call defaults by declared parameter type, so
  string Date text reaches the VM as a deterministic Date serial instead of a string that the
  numeric Date coercer would reject. Evidence: Excel observed `7`, `False`, `2026/02/28`, and
  `1.25`; `optional_parameter_string_defaults_coerce_to_declared_metadata`;
  `optional_string_to_typed_defaults_are_bound_for_omitted_args`; `cargo test -p oxvba-symbol
  --quiet`; `cargo test -p oxvba-bind --quiet`; `cargo fmt --check -p oxvba-symbol
  -p oxvba-bind`; `cargo check --workspace`; `git diff --check`;
  `./scripts/check-governance.ps1`.
- Follow-up invalid-default work removes the raw/default-zero fallback for explicit optional
  defaults that fail declared-type coercion. Excel/VBA oracle compile rows reject
  `Optional ... As Long = "abc"` with `Type mismatch` and
  `Optional ... As Long = 5000000000^` with `Overflow`, while accepting fractional numeric
  `Long` defaults and object `Nothing`/zero defaults. The symbol model now reports
  `SYM-E-INVALID-OPTIONAL-DEFAULT` for invalid explicit defaults instead of publishing a raw
  carrier or silently treating the argument as defaultless. Declared enum parameters still use
  their underlying `Long` default-coercion target. Evidence:
  `docs/evidence/frontend_rework/OPTIONAL_DEFAULT_INVALID_FALLBACK_2026-07-02.md`;
  `invalid_optional_defaults_reject_instead_of_falling_back`;
  `invalid_literal_optional_default_does_not_publish_raw_metadata`;
  `optional_object_defaults_accept_nothing_and_zero`; `invalid_optional_default_is_bind_error`;
  `cargo test -p oxvba-symbol --quiet`; `cargo test -p oxvba-bind --quiet`.
- Follow-up class-visibility work makes typed receiver member/default-member lookup source-context
  aware: `Private` class members bind only from their declaring class, while same-project `Friend`
  and `Public` members remain reachable. Excel/VBA oracle rows accept an internal private call and
  same-project Friend call, and reject external `w.Secret()` with `Method or data member not
  found`. Evidence:
  `docs/evidence/frontend_rework/PRIVATE_CLASS_MEMBER_VISIBILITY_2026-07-02.md`;
  `docs/evidence/conformance/vm3_scoping_visibility_private_class_20260702T1948Z/summary.md`;
  `context_member_resolution_honors_private_class_visibility`; `private_class_method_is_not_accessible_from_other_module`;
  `private_class_field_is_not_accessible_from_other_module`;
  `private_class_method_is_accessible_from_declaring_class`;
  `friend_class_method_remains_accessible_inside_project`.
- Follow-up string constant-expression work evaluates string concatenation trees (`&`) over string
  literals and module constants into the existing explicit string optional-default carrier.
- Follow-up scalar-concat default work reuses the same exact scalar-to-string operand formatting
  for optional `String` defaults. `Optional ... As String = Prefix & CNumber & CFlag` now binds to
  `ExplicitString("v7True")` through resolver parsing, HIR/default metadata, route audit, and VM
  omitted-argument execution.
- Follow-up Boolean constant-expression work evaluates Boolean literals, module constants, `Not`,
  `And`, and `Or` into the existing explicit Boolean optional-default carrier.
- Follow-up Boolean comparison-default work evaluates bounded numeric comparison expressions and
  Boolean equality/inequality expressions into the same explicit Boolean optional-default carrier.
- Follow-up exact string comparison-default work evaluates unambiguous string equality/inequality
  expressions where both static string operands are byte-identical, including string concatenation
  and module constants, into the same explicit Boolean optional-default carrier. Collation-sensitive
  unequal strings, ordering comparisons, full `Like` pattern semantics, and `Is` remain outside this
  bounded evaluator.
- Follow-up Boolean `Like` default work narrows that residual for the current runtime `Like` subset.
  `Optional ... As Boolean = Prefix & "llo" Like "hello"` now binds to `ExplicitBool(true)`, and the
  compile-time optional-default path now also folds charlists/ranges/literal-`]` cases such as
  `"f" Like "[a-z]"` and `"]" Like "[]x]"` through HIR/default metadata, route audit, and VM
  omitted-argument execution; locale/database collation and unsupported pattern edge diagnostics
  remain outside this slice.
- Follow-up i64 optional-default work adds an explicit `OptionalDefaultValue::ExplicitI64` carrier
  and binds covered `LongLong`/`LongPtr` source-prior integer constant defaults through resolver,
  HIR/default metadata, direct optional-entry bytecode, and VM omitted-argument execution for
  `LongLong`. This is an exact carrier fix, not a full platform `LongPtr` semantics claim.
- Follow-up ParamArray declaration validation now rejects invalid clean-stack signatures before
  publishing partial metadata. The scanner reports `SYM-E-INVALID-PARAMARRAY-DECLARATION` for
  `ParamArray` combined with `Optional`, explicit `ByVal`, explicit `ByRef`, or a non-final
  parameter position, and for `ParamArray` parameters that are not an array of `Variant` elements,
  covering ordinary procedures and `Declare` statements while preserving implicit
  `ParamArray xs()` as `Variant()` even under DefType defaults. This follows the VBA Function
  statement rule documented by Microsoft Learn
  (`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/function-statement`)
  that `ParamArray` is the final variable-argument slot, not combined with `Optional`, `ByVal`, or
  `ByRef`, and represents an array of `Variant` elements. Evidence:
  `scanner_rejects_invalid_paramarray_modifiers`;
  `scanner_accepts_implicit_variant_paramarray_array`;
  `invalid_paramarray_declaration_is_bind_error`; `non_variant_paramarray_declaration_is_bind_error`;
  existing ParamArray binder/runtime regressions remain green.
- Follow-up Optional declaration-order validation now rejects signatures that publish a required
  parameter after an `Optional` parameter. The scanner reports
  `SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION` before building ordinary procedure,
  `Property Get`, property index-argument, or `Declare` descriptors, including the
  `Optional ..., ParamArray ...` case where the `ParamArray` slot is still not explicitly
  Optional. `Property Let`/`Set` keep their VBA-specific required final value/reference parameter
  after optional index arguments, reject missing writer slots with
  `SYM-E-MISSING-PROPERTY-WRITER-PARAMETER`, and reject a present final parameter if it is
  explicitly marked `Optional` or `ParamArray`; `Property Set` now also rejects scalar/value final
  reference parameters with `SYM-E-INVALID-PROPERTY-SET-REFERENCE`, and a provider-level resolved
  type-index pass rejects known UDT final reference parameters with the same diagnostic after all
  modules are scanned, while preserving default/explicit `Variant`, `Object`, and named
  class/object reference parameters. The same type-index pass now rejects `Optional` parameters
  declared as same-module or cross-module user-defined types with
  `SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION`, matching the Function statement rule that UDT
  parameter types are only valid when the parameter is not Optional. Procedure, Property, and
  Declare signatures now also reject `As New` on parameters and return types with
  `SYM-E-INVALID-AS-NEW-DECLARATION`, keeping auto-instantiating `As New` scoped to variable
  declarators rather than silently publishing normalized signature metadata. UDT `Type` fields now
  reject `As New` with the same diagnostic before field-table metadata normalizes the type. The
  scanner also rejects duplicate UDT field names with `SYM-E-DUPLICATE-TYPE-FIELD` before a
  duplicate folded field can publish ambiguous record metadata, and rejects public/default-public
  `Type` blocks in class/object modules with `SYM-E-PUBLIC-TYPE-NOT-VALID-IN-OBJECT-MODULE`,
  matching the Type statement rule that object-module UDTs must be `Private`. Public or
  implicit-public `Declare` statements in object modules now reject with
  `SYM-E-PUBLIC-DECLARE-NOT-VALID-IN-OBJECT-MODULE`, while explicit-private class-module declares
  and public/implicit-public standard-module declares remain accepted. Public object-module
  constants, fixed-length string fields, and array fields now reject with
  `SYM-E-PUBLIC-OBJECT-MODULE-DATA-NOT-VALID`, while private storage and ordinary public scalar
  fields remain accepted. Exposed/public object modules now also reject public data members typed as
  private object modules with
  `SYM-E-PRIVATE-OBJECT-MODULE-TYPE-NOT-VALID-IN-PUBLIC-MEMBER`; private containers, private
  members, and public object-module target types remain accepted. Literal fixed-length `String * N`
  declarations now also reject zero,
  negative, and over-65,526 lengths with `SYM-E-INVALID-FIXED-STRING-LENGTH` before variable or
  UDT metadata is published, matching the Microsoft Learn fixed-length string diagnostic. The
  scanner also publishes the final `Property Let`/`Set` writer slot as
  `ByVal` in signature metadata while
  preserving declared/default `ByRef` modes for preceding indexed arguments. Duplicate property
  accessors now reject with `SYM-E-DUPLICATE-PROPERTY-ACCESSOR`, and paired `Property Get`/`Let`
  declarations reject with `SYM-E-INCOMPATIBLE-PROPERTY-ACCESSOR` when their index parameter
  names/types or final value-vs-return type do not match; `Property Get`/`Set` and `Property
  Let`/`Set` index count/type mismatches use the same diagnostic while still allowing different
  index parameter names and allowing the final `Property Set` reference to differ from the Get
  return type if it is object-compatible. Required procedure/Declare parameters now reject
  default values before signature metadata is published, and Event declarations now reject
  standard-module placement with `SYM-E-EVENT-ONLY-VALID-IN-OBJECT-MODULE` plus `Optional`,
  default-valued, `ParamArray`, or `As New` arguments through scanner-owned declaration diagnostics
  before event metadata is published. This
  follows the same Microsoft Learn Function/Event/Property Let rules, the Property Set statement's
  object-reference contract, and the inconsistent property definitions diagnostic
  (`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/event-statement`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/function-statement`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/property-let-statement`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/property-set-statement`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/type-statement`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/declare-statement`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/constants-fixed-length-strings-arrays-user-defined-types-and-declare-statements`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/fixed-length-strings-not-allowed-as-the-type-of-a-public-member-of-an-object-mod`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/invalid-length-for-fixed-length-string`,
  `https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/definitions-of-property-procedures-for-the-same-property-are-inconsistent`),
  and remains a declaration-shape diagnostic rather than broader call-site
  optional argument closure. Evidence: `scanner_rejects_required_parameters_after_optional`;
  `scanner_rejects_required_parameter_defaults`;
  `scanner_rejects_invalid_event_parameter_modifiers`;
  `event_declaration_rejects_as_new_parameters`;
  `duplicate_property_accessors_reject`;
  `property_get_let_pairing_accepts_matching_accessors_in_any_order`;
  `property_get_let_pairing_rejects_mismatches`;
  `property_get_set_pairing_accepts_matching_index_accessors_in_any_order`;
  `property_get_set_pairing_rejects_index_mismatches`;
  `property_let_set_pairing_rejects_index_mismatch_without_get`;
  `property_let_allows_required_value_after_optional_index_args`;
  `property_writers_require_final_value_or_reference_parameter`;
  `property_writer_final_parameter_is_published_byval`;
  `property_let_value_parameter_cannot_be_optional`;
  `property_set_reference_parameter_cannot_be_optional`;
  `property_set_reference_parameter_must_be_object_compatible`;
  `property_set_reference_parameter_rejects_udt_types`;
  `optional_parameter_rejects_udt_types`;
  `signature_declarations_reject_as_new_types`;
  `type_block_fields_reject_as_new_types`;
  `type_block_fields_reject_duplicate_names`;
  `object_module_type_blocks_must_be_private`;
  `public_declare_is_not_valid_in_object_modules`;
  `public_object_module_data_members_are_rejected`;
  `fixed_string_lengths_must_be_in_vba_range`;
  `public_object_module_data_members_cannot_use_private_object_module_types`;
  `property_set_accepts_variant_object_and_class_reference_parameters`;
  `property_writer_final_parameter_cannot_be_paramarray`;
  `property_set_udt_reference_parameter_is_bind_error`;
  `scanner_rejects_withevents_in_local_scope`;
  `scanner_rejects_withevents_const_declarations`;
  `scanner_accepts_object_withevents_fields_in_class_modules`;
  `scanner_rejects_non_object_withevents_fields`;
  `scanner_rejects_withevents_as_new_fields`;
  `withevents_local_declaration_is_bind_error`;
  `withevents_const_declaration_is_bind_error`;
  `withevents_scalar_field_type_is_bind_error`;
  `withevents_as_new_field_is_bind_error`;
  `required_parameter_default_is_bind_error`;
  `optional_udt_parameter_is_bind_error`;
  `signature_as_new_is_bind_error`;
  `type_block_field_as_new_is_bind_error`;
  `type_block_duplicate_field_is_bind_error`;
  `class_module_public_type_block_is_bind_error`;
  `class_module_public_declare_is_bind_error`;
  `class_module_public_data_members_are_bind_errors`;
  `invalid_fixed_string_length_is_bind_error`;
  `public_object_member_private_class_type_is_bind_error`;
  `event_declaration_rejects_invalid_parameters`;
  `event_declaration_as_new_parameter_is_bind_error`;
  `required_parameter_after_optional_is_bind_error`.
- The latest declaration/type-surface pass also stops local-scope and `Const` `WithEvents`
  declarations, plus fields with value, explicit/implicit `Variant`, array, or `As New`
  declarations before event-field metadata is published. This matches the Microsoft Learn
  `Private` statement rule that `WithEvents` is valid only in class modules and cannot create
  arrays or use `New`
  (`https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/private-statement`).
  Named object/class/COM source types remain the accepted event-source contract; this is a
  compile-time declaration guard, not a full proof that every accepted object type has source
  events.
- The i64 optional-default follow-up also found a front-end symbol-model miss where a later
  parameter following a string default could be absent from the HIR parameter list even though the
  signature parser saw it. Procedure symbol collection now reconciles missing parameter symbols
  against the signature parser instead of letting the default-route gate reject the source.
- This deliberately does not claim arbitrary typed coercion of default expressions, locale-sensitive
  Date literal breadth, or broader expression-default metadata expansion beyond the covered integer
  plus string/Boolean constant-expression subset, bounded Boolean comparison subset, exact
  same-string equality/inequality subset, bounded Date/Currency arithmetic numeric subset, and
  exact i64 optional-default carrier subset, plus the covered `Single` f32 metadata carrier and
  string-to-declared-scalar default coercions, nor exact Excel modal text for invalid default
  diagnostics beyond the covered reject-vs-accept behavior. Collation-sensitive string comparisons,
  locale/database-sensitive `Like`, `Is`, and coercive comparison defaults remain FE-8.5.f
  residuals.

## Checks

- `cargo test -p oxvba-compiler resolve_optional_params_with_integer_constant_expression_defaults --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_optional_integer_expression_defaults_through_hir --quiet`
- `cargo test -p oxvba-compiler resolve_optional_params_with_module_constant_defaults --quiet`
- `cargo test -p oxvba-compiler parse_optional_module_constant_default_rejects_cycles --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_optional_module_constant_defaults_through_hir --quiet`
- `cargo test -p oxvba-compiler resolve_optional_params_with_enum_constant_defaults --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_optional_enum_constant_defaults_through_hir --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_optional_string_bool_defaults_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_string_boolean_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-compiler optional_date_currency_defaults_route_through_hir --quiet`
- `cargo test -p oxvba-compiler resolve_optional_date_currency_numeric_expression_defaults --quiet`
- `cargo test -p oxvba-compiler resolve_optional_date_literal_default --quiet`
- `cargo test -p oxvba-compiler resolve_optional_unambiguous_numeric_month_day_date_literal_default --quiet`
- `cargo test -p oxvba-compiler resolve_optional_string_concat_default --quiet`
- `cargo test -p oxvba-compiler optional_string_concat_defaults_route_through_hir --quiet`
- `cargo test -p oxvba-compiler resolve_optional_boolean_expression_default --quiet`
- `cargo test -p oxvba-compiler optional_boolean_expression_defaults_route_through_hir --quiet`
- `cargo test -p oxvba-compiler type_hooks_collect_parameter_descriptors_from_source_backed_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_string_concat_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_boolean_expression_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_date_currency_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_expression_const_statement --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_keeps_untyped_true_division_const_expression_unfolded --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_untyped_integer_const_expression_executes --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_collects_unambiguous_numeric_month_day_date_const_literal --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_numeric_month_day_date_const_carrier_executes --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_collects_typed_same_statement_const_expression --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_folds_typed_byte_integer_const_expressions --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_keeps_true_division_const_expression_unfolded --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_typed_integer_const_expressions_execute --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_folds_untyped_string_const_expression --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_untyped_string_const_expression_through_hir --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_folds_untyped_string_const_scalar_concat_expression --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_untyped_string_const_scalar_concat_through_hir --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_collects_typed_string_const_expression --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_typed_string_const_through_hir --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_coerces_scalar_string_const_concat_operands --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_string_const_scalar_concat_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_string_const_scalar_concat_expression_executes --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_folds_option_compare_text_boolean_const --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_keeps_option_compare_database_const_binary --quiet`
- `cargo test -p oxvba-compiler compile_default_routes_option_compare_text_boolean_const_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_option_compare_text_boolean_const_expression_executes --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_folds_typed_boolean_xor_const_expression --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_typed_boolean_xor_const_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_boolean_xor_const_expression_executes --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_folds_typed_boolean_eqv_imp_const_expressions --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_typed_boolean_eqv_imp_const_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_boolean_eqv_imp_const_expressions_execute --quiet`
- `cargo test -p oxvba-symbol longlong_const_comparisons_preserve_integer_precision -- --nocapture`
- `cargo test -p oxvba-bind scalar_longlong_const_comparison_preserves_integer_precision -- --nocapture`
- `cargo test -p oxvba-compiler resolve_optional_boolean_like_default --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_typed_boolean_like_const_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_boolean_like_const_expression_executes --quiet`
- `cargo test -p oxvba-symbol optional_boolean_like_default_folds_charlists_and_ranges -- --nocapture`
- `cargo test -p oxvba-bind scalar_boolean_like_charlist_const_expression_executes -- --nocapture`
- `cargo test -p oxvba-compiler resolve_optional_string_scalar_concat_default --quiet`
- `cargo test -p oxvba-compiler optional_string_scalar_concat_defaults_route_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_string_scalar_concat_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-compiler optional_boolean_like_defaults_route_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_boolean_like_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-bind optional_boolean_like_charlist_defaults_are_bound_for_omitted_args -- --nocapture`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_untyped_string_const_expression_executes --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_untyped_string_const_scalar_concat_expression_executes --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage scalar_string_const_expression_executes --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_rejects_overflowing_typed_long_const --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_rejects_overflowing_typed_integer_const --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_rejects_overflowing_typed_long_const --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_rejects_overflowing_typed_byte_const --quiet`
- `cargo test -p oxvba-compiler compile_project_production_selector_uses_module_aware_plan --quiet`
- `cargo test -p oxvba-compiler compile_project_applies_manifest_conditional_constants --quiet`
- `cargo test -p oxvba-compiler compile_project_source_const_overrides_manifest_conditional_constant --quiet`
- `cargo test -p oxvba-compiler resolve_conditional_compilation_boolean_xor_eqv_imp_branch --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_conditional_boolean_xor_eqv_imp_through_hir --quiet`
- `cargo test -p oxvba-compiler resolve_optional_longlong_module_constant_defaults --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_optional_longlong_defaults_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage optional_longlong_module_constant_defaults_are_bound_for_omitted_args --quiet`
- `cargo test -p oxvba-host embedded_host_build_workspace_applies_manifest_conditional_constants --quiet`
- `cargo test -p oxvba-compiler frontend_legacy_route_audit --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_indexed_property_get_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage indexed_property_get_executes_through_package_vm --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_same_module_indexed_property_let_write --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_indexed_property_let_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage indexed_property_let_executes_through_package_vm --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_same_module_indexed_property_set_write --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_indexed_property_set_through_hir --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_named_indexed_property_let_write --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_named_indexed_property_let_through_hir --quiet`
- `cargo test -p oxvba-vm --test vm_feature_coverage named_indexed_property_let_executes_through_package_vm --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_default_routes_named_indexed_property_set_through_hir --quiet`
- `cargo test -p oxvba-symbol property_set_reference_parameter_rejects_udt_types -- --nocapture`
- `cargo test -p oxvba-symbol optional_parameter_rejects_udt_types -- --nocapture`
- `cargo test -p oxvba-symbol signature_declarations_reject_as_new_types -- --nocapture`
- `cargo test -p oxvba-symbol type_block_fields_reject_as_new_types -- --nocapture`
- `cargo test -p oxvba-symbol type_block_fields_reject_duplicate_names -- --nocapture`
- `cargo test -p oxvba-symbol object_module_type_blocks_must_be_private -- --nocapture`
- `cargo test -p oxvba-symbol public_declare_is_not_valid_in_object_modules -- --nocapture`
- `cargo test -p oxvba-symbol public_object_module_data_members_are_rejected -- --nocapture`
- `cargo test -p oxvba-symbol withevents -- --nocapture`
- `cargo test -p oxvba-symbol required_parameter_defaults -- --nocapture`
- `cargo test -p oxvba-symbol invalid_event_parameter_modifiers -- --nocapture`
- `cargo test -p oxvba-symbol event_declaration_rejects_as_new_parameters -- --nocapture`
- `cargo test -p oxvba-bind property_set_udt_reference_parameter_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind optional_udt_parameter_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind signature_as_new_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind type_block_field_as_new_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind type_block_duplicate_field_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind class_module_public_type_block_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind class_module_public_declare_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind class_module_public_data_members_are_bind_errors -- --nocapture`
- `cargo test -p oxvba-bind required_parameter_default_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind event_declaration_rejects_invalid_parameters -- --nocapture`
- `cargo test -p oxvba-bind event_declaration_as_new_parameter_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind withevents_local_declaration_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind withevents_const_declaration_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind withevents_scalar_field_type_is_bind_error -- --nocapture`
- `cargo test -p oxvba-bind withevents_as_new_field_is_bind_error -- --nocapture`
- `cargo test -p oxvba-compiler compile_project_uses_hir_capable_boundary_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler compile_project_does_not_inject_runtime_validation_for_rewritten_internal_class_object_locals --quiet`
- `cargo test -p oxvba-compiler compile_project_infers_non_authoritative_single_candidate_indexed_default_member_let --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_bang_member_access --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_statement_form_dispatchinvoke_arguments --quiet`
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
- `cargo test -p oxvba-syntax parses_type_block_field_declaration_shapes_losslessly --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_preserves_rich_udt_field_metadata --quiet`
- `cargo test -p oxvba-compiler udt_field_read_write --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_nested_udt_member_chain_aliases --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_udt_array_field_index_aliases --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_accepts_dynamic_udt_array_field_index_aliases --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_rejects_non_static_udt_array_field_index --quiet`
- `cargo test -p oxvba-compiler hir_production_lowering_rejects_cross_type_udt_whole_assignment --quiet`
- `cargo test -p oxvba-compiler compile_udt_whole_assignment_emits_field_copy_slots --quiet`
- `cargo test -p oxvba-compiler frontend_diff_v2_smoke_matches_legacy_for_supported_assignment --quiet`
- `cargo test -p oxvba-compiler frontend_diff --quiet`
- `cargo test -p oxvba-compiler compile_with_runtime_metadata_uses_hir_for_completed_constructs --quiet`
- `cargo test -p oxvba-compiler syntax_bridge --quiet`
- `cargo test -p oxvba-compiler --quiet`
- `cargo check -p oxvba-compiler`
- `cargo fmt --check -p oxvba-compiler`
- `cargo test -p oxvba-symbol deftype --quiet`
- `cargo test -p oxvba-symbol --quiet`
- `cargo test -p oxvba-bind scalar_deftype_defaults_affect_variables_params_and_returns --quiet`
- `cargo test -p oxvba-bind --quiet`
- `cargo check --workspace`
- `cargo fmt --check -p oxvba-symbol -p oxvba-bind`
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
