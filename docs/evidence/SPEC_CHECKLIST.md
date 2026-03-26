# SPEC_CHECKLIST.md

Spec-oriented execution checklist for OxVBA.

Status legend:
- `[x]` implemented in current executable subset
- `[~]` partially implemented (subset/scaffold/diagnostic-only)
- `[ ]` planned (not implemented yet)

Primary evidence sources:
- Language coverage index: `docs/evidence/language/COVERAGE_INDEX.csv`
- Intrinsic surface index: `docs/evidence/runtime/INTRINSIC_SURFACE.csv`
- Module/project requirements: `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`
- PMR formal clauses: `docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv`
- Conformance corpus: `conformance/tests/`

## Language Feature Checklist

| State | Spec Family | Feature | Scope/Evidence | Notes |
|---|---|---|---|---|
| `[x]` | Directives/options | `Option Explicit`, `Option Compare`, `Option Base` | `COVERAGE_INDEX.csv` (`v11`, `v78`, `v81`) | Implemented subset is covered by fixtures. |
| `[x]` | Lexical/syntax | Line continuation (`_`) | `conformance/tests/line_continuation_basic.bas` | In execution path. |
| `[x]` | Conditional compilation | `#Const`, `#If/#ElseIf/#Else/#End If` | `conformance/tests/conditional_compilation_basic.bas` | Deterministic compile-time selection subset. |
| `[x]` | Conditional flow | `If/ElseIf/Else/End If` | `if_true.bas`, `if_elseif_path.bas`, `if_else_path.bas` | Implemented. |
| `[x]` | Counted loops | `For ... Next` | `for_basic.bas` | Base form implemented. |
| `[x]` | Counted loops | `For ... Step ... Next` | `for_step_positive.bas`, `for_step_negative.bas`, `for_step_zero_error.bas` | Positive/negative step execution and zero-step diagnostic implemented. |
| `[x]` | Collection loops | `For Each ... Next` | `for_each_array_literal_basic.bas`, `for_each_array_variable_basic.bas` | Implemented for array-literal and declared-array iteration subset. |
| `[x]` | Conditional loops | `Do While ... Loop`, `Do ... Loop While`, `Exit Do` | `do_while_basic.bas`, `do_loop_while_basic.bas`, `do_exit_do.bas` | Implemented. |
| `[x]` | Conditional loops | `Do Until ... Loop`, `Do ... Loop Until` | `do_until_basic.bas`, `do_loop_until_basic.bas` | Implemented through normalized loop condition lowering. |
| `[x]` | Conditional loops | `While ... Wend` | `while_wend_basic.bas` | Implemented via normalization to existing loop IR. |
| `[x]` | Loop control | `Exit For` | `for_exit_for_basic.bas` | Implemented with innermost `For` unwind patching. |
| `[x]` | Multi-way branch | `Select Case` basic | `select_case_basic.bas` | Current support is value-list subset. |
| `[x]` | Multi-way branch | `Select Case Is` and range clauses | `select_case_is_range.bas` | Implemented for integer-domain clause matching. |
| `[x]` | Unstructured control | `GoSub` / `Return` | `gosub_basic.bas` | Implemented intra-procedure subset. |
| `[x]` | Unstructured control | `GoTo <label>` | `goto_label_basic.bas`, `goto_missing_label_error.bas` | Implemented with label target diagnostics. |
| `[x]` | Unstructured control | Line-number labels and numeric `GoTo` | `goto_numeric_basic.bas`, `goto_line_number_statement_basic.bas` | Explicit-colon and statement-prefix line-number labels are executable. |
| `[x]` | Procedure calls | `Sub`, `Function`, `ByVal`, `ByRef`, `Optional`, named args | `proc_call_chain.bas`, `function_call_basic.bas`, `params_*.bas` | Implemented subsets. |
| `[x]` | Procedure calls | `ParamArray` packing subset | `params_paramarray_pack.bas`, `params_paramarray_empty.bas` | Implemented subset with current constraints. |
| `[x]` | Assignment forms | `Set`/`Let` keyword assignment forms (subset) | `assignment_set_let_basic.bas` | `Set`/`Let` prefixes normalize to assignment path in current subset. |
| `[x]` | Properties | `Property Let/Set/Get` | `property_let_byref_route.bas`, `property_get_expression_basic.bas` | Let/Set routes plus Property Get RHS expression subset are executable. |
| `[~]` | Project model | Project/module/reference deterministic scaffold (`ProjectGraph` + `ProjectManifest`) | `crates/oxvba-host/src/project.rs`, `crates/oxvba-compiler/src/project.rs`, `docs/evidence/conformance/PMR_PROJECT_MODEL_FIXTURE_MATRIX_V1.md` | Identity/module/reference invariants, deterministic project compile entry, qualification rewrite subset, and host export registry are executable; full parity and cross-project runtime breadth remain partial. |
| `[~]` | Class/project event model | `WithEvents`/`Implements`/`RaiseEvent` legality + runtime dispatch semantics | PMR compiler tests + `DIV-0004` + `ODG-038` | Compile-time legality diagnostics are implemented; Implements baseline runtime prefixed-flow is executable; full instance-level event subscription/reassignment ordering remains deferred. |
| `[x]` | External calls | `Declare` / external binding | `declare_function_stub_basic.bas`, `declare_sub_stub_basic.bas` | Deterministic stub-execution subset; richer host binding remains future work. |
| `[x]` | Arrays | Fixed arrays, lower bounds, multidim indexing | `array_store_load.bas`, `array_option_base_one_bounds.bas`, `array_multidim_indexing.bas` | Implemented subset. |
| `[x]` | Arrays | `ReDim` / `ReDim Preserve` legality subset | `redim_preserve_*.bas`, `redim_without_preserve_resets.bas` | Implemented scoped behavior. |
| `[x]` | Arrays | `Erase` statement semantics | `erase_array_basic.bas` | Implemented fixed/dynamic array-slot reset subset. |
| `[x]` | Error handling | `On Error Resume Next`, `On Error GoTo 0`, `On Error GoTo <label>`, `Resume Next` | `on_error_*.bas`, `resume_next_statement_ok.bas` | Implemented subset. |
| `[x]` | Error handling | `Resume` (same statement / label targets) | `resume_statement_basic.bas`, `resume_label_basic.bas` | Implemented subset for `Resume` and `Resume <label>`. |
| `[x]` | Diagnostics timing | Compile-time vs runtime diagnostic phase classification (host execution path) | `v157` evidence (`formal_v157_*`) | Compile-time diagnostics preempt runtime execution; runtime diagnostics are phase-classified after successful compile. |
| `[x]` | Error object | Full `Err` object surface (non-HAL deterministic subset) | `stdlib_error_err_raise_resume.bas`, `err_clear_basic.bas`, `err_clear_full_surface_reset.bas`, `err_surface_fields_subset.bas`, `err_resume_next_clears.bas`, `err_proc_call_boundary_clears.bas` | In-scope deterministic Err surface and lifecycle behavior is executable; full VBA oracle parity remains deferred. |
| `[x]` | Types | Typed scalar lattice + coercion matrix + defaults (`Def*`, type chars) | `v67..v76` artifacts and compiler tests | Implemented subset. |
| `[x]` | Types | `String` BSTR and UDT runtime semantics (non-boundary deterministic subset) | `COVERAGE_INDEX.csv` (`String BSTR core`) | Non-boundary string semantics and UDT runtime subset are implemented; boundary interop parity remains deferred. |
| `[x]` | Types | UDT field access/assignment subset | `udt_field_access_basic.bas`, `udt_whole_assignment_copy.bas`, `udt_whole_assignment_overwrite.bas` | Type declarations plus flattened field-alias read/write subset and deterministic whole-value copy lowering are implemented. |
| `[x]` | Late binding | Object default-member late-bound calls | `late_bound_default_member_exec.bas`, `late_bound_named_argument_exec.bas` | Deterministic late-bound execution subset supports up to one argument. |
| `[x]` | Backends | VM + JIT subset with fallback | `run-conformance.ps1`, `v24+` evidence | Implemented for current supported op surface, with explicit fallback parity tests for unsupported financial tolerance and sentinel-tag introspection paths (`v159`). |

