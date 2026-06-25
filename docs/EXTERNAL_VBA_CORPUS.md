# External VBA Corpus — Test Summary & Findings

Companion to [`INTERESTING_VBA_PROJECTS.md`](INTERESTING_VBA_PROJECTS.md). That file
is the *watchlist*; this file is the *running summary* of what we actually exercised
against OxVBA and what we learned.

## How this corpus is stored

Web-sourced sample code is **deliberately not committed**. The samples and any
adapted `.basproj` projects live in a gitignored working area
(`.external/vba-corpus/`, see `.gitignore`); **only this summary and the findings
below are committed.** This keeps the clean-room boundary: we observe and describe
external projects, we don't redistribute their code in the OxVBA tree.

Clean-room rule (inherited from the watchlist): use these projects only through
public docs, high-level source review, and reproducible black-box behavior. Inclusion
here is not a compatibility claim.

## Status legend

`watchlisted` → noted only · `gathered` → source pulled into the local area ·
`building` → `.basproj` authored · `running` → executes under the VM ·
`characterized` → behavior/limitations documented below.

## Corpus

| Project | Reference | What we're checking | Status |
| --- | --- | --- | --- |
| Riff | https://github.com/uesleibros/riff | `Declare`/Win32 interop, COM vtable dispatch, pointer-heavy VBA, conditional 32/64-bit compilation, UDTs/arrays, machine-code callback thunks, host-reset safety | running/initial characterization |
| Wasabi | https://github.com/uesleibros/wasabi | Win32 networking `Declare`s, byte-array/string transport, async callback/event patterns, handler-class lifetimes, `DoEvents` host behavior, TLS/proxy surfaces | gathered |
| VBA-Web | https://github.com/VBA-tools/VBA-Web | Real-world VBA web-service library: `WinHttpRequest`, `InternetExplorer.Application`, `Scripting.Dictionary`, `MSXML2.DOMDocument`, `System.Security.Cryptography`, `WithEvents`, `Implements`, Win32/macOS `Declare`s, conditional compilation, JSON/URL/XML helpers, optional args, arrays, collections, class modules | running/characterized |
| Awesome VBA | https://github.com/sancarn/awesome-vba | Index for sourcing further candidates (JSON/CSV/XML, data structures, parsers, Win32, add-ins, …) | watchlisted |

## Method note (how to read sweep failures)

A first pass compiled each module **in isolation** (`oxvba-cli compile <one-file>`).
Two failure classes from that are test-method artifacts, not engine defects, and are
verified as such:

- **Cross-module calls** (`call to unknown procedure: wasabi_tcpconnect`, etc.): OxVBA is
  a whole-program compiler; a lone module calling a sibling module's `Public` proc has
  nothing to resolve against. A 2-module project resolves it (`Helper.Add(2,3)` → `5`). ✓
- **`.cls VERSION 1.0 CLASS / BEGIN…END` header**: single-file `compile` chokes on the
  exported class header, but loading the same `.cls` as a `ClassModule` in a `.basproj`
  does **not** error — so this is a single-file-`compile` limitation, not a class-ingest
  bug. (Originally mis-described as expected; corrected after checking.)

## Findings (compiler/runtime gaps surfaced by the corpus)

Each has a minimal standalone repro (kept under the gitignored `temp/` while iterating).

- **VBA7 dialect** — `VBA7` was already predefined `True`, so the corpus's `#If VBA7`
  `PtrSafe` branch was taken; made the predefined `#If` set explicit/complete for VBA 7.1
  (`Vba6`/`Win32`/`Win16` added, `Win64`/`Win32` keyed to pointer width). Status: **done.**
- **F1 — intrinsic `vb*` constants unresolved.** `vbCrLf`, `vbCr`, `vbTab`, `vbObjectError`,
  `vbBinaryCompare`, `vbFromUnicode`, … reported `use of undeclared variable` under
  `Option Explicit`. Fixed by binding the always-available `vbConstants` family to literal
  values in `resolve.rs`. Status: **FIXED** (regression test
  `resolve_intrinsic_vb_constants_to_literals`).
