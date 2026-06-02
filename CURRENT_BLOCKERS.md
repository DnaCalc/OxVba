# Current Blockers

Date: 2026-03-11
Run context: active parity/compliance execution plus in-progress feature worklist execution pass

## Status update

### BLK-EVT-001: Runtime subscription graph execution model
- Status: resolved in current run.
- Resolution summary:
  - Removed compile-time bounded owner fanout from `RaiseEvent` lowering.
  - Added runtime owner-iteration intrinsics:
    - `__oxvba_withevents_first_owner(source, binding)`
    - `__oxvba_withevents_next_owner()`
  - Wrapper lowering now iterates runtime owner bindings dynamically and dispatches handlers with sink-owner identity.
  - Added/updated compiler/optimizer/VM/host tests to lock deterministic behavior.

### BLK-RUNTIME-VALUE-MODEL-001: Runtime value-model migration
- Status: resolved in current run.
- Resolution summary:
  - VM/register/host execution is now value-first end to end:
    - register storage persists `RuntimeValue`,
    - public VM/JIT/host execution APIs are semantic-snapshot first,
    - retained `Variant`/`RuntimeValue` snapshots are the supported execution observation surface; the old integer-slot observation lane has since been removed.
  - The interpreter loop no longer executes through the old raw slot-helper vocabulary:
    - core compare/boolean/jump/increment lanes now read/write semantic runtime values,
    - the wider loop now uses explicit legacy-projection helpers over `RuntimeValue` where scalar compatibility is still intentional,
    - `CopySlot` now preserves full `RuntimeValue` shape instead of collapsing through the integer lane.
  - The owned runtime `Variant` bridge now honestly covers the current scalar/error subset:
    - `Empty`,
    - `Null`,
    - `ErrorCode`,
    - `I32`,
    - `Bool`.
  - The dynamic-object protocol blocker that followed this migration is now also resolved:
    - native project class methods, properties, and default-member dispatch all execute on the shared dynamic-object protocol.

### BLK-COM-BOUNDARY-001: Final oxvba-com extraction from HAL
- Status: resolved in current run.
- Resolution summary:
  - oxvba-com now exposes WindowsComBridge as the live Windows COM client facade.
  - standard.rs now delegates create-object activation, invoke execution, object description/release, event subscription/callback access, and typelib resolve/load/invalidate through that bridge.
  - native subscription transport teardown for object release now also executes inside oxvba-com, removing the last substantive COM lifecycle seam from HAL.
  - the remaining HAL COM code is limited to capability/policy gating, apartment/bootstrap hooks, deterministic projection fallback, and error mapping.
  - the IP-04 closure verification matrix is green:
    - cargo fmt --all,
    - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings,
    - cargo test -p oxvba-com -p oxvba-hal -p oxvba-host --quiet,
    - ./scripts/check-governance.ps1,
    - ./scripts/meta-check.ps1 -Fast -NoArtifacts.

## Active blocker entries

### FE-PROD-001: Frontend production replacement not complete
- Status: open workset blocker after 2026-06-01 scope audit.
- Impact:
  - Reopens `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`.
  - Prior terminal evidence is superseded as closure evidence because it proved broad tests and
    descriptor/runtime fixes, not end-to-end production front-end replacement.
- Current state:
  - Useful partial work exists: `oxvba-syntax`, HIR/SemanticModel scaffolds, route policy,
    semantic harness pieces, descriptor metadata, and VM/host snapshot fixes.
  - Production compilation still has load-bearing legacy front-end routes:
    - `frontend_v2` is an opt-in CST-validation bridge before legacy compile;
    - `resolve.rs` legacy expression/string-derived binding remains authoritative for key paths;
    - `project.rs` source-text rewrite behavior remains production behavior for project/class/COM
      semantics.
  - 2026-06-01 narrowing:
    - scoped single-source HIR lowering now covers the broad completed subset recorded in
      `docs/evidence/frontend_rework/PRODUCTION_HIR_LOWERING_2026-06-01.md`;
    - the HIR `Const` route now includes simple constant expressions such as `Const CBase = 1 + 2`
      without allocating runtime local slots for the constant symbol, including references to
      earlier declarators in the same `Const` statement;
    - read-side bang member access such as `obj!Field` now reaches HIR production lowering through
      the existing member-expression dispatch route;
    - simple member assignment targets such as `obj.Value = ...`, `obj!Value = ...`,
      `Set obj.Ref = ...`, and `With obj: .Value = ...` now lower through HIR to late-bound
      dispatch with explicit property Let/Set hints; default-member/property selection,
      project/class property routing, early-bound COM property-put resolution, indexed/named
      writeback breadth, and type overload validation remain open;
    - `Option Base 0`, `Option Base 1`, and default-equivalent `Option Compare Binary` no longer
      disqualify otherwise completed lightweight HIR default-route sources; other `Option` forms
      remain fallback-only until HIR owns explicit declaration enforcement, text/database compare
      mode, and module privacy semantics;
    - `New <Class>` is represented as first-class HIR and can lower through typed
      `StructuralIntrinsic::ProjectInstance(handle)` when supplied with
      `HirNewExpressionBinding` facts;
    - a HIR compile-to-bytecode entry point now accepts those construction facts and emits the
      existing project-object reference bytecode path;
    - single active procedural-module projects with no reference projects now enter the
      HIR-capable metadata compiler at the project boundary, while broader project shapes remain
      on the legacy project backend until their metadata/rewrite semantics are owned by HIR;
    - project binding now materializes those HIR construction facts in source order, and the
      project compile boundary consumes them for the accepted direct active-project
      `Set x = New Widget` and accepted active-project `Dim x As New Widget` shapes by compiling a
      HIR construction candidate with `HirNewExpressionBinding` facts instead of using the
      generated `__oxvba_project_instance(handle)` helper assignment as the production compile
      artifact. The `As New` HIR candidate now removes the fallback-compatible eager construction
      carrier and inserts guarded first-use/after-`Nothing` `New` sites for accepted dereference
      lines.
    - downstream object-state regressions narrowed during this pass:
      - `bd-7ins` is closed: source-class public field reads such as `c.Total` now observe
        object-owned per-instance field state after member calls;
      - the concrete `bd-asrd` WithEvents rewrite failure is partially fixed:
        `Set <WithEventsField> = New <ActiveProjectClass>` now lowers to a temporary
        project instance before `__oxvba_withevents_set(...)`, so the legacy parser no longer
        sees raw `New <Class>` text on that path.
      - `bd-asrd` remains open for arbitrary/nested legacy `New <Class>` expression parsing;
        the intended production replacement is still the HIR/project-aware construction route.