## Built-in Functions and Library Checklist

Reference inventory: `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`.

| State | Library Family | Functions / Surface | Scope/Evidence | Notes |
|---|---|---|---|---|
| `[x]` | Conversions (core subset) | `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, `Str`, `CVErr` | `COVERAGE_INDEX.csv` (`v45`, `v51`, `v153`) | Current coercion/domain subset with deterministic `CVErr` error-tag encoding. |
| `[x]` | String intrinsics (core/advanced subset) | `Len`, `Left`, `Right`, `Mid`, `InStr`, `InStrRev`, `LCase`, `UCase`, `Split`, `Join`, `Replace`, `Trim/LTrim/RTrim`, `StrComp`, `Like` | `INTRINSIC_SURFACE.csv` + conformance fixtures | Deterministic subset semantics; `Join` now maps array-tag inputs to element count in current runtime model (`string_join_array_tag_count.bas`). |
| `[x]` | Date/time subset | `DateSerial`, `TimeSerial`, `DateValue`, `TimeValue`, `DateAdd`, `DateDiff` | `v48` evidence | Deterministic numeric projection subset. |
| `[x]` | Math/financial subset | `Abs`, `Int`, `Fix`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`, `Log`, `Exp`, `FV`, `PV`, `PMT` | `v49` evidence | Integer/zero-rate subset semantics. |
| `[x]` | Array/type inspection subset | `Array`, `LBound`, `UBound`, `IsArray`, `VarType`, `TypeName`, `IsNumeric`, `IsDate`, `IsObject` | `v50` evidence | Tag and bounds projection subset. |
| `[x]` | Error subset | `Err.Raise`, `CVErr` pathways | `v51` evidence | Limited `Err` model. |
| `[x]` | Collection subset | `CollectionAdd`, `CollectionItem`, `CollectionRemove`, `CollectionCount` | `v53` evidence | Deterministic model subset. |
| `[~]` | Host-sensitive runtime | `Shell`, `Environ`, `Dir` | `v52` evidence | Deterministic fallback subset, not full host parity. |
| `[~]` | COM/dispatch bridge | `CreateObject`, `DispatchInvoke` | `v55`, `v84` evidence | Deterministic dispatch-projection subset. |
| `[x]` | String expansion | `Space`, `String$`, `Chr/Chr$`, `Asc`, `StrConv`, `Format/Format$` subset | `stdlib_string_expansion_core.bas`, `stdlib_format_core.bas` | Deterministic identity/count projection subset. |
| `[x]` | Date/time expansion | `Date`, `Time`, `Now`, `Timer`, `Year/Month/Day`, `Weekday`, `MonthName` subset | `stdlib_datetime_expansion.bas` | Deterministic constant/date-part projection subset. |
| `[x]` | Numeric/random expansion | `CSng`, `CByte`, `CCur`, `CDec`, `Hex`, `Oct`, `Atn`, `Tan`, `Rnd`, `Randomize` subset | `conversion_extended_scalar_subset.bas`, `stdlib_numeric_expansion.bas`, `stdlib_random_financial_expansion.bas` | Deterministic projection subset. |
| `[x]` | Financial expansion | `NPV`, `IRR`, `MIRR`, `Rate`, `NPer`, and related suite (in-scope subset) | `stdlib_random_financial_expansion.bas`, `financial_algorithm_npv_irr_mirr_subset.bas`, `financial_algorithm_rate_nper_subset.bas`, `financial_tolerance_non_convergence.bas`, `financial_tolerance_mixed_modes.bas` | Algorithmic subset is implemented with deterministic solver-failure error tags and expanded corpus coverage; host-oracle parity remains deferred. |
| `[x]` | Info/introspection expansion | `IsEmpty`, `IsNull`, `IsError`, `VarType`, `IsNumeric`, `TypeOf ... Is` subset | `stdlib_introspection_expansion.bas`, `coercion_null_empty_error_predicates.bas`, `introspection_vartype_isnumeric_tags.bas`, `typeof_is_condition_basic.bas` | Deterministic sentinel-tag subset with distinct `Empty`/`Null`/`CVErr`-error handling and explicit `VarType`/`IsNumeric` parity checks. |
| `[~]` | File I/O library | `Open/Close`, `Input/Line Input`, `Print #/Write #`, `EOF/LOF/Seek`, `FreeFile` | `stdlib_file_stub_intrinsics.bas` | Host-backed statement subset is now evidenced for `Output` / `Print` / `Close` / `Line Input`, quoted-string `Write#` / `Input#`, and `EOF` / `LOF` / `Seek`; richer `Input#`, error-path, and wider-mode parity remain pending. |
| `[ ]` | Interaction/UI | `MsgBox`, `InputBox` | No execution evidence yet | Planned (host-policy dependent). |
| `[ ]` | External automation libraries | Rich COM library surface (beyond current deterministic bridge) | No execution evidence yet | Planned. |

## Next Use

Use this checklist to drive profile/workset decomposition:
1. Add missing features first to `COVERAGE_INDEX.csv` and `LIBRARY_CHECKLIST.csv` as `planned`.
2. Promote to `partial` when parser/binder/typecheck scaffolding exists.
3. Promote to `implemented` only with executable conformance evidence.
4. For semantically uncertain items, add/track oracle probes in `docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv`.
5. For full MS-VBAL closure, drive module/project backlog via `MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`.