- **F2 — omitted / Optional arguments rejected.** Two layered defects:
  1. **Root cause (high impact):** `parse_proc_signature` rejected any `Optional` parameter
     that was `ByRef`. VBA parameters are ByRef by default, so the ubiquitous
     `Optional b As Long` made the *whole procedure* fail to register — every caller then
     hit `call to unknown procedure`. So essentially all real-world `Optional` usage was
     broken, full-arity calls included.
  2. Omitted positional args via bare commas (`Foo(1, , 5)`) were rejected by
     `split_call_args` before binding.
  Fixed by removing the bogus `optional && by_ref` rejection and adding an omitted-allowing
  arg split that binds an `__omitted` sentinel (lowered to the parameter's Optional default).
  Verified end-to-end via `run-project`: `Foo(1)`→default, `Foo(1, , , 5)`→defaults fill the
  gaps. Status: **FIXED** (regressions `resolve_optional_byref_param_is_accepted`,
  `resolve_omitted_positional_arguments_bind_sentinel`).
- **F3 — parameterless `Function` member read without parens on a project class returns
  `Empty`.** _(Re-scoped twice: it is **not** a `New`-instantiation bug — `Dim w As New
  Widget` does instantiate; the `i32:1` slot is the project-object handle. And the site is
  **not** `emit.rs`/a VM property-get hint — the generic dynamic-dispatch path passes
  `call_kind_hint: None`, which already get-or-calls.)_ Boundary, all via `run-project`:
  `Property Get` (default or not) → OK; `widget.GetScore()` (parens) → OK;
  `widget.AddOne(6)` (args) → OK; **`widget.GetScore` (parameterless `Function`, no parens)
  → `Empty`.**

  **Real site:** the compiler rewrite `rewrite_internal_class_property_expression_reads`
  (and its statement-form sibling) in `project.rs` resolves a no-paren project-class member
  read with `allowed_kinds = [ProcedureDeclKind::PropertyGet]` **only** (`project.rs:~4710`).
  A parameterless `Function` matches nothing, the line is left unrewritten/unresolved, and
  the bind plan silently yields `Empty`. The COM early-bound read rewrite already probes
  `[PropertyGet, Method]` — so the internal-class path is the inconsistent one.

  **Observed today by receiver-declaration form (each should return `42` per VBA):**
  - `Dim a As New Widget` → `out = a.GetScore` → silent **`Empty`** (no error).
  - `Dim b As Widget : Set b = New Widget` → `out = b.GetScore` → **wrong compile error**
    `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` (a *different* resolution path; VBA would call).
  - `Dim c As Object : Set c = New Widget` → **`Set c = New Widget` is itself rejected**
    (`unsupported statement`): late-bound assignment of a project class into `Object`/`Variant`
    is unsupported — a separate prerequisite defect that must be fixed for the late-bound lane.

  Three forms, three different wrong outcomes for one operation, across ≥3 resolution paths
  (property-expression read, default-member read, late-bound `Set`). The fix must unify them.

  **Receiver-type matrix (the comprehensive fix spans compile-time *and* runtime — we do
  not always have a static type):**
  - **Statically-typed project class** (`As Widget`/`As New Widget`): fix at compile time —
    make the rewrite a multi-step probe `[PropertyGet]` → parameterless `Function`
    (precedence: a real property wins), mirroring the COM early-bound `[PropertyGet, Method]`.
  - **Late-bound to a VBA/project type** (`As Object`/`Variant` receiver): kind is unknown
    at compile time → must get-or-call at **runtime** in the VM project-dynamic dispatch.
    The unhinted (`None`) path already name+arity-matches; verify it is the path taken and
    that any property-get-hinted read also falls back. (Keep the `PropertyGet` matcher
    unchanged; *add* a method probe — do not relax the property-get predicate.)
  - **Late-bound to COM**: already covered by the combined `DISPATCH_METHOD |
    DISPATCH_PROPERTYGET` fix (DAO commit) and the COM early-bound `[PropertyGet, Method]`.

  **Diagnostic-parity obligation** (see `docs/CONFORMANCE.md#conformance-principle-diagnostic--error-behaviour-parity`):
  the get-or-call probe matches a *parameterless* `Function` only. The edges must match
  VBA's **error** behaviour, not silently return `Empty`:
  - `x = obj.NeedsArg` (required-arg function/indexed property, no parens) → VBA "Argument
    not optional"; OxVBA must raise an equivalent diagnostic at the same point.
  - `x = obj.SomeSub` (value context) → VBA "Expected Function or variable"; so the
    value-read probe is `[PropertyGet, Function]`, **excluding `Sub`**, and a `Sub` in value
    context must error rather than be called.
  These error cases are oracle-confirmable but the silent-`Empty`-on-unresolved-read
  behaviour is itself a parity bug to fix alongside.

  Status:
  - **F3b (no-paren read get-or-call): FIXED** (commit `a3f6ee34`). The internal-class
    no-paren read rewrite now probes a parameterless `Function` after `PropertyGet`, so
    `Dim w As New Widget : x = w.GetScore` → `42`. Regression test
    `pure_oxvba_class_no_paren_read_invokes_parameterless_function`; full suites green.
  - **F3a + F3c(a) (`Set <var> = New <ProjectClass>`): FIXED (5/5 receiver forms).**
    `expand_bound_source_line` recognises `Set <var> = New <ProjectClass>` (and `As New`) and
    lowers it to `Set <var> = __oxvba_project_instance(<handle>)`, with a
    `referenced_typelib_blob` guard so COM `New` still routes to the early-bound rewrites.
    `As <Class>`, `As Object`, `As Variant`, untyped, and `As New` all give `42`. Regression
    tests `pure_oxvba_class_explicit_set_new_instantiates_and_dispatches` and
    `pure_oxvba_class_set_new_into_object_variable_instantiates_and_dispatches`.
  - **Object representation: integer handle removed from the value model.** The instance is
    no longer an `i32` in the slot. `__oxvba_project_instance(<handle>)` is typed `Object` and
    lowers to a VM instruction `LoadProjectObjectRef` that materialises the route's
    reference-counted `ObjectRef` as a `Variant::Object`; the `Set` form assigns it as an
    object reference. `Set`/overwrite/scope-exit therefore AddRef/Release the instance through
    the **same COM `Variant` Clone/Drop path** COM objects use — that is the reference counting.
    This dissolved the earlier F3c integer-seed scaffolding (the runtime-guard bypass, the
    Variant-typing trick, the default-member seed exemption — all removed). Regression test
    `pure_oxvba_class_new_instance_is_a_reference_counted_object` (`IsObject`/`Object`-typed
    slot + `Set d = c` aliasing). The object's compat identity is still the route handle, so
    project-dynamic dispatch is unchanged. **Deferred follow-ups** (beyond reference counting):
    per-instance field-state isolation + runtime-distinct instances per `New` (same-site `New`s
    currently share state via the route), and `Class_Terminate`-on-last-release (objects free
    memory at refcount 0 but the VBA teardown hook — a VM-drained termination queue — is not
    yet wired). See [[project_object_cycle_leaks_ok]].
  - **F3c(c) (edge diagnostics): FIXED.** A no-paren *value-context* read that does not
    resolve to a `Property Get` or a no-arg-callable `Function` now raises a VBA-equivalent
    compile-time diagnostic instead of silently yielding `Empty`: a `Sub` →
    `PMR-E-MEMBER-READ-EXPECTED-FUNCTION-OR-VARIABLE` (VBA "Expected Function or variable"),
    a required-arg `Function` → `PMR-E-MEMBER-READ-ARGUMENT-NOT-OPTIONAL` (VBA "Argument not
    optional"). The get-or-call probe was also widened from *parameterless* to
    *no-arg-callable* (all-`Optional`/`ParamArray` functions are now called, not flagged).
    Statement-form `obj.DoThing` Sub calls are excluded (they route to member dispatch).
    Regression tests `pure_oxvba_class_value_read_of_sub_is_expected_function_or_variable`,
    `pure_oxvba_class_value_read_of_required_arg_function_is_argument_not_optional`,
    `pure_oxvba_class_statement_sub_call_and_all_optional_function_read_are_not_diagnosed`.
  - **F3c(b) (oracle matrix): registered deferred gate `ODG-049` / `CCT-051`.** Excel/VBA
    oracle must confirm the exact diagnostic text + compile-vs-runtime timing and the full
    receiver-form × member-kind matrix (incl. statement-context required-arg and indexed
    `Property Get` reads). Deferred per the Excel-differential policy; see
    `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`.
- **F4 (minor, observed) — single-file `oxvba-cli run`/`compile` can't resolve sibling
  procedures.** A bare `.bas` with `Sub Main` + `Function Foo` reports `call to unknown
  procedure: foo`; the same code in a `.basproj` (`run-project`) resolves fine. Affects only
  the single-file CLI convenience path; noted so corpus testing uses `run-project`.

## VBA-Web first-pass issue-family catalog

VBA-Web is being used as the first larger library proofing lane from the showcase
candidate list. Local upstream source and the adapted `.basproj` live only under
`.external/vba-corpus/vba-web/`; this section records our own observations and
general issue families. Do not treat these as one-off patches for VBA-Web only:
each item needs a broader grammar/semantic audit and non-corpus regression tests.

| ID | Status | Family | What VBA-Web exposed | Broader scrutiny needed |
| --- | --- | --- | --- | --- |
| VW-00 | fixed, needs periodic audit | Toolchain exhaustiveness after runtime type expansion | The first `oxvba-cli` build for this lane failed because newer `VarType::Record`, `TypeLibParamType::Record`, `TypeLibWireType::SafeArray { .. }`, and record wire variants were not handled in HAL conversion, CLI value formatting, pure-library `IsNumeric`, and generated COM descriptor mapping. | Keep an explicit audit rule after runtime value/wire enum expansion: every host adapter, CLI formatter, wrapper descriptor mapper, and intrinsic classifier must either support the new family or decline with a deliberate diagnostic. |
| VW-01 | fixed, broadened | Module top-level classification | Loading VBA-Web as `OutputType=Library` reported `PROJ-E-TOP-LEVEL-MAINLINE-UNSUPPORTED` for `WebHelpers`, even though the relevant early lines are declarations, conditional declarations, module variables, public constants, enums, and types, not executable mainline code. | Loader top-level policy now preprocesses conditional compilation before classifying mainline code, so inactive top-level executable lines do not reject library/addin/server modules. Regression coverage includes `Attribute VB_Name`, `#Const`, inactive executable branches, active conditional declarations, `Type`, procedure-local `#If`, and duplicate active/inactive line text during `Exe` top-level rewrites. Remaining scrutiny: offset-preserving diagnostics for malformed conditional directives in project-loading contexts. |
| VW-02 | fixed, broadened | Colon statement separators and single-line `If` grammar | VBA-Web uses idioms such as `If Len(x) > 0 Then: x = x & "&"` and `If cache Is Nothing Then: Set cache = New Dictionary`. The parser treated a colon immediately after `Then` as the start of an unexpected statement. | Parser coverage now includes leading/trailing/empty colon-separated statement segments, `Then::`, multiple inline statements, same-line `Else`, and interaction with `On Error`, `Set`, and `Exit`. Remaining scrutiny: broader statement families such as `For`, `Do`, `With`, file I/O, and error labels adjacent to empty inline segments. |
| VW-03 | fixed, broadened | Multi-line `If`/inline `If` boundary around `Else` | Nested inline `If` statements inside multiline `If` bodies could steal the enclosing `Else` after the `Then:` separator fix. | `Else` is now treated as an expression/statement terminator for inline branch parsing, and regressions cover nested single-line `If ... Then ... Else ...` inside a multiline branch, an enclosing `Else`, and inline statements after multiline `ElseIf ... Then:` / `Else:` headers. Remaining scrutiny: branch termination around nested `Select Case`, line-continuation-before-`Else`, and preprocessed source offset reporting. |
| VW-04 | fixed | Qualified standard library namespace binding | `VBA.Split`, `VBA.Replace`, `VBA.DateSerial`, `VBA.Len`, and `VBA.vbString` style references needed to resolve through the standard-library namespace rather than as receiver member calls. | Expand catalog parameter metadata beyond the currently exercised intrinsics so named arguments and optional defaults stay correct for all migrated standard-library members. |
| VW-05 | broadened, diagnostics lane open | Conditional compilation breadth | VBA-Web has nested `#If Mac Then` / `#ElseIf VBA7 Then` / `#Else` declaration blocks and procedure-local conditional sections. The Windows/VBA7 fixture now compiles through active branches. | Whole-project loader tests now cover conditional declarations, procedure-local `#If`, nested/inactive branches, `#Const`, project `DefineConstants`, and conditional-aware top-level classification. Remaining scrutiny: diagnostic location preservation and malformed directive reporting through project load/build surfaces. |
| VW-06 | policy-gated Dictionary lane added, live lanes remain | COM/library object model dependencies | `New Dictionary` required Scripting typelib metadata to expose a library-level coclass. The fixture now imports `Scripting` and compiles through dictionary creation and use. Other late-bound COM creation sites are admitted syntactically but not yet a VBA-Web runtime compatibility claim. | Existing COM proof layers are explicit: non-ignored HAL tests cover COM activation policy denial and a Windows Dictionary invoke-when-available lane; ignored `oxvba-host` COM matrix tests cover live Dictionary methods/properties/collections across late-bound, early `PreferVtable`, and early `DispatchOnly` legs. A tracked `vba_web_com_lanes` test now proves VBA-Web-shaped Dictionary activation is policy-gated by default and provides ignored live Dictionary and no-network WinHTTP activation/setup smokes. Remaining VBA-Web work: graduate the ignored project harness itself into a COM lane, then `WithEvents`/async WinHTTP `Send`/callback behavior as separate live lanes. |
| VW-07 | fixed, broadened | Continued parameter-list parsing and keyword identifiers | VBA-Web declarations and procedures use continued parameter lists and parameter names such as `Name`. These surfaced parser/scanner assumptions about newline trivia inside parameter lists and keyword tokens as identifiers. | Parser regressions now cover keyword-like names in parameters, named arguments, UDT fields, enum members, contextual-keyword labels, and property names, while retaining separate continued-parameter-list tests. Remaining scrutiny: non-contextual reserved words that require bracketed identifiers and binder/runtime behavior for keyword-named public members across project boundaries. |
| VW-08 | fixed, catalog expansion remains | Named intrinsic arguments and omitted optional defaults | `Replace(..., Count:=1)` needed named-argument reordering to the intrinsic's parameter slots, and omitted earlier optional slots must preserve default behavior. | Populate parameter-name metadata for more intrinsic functions as corpus/oracle coverage expands; missing metadata should be a visible catalog gap, not an ad hoc binder rule. |
| VW-09 | fixed, semantic edge remains | Default-member indexing on function results | `web_GetConverter(CustomFormat)("MediaType")` indexes the default member of an object returned by a function. | Distinguish default-member indexing from array indexing for function results when static return types become more precise; object/variant receivers currently lower through late `Item` dispatch. |
| VW-10 | fixed, broadened | `Mid$` assignment form | `Mid$(target, start, len) = replacement` is a statement form, not a normal function call or property assignment. | Coverage now includes explicit length, omitted length, and qualified `VBA.Mid$` assignment spelling. Remaining scrutiny: bounds behavior and ByRef/writeback interactions around fixed-length and aliased strings. |
| VW-11 | fixed, metadata lane remains | Dynamic `Err.Raise` numbers | VBA-Web raises stored or computed error numbers such as `Err.Raise Err.Number` and module constants, not only literals. | Coverage now includes foldable, dynamic, and named `Number:=` error numbers plus `Err.Clear`. Remaining scrutiny: storing/reporting `Source`, `Description`, `HelpFile`, `HelpContext`, and runtime type mismatch/error-range behavior. |
| VW-12 | fixed | Qualified standard-module variables | `WebHelpers.AsyncRequests` is a public variable in a standard module and is used both as an l-value and as an object receiver (`WebHelpers.AsyncRequests.Add`). The binder previously treated `WebHelpers` as a value receiver and rejected the route. | Keep module-qualifier handling consistent across reads, writes, calls, indexed puts, and property setters, with local/parameter shadowing taking precedence. |
| VW-13 | Engine host-root proof added, live Excel semantics remain | Ambient host `Application` root | The ignored local fixture includes a tiny `Application` shim for `Application.Run` and `Application.OnTime` so the core library can compile without claiming Excel host identity or scheduling semantics. | The binder has a deterministic host-injected Excel `Application` metadata proof for `Application.Run` and `Application.OnTime`, and tracked `vba_web_com_lanes` host tests now execute those calls through a registered portable host-provided `Excel.Application` object at the low-level bind/link layer, through the public `Engine` manifest path, and through the `Engine` project-closure path with injected typelib metadata. Remaining work: graduate the ignored CLI project fixture off its local `Application` shim and keep real Excel identity/scheduling semantics as a separate live-host lane. Do not report this fixture as full Excel-host compatibility. |
| VW-14 | smoke passing, broader lane open | Harnessed referenced-library execution | An ignored `VbaWebHarness.basproj` host project now references `VbaWebCore.basproj` and calls selected pure helper functions from `Sub Main`; `UrlEncode("a b+c")` with omitted optionals, `UrlDecode("a%20b%2Bc")`, `JoinUrl`, and `MethodToName(WebMethod.HttpPost)` currently self-check and exit successfully. This proved that the previous core fixture was only a library compile/admission proof, not an execution proof. | Keep a host-project harness for every external library proof. Harnesses should self-check expected values with `Err.Raise`, then graduate from pure helpers to object/COM/host lanes. |
| VW-15 | fixed | Referenced-project enum type exposure | The first harness used `WebHelpers.MethodToName(WebMethod.HttpPost)` and failed binding with unresolved `WebMethod`, even though `WebMethod` is a public enum in the referenced VBA-Web library. | Referenced project enum type qualifiers now resolve through the synthesized surface, including both `Enum.Member` and `Project.Enum.Member` forms, without misclassifying coclass members as namespace-qualified static calls. Keep coverage for local variable shadowing of qualifier names. |
| VW-16 | fixed, const-default lane remains | Cross-project optional defaults at runtime | Calling `WebHelpers.UrlEncode("a b+c")` from the harness reached runtime and failed with error 13, `unsupported coercion from Error to Double`. The omitted optional parameters arrived as the Missing/Error sentinel, then `If SpaceAsPlus = True` tried to coerce that sentinel numerically. | Referenced-project surfaces now carry literal optional defaults and the cross-bundle binder synthesizes explicit defaults, typed zero defaults, object `Nothing`, and Variant Missing as appropriate. Remaining scrutiny: optional defaults expressed as module constants or enum-qualified constants need folded-default metadata in the surface rather than only signature-literal metadata. |
| VW-17 | fixed, parity edge remains | Conversion functions and VBA radix-prefixed strings | `WebHelpers.UrlDecode("a%20b%2Bc", True, 0)` reaches `VBA.CInt("&H" & web_Temp)` and previously failed at runtime with error 13, `expected a numeric value`. VBA conversion functions accept hex/octal-prefixed numeric strings such as `&H20`; OxVBA treated that as non-numeric text. | `CInt`/`CLng`/`CLngLng`/`CDbl`/`CBool` conversion parsing now accepts VBA `&H`/`&O`/bare-octal string forms, including signed prefixes where valid, and rejects malformed radix strings. Remaining scrutiny: exact VBA overflow and type-suffix behavior for very large prefixed strings. |
| VW-18 | Engine no-shim corpus proof added, CLI/live-host boundary remains | Raw upstream VBA-Web source execution without source shims | Review showed the local `.external` fixture still carried an untracked `ExcelApplicationShim.bas`, so the previous CLI fixture was not a no-changes proof. | Ignored tracked tests in `vba_web_external_corpus` now synthesize temporary `.basproj` files that reference the raw upstream VBA-Web `.bas`/`.cls` files without `ExcelApplicationShim.bas`, import `Scripting` as a normal COM reference, inject only `Excel.Application` as a host object-model reference, and execute a broad pure-helper harness through `Engine`. The symbol layer now splits host-injected ProgID-style references such as `Excel.Application` into library/coclass resolver requests, and `oxvba-host::HostProfileProvider` composes the host-owned typelib resolver/projection/policy for these Engine lanes. Remaining work: direct `oxvba-cli run-project` no-shim execution still depends on live host metadata availability and is not a deterministic CI claim; Dictionary runtime behavior belongs to the normal COM lanes, not to a host-profile shim. |
| VW-19 | fixed test debt, guard principle recorded | `Scripting.Dictionary` must not be host-special-cased | A deterministic no-shim VBA-Web harness previously registered a portable fake `Scripting.Dictionary`, which blurred the boundary between host-provided object models and ordinary COM library imports. | The fake Dictionary projection was removed. Host profiles may provide host-owned roots such as Excel `Application`; `Scripting.Dictionary` remains a normal COM typelib/activation dependency exercised by the live COM matrix and the ignored `vba_web_com_lanes` Dictionary smoke. Any future special casing of `Scripting.Dictionary` outside the normal COM import/activation path should be treated as a bug and recorded as such. |
| VW-20 | three-way host equivalence smoke passing | Host-file, referenced-host-project, and true host-injected VBA-Web execution | The next infrastructure question was whether the same raw VBA-Web source can run when the host root is supplied three different ways. | The ignored `vba_web_external_corpus` test now synthesizes all three host shapes from tracked Rust test code: an inline `Application.bas` in the core project, a separate referenced `FakeExcelHost.basproj`, and a true `HostInjected` `Excel.Application` resolved by `HostProfileProvider`. The same harness calls a core `HostProbe.AssertHostRoot` plus pure helper checks through each shape; the host-injected lane also asserts the portable host call log is exactly `Run:2`, `OnTime:2`. Remaining work: broaden the harness from infrastructure smoke into WebHelpers/WebRequest/WebResponse/WebClient/authenticator execution, then add normal live-COM lanes for Dictionary/MSXML/crypto/WinHTTP. |
| VW-21 | broad live-COM harness passing | WebHelpers/WebRequest/WebResponse/WebClient/authenticator execution | The broadened no-shim harness now runs inside the VBA-Web core project and exercises host root calls, URL encode/decode/join helpers, format/media helpers, URL-encoded parsing, scalar and nested JSON parsing, key-value dictionary helpers, Collection cloning/default-member indexing, Dictionary cloning, WebRequest URL segments/query/header/cookie/body-parameter mutation and exact JSON body serialization, WebRequest cloning, `CreateFromOptions` for `UrlSegments`, WebResponse update, WebClient URL/proxy fields, and Basic/Digest authenticator setup. This exposed general fixes for ByRef arguments supplied from object default-member index expressions, `TypeOf` on Variant-held COM objects, built-in Collection default-member metadata, class header `MultiUse` instancing, enum declared-type normalization, enum-qualified optional defaults, VBA `VarType(Object)` parity (`vbObject = 9`, while still accepting foreign `VT_UNKNOWN = 13`), and COM runtime-object argument/result hardening. | The broad harness imports `Scripting` as a normal COM reference and only host-injects Excel `Application`; this remains an ignored live-COM proof, not deterministic CI. Runtime objects such as built-in `Collection` now cross native COM containers through bridge-owned `IDispatch` wrappers and unwrap back to their original `ObjectRef` when returned, so nested `ParseJson` object/array storage through `Scripting.Dictionary` is no longer a recorded residual. |
| VW-22 | investigated, fixed for focused residual set | Residual JSON/URL/header paths before spec-runner work | A focused residual probe isolated the previously broad failures so the spec runner does not inherit vague "VBA-Web still fails" buckets. | Fixed and pinned: dictionary/collection JSON conversion now works because runtime `VarType(Object)` reports `9`; `WebRequest.Body` exact JSON is asserted in the broad harness; ByRef writeback to a Variant-held Dictionary default member no longer faults; ordinary `ExtractHeaders`/`ExtractCookies` pass; enum-qualified optional defaults such as `UrlEncodingMode.FormUrlEncoding` are honored in omitted-argument paths, so `ConvertToUrlEncoded` uses form URL encoding by default; nested JSON object/array storage through `Set json_ParseObject.Item(key) = json_ParseValue(...)` now succeeds without any `Scripting.Dictionary` special case because internal OxVBA objects are projected at the COM boundary as unwrap-capable `IDispatch` wrappers. |
| VW-23 | extracted WebRequest spec runner passing | Extracted `.xlsm` spec workbook runner | The ignored `vba_web_external_corpus` tests now synthesize a temporary project from locally extracted `VBA-Web - Specs.xlsm` modules and run workbook spec-framework slices through OxVBA. The runner keeps upstream modules in the ignored corpus area, relocates exported default-member attributes such as `Attribute Item.VB_UserMemId = 0` so existing symbol scanning preserves semantics, replaces workbook/UI reporter behavior with console-style `ImmediateReporter` and `WorkbookReporter` shims instead of dropping those concepts, and imports `Scripting.Dictionary` through the normal COM reference path. This pass produced general regressions for unqualified enum members, `RaiseEvent` when an event and property share a name, `VBA.Collection` qualified type/member lookup, late-bound property reads on Variant-held objects, `With <function result>` one-time receiver evaluation, same-class bare method calls needing implicit `Me`, dimension-aware `LBound`/`UBound`, project-instance default-member dispatch, default accessor metadata propagation across Get/Let/Set, and ByRef copy-back from nested/indexed/default-member expressions. | Bounded ignored probes now pass for returned-suite `Class_Terminate` registration, `With Suite.It(...)` temporary behavior, WebRequest collection/dictionary/array body formatting, `SpecExpectation` named-argument paths, cookie/default-member expectation chains, and the full extracted upstream `Specs_WebRequest.Specs` run. The `Array("A","B","C")` JSON mismatch is fixed by binding/lowering optional `LBound`/`UBound` dimensions instead of ignoring the second argument, and the nested `Spec.Expect(Request.Cookies(1)("Key"))` path is fixed by guarded ByRef copy-back that stores back to non-slot places only when the callee actually mutates the temporary. Direct parser support for member-level exported attributes remains a follow-up; the runner currently relocates only default-member metadata in the temporary copy. |

## Riff first-pass issue-family catalog

Riff is the next external proofing lane after VBA-Web. Local upstream source is
stored only under `.external/vba-corpus/riff/`; tracked tests synthesize temporary
projects over that source instead of committing or modifying it. The first pass
intentionally stays short of `RiffOpen`, because that path activates WASAPI and
Media Foundation, allocates executable memory, installs timer callbacks, and uses
manual COM/vtable thunks. Those are the core future compatibility targets, not
things to mask inside a small smoke.

| ID | Status | Family | What Riff exposed | Broader scrutiny needed |
| --- | --- | --- | --- | --- |
| RF-01 | fixed | Bare standard-module `Property Let` assignment | Riff exposes module-level public properties such as `RiffMasterVolume`. A simple harness assignment `RiffMasterVolume = 0.25!` was rejected as `BIND-E-INVALID-ASSIGNMENT` because the binder only lowered bare property setters as implicit class `Me.Property` calls. | Standard-module property setters now lower directly to the owning `VbaProc` when the accessor has no class owner, while class properties still use implicit `Me`. Regression coverage includes scalar and module-level UDT-backed storage. Keep indexed standard-module properties and `Property Set` in the same semantic family when expanding tests. |
| RF-02 | characterized/running | Closed-state public property behavior | Before `RiffOpen`, Riff intentionally no-ops property setters and returns default/empty values from reads guarded by `rCtx.Initialized`. A tracked ignored harness now executes `RiffIsInitialized`, `RiffMasterVolume`, `RiffBusVolume`, and invalid `RiffVoiceVolume` paths without opening native audio devices. | Treat this as a real runtime smoke for raw Riff source, not a native-audio compatibility claim. The next useful Riff pass should move from closed-state property execution into a controlled native lane with explicit host policy for COM activation, `Declare`, callbacks, and executable memory. |
| RF-03 | fixed | Statement-call parsing with negative first argument | Riff uses statement calls such as `RiffFadeIn -1, 0.2!` and `RiffVoiceGetPeak -1, pL, pR`. The parser treated `Proc - 1` as a binary-expression callee instead of splitting the callable name from a bare argument list. | Implicit statement-call parsing now parses only a name/member/index callee before the bare arguments, so negative first arguments bind normally. Regression coverage pins the general `Capture -1, 2` shape. Continue checking this around `ByVal`/`ByRef` call-site overrides, member calls, and parenthesized expression arguments. |
| RF-04 | fixed | Fixed-size arrays inside UDT fields | `RiffContext` contains `Buffers(0 To 63) As RiffBuffer` and `Buses(0 To 7) As Single`. `RiffOpen` originally failed before native policy with runtime error 13, `expected an array`, because default UDT record initialization allocated scalar fields and nested scalar UDTs but left fixed array fields unallocated. | UDT record initialization now emits `ReDim` for fixed array fields using the retained CST bounds, while dynamic array fields remain unallocated until explicit `ReDim`. Regression coverage includes scalar fixed array fields and fixed array fields whose elements are UDT records. Remaining scrutiny: field bounds expressed through named constants across modules/projects. |
| RF-05 | characterized | `RiffOpen` deterministic native boundary | After RF-04, `RiffOpen` reaches the intended native boundary under deterministic host policy instead of failing on record layout. The ignored Riff harness now asserts that deterministic execution blocks the native path rather than opening WASAPI. | The next native lane should run under an explicit interactive/native policy and isolate each host side effect: `VirtualAlloc`/`VirtualFree`, `RtlMoveMemory`/`RtlZeroMemory`, `AddressOf` callback thunks, `MFStartup`/`MFShutdown`, `CoCreateInstance`, `SetTimer`/`KillTimer`, and `DispCallFunc` vtable calls. |
| RF-06 | future native lane | WASAPI/Media Foundation, hand-written COM vtables, callback thunks | The interesting Riff path uses `CoCreateInstance`, `MFStartup`, `DispCallFunc`, `AddressOf`, `VirtualAlloc`, `CallWindowProcW`, timer callbacks, packed UDTs, and 32/64-bit conditional assembly. | Build this as an isolated native execution lane with narrow admission diagnostics and cleanup proofs: policy-gated `Declare` execution, pointer-size/UDT layout checks, callback lifetime/reset safety, COM object release discipline, and vtable/`DispCallFunc` equivalence against the current COM client path. |
| RF-07 | fixed | Scalar `VarPtr` native storage and writeback | Riff's memory-copy lane depends on `RtlMoveMemory`/`RtlZeroMemory` over scalar addresses such as `VarPtr(copied)` and `VarPtr(src)`. A Riff-shaped `memmove` probe crashed because an unassigned `Dim copied As Long` was still the semantic `Empty` value, so `VarPtr(copied)` lowered to a null pointer instead of the address of a default-zero 4-byte `Long` cell. | Scalar `VarPtr` l-values now pin declared-width native cells (`Byte`, `Integer`, `Long`, `LongLong`/`LongPtr`, `Single`, `Double`, `Currency`, `Date`, `Boolean`) and read native writes back into the original variable. Regression coverage includes descriptor preservation for `LongPtr` pointer parameters, bridge-level `memmove` over raw pointers, and a host-level Riff-shaped scalar `memmove` through `VarPtr`. Remaining scrutiny: fixed-string and `Variant` cell mutation edges, and UDT/record-address lanes. |
| RF-08 | fixed | Exact kernel32 allocation and memory primitives | Riff uses `VirtualAlloc`/`VirtualFree`, exact `kernel32!RtlMoveMemory`, exact `kernel32!RtlZeroMemory`, raw pointer arithmetic such as `ByVal (ptr + offset)`, and typed aliases like `RtlMoveMemoryToSingle` / `RtlMoveMemoryToInteger` with `ByRef` destinations. | Native host coverage now exercises those exact Win32 declarations in a bounded private allocation: copy into a raw pointer offset, copy back to scalars, zero the region, free it, and verify typed ByRef destination writeback for `Single` and `Integer`. Remaining native lanes are callback/executable thunking (`AddressOf`, `CallWindowProcW`, `SetTimer`/`KillTimer`), Media Foundation/WASAPI activation, `DispCallFunc` vtable calls, and UDT/record-address layout. |
| RF-09 | fixed | `AddressOf` native callback thunk argument mapping and lifecycle | Riff builds timer callback thunks around `AddressOf RiffTimerCallback` and routes through `CallWindowProcW`-style four-argument native callbacks. A bounded `CallWindowProcW(AddressOf ...)` probe showed the callback invoked but standard-module callback arguments were shifted because VM inbound value delivery assumed an implicit `Me` slot. The native callback table also keyed cleanup by the VM's current stack address, so moving a `Vm` value before drop could leave stale callback registrations. | Standard-module native callbacks now load arguments from slot 0, while object/member callbacks keep the receiver slot convention. Native callback registrations now have a stable per-VM owner id and are removed on `Vm::drop` without disturbing another live VM. Regression coverage includes synchronous `CallWindowProcW` callback invocation with four native arguments and a VM lifecycle test for duplicate registration reuse plus drop cleanup. Remaining work: actual `SetTimer`/`KillTimer` async dispatch, executable thunk bytes generated by Riff, and host-reset behavior while callbacks are pending. |
| RF-10 | fixed | Manual vtable slot extraction | Riff's `VTableProc` reads the vtable pointer from a COM interface pointer, then reads a procedure pointer at `vTableIndex * LenB(LongPtr)` using `RtlMoveMemory` and `VarPtr(function-return)`. This exposed two broader VBA-runtime gaps: `LenB` on scalar variables must report declared storage width, and typed locals/function return cells must exist with their default values before first assignment. | Native host coverage now allocates a synthetic object pointer and vtable, writes a known slot value with exact kernel32 `RtlMoveMemory`, and verifies the exact Riff `VTableProc` pattern reads slot 2 back. General regressions pin scalar `LenB` widths (`Byte`/`Integer`/`Long`/`LongPtr`/`Single`/`Double`) and the binder now emits default initialization for typed scalar locals and function return cells while keeping `Variant` as `Empty` and dynamic arrays unallocated. Remaining work: `oleaut32!DispCallFunc` argument arrays/results, real COM/WASAPI interface acquisition, and vtable-call equivalence with the normal COM client path. |
| RF-11 | fixed, bounded native-record subset | `ByRef As Any` native cells and UDT writes | Riff calls `IIDFromString StrPtr("{...}"), guidResample`, where `guidResample` is a VBA UDT with `Long`, `Integer`, `Integer`, and `Byte(0 To 7)` fields. Its `DispCallFunc` wrapper also passes scalar array elements such as `vTypes(0)` and `pArgs(0)` to `ByRef As Any` parameters. The native `Declare` descriptor correctly admitted these parameters as `Any`, but the Windows marshaller had no `ByRef As Any` storage and rejected the call. | The native `Declare` bridge now admits scalar `ByRef As Any` cells using their native widths, plus descriptor-backed `Variant::Record` values whose fields are plain native ABI storage: numeric primitives, Booleans, nested UDT records, and fixed arrays of those shapes. The old GUID-only SAFEARRAY packing branch is removed; GUID is now just one native record layout. Regressions cover scalar `RtlMoveMemory` writeback through `As Any`, real `ole32!IIDFromString` with `Data4(0)=&HC0`, `Data4(7)=&H46`, and `S_OK`, and a general nested UDT/fixed-byte-array `RtlMoveMemory` writeback from a bounded native buffer. Records with owning `String`/`Variant` fields and unknown/foreign record layouts still decline instead of guessing. Remaining work: record `VarPtr`/source-`As Any` addressability, named-constant fixed bounds in early record metadata, `CoCreateInstance` pointer out-params, and broader `DispCallFunc` argument-array coverage. |
| RF-12 | fixed, bounded virtual-call subset | `oleaut32!DispCallFunc` vtable invocation and ByRef `Variant` result cells | Riff's `vCall` declares `DispCallFunc` with `ByRef prgvt As Any`, `ByRef prgpvarg As Any`, and `ByRef pvargResult As Variant`, then returns `CLng(vRet)` when OleAut32 reports `S_OK`. Before this pass the native `Declare` lane could not pass a real Windows `VARIANT*` for the result cell, so even zero-argument vtable calls could not execute through the Riff-shaped wrapper. | The native `Declare` bridge now stages ByRef `Variant` arguments as real Windows `VARIANT` cells, reads native writes back into OxVBA `Variant` values, and clears owned payloads. Coverage includes a lower-level OleAut32 synthetic vtable fixture that calls slot 0 and slot 1 with a typed `VT_I4` argument, plus a host-level Riff-shaped `DispCallFunc` call over a private synthetic vtable and ByRef result `Variant`. Remaining work: preserve native-array context for `vTypes(0)` and `pArgs(0)` when `argCount > 1`; the current generic ByRef-lowered array element only carries the first scalar value by the time HAL sees it. |

### Per project
- **Riff**: upstream fetched into the ignored corpus area from
  `uesleibros/riff` commit `cafe9bc01cba35f4e226e445d855ea0d701138e1`.
  A tracked ignored `riff_external_corpus` test now synthesizes a temporary host project
  over raw `Riff.bas` and executes the closed-state public property paths. This surfaced
  and fixed the general bare standard-module `Property Let` binder gap. Separate native
  `Declare` probes now cover Riff-shaped scalar `VarPtr` memory copying, exact
  `kernel32!RtlMoveMemory`/`RtlZeroMemory`, raw pointer offsets, `VirtualAlloc`/`VirtualFree`,
  typed `RtlMoveMemory` aliases with ByRef destination writeback, synthetic vtable slot
  extraction through Riff's exact `VTableProc` pattern, scalar and native-record
  UDT `ByRef As Any` writeback through real `ole32!IIDFromString` and a bounded
  nested-UDT `RtlMoveMemory` probe, bounded `oleaut32!DispCallFunc`
  vtable invocation with a real ByRef `Variant` result cell, and synchronous
  `CallWindowProcW(AddressOf ...)` callback delivery with cleanup of VM-owned native callback
  registrations. The native
  `RiffOpen` path remains future work because it exercises WASAPI/Media Foundation,
  hand-written COM vtables, callbacks, executable memory, and host reset safety.
- **Wasabi**: surfaces F1, F2, and (examples/tests) cross-module + project-class usage.
- **VBA-Web**: first pass gathered upstream at commit
  `9dbcc751d177099f20c96c5ee332ec10ef47423c` into the ignored corpus area and authored an
  ignored local `VbaWebCore.basproj` fixture. As of this pass,
  `oxvba-cli run-project .external/vba-corpus/vba-web/fixtures/VbaWebCore.basproj
  --diagnostic-format json` exits successfully with no diagnostics. This is a compile/admission
  proof for the adapted core fixture, not a runtime or Excel-host parity claim; `Application`
  remains shimmed in that ignored fixture. Tracked ignored `vba_web_external_corpus` tests now
  provide the no-shim raw-source Engine proof by synthesizing temporary project files over the
  upstream source tree. A separate ignored `VbaWebHarness.basproj` starts runtime proofing through
  a host `Exe` that references the core library. The current passing deterministic smoke covers
  selected pure helpers with `Excel.Application` supplied either as an inline module, a referenced
  fake-host project, or a true host-injected profile. Dictionary-backed runtime cases remain
  normal live-COM lanes rather than portable host shims. Blocked adjacent cases are cataloged as
  VW-15/VW-16/VW-18 and later rows.

## Related prior corpus work

- `.external/sqliteforexcel/` — SQLite-for-Excel VBA modules, vendored earlier for
  `Declare`/FFI integration testing (this one **is** committed; provenance under
  `docs/evidence/SQLITEFOREXCEL_*`). Distinct from this gitignored watchlist area.