- Unblocking path:
  - execute reopened `bd-aprs` beads under the production replacement criteria;
  - convert scaffold/evidence surfaces into default production routing;
  - delete or compatibility-quarantine replaced legacy routes;
  - rerun terminal compiler/syntax/VM/host/conformance/oracle gates with route proof.
  - 2026-06-02 bead-graph repair keeps the existing workset/root bead and makes the remaining
    production replacement work explicit:
    - `bd-aprs.8.7`: property/default-member production semantics;
      - continuation progress: imported-COM dispatch classification now records typelib invoke
        kind and early-bound COM property read/put/putref rewrite paths validate dispatch id and
        invoke kind before retaining the compatibility `DispatchInvoke` carrier; selected
        host-injected property/default-member routes now validate through `HostGlobal`
        classification before retaining the compatibility PMR rewrite carrier; statement-form
        named arguments now survive HIR and HIR production lowering into call-site argument binding
        metadata, including explicit no-paren `Call Proc name := value` and parenthesized
        `Call Proc(name := value)`; late-bound variable default-member calls such as `obj(42)` now
        lower through HIR into default-member dispatch metadata; late-bound variable indexed
        default-member assignments now lower through `BoundStmt::AssignDefaultMember`, preserve
        indexed argument names, and emit dispatch member id `0` with `PropertyLet`/`PropertySet`
        hints plus `LateBoundDefaultMember`/`SyntheticPropertyAssignment` call-site metadata with
        the synthetic `value` argument; multiple authoritative `VB_UserMemId = 0` candidates of
        the required accessor kind now reject as default-member ambiguity instead of selecting the
        first sorted candidate; selected active-project default-member accessors now validate
        source argument count before rewrite; selected active-project property/default-member
        rewrite routes now validate `EarlyBoundProject` member-dispatch classification before
        retaining the compatibility carrier; predeclared `Property Get` read rewrite maps are now
        fallible and classifier-backed, requiring active-project `EarlyBoundProject` property-get
        proof or host-injected `HostGlobal` proof before retaining the compatibility carrier;
        selected default-member assignment routes now reject clear assignment-form/type
        mismatches, covering `Let` into explicitly object-typed value parameters and `Set` into
        definitely scalar-typed value parameters; imported bare/indexed default-member assignment
        syntax now resolves through typelib metadata for the required `PropertyPut`/`PropertyPutRef`
        kind before rewrite, producing frontend/typelib diagnostics instead of backend parse
        fallout when the imported type has no default setter; fixture-backed default put/putref
        assignments, named/indexed imported COM property put/putref assignments, and selected
        imported COM property-get read/invoke routes now use
        accessor-specific internal early-invoke carriers and, for the single-active-module
        type-library-only subset, compile the active module through the HIR-capable boundary with
        early-bound COM dispatch-id metadata plus `PropertyGet`/`PropertyLet`/`PropertySet`
        bytecode hints instead of the legacy project backend; representative host-injected
        default-member/property get plus non-indexed and indexed let/set compatibility-helper
        routes now also assert patched `CallProc.project_member` bytecode metadata for the
        expected `pmr_*` helper identity and accessor kind, making the current rewrite-backed host
        semantics explicit while HIR ownership remains open;
      - still open: broader project/host/imported-COM default-member writeback breadth, type
        overload validation, and replacement/quarantine of the remaining rewrite bodies;
    - `bd-aprs.8.8`: reference/COM activation and member binding;
    - `bd-aprs.9.6`: completed for direct active-project `Set obj = New Class` construction on
      HIR using generated `HirNewExpressionBinding` facts, without compiling the generated
      `__oxvba_project_instance(...)` assignment as the production artifact;
    - `bd-aprs.9.7`: completed for scoped accepted active-project lazy first-use/after-`Nothing`
      `As New`, field-mutating `Class_Initialize`, narrowed source-class
      `WithEvents Set x = New T` construction, private `Class_Terminate` metadata, and
      first-use/after-`Nothing` source maps; imported/reference/COM activation remains owned by
      `bd-aprs.8.8`, unsupported fallback shapes by the broad route audit, and broader event
      semantics by FE-7/FE-9 coverage;
    - `bd-aprs.9.8`: arrays, indexing, and `ReDim` parity;
      - continuation progress: dynamic-array runtime `ReDim` lowering now covers
        one-dimensional and two-dimensional runtime bounds, static integer explicit lower-bound
        `To` forms, read/write dynamic-array element access, initial fixed-array alias
        materialization plus fixed-array `ReDim` rematerialization for static integer bounds, and
        local multidimensional dynamic/fixed element access with static integer fixed indices, and
        array-shape rank metadata updates from observed `ReDim` bounds; the front-end
        `ProjectSymbolIndex` now records class and procedural module array-field descriptors,
        including dynamic fields, multidimensional fixed bounds, and `Option Base`-derived omitted
        lower bounds, and class array-field descriptors now flow into `ProjectDynamicObjectRoute`
        metadata with stable field tokens; dynamic class array-field `ReDim`, element writes, and
        element reads now bridge through the per-instance field token;
        fixed project/class array-field executable semantics and broader project-owned array shapes
        remain open;
    - `bd-aprs.9.9`: compile-time options, declarations, and constants;
      - continuation progress: `Option Explicit` now preserves the HIR-bound module flag and has
        production route-audit coverage; `Option Compare Text` now routes through HIR for otherwise
        completed lightweight sources and emits text comparison bytecode; `Option Compare Database`
        now routes through HIR/default production with the current binary-runtime compare
        approximation; `Option Private Module` now routes through single-source/default HIR while
        project module-kind/reference visibility validation remains owned by the project route;
        basic DefType default table preservation now covers local untyped `Dim`,
        parameters, function returns, and module-scope scalar `Dim` declarations through HIR;
        basic `#Const`/`#If`/`#Else`/`#End If` filtering now runs before the default HIR route for
        otherwise completed single-source inputs;
        basic single-source module `Attribute VB_Name` lines now route through default HIR as
        ignored metadata;
        basic `Const Name As Long = <literal/simple-expression>` declarators now route through
        default HIR;
        broader DefType surfaces for visibility-prefixed class/project fields, project/member
        attribute semantics, broader conditional-compilation/preprocessor parity, typed constant
        coercion/type validation breadth, and broader compile-time constant evaluation remain open;
    - `bd-aprs.9.10`: broader declaration/type surface;
      - continuation progress: optional parameters with simple explicit defaults now remain eligible
        for the default HIR route and preserve optional/default signature metadata;
        HIR lowering now preserves `Property Get`/`Property Let` declaration metadata and binds the
        property getter self-assignment return slot; same-module zero-argument `Property Get`
        reads and simple same-module `Property Let`/`Property Set` writes now lower through HIR as
        procedure calls; simple non-indexed property declarations now remain eligible for the
        default HIR route; simple `ParamArray` declarations with positional packed calls now remain
        eligible for the default HIR route and preserve ParamArray signature/call-site pack
        metadata while retaining the named ParamArray-target rejection; HIR now resolves and lowers
        `LBound`/`UBound` array-bound intrinsics plus the one-argument `IsArray`, `VarType`,
        `TypeName`, `IsNumeric`, `IsDate`, `IsObject`, `IsEmpty`, `IsNull`, and `IsError`
        introspection/predicate intrinsics, including those forms inside a ParamArray callee;
        deterministic string/search intrinsics `Len`, `Left`, `Right`, `Mid`, `InStr`,
        `InStrRev`, `Replace`, and `StrComp` now lower through HIR;
        deterministic numeric/math intrinsics `Abs`, `Int`, `Fix`, `Sgn`, `Round`, `Sqr`, `Sin`,
        `Cos`, `Log`, `Exp`, `Atn`, and `Tan` now lower through HIR;
        general unary minus/plus and `Not` expressions now lower through HIR;
        deterministic date/time intrinsics `Year`, `Month`, `Day`, `Weekday`, `MonthName`,
        `DateValue`, `TimeValue`, `DateSerial`, `TimeSerial`, `DateAdd`, and `DateDiff` now lower
        through HIR;
        deterministic conversion/formatting intrinsics `CStr`, `Str`, `Val`, `CDate`, `Hex`, and
        `Oct` now lower through HIR;
        deterministic string transform/format intrinsics `LCase`, `UCase`, `Trim`, `LTrim`,
        `RTrim`, `Space`, `String`, `Chr`, `Asc`, `StrReverse`, `StrConv`, `Format`, `Split`, and
        `Join` now lower through HIR;
        deterministic collection intrinsics `CollectionAdd`, `CollectionItem`, `CollectionRemove`,
        and `CollectionCount` now lower through HIR;
        deterministic financial intrinsics `FV`, `PV`, `Pmt`, `NPV`, `IRR`, `MIRR`, `Rate`, and
        `NPer` now lower through HIR;
        pointer helpers `StrPtr`, `VarPtr`, and `ObjPtr` have explicit production-HIR proof through
        the typed structural intrinsic route;
        `Array(...)` now lowers through HIR to the existing array-literal intrinsic bytecode;
        VM-stateful deterministic `Rnd`/`Randomize` no-seed and seeded forms now lower through HIR;
        `TypeOf ... Is ...` now lowers through HIR as a dedicated type-test expression;
        time-locale host intrinsics `Date()`, `Time()`, `Now()`, and `Timer()` now lower through HIR;
        host utility intrinsics `FreeFile()`/`FreeFile(range)` and `DoEvents()` now lower through HIR;
        file-position host intrinsics `EOF(handle)`, `LOF(handle)`, `Seek(handle)`, and
        `Loc(handle)` now lower through HIR;
        dialog host intrinsics `MsgBox(prompt[, style])` and `InputBox(prompt[, default])` now lower
        through HIR;
        process/environment host intrinsics `Shell(command)`, `Environ(key)`, `Dir()`, and
        `Dir(path)` now lower through HIR;
        `CreateObject(progId)` now lowers through HIR to the existing COM object creation host
        bytecode;
        explicit `DispatchInvoke`/`__oxvbaearlyinvoke` structural dispatch helpers now preserve
        named arguments through HIR into the existing host dispatch bytecode;
        console `Print` and diagnostics `Debug.Print` now lower through HIR to the existing host
        bytecode;
        file-system statement `Kill path` now lowers through HIR to the existing file-kill host
        bytecode;
        console `Input a[, b...]` now lowers through HIR to the existing console-input host
        bytecode;
        console `Line Input target` now lowers through HIR to the existing console line-input host
        bytecode;
        file-handle `Close #handle` and close-all `Close` now lower through HIR to the existing
        file-close host bytecode;
        file-handle `Print #handle, data` now lowers through HIR to the existing file-print host
        bytecode for simple handle/payload expressions;
        file-handle `Write #handle, item[, ...]` now lowers through HIR to the existing file-write
        host bytecode for simple handle/payload expressions;
        file-handle `Input #handle, target[, ...]` now lowers through HIR to the existing file-input
        host bytecode;
        file-handle `Line Input #handle, target` now lowers through HIR to the existing file
        line-input host bytecode;
        file-handle `Open path For mode As #handle` now lowers through HIR to the existing file-open
        host bytecode for simple path/handle expressions;
        `Mod` and `Like` expressions now lower through HIR to the existing modulo and like bytecode;
        richer default expressions, remaining deterministic intrinsic families, host-sensitive
        intrinsic breadth inside ParamArray callees, and
        broader call-entry optional/missing-state behavior, plus indexed property invocation and
        default-route property semantics, remain open;
    - `bd-aprs.10.7`: broad matrix/corpus route audit;
    - `bd-aprs.10.8`: final legacy route retirement/quarantine.

### FE-TERM-001: Frontend rework terminal evidence compiler metadata failure
- Status: resolved in current run; superseded by FE-TERM-002 for terminal closure.
- Impact:
  - Blocks truthful closure of
    `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`.
  - Prevents claiming full compiler parity for the frontend rework workset.
  - Does not invalidate the focused frontend module checks or syntax checks, which pass.
- Resolution summary:
  - Preserved the runtime-facing argument `source_slot` as the descriptor-transfer temporary.
  - Added metadata-only `ArgumentBindingDescriptor::source_declared_type` so call-entry coercion
    descriptors can report the caller variable's declared type without corrupting VM argument
    transfer.
  - Fixed `ParamArrayPack` call-site descriptors to carry the packed array source slot.
  - Updated VM descriptor evidence to recognize descriptor-native return copyout, ByRef
    writeback, optional default, ParamArray pack, and selected call-entry coercion evidence.
  - Passing checks after this fix:
    - `cargo test -p oxvba-compiler procedure_runtime_metadata_carries_expression_operator_and_coercion_descriptors -- --nocapture`
    - `cargo test -p oxvba-compiler --quiet`
    - `cargo test -p oxvba-vm --quiet`
    - `cargo test -p oxvba-syntax --quiet`
- Evidence:
  - `docs/evidence/frontend_rework/TERMINAL_CLOSURE_2026-06-01.md`

### FE-TERM-002: Frontend terminal gate host snapshot regressions
- Status: resolved in current run.
- Resolution summary:
  - Added a VM-owned completed activation-frame snapshot surface keyed by procedure entry PC.
  - Host project-visible snapshots now project from the completed entry-procedure frame instead of
    relying on the retained global register window after the startup shim returns.
  - Captures occur after local/temp release and termination drain, while the frame is still
    current, so `Class_Terminate` timing is not perturbed.
  - Completed-frame snapshots sanitize terminating project-object references to `Empty`; host/COM
    object references remain observable.
  - Updated snapshot tests whose old expectations encoded the pre-activation-frame empty shape or
    compared raw per-run project-object pointers instead of canonical object identity.
- Passing checks after this fix:
  - `cargo test -p oxvba-host --quiet`
  - `cargo test -p oxvba-vm --quiet`
  - `cargo test -p oxvba-compiler --quiet`
  - `cargo test -p oxvba-syntax --quiet`
  - `cargo fmt --check -p oxvba-compiler -p oxvba-vm -p oxvba-host`
  - `git diff --check`
- Evidence:
  - `docs/evidence/frontend_rework/TERMINAL_CLOSURE_2026-06-01.md`

### RV-BRIDGE-001..004: resolved by RuntimeValue source-carrier removal
- Status: resolved in recovery run.
- Resolution summary:
  - `bd-0w46` / commit `8d5fdfc0` removed the `RuntimeValue` carrier enum,
    runtime/host compat modules, Variant/SAFEARRAY bridge helpers, and active
    VM/JIT/host compatibility shims.
  - Active Rust source search is clean:
    `rg -n "RuntimeValue|runtime_value" crates --glob '*.rs'` returns no matches.
  - The older blocker register
    `docs/evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md`
    is retained as historical evidence only.
  - Native-Ready phase-3/phase-4 recovery has executable proof again:
    `VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md` and
    `CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md`.
  - Native-Ready runner proof is recovered for VM/JIT and wrapper EXE; wrapper
    library row production remains open as non-blocking follow-up bead `bd-9xmu.5.9`.
- Evidence:
  - `docs/evidence/native_ready/RUNTIMEVALUE_ACTIVE_RUST_SOURCE_REMOVAL_2026-05-01.md`
  - `docs/evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md`
  - `docs/evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md`


### NR-RUNNER-WRAPPER-LIB-001: wrapper library schema producer follow-up
- Status: open non-blocking follow-up.
- Impact:
  - Blocks only future claims of real wrapper-library row production.
  - Does not block recovered Native-Ready VM/JIT or wrapper EXE schema producer evidence.
- Current state:
  - `bd-9xmu.5.7` added active VM/JIT row production through
    `oxvba_host::emit_native_ready_vm_jit_csv` and the
    `oxvba native-ready-runner` CLI command.
  - `bd-9xmu.5.8` added `oxvba native-ready-runner --wrapper-exe`, which
    builds and executes a real wrapper EXE artifact row.
  - Wrapper library rows remain sample/schema-only.
  - Follow-up delivery bead: `bd-9xmu.5.9`.
- Exact unblocking steps:
  - implement wrapper library artifact execution under the shared schema,
  - capture `artifact_path`, `artifact_size_bytes`, exported-call result digest,
    elapsed timing fields, and claim boundary,
  - add executable tests/evidence and update phase-5 docs.
- Evidence:
  - `docs/evidence/native_ready/RUNNER_PRODUCER_RECOVERY_2026-05-02.md`

### BLK-XLL-EXCEL-HOST-001: resolved in current run
- Status: resolved.
- Resolution summary:
  - `bd-xll1.2` delivered generated `xlAutoOpen` / `xlfRegister` registration source from native export metadata.
  - `bd-xll1.3` delivered generated XLL export wrappers that bridge XLOPER12-shaped arguments/results to retained `Variant` procedure invocation.
  - `bd-xll1.4` published the validation matrix and explicit non-claims.
  - `bd-xll1.5.1` proved local compile staging: generated source compiles through `ShimOutputType::Xll` and produces a non-empty `.xll` artifact.
  - `bd-xll1.6` replaced the placeholder XLOPER12 scalar lane with the Excel 12 scalar ABI shape, xltype-driven argument decoding, and owned counted-wide-string return handling.
  - `bd-xll1.7` wired `oxvba build` for `OutputType=Addin` to emit a real local `.xll` package by default.
  - The staged child workset
    `docs/worksets/WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`
    is complete.
  - Excel-host validation now proves:
    - `Application.RegisterXLL(...)` loads the staged `.xll`,
    - `xlAutoOpen` runs,
    - `MdCallBack12` is resolved from `EXCEL.EXE`,
    - `xlGetName` succeeds,
    - `xlfRegister` succeeds for all four scoped exported functions,
    - worksheet formulas return expected Double, String, Boolean, and Long
      scalar values.
  - Implementation-owned findings were fixed in code rather than by deleting
    tests:
    - callback resolution now uses `MdCallBack12` instead of assuming an
      exported `Excel12v`,
    - compiler/native-export metadata now carries typed procedure signatures,
    - registration uses SDK-shaped `xlfRegister` arguments with `xlGetName`,
    - type strings now match the generated XLOPER12 pointer wrapper ABI.
  - Evidence:
    - `docs/evidence/XLL_EXCEL_REGISTRATION_TRACE_2026-04-28.md`
    - `docs/evidence/XLL_EXCEL_WORKSHEET_INVOCATION_2026-04-28.md`

### BLK-OXIDE-DIRECT-CONSUMPTION-001: Direct Immediate/debug consumption not yet evidenced
- Impact:
  - Blocks closure of `bd-oxi1.6.2`, `bd-oxi1.6`, and the `bd-oxi1` parent.
  - Blocks any cross-repo claim that DnaOxIde/OxIde has consumed the new OxVba
    Immediate/debug/watch/breakpoint DTOs directly in UI wiring.
  - The OxIde-side direct workspace/project-helper/runtime evidence is now
    captured, but the bead explicitly also requires direct Immediate Window and
    debug-seam consumption evidence.
- Current state:
  - Evidence captured in `docs/evidence/OXIDE_DIRECT_HOST_CONSUMPTION_EVIDENCE_2026-04-27.md`.
  - `C:\Work\DnaCalc\OxIde\src\shell\oxvba.rs` directly consumes
    `HostWorkspaceSession` for workspace semantics and direct hover/definition
    style queries.
  - `C:\Work\DnaCalc\OxIde\src\shell\project_actions.rs` directly consumes
    `oxvba_project` host-helper surfaces for project/module/reference flows.
  - OxIde focused validation passes:
    - `cargo test real_oxvba --quiet`
    - `cargo test project_actions --quiet`
  - OxIde Immediate/debug docs still describe planned or future
    OxVba-contract-dependent surfaces rather than proved direct consumption.
  - OxVba-side direct APIs and fixture evidence for the DnaOxIde handoff are now
    available under `bd-avdu`:
    - `EmbeddedRunSession::into_immediate_session`
    - `EmbeddedRunSession::into_debug_session`
    - stable Immediate/debug/runtime/frame/watch/breakpoint IDs
    - debugger-owned watch registry and breakpoint binding DTOs
    - `ComCapabilityProfile`, `ComRuntimeInvocationAvailability`, and
      `ComReferenceReorderPlan`
    - `crates/oxvba-languageservice/tests/dnaoxide_thin_slice_hello.rs`
    - `docs/evidence/DNAOXIDE_THIN_SLICE_HELLO_FIXTURE_2026-05-07.md`
- Exact unblocking steps:
  - land or identify OxIde code that routes Immediate Window evaluation through
    the direct OxVba immediate/debug/session APIs,
  - land or identify OxIde code that routes debug controls/state through the
    direct OxVba debugger seam,
  - add or run focused OxIde tests proving those paths do not use CLI or LSP
    fallbacks,
  - update the OxIde evidence file to cite the new OxVba direct APIs/fixture
    evidence and then close `bd-oxi1.6.2`, `bd-oxi1.6`, and the parent if no
    other child blockers remain.

### BLK-COM-IDISPATCH-001: Late-bound COM parity remains below VBA/Excel `IDispatch` behavior
- Impact:
  - Blocks `IP-03` Windows late-bound COM client parity.
  - Blocks full closure of `HAL-DYN-008` and parts of `IP-09` declare/marshaling parity.
- Current state (tabular evidence matrix):

  **Invoke transport:**

  | lane                          | status       | evidence                              |
  |-------------------------------|--------------|---------------------------------------|
  | named/omitted arg metadata    | proved-exec  | ComInvokeRequest carries per-arg name |
  | named-arg DISPPARAMS packing  | proved-exec  | method/property-get lanes             |
  | property-put/putref canonical | proved-exec  | indexed/named arg canonicalization     |
  | omitted-arg fault             | proved-exec  | deterministic required-arg faults      |

  **Scalar result conversion:**

  | VT code   | carrier        | status       |
  |-----------|----------------|--------------|
  | VT_EMPTY  | Empty          | proved-exec  |
  | VT_NULL   | Null           | proved-exec  |
  | VT_ERROR  | ErrorCode      | proved-exec  |
  | VT_BOOL   | Bool           | proved-exec  |
  | VT_I1..I4 | I32            | proved-exec  |
  | VT_I8     | I32 or I64     | proved-exec  |
  | VT_UI1..2 | I32            | proved-exec  |
  | VT_UI4    | I32 or I64     | proved-exec  |

### BLK-COM-TYPELIB-REALHOST-001: resolved in current run
- Status: resolved in current run.
- Resolution summary:
  - `Scripting.FileSystemObject` now keeps a class-specific `scrrun.dll` typelib identity instead of collapsing onto the `Scripting.Dictionary` fast path.
  - Live typelib loading now scopes member/event extraction to the requested coclass's default interface when a class-qualified ProgID is known.
  - Active host-backed regression coverage now proves the deterministic early-bound `Scripting.FileSystemObject` helper subset via `GetExtensionName` and `GetBaseName`.
  - The broader `COM-0004` residual scope remains open, but it is no longer blocked on `BIND-E-TYPELIB-MEMBER-AMBIGUOUS` for `Scripting.FileSystemObject`.

### BLK-PH-TOPLEVEL-MODULESTATE-001: Project-hosted helper procedures do not yet share rewritten top-level module state
- Impact:
  - Blocks closure of the remaining accepted `PH-0002` mixed-source/module-state lane.
  - Keeps `bd-cyr.4.2` from closing as a full project/hosting residual sweep.
- Current state:
  - New console-backed host tests prove unique basproj/VBP top-level mainline behavior plus mixed module-scope declaration preservation in VM/JIT.
  - The sharper Option Private residual repro now shows:
    - `pre=41`
    - `bump=1`
    - `post=41`
  - That means the rewritten `__OxVbaTopLevelMainline` sees module-scope state, but the declared helper procedure resolves the same module-scope name as separate procedure-local state.
- Exact unblocking steps:
  - inspect project-hosted procedure binding/lowering for rewritten top-level modules,
  - make declared helper procedures share module-scope storage with the rewritten top-level mainline,
  - then unignore the new host regression and update `PH-0002`.
  | VT_UI8    | I32 or I64     | proved-exec  |
  | VT_INT    | I32            | proved-exec  |
  | VT_UINT   | I32 or I64     | proved-exec  |
  | VT_R4     | F64(Single)    | proved-exec  |
  | VT_R8     | F64(Double)    | proved-exec  |
  | VT_DATE   | F64(Date)      | proved-exec  |
  | VT_CY     | Currency       | proved-exec  |
  | VT_DECIMAL| Decimal96      | proved-exec  |
  | VT_BSTR   | String         | proved-exec  |
  | VT_DISPATCH| ObjectHandle  | proved-exec  |
  | VT_UNKNOWN (IDispatch)| ObjectHandle | proved-exec |
  | VT_UNKNOWN (no IDispatch)| — | deterministic E_NOINTERFACE |
  | VT_BYREF  | —              | deterministic unsupported diagnostic |

  **SAFEARRAY result conversion:**

  | element VT    | rank | carrier              | status       |
  |---------------|------|----------------------|--------------|
  | 17 typed VTs  | 1    | matching scalar      | proved-exec  |
  | VT_VARIANT    | 1    | nested scalar/object | proved-exec  |
  | VT_DISPATCH   | 1    | ObjectHandle         | proved-exec  |
  | VT_UNKNOWN (IDispatch)| 1 | ObjectHandle  | proved-exec  |
  | VT_UNKNOWN (no IDispatch)| 1 | —           | deterministic E_NOINTERFACE |
  | typed VTs     | 2+   | matching scalar + bounds | proved-exec |
  | VT_VARIANT    | 2+   | nested scalar + bounds   | proved-exec |

  **Outbound argument conversion:**

  | value shape     | VT out      | status       |
  |-----------------|-------------|--------------|
  | Bool(True)      | VT_BOOL     | proved-exec  |
  | String/BSTR     | VT_BSTR     | proved-exec  |
  | Empty/Null/CVErr| matching VT | proved-exec  |
  | ObjectHandle    | VT_DISPATCH | proved-exec  |
  | F64(Single/Double/Date)| VT_R4/R8/DATE | proved-exec |
  | Currency/Decimal| VT_CY/DECIMAL | proved-exec |
  | Array(...)      | VT_ARRAY\|VT_VARIANT | proved-exec |
  | I64             | VT_I8       | proved-exec  |

  **Invoke error classification:**

  | error shape              | status       |
  |--------------------------|--------------|
  | DISP_E_TYPEMISMATCH+ArgErr | proved-exec |
  | DISP_E_EXCEPTION+ExcepInfo | proved-exec |
  | DISP_E_BADPARAMCOUNT     | proved-exec  |
  | DISP_E_PARAMNOTFOUND     | proved-exec  |
  | DISP_E_MEMBERNOTFOUND    | proved-exec  |
  | DISP_E_UNKNOWNNAME       | proved-exec  |
  | E_NOINTERFACE            | proved-exec  |

  **All gaps closed:**

  | gap (previously open)                          | resolution                                    |
  |------------------------------------------------|-----------------------------------------------|
  | natural default-member for non-metadata bindings | passthrough for runtime GetIDsOfNames resolution |
  | broad non-IDispatch interface-pointer handling   | deterministic E_NOINTERFACE rejection          |
  | non-IDispatch element arrays                     | deterministic E_NOINTERFACE per-element        |
  | fuller external VarResult surface                | full ExcepInfo (help_file/help_context/wcode)  |
  | richer external ExcepInfo/arg-fault coverage     | HAL-DYN-008 verified, full EXCEPINFO surface   |
  | practical Office automation lanes                | oracle concern under IP-10                     |

- Status: **resolved** on 2026-03-20. All implementation-owned late-bound COM parity lanes are closed.
  Remaining external Office runtime-behavior verification is an oracle concern under `IP-10`.
- Recommendation:
  - close this blocker; remaining oracle/formal verification is owned by `IP-10` / `IP-11`.

### BLK-COM-VALUE-TRANSPORT-001: Shared COM value transport still lacks full COM payload fidelity
- Impact:
  - Blocks the remaining high-value closure work in `IP-03` Windows late-bound COM client parity.
  - Blocks practical SAFEARRAY/object/string COM transport and therefore parts of `IP-09` marshaling parity and downstream COM parity work.
- Current state (tabular evidence matrix):

  **ComValue carrier coverage:**

  | carrier          | runtime mapping   | outbound VT   | status       |
  |------------------|-------------------|---------------|--------------|
  | Empty            | RuntimeValue::Empty | VT_EMPTY    | proved-exec  |
  | Null             | RuntimeValue::Null  | VT_NULL     | proved-exec  |
  | ErrorCode(i32)   | RuntimeValue::ErrorCode | VT_ERROR | proved-exec |
  | Bool(bool)       | RuntimeValue::Bool  | VT_BOOL     | proved-exec  |
  | I32(i32)         | RuntimeValue::I32   | VT_I4       | proved-exec  |
  | I64(i64)         | RuntimeValue::I64   | VT_I8       | proved-exec  |
  | F64(Single)      | RuntimeValue::F64   | VT_R4       | proved-exec  |
  | F64(Double)      | RuntimeValue::F64   | VT_R8       | proved-exec  |
  | F64(Date)        | RuntimeValue::F64   | VT_DATE     | proved-exec  |
  | Decimal(Decimal96)| RuntimeValue::Decimal | VT_DECIMAL | proved-exec |
  | Currency         | RuntimeValue::Currency | VT_CY     | proved-exec  |
  | String(BStr)     | RuntimeValue::String | VT_BSTR    | proved-exec  |
  | ArrayIntent      | RuntimeValue::ArrayIntent | VT_ARRAY | proved-exec |
  | ObjectHandle     | RuntimeValue::ObjectHandle | VT_DISPATCH | proved-exec |

  **SAFEARRAY transport:**

  | dimension | element vartypes          | direction | status       |
  |-----------|---------------------------|-----------|--------------|
  | rank-1    | 17 typed scalar VTs       | result    | proved-exec  |
  | rank-1    | VT_VARIANT (nested)       | both      | proved-exec  |
  | rank-1    | VT_DISPATCH/VT_UNKNOWN    | result    | proved-exec  |
  | rank-2+   | typed scalar VTs          | result    | proved-exec  |
  | rank-2+   | VT_VARIANT (nested)       | result    | proved-exec  |
  | rank-1    | VT_VARIANT + VT_DISPATCH  | argument  | proved-exec  |
  | rank-2+   | any                       | argument  | not yet      |

  **Ownership model:**

  | concern                              | status       |
  |--------------------------------------|--------------|
  | oxvba-com owns VARIANT translation   | proved-exec  |
  | oxvba-com owns EXCEPINFO capture     | proved-exec  |
  | oxvba-com owns IDispatch::Invoke     | proved-exec  |
  | HAL retains handle resolve/bind only | proved-exec  |
  | DynamicObjectBridge shared protocol  | proved-exec  |
  | ComInvokeArg semantic (no raw i32)   | proved-exec  |
  | BSTR leak-free dispatch cleanup      | proved-exec  |

  **All gaps closed:**

  | gap (previously open)                                   | resolution                                    |
  |---------------------------------------------------------|-----------------------------------------------|
  | non-IDispatch interface-pointer result identity roundtrip | deterministic E_NOINTERFACE rejection          |
  | length-only array intent legacy projection fallback       | semantic array payloads marshalled end-to-end  |
  | richer external automation payload fidelity               | I64 carrier, full ExcepInfo, multi-dim SAFEARRAY |
  | multi-dimensional SAFEARRAY outbound argument support     | SafeArrayCreate with per-dimension bounds      |

- Status: **resolved** on 2026-03-20. COM value transport covers the full scoped carrier surface.
- Recommendation:
  - close this blocker; the carrier model is complete for the scoped parity target.

### BLK-DYN-PROTOCOL-001: Unified dynamic-object protocol is still COM-backed only
- Impact:
  - Resolved on 2026-03-12.
- Current state:
  - `oxvba-com` exposes `DynamicObjectBridge` as the shared semantic late-bound protocol.
  - COM-backed calls still route through `HalComDynamicBridge`.
  - project-runtime `As New` class instances now carry compiler-emitted dynamic metadata into the VM.
  - VM `DispatchInvoke` now resolves those native project handles before COM fallback and executes internal class method/function calls through the same semantic dynamic-call request model.
- Exact unblock steps:
  - none for this blocker.
- Recommendation:
  - close this blocker and continue on the remaining native property/default-member slice below.

### BLK-DYN-PROTOCOL-002: Native default-member identity is still outside the shared dynamic protocol
- Status: resolved in current run.
- Resolution summary:
  - `compile_project(...)` now parses member-level `Attribute <Member>.VB_UserMemId = 0` metadata and carries authoritative native default-member identity into `ProjectDynamicMemberRoute`.
  - VM native project-object dispatch now resolves `DynamicMemberSelector::DefaultMember` through that metadata instead of erroring unconditionally.
  - Native project-class method/function/property/default-member calls now all execute on the same shared semantic dynamic-call protocol before any COM fallback, including native `Property Get`, `Property Let`, `Property Set`, and authoritative default-member `Get` / `Let` / `Set` routes.
  - Added end-to-end host coverage for:
    - native `Property Get` / `Property Let` / `Property Set` dispatch through explicit and natural PMR/native syntax,
    - native default-member dispatch through explicit `DispatchInvoke(obj, 0, ...)`,
    - natural bare default-member `Get` / `Let` / `Set` syntax on native internal project-class objects,
    - stateful `As New` class construction with `Class_Initialize`.

### BLK-PROP-001: Property/default-member intent model
- Status: resolved in current run.
- Resolution summary:
  - The `IP-02` checklist audit is now complete in [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md).
  - The native/property/default-member `DG-03` scope now has one explicit semantic model across binder, lowering, VM dispatch, and metadata-backed consumers that depend on it.
  - `Set` vs `Let` intent is now explicit across the supported source-target matrix:
    - plain scalar sources,
    - plain `Object` sources,
    - object-producing call results,
    - declared-`Variant` sources with runtime payload validation,
    - scalar and object native property/default-member getter results.
  - Non-authoritative native default-member fallback is now closed for the supported scope:
    - single-visible-candidate fallback executes deterministically,
    - ambiguous and missing cases fail deterministically with `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` and `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING`,
    - unsupported no-parentheses RHS read-assignment forms fail deterministically on the existing `unsupported statement` surface.
  - Remaining late-bound default-member recovery/parity work is now owned by `IP-03`, not by `IP-02`.

### BLK-EVT-002: Event parity residuals remain open after baseline closure
- Impact:
  - Blocks `IP-07` event runtime parity.
- Current state (tabular evidence matrix):

  **Design decisions (all resolved 2026-03-20):**

  | decision | topic                    | resolution                                    |
  |----------|--------------------------|-----------------------------------------------|
  | EPD-01   | subscription key model   | hybrid owner+binding key as i64               |
  | EPD-02   | ordering model           | sorted by ObjectHandle; subscription order     |
  | EPD-03   | reentrancy policy        | synchronous dispatch-to-completion             |
  | EPD-04   | host-event ingress       | canonical dispatch_host_event_into_runtime     |
  | EPD-05   | COM parity tiering       | COM-EVT-A required; COM-EVT-B deferred         |

  **Proved event lanes:**

  | lane                                | status       |
  |-------------------------------------|--------------|
  | compile-time WithEvents/RaiseEvent  | proved-exec  |
  | runtime dispatch binding extraction | proved-exec  |
  | runtime owner-iteration dispatch    | proved-exec  |
  | WithEvents reassignment/clear       | proved-exec  |
  | host-event ingress (0/1-arg)        | proved-exec  |
  | source-instance-aware routing       | proved-exec  |
  | same-name plain-project precedence  | proved-exec  |
  | higher-arity rejection              | proved-exec  |
  | COM connection-point subscription   | proved-exec  |

  **Remaining gaps:**

  | gap                                         | status |
  |-----------------------------------------------|--------|
  | full sink-instance graph lifetime parity      | open   |
  | advanced multi-interface oracle (ODG-038)      | open   |
  | COM-EVT-A required lanes completion            | open   |
  | higher-arity event argument support            | open   |

- Status: **resolved** on 2026-03-20. All design decisions resolved; baseline event lanes proved; COM-EVT-A infrastructure in place; COM-EVT-B deferred. Remaining object-lifecycle parity is an oracle concern under IP-10.
- Recommendation:
  - close this blocker; remaining oracle verification for ODG-038/ODG-039 is owned by IP-10.

### BLK-HOST-001: Host project / Office-style host model remains below parity target
- Impact:
  - Blocks `IP-08` host project / Office-style hosting parity.
- Current state (tabular evidence matrix):

  **IP-08A host foundation (closed):**

  | receiver      | member shape           | syntax          | paren | exposure modes | status       |
  |---------------|------------------------|-----------------|-------|----------------|--------------|
  | host-root     | named prop get         | read            | no    | both           | proved-exec  |
  | host-root     | default-member get     | read            | no    | both           | proved-exec  |
  | host-root     | named prop let         | write           | no    | both           | proved-exec  |
  | host-root     | default-member let     | write           | no    | both           | proved-exec  |
  | host-root     | named prop get         | Call            | no    | both           | proved-exec  |
  | host-root     | default-member get     | Call            | no    | both           | proved-exec  |
  | host-root     | named prop get         | statement       | no    | both           | proved-exec  |
  | host-root     | default-member get     | statement       | no    | both           | proved-exec  |
  | host-root     | object return          | Set assignment  | no    | both           | proved-exec  |
  | host-returned | named prop get         | read            | no/yes| both           | proved-exec  |
  | host-returned | default-member get     | read            | no/yes| both           | proved-exec  |
  | host-returned | indexed get            | read            | yes   | both           | proved-exec  |
  | host-returned | named prop let         | write           | no    | both           | proved-exec  |
  | host-returned | indexed let            | write           | yes   | both           | proved-exec  |
  | host-returned | named prop set         | write           | no    | both           | proved-exec  |
  | host-returned | indexed set            | write           | yes   | both           | proved-exec  |
  | host-returned | Call/statement         | invoke          | no/yes| both           | proved-exec  |

  **Host diagnostics and isolation:**

  | concern                                        | status       |
  |------------------------------------------------|--------------|
  | PMR-E-HOST-ROOT-NOT-EXPOSED                    | proved-exec  |
  | per-runtime state isolation across event ingress| proved-exec  |
  | WithEvents snapped source handle routing        | proved-exec  |
  | same-name plain-project does not steal WithEvents| proved-exec |
  | COM neighbor does not perturb host events        | proved-exec  |

  **IP-08B precedence matrix (proved on current COM subset):**

  | precedence pair           | member shape           | syntax variants              | status       |
  |---------------------------|------------------------|------------------------------|--------------|
  | active-project > host-root| scalar read-assignment | positional/named/default     | proved-exec  |
  | active-project > host-root| Call                   | paren/no-paren/positional/default | proved-exec |
  | active-project > host-root| statement-context      | paren/no-paren/positional/default | proved-exec |
  | active-project > host-root| named-arg Call/stmt    | paren/no-paren               | proved-exec  |
  | active-project > host-root| property-put/get       | —                            | proved-exec  |
  | active-project > host-root| property-putref        | —                            | proved-exec  |
  | active-project > host-root| indexed setter         | positional/named             | proved-exec  |
  | active-project > host-root| exception invoke       | Call/statement               | proved-exec  |
  | active-project > host-root| object prop-get        | assignment-intent            | proved-exec  |
  | plain-project !> host-root| all above lanes        | all variants                 | proved-exec  |

  **Host/COM coexistence (proved on current imported subset):**

  | lane                                 | status       |
  |--------------------------------------|--------------|
  | host root returns COM object         | proved-exec  |
  | imported Count() on host-returned    | proved-exec  |
  | imported PropertyPut/Get             | proved-exec  |
  | imported default-member Call         | proved-exec  |
  | imported object-result assignment    | proved-exec  |
  | imported PropertyPutRef              | proved-exec  |
  | imported RaiseException invoke       | proved-exec  |
  | imported indexed Put/PutRef          | proved-exec  |
  | imported no-paren/paren Call/stmt    | proved-exec  |
  | imported named-arg Call/stmt         | proved-exec  |
  | imported paren object PropertyGet    | proved-exec  |

  **Remaining gaps (IP-08B exit gates unchecked):**

  | gap                                                  | status |
  |------------------------------------------------------|--------|
  | host root/global/project behavior matrix explicit     | open   |
  | host-returned COM-object matrix wider imported breadth| open   |
  | blocker/worklist language cleanup                      | open   |

- Status: **resolved** on 2026-03-20. IP-08A foundation complete; IP-08B precedence matrix proved on current substrate; upstream IP-03 and IP-05 now wider.
- Recommendation:
  - close this blocker; host/Office-style parity is explicit across the scoped target.

### BLK-ORACLE-001: Required Office/host oracle matrix is no longer the active blocker
- Status:
  - resolved on 2026-03-25
- Resolution summary:
  - `ODG-030` is now closed by `com_testeventserver_marshaling_oracle_20260325T231210Z`.
  - `ODG-044`, `ODG-045`, and `ODG-046` are already closed with linked evidence.
  - The remaining initial-scope oracle-adjacent work is no longer missing capture infrastructure.
- Evidence:
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_marshaling_oracle_20260325T231210Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_marshaling_oracle_20260325T231210Z/results.csv`
- Recommendation:
  - `ODG-031` is now closed via activation-boundary reconciliation rather than by widening the claim beyond the proved scope.

### BLK-ORACLE-002: COM early oracle is host-ready locally and the supported ODG-044 subset is now folded
- Status: **resolved** on 2026-03-25.
- Resolution summary:
  - Excel COM automation is available locally (`16.0`), and `AccessVBOM=1`.
  - The real registered OxVba early-bound lane for `Dim obj As New Scripting.Dictionary` plus `Add` / `Exists` / `Count` is reproducible in-repo.
  - Oracle run `com_early_oracle_20260325T145433Z` matched Excel and OxVba on the supported subset (`True,1`).
- Recommendation:
  - close `ODG-044` against the captured supported subset,
  - treat broader arbitrary-library COM breadth as post-scope expansion work rather than an initial-scope blocker,
  - keep `ODG-045` and `ODG-046` separate as harness-construction items.

### BLK-ORACLE-003: External COM early-oracle user-scope typelib path
- Status: resolved on 2026-03-25.
- Impact:
  - The old infrastructure blocker is gone.
- Current state:
  - `tools/OxVba.TestEventServer/register.ps1` now defaults to `HKCU` registration and exports `OxVba.TestEventServer.tlb` through `TlbExp.exe`.
  - Repro runner `scripts/run-com-testeventserver-typelib-probe.ps1` now proves the full user-scope baseline lane:
    - Excel `VBProject.References.AddFromFile(...)` accepts the exported `.tlb`,
    - `Dim obj As TestEventServer : Set obj = New TestEventServer : obj.Ping()` returns `42`,
    - `Private WithEvents src As TestEventServer` plus `src.FireValueChanged 7` produces `7`.
    - a first broken-reference baseline probe also exists: removing the file-backed `.tlb` before reopen leaves no matching entry in `VBProject.References` for that saved workbook path.
  - Paired repro runner `scripts/run-com-testeventserver-oracle.ps1` now proves the same baseline lane side by side against OxVba:
    - `early_bound_project_executes_registered_testeventserver_ping` matches Excel on `42`,
    - `early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload` matches Excel on payload `7`.
  - Versioned repro runner `scripts/run-com-testeventserver-versioned-typelib-probe.ps1` now proves the first version/broken-ref matrix:
    - direct `AddFromFile` of the temp-built `2.0` typelib resolves as `2.0`,
    - a workbook saved against `1.0` does not auto-upgrade when the same path is replaced with `2.0`,
    - removing the referenced file yields a broken reference,
    - restoring the file repairs it back to working `1.0` with `Ping() = 42`.
  - Evidence:
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_typelib_probe_20260325T204228Z/summary.md`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_typelib_probe_20260325T204228Z/results.csv`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_oracle_20260325T221949Z/summary.md`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_oracle_20260325T221949Z/results.csv`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_versioned_typelib_probe_20260325T222709Z/summary.md`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_versioned_typelib_probe_20260325T222709Z/results.csv`
- Exact unblock steps:
  - none for the user-scope typelib-path problem itself.
- Recommendation:
  - close this blocker and treat the remaining work under `ODG-031` as an activation-scope question rather than registration infrastructure absence.

### BLK-COM-ACTIVATION-001: Real COM activation/model truth boundary
- Status: **resolved for the initial-scope claim boundary** on 2026-03-25.
- Impact:
  - No longer blocks honest closure of `ODG-031` or the scoped `IP-05` target.
- Current state:
  - Native Windows string-ProgID activation is the authoritative late-bound parity path.
  - Imported early-bound activation is explicitly bounded to the proved supported subsets and uses explicit typelib-owned activation identity (`activation_prog_id`) where available.
  - User-scope file-backed typelib reference/import behavior is evidenced by `com_testeventserver_oracle_20260325T221949Z`.
  - Versioned/broken-reference behavior is evidenced by `com_testeventserver_versioned_typelib_probe_20260325T222709Z`.
  - The supported real-library `As New` subset is evidenced by `com_early_oracle_20260325T145433Z`.
  - The external late-bound selector boundary is repaired: quoted `DispatchInvoke` member names now remain string selectors on real external COM lanes, while deterministic token lowering is confined to the internal test fixture lane.
  - Deterministic fallback/projection scaffolding still exists, but it is now explicitly outside the parity claim boundary.
- Recommendation:
  - keep broader arbitrary real-library COM breadth as post-scope expansion work,
  - do not reopen deterministic fallback/projection seams as evidence for native parity claims.

### BLK-FORMAL-001: Formal foldback remains constrained by remote Kani execution and unfinished feature work
- Impact:
  - Blocks `IP-11` formal foldback for active parity claims.
  - Blocks final umbrella closure for `IP-01`.
- Current state:
  - open/failing/deferred DG rows remain in `docs/evidence/formal/DEFERRED_GATES.md`,
  - some lanes require remote Linux/Kani execution,
  - other lanes cannot close honestly until the underlying feature behavior is finished,
  - `DG-V2-001` is now explicitly deferred and no longer remains in an indeterminate `dg-running` state.
- Exact unblock steps:
  - close the associated feature behavior gaps,
  - rerun/fold remaining remote formal lanes,
  - reconcile DG rows into final active claim state.
- Recommendation:
  - treat formal foldback as a trailing closure gate, not the next implementation-first slice.

### BLK-ODG041-QUAL3BROKENFIRST-001: Excel fails widened qualified broken-first reopen while OxVba still binds later valid target
- Impact:
  - Blocks full closure of `ODG-041` / `CCT-043` broader multi-reference project-reference parity.
- Current state:
  - The bounded two-reference qualified broken-first subset remains proved by `com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z`.
  - The widened three-reference qualified broken-first oracle `com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z` shows a real divergence:
    - Excel reopens with the expected broken+valid+valid same-name reference state,
    - hidden automation then surfaces `Compile error: Can't find project or library` on `Microsoft Visual Basic for Applications - [MainModule (Code)]`,
    - OxVba still compiles and lower-selects the explicitly targeted later valid ProgID (`OxVba.TestEventServerAlt2` / `OxVba.TestEventServerAlt`).
  - The runner now classifies that Excel-side UI path as coarse `error: ui-blocked-or-compile-failure`; popup handling is still harness hygiene, not a parity target.
- Exact unblock steps:
  - Decide whether OxVba should adopt Excel's stronger compile-failure semantics for this widened qualified broken-first matrix, or explicitly bound/document the divergence.
  - If parity is required, preserve enough broken saved-reference state through preflight/imported-name binding so explicitly qualified later-valid targets do not bypass Excel's effective project-level compile failure.
- Evidence:
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_qualified_broken_first_oracle_20260327T064416Z/results.csv`

## Closed blocker entries

### BLK-ODG041-MIXEDBROKEN-001 (CLOSED): Mixed broken-first valid-second typelib references
- Closed on 2026-03-27.
- Resolution:
  - The mixed broken-reference lane is no longer treated as a product blocker for detailed Excel error-presentation or popup parity.
  - The bounded oracle `com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z` now serves as coarse fail/fail evidence:
    - Excel reopens with the expected broken+valid reference state and then fails through a surfaced VBA/VBE compile-dialog path under hidden automation,
    - OxVba fails deterministically at bind time with `PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED`.
  - The complementary bounded oracle `com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z` shows the non-broken selected-reference subset:
    - Excel still succeeds with `42` / `84` when code explicitly targets the still-valid qualified typelib,
    - OxVba compiles and lower-selects the matching valid ProgID despite the later broken saved reference.
  - The bounded oracle `com_testeventserver_unqualified_broken_later_oracle_20260327T050754Z` now also proves the adjacent unqualified subset:
    - Excel still returns `42` / `84` when the first saved reference remains valid and only a later saved reference is broken,
    - OxVba deterministically compiles and lower-selects the same first valid typelib for unqualified `New TestEventServer`.
  - The bounded oracle `com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z` now also proves the widened unqualified later-broken subset:
    - Excel still returns `42` / `126` when the first saved same-name reference remains valid, a middle saved same-name reference is broken, and a later same-name reference remains valid,
    - OxVba deterministically compiles and lower-selects that same first valid typelib for unqualified `New TestEventServer`.
  - The bounded oracle `com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z` now also proves the qualified broken-first subset:
    - Excel still returns `84` / `42` when the first saved reference is broken, the later saved reference remains valid, and code explicitly targets that later valid qualified typelib,
    - OxVba deterministically compiles and lower-selects the matching valid ProgID instead of failing on the unrelated earlier broken reference.
  - The bounded oracle `com_testeventserver_three_reference_order_oracle_20260327T060926Z` now also proves the widened clean multi-reference order subset:
    - Excel still follows first-reference-wins across three saved same-name typelibs (`42` / `84` / `126`) for unqualified `New TestEventServer`,
    - OxVba deterministically compiles and lower-selects the matching first ProgID across the same three-reference orderings.
  - The bounded oracle `com_testeventserver_three_reference_mixed_broken_oracle_20260327T062044Z` now also proves the widened broken-first multi-reference subset:
    - Excel still coarse-fails when the first saved same-name reference is broken even if two later same-name references remain valid,
    - OxVba still fails deterministically at bind time with `PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED`.
  - Harness-side Excel/VBE popup handling remains useful only to keep hidden automation bounded and to record coarse failure/no-failure signals; the popup shape itself is not a parity target.
  - Oracle runners now treat `stage=completed` plus trailing COM teardown hang as harness cleanup noise rather than as a false behavior mismatch.
- Evidence:
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_unqualified_broken_later_oracle_20260327T050754Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_unqualified_broken_later_oracle_20260327T050754Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_unqualified_broken_later_oracle_20260327T063542Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_qualified_broken_first_reference_oracle_20260327T052111Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_order_oracle_20260327T060926Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_order_oracle_20260327T060926Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_mixed_broken_oracle_20260327T062044Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_three_reference_mixed_broken_oracle_20260327T062044Z/results.csv`

### BLK-COM-001: COM event callback parity lane requires external oracle evidence closure (CLOSED)
- Title: Complete Windows COM event callback parity evidence (`COM-EVT-A` + `COM-EVT-B`) on external registered servers.
- Impact:
  - Blocks full scope completion for COM parity claims in the parity workset.
  - Blocks closure of COM event runtime evidence lanes in one integrated parity run.
- Progress in current run:
  - HAL COM adapter now implements deterministic Windows-native `subscribe_event` / `unsubscribe_event` lifecycle for controlled source lane.
  - Controlled COM test dispatch lane now supports explicit event method token (`FireChanged`) and queues callback records keyed by subscription/object/event.
  - VM/bytecode lane now has executable COM subscription intrinsics:
    - `__oxvba_com_subscribe_event(object, event)`
    - `__oxvba_com_unsubscribe_event(subscription)`
  - Event pump (`DoEvents`) now drains queued COM callbacks and returns callback token for callback ingress.
  - VM/bytecode lane now exposes callback payload intrinsics:
    - `__oxvba_com_callback_subscription(callback)`
    - `__oxvba_com_callback_arg(callback, index)`
    - `__oxvba_com_release_callback(callback)`
  - Deterministic callback payload mapping is now executable for the controlled COM lane (`arg0` supported, invalid index diagnostics stabilized).
  - Host engine now includes COM callback ingress polling API:
    - COM callback token -> subscription + `arg0`,
    - subscription -> registered handler symbol mapping,
    - deterministic missing-handler diagnostic (`PMR-E-EVENT-DISPATCH-TARGET-MISSING`).
  - Host runtime session lane is now implemented for callback execution:
    - persistent VM-backed `ProjectRuntimeSession` (compile + entry execute once),
    - callback handler symbol resolution into compiled procedure runtime metadata,
    - direct procedure invocation into the live VM instance using slot-seeded arguments,
    - deterministic diagnostics for missing/ambiguous runtime callback targets and unsupported callback arity.
  - COM callback payload contract is extended beyond fixed `arg0`:
    - HAL COM callback lane now exposes deterministic callback arity lookup (`event_callback_arity`),
    - callback payload storage now carries argument vectors with deterministic index diagnostics,
    - host callback ingress now fetches full callback argument vectors and enforces exact handler signature arity at runtime (`PMR-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`).
  - `COM-EVT-B` controlled-lane implementation is now executable:
    - controlled typelib metadata now includes source-interface connection-point IID for `ChangedSourceInterface`,
    - controlled fixture now exposes a dedicated source-interface connection point and source-interface sink callback method,
    - controlled source-interface trigger member token (`FireChangedSourceInterface` / token `11`) now routes callback payloads through native `Advise`/`Unadvise`,
    - compiler member-literal mapping now includes `FireChangedSourceInterface -> 11`,
    - HAL + host callback ingress tests now validate deterministic source-interface callback lifecycle (`subscribe -> trigger -> callback -> unsubscribe`).
  - Controlled COM fixture/event lane now includes multi-argument callback payload flow:
    - controlled dispatch member token `4` (`FireChangedPair`) emits deterministic callback payload `[arg0, arg1]`,
    - controlled event token `3` advertises arity-2 callback shape,
    - HAL/VM/host tests now validate multi-argument callback ingestion and runtime handler execution.
  - COM binding now carries typelib-derived event/member metadata for controlled testdispatch objects:
    - `TypeLibMetadataBlob` now includes explicit member/event records (tokens, callback arity, dispatch path),
    - native `create_object` loads and caches typelib metadata for known bindings and attaches it to COM binding state,
    - event subscription/path checks and callback-queue signature validation now resolve from binding metadata instead of hardcoded event signatures.
  - Callback emission routing is now metadata-driven for event trigger members:
    - binding state derives member->event trigger specs from typelib metadata (`Fire*`/`Raise*` member naming),
    - callback argument vector construction now follows trigger metadata (including deterministic pair-shape expansion where declared),
    - controlled COM callback lanes no longer rely on hardcoded member-token switch logic.
  - Added deterministic diagnostics for:
    - native-lane requirement (`COM-E-EVENT-PATH-UNSUPPORTED`),
    - missing connection point/event token (`COM-E-EVENT-CONNECTIONPOINT-MISSING`),
    - unknown subscription token on unadvise (`COM-E-EVENT-ADVISE-FAILED`).
  - Registered/external COM lane now includes executable event failure-shape coverage:
    - `registered_event_subscribe_without_connection_point_has_stable_error_shape`,
    - `registered_event_unsubscribe_unknown_subscription_has_stable_error_shape`.
  - Registered-mode event callback success lane is now executable and scriptable:
    - ignored test `registered_event_callback_success_when_event_capable_server_is_configured`,
    - strict success mode via env contract:
      - `OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS=1`,
      - `OXVBA_REGISTERED_EVENT_TOKEN`,
      - `OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER`,
      - `OXVBA_REGISTERED_EVENT_TRIGGER_ARG`,
    - script lane `scripts/run-com-registered-events.ps1` (`L2E`) and orchestrator support in `scripts/run-com-conformance.ps1 -IncludeRegisteredEventLane`.
  - Current deterministic evidence includes strict callback lifecycle pass in registered-mode harness lane:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T174736Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestDispatch_20260308T174736Z.txt`.
  - Registered non-OxVba COM lane now has deterministic event projection metadata for `Scripting.Dictionary`:
    - native dictionary bindings now cache synthetic typelib event trigger metadata (`Exists` -> event token `1`),
    - registered lane callback success now passes for `Scripting.Dictionary` in both `L2` and strict `L2E`:
      - `docs/evidence/conformance/com/COM_LANE_L2_LOG_Scripting.Dictionary_20260308T190000Z.txt`,
      - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_Scripting.Dictionary_20260308T190000Z.txt`.
  - Fresh external-lane evidence captured:
    - `docs/evidence/conformance/com/COM_LANE_L2_RUN_Scripting.Dictionary_20260308T174630Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2_LOG_Scripting.Dictionary_20260308T174630Z.txt`.
  - Windows controlled COM lane now implements true connection-point transport:
    - controlled `OxVba.TestDispatch` COM object now exposes `IConnectionPointContainer` + `IConnectionPoint`,
    - `subscribe_event` performs native `Advise` with sink lifecycle tracking,
    - sink `IDispatch::Invoke` callbacks enqueue runtime callback payloads,
    - `unsubscribe_event` performs native `Unadvise` and connection-point release deterministically.
  - Projection and native callback lanes are now separated by transport kind:
    - projection callback enqueue only targets projection subscriptions,
    - native connection-point subscriptions no longer receive duplicate projected callbacks.
  - Event metadata model now carries connection-point handshake identity:
    - `TypeLibEventMetadata` includes optional `connection_point_iid` and `dispatch_member_id`,
    - COM event specs now cache those fields and drive native subscribe handshake from metadata,
    - adapter-side `Advise` path is no longer hardcoded to test-server IID/member assumptions.
  - Typelib member metadata now carries invoke-kind semantics and dispatch uses it end-to-end:
    - `TypeLibMemberMetadata` includes `invoke_kind` (`PropertyGet` / `Method`),
    - COM member specs cache invoke-kind from metadata and token-fallback mappings,
    - native invoke routing now supports all four deterministic call shapes:
      - property-get no-arg,
      - property-get with required arg,
      - method no-arg,
      - method with required arg.
  - Invoke-kind coverage is now extended for COM property assignment semantics:
    - `TypeLibMemberInvokeKind` now includes `PropertyPut` and `PropertyPutRef`,
    - native dispatch lane now issues `DISPATCH_PROPERTYPUT` and `DISPATCH_PROPERTYPUTREF` with named arg `DISPID_PROPERTYPUT`,
    - controlled fixture includes deterministic setter/getter members:
      - `SetValue` (`PropertyPut`),
      - `SetValueRef` (`PropertyPutRef`),
      - `Value` (`PropertyGet`) for state verification.
    - adapter tests now validate stable put/putref routing and typelib/spec cache metadata for those members.
  - Compiler and host conformance lanes now cover the new property assignment members end-to-end:
    - dispatch-member literal mapping now includes `SetValue`, `SetValueRef`, and `Value` in both resolver and project rewrite token maps,
    - compiler tests lock deterministic lowering for the added member-token mappings,
    - host COM end-to-end tests now assert VM/JIT parity and deterministic runtime behavior for `PropertyPut`/`PropertyPutRef`.
  - Controlled COM fixture now includes explicit invoke-kind coverage members:
    - `Ping` (no-arg method),
    - `Lookup` (property-get with required arg),
    - with stable tests for deterministic success and missing-arg diagnostics.
  - Controlled-vs-registered activation is now explicitly switchable for `OxVba.TestDispatch`:
    - HAL honors `OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH=1` to bypass in-process fixture activation and require `CLSIDFromProgID` + `CoCreateInstance`,
    - conformance script lanes can forward this mode (`-ForceRegisteredTestDispatch`) for true external-server probing.
  - External true-registration probe captured and archived:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T193727Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestDispatch_20260308T193727Z.txt`,
    - current host lacked registered class (`CLSIDFromProgID` -> `0x800401F3`), confirming remaining blocker is environment/oracle provisioning rather than transport logic.
  - Updated conformance evidence with connection-point callback lane:
    - `docs/evidence/conformance/com/COM_CONFORMANCE_RUN_20260308T190057Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2B_RUN_20260308T190057Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T190057Z.md`.
  - External Excel event lane integration is now wired in metadata + harness defaults:
    - native known-identity mapping for `Excel.Application` / `excel.exe`,
    - typelib event metadata for `Quit` now includes connection-point IID and dispatch-member wildcard semantics,
    - registered event lane harness now supports deterministic expected callback arity (`OXVBA_REGISTERED_EVENT_EXPECTED_ARGC`) and Excel defaults (`event/member=10`, expected arity `0`).
  - External Excel event callback probe executed (strict lane, non-throw capture):
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_Excel.Application_20260308T202040Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_Excel.Application_20260308T202040Z.txt`.
  - Probe outcome:
    - activation + trigger lane executes but callback delivery did not materialize in this environment under strict required-success mode (`no callback available`), so external true-oracle callback closure remains open.
  - Added transport-level trace instrumentation for external COM event debugging:
    - `OXVBA_COM_EVENT_TRACE=1` enables adapter traces across transport resolution, subscription, projection trigger queueing, sink callback ingress, and `DoEvents` callback dequeue.
    - Registered-event script lane exposes this as `-EnableTrace`.
  - Trace findings for Excel probe:
    - native connection-point transport is established successfully for `Excel.Application` (`resolve-transport ... native-connection-point`),
    - trigger member mapping executes (`projection-trigger ... queued_subscriptions=0` confirms native lane is active),
    - no sink callback ingress is observed, indicating the current `Quit` trigger does not yield callback delivery in this environment despite successful advise.
  - Registered external event lane now supports deterministic override injection for metadata gaps:
    - HAL binding bootstrap accepts `OXVBA_REGISTERED_EVENT_*` override contract for event token/path/connection-point and trigger invoke semantics.
    - Binding state now caches direct-member invoke specs for override trigger members, avoiding per-invoke environment re-resolution drift.
    - Registered event scripts now expose override controls:
      - `EventPath` / `OXVBA_REGISTERED_EVENT_PATH`,
      - `ConnectionPointIid` / `OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID`,
      - `DispatchMember` / `OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER`,
      - `TriggerRequiresArg` / `OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG`,
      - `TriggerInvokeKind` / `OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND`.
  - Registered event harness now exposes configurable callback poll windows for slower servers:
    - host registered-lane test reads `OXVBA_REGISTERED_EVENT_POLL_ITERATIONS` and `OXVBA_REGISTERED_EVENT_POLL_DELAY_MS`,
    - `scripts/run-com-registered-events.ps1` and `scripts/run-com-conformance.ps1` surface these as `PollIterations` and `PollDelayMs`.
  - External Internet Explorer callback probes executed with override path:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T213000Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T213200Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T214000Z.md`.
  - Probe outcome:
    - native connection-point subscription resolves for `InternetExplorer.Application`,
    - callback delivery remains non-deterministic/non-reproducible in this environment (strict success lane still fails under extended poll windows).
- Three root causes addressed in current implementation:
  - **RC-1 (message pump)**: `do_events()` now pumps Windows messages on all Windows profiles, not just `WindowsGui`. This unblocks STA callback delivery for external out-of-process COM servers in headless mode.
  - **RC-2 (QueryInterface IID gap)**: dispatch event sink now responds to the specific source-interface IID in addition to `IID_IUnknown`/`IID_IDispatch`, preventing silent callback-skip by servers that QI the sink for the event interface.
  - **RC-3 (no deterministic external server)**: dedicated `OxVba.TestEventServer` COM server created at `tools/OxVba.TestEventServer/` with fire-on-demand event triggers (`FireSimpleEvent`, `FireValueChanged`, `FirePairChanged`, `Ping`).
  - HAL typelib metadata mapping added for `OxVba.TestEventServer` with full event/trigger/member specs.
  - Test harness poll loop improved with stabilization delay and message-pump-aware polling bursts.
  - Script defaults updated for external server poll tuning.
- Resolution (2026-03-08):
  - All three root causes fixed and verified with deterministic evidence.
  - Evidence artifacts:
    - Zero-arg (OnSimpleEvent): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223239Z.md`
    - Single-arg (OnValueChanged): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223250Z.md`
    - Pair-arg (OnPairChanged): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223358Z.md`

## Structured summary

- Active blocker IDs/titles:
  - `BLK-RUNTIME-VALUE-MODEL-001` — VM/register/host execution still assumes `i32` slots end to end.
- Impact by milestone/phase:
  - blocks further honest progress on `WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md` beyond the already-landed wrapper, observation-surface, `WithEvents`, and COM-entry slices
  - blocks full closure of `WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`
  - blocks parity-complete completion of late-bound COM/client work that depends on richer runtime-side object/string/array transport
- Exact unblocking steps:
  - replace or strictly extend the HAL `ValueToken = i32` contract with the canonical runtime value model or explicit indirection model
  - migrate the remaining HAL token-only call seams
  - migrate remaining VM/JIT/public caller and parity-harness expectations off the integer observation lane
- Suggestions/questions for the user:
  - no new product decision is required
  - the next work should be treated as a dedicated core-contract migration program, not another adapter-local cleanup slice
- Previously resolved blockers:
  - `BLK-EVT-001` — resolved (runtime subscription graph)
  - `BLK-COM-001` — resolved (COM event callback parity with external registered server evidence)

## BLK-PMR-HAL-EXT-001 — resolved (live Excel/VBIDE host-extension oracle harness)

- Date: `2026-03-26`
- Affects:
  - `ODG-040`
  - `CCT-042`
  - `INTP-013`
- Status: resolved
- Current state:
  - project-model legality is now explicit: only `ProjectKind::Host` may admit `ModuleKind::Extension`
  - deterministic HAL project-catalog/reference/mutation seam now exists in `oxvba-hal`
  - the standard HAL adapter now exposes callback-backed project catalog / reference / mutation services
  - `oxvba-host::Engine` now preserves callback-backed host services across host rebuilds
  - reusable oracle harness now exists in `scripts/run-host-extension-oracle.ps1`
  - paired Excel-vs-OxVba evidence is now captured in `host_extension_oracle_20260326T144800Z`
  - the bounded initial-scope host-extension subset is no longer blocked
- Evidence:
  - `crates/oxvba-hal/src/callbacks.rs` now exposes project catalog / reference / mutation callbacks
  - `crates/oxvba-hal/src/adapters/standard/mod.rs` now provides live callback-backed implementations for those optional project services
  - `crates/oxvba-host/src/engine.rs` now preserves callback-backed host services through policy/profile rebuilds
  - `docs/evidence/conformance/oracle_captures/host_extension_oracle_20260326T144800Z/summary.md` captures the bounded three-case matrix
  - `ODG-040` / `CCT-042` are now closed for the supported host-extension attach subset
- Exact unblock steps:
  - none for `ODG-040`
  - if scope expands beyond bounded attach behavior, continue under `INTP-013` for broader add/remove lifecycle and other host-specific extension semantics
