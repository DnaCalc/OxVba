# vm3 vs complete-VBA-spec gap inventory

Source: the `vm3-spec-gap-audit` workflow (14 spec-dimension finders → synthesis;
2026-06-29) **+ the `vm3-gap-critique-redo` completeness-critic pass** (the original critic
died on a connection error; redone 2026-06-29). Authority = **complete MS-VBAL spec + live
Office VBA 7.1**, NOT vm2. 160 raw findings → 110 synthesized → **after critique: ~118
actionable gaps** (110 − 7 reclassified-DONE + 15 critic-added). Each bead still re-confirms
the gap against the code before fixing.

**Critique verdict (see "Critique addendum" below):** 27 of 29 Critical/High claims were
independently re-confirmed against live `file:line`; the 2 "refuted" were the redim-preserve
fixes I'd already landed (correctly caught). No behavior_class was mis-assigned. 15 net-new
gaps were found — most notably a whole **intra-project visibility/scoping cluster** the
per-dimension finders missed (none built a 2-module program).

`behavior_class`: **SilentWrong** (runs, wrong value — worst) · **HonestDecline**
(`Unimplemented`) · **BinderReject** (`Unsupported`/`Malformed`) · **Absent** (not in
library) · CorrectNoGap (dropped here).

Coverage already correct (from the audit's dropped CorrectNoGap notes): On Error/Resume/
propagation, Err.Raise rich fields + §9071 inheritance, GoSub/Return LIFO, For-range
evaluate-once, With/colon/line-continuation, Do/While, ByRef aliasing + Optional/IsMissing/
named-args/ParamArray-read/recursion/Static, Property Get/Let/Set, precedence/Mod/IntDiv/
concat/Like, Null-through-operators, scalar VarType/IsNumeric, CInt/CLng banker's + overflow,
Int/Fix, Rnd LCG, positive-date DateSerial/DatePart/DateAdd, date-literal interning, Get/Put
binary, FreeFile, Dir/Kill/attrs, conditional compilation, Enum folding, UDT layout, Implements.

> The handoff doc's residual list is partly STALE: Collection happy-path, cross-project link,
> For-Each-over-COM happy-path are implemented; only the edge cases below remain.

Status legend: ☐ open · ◐ in progress · ☑ done (commit).

## Performance defects (not spec-conformance — tracked here so they aren't lost)

These are correctness-neutral but real vm3 defects. Queued **after** the correctness sweep.

| # | id | sev | gap | fix locus |
|---|----|-----|-----|-----------|
|◐|vm3-dynamic-array-access-on (bead **bd-us4v**, blocked-by bd-4ktq)|High/Perf|Reading `arr(i)` of a module-level **dynamic** array inside a loop is **O(N)** in array length → array loops are **O(N²)** (100-elem loop=130ms, 400-elem=2.2s; native ~1µs). Isolated to the element read (ScanArray vs ScanPlain). From OxForms — `docs/handovers/HANDOVER_OxVba_vm3_dynamic_array_access_perf.md` (commit 9e3dbd6a)|**SLOT + CLASS-FIELD ARRAYS DONE (the OxForms shape); only rare UDT-record-field arrays remain.** Round 1 (019a39df): slot-held arrays (Global/Local/Temp) O(1) via `SafeArray::raw_safearray_variant_element`/`raw_safearray_bounds_len` → `Variant::safearray_element`/`safearray_bounds_len` → vm3 `array_get_fast`/`array_set_fast`. Round 2 (this fix): **class-instance-field arrays** (`Private mX()` in `.cls`) now O(1) via fused `FieldArrayGet`/`FieldArraySet` (elaborator fuses `Index`-over-`Field`; vm3 borrows the field `Variant` in place via `ObjectRef::with_project_field`/`with_project_field_mut`, reads/writes ONE element; non-array fields fall back to materialize-then-index). Both rows now flat ~12-13µs/elem (was class-field 270→961). Tests: `field_array_access_vm3.rs` (correctness+perf), `array_perf_diagnose.rs` (scaling). **REMAINING (rarer, deferred): UDT-record-field arrays `rec.arr(i)` still O(N)** — `RecordGet` clones; deeper fix (VbaRecord packs fields in a flat `Vec<u64>` buffer, needs raw-SAFEARRAY-through-record-offset access), NOT the OxForms shape|

## Critique addendum (completeness-critic redo, 2026-06-29)

The redone critic re-verified every Critical/High item against live code (27/29 Confirmed; 2
were already-fixed), found the 15 gaps below that the single-dimension finders missed, and
corrected the sequence. **Sequence fixes:** (1) `isarray-unallocated-false` was listed twice —
drop one; (2) `redim-undeclared-rejected` must come *after* `option-explicit-awareness` (its
dep); (3) the 7 DONE items (redim-fixed-array-reject, redim-preserve-{multidim-corrupt,
no-dimension-guard}, redim-multidim-count-overflow, erase-fixed-array-in-variant-element-type,
isdate-always-false, datevalue-cdate-of-date-raises-13) are struck from the actionable head, so
the remaining user-named work (AddressOf, GetObject) + the 3 Critical file-I/O bugs lead.
**Residual risk:** no multi-module/multi-project differential fixtures were built, so other
cross-module resolution edges (WithEvents source visibility, Public Const cross-module collision)
may remain unexamined.

**Scoping fixture baseline (bd-4ktq.9.1):** live Excel/VBA oracle evidence for the
multi-module visibility batch is captured in
`docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/` using
VBE Debug -> Compile (`ID=578`) plus PID-scoped UI Automation modal capture.
Current-green vm3 baselines and active follow-on assertions live in
`crates/oxvba-differential/tests/scoping_visibility_vm3.rs`.

**Scoping follow-up surface (bd-4ktq.36.1):** the cross-project expansion is
captured in
`docs/evidence/conformance/vm3_scoping_followup_oracle_20260701T1655Z/` using the
same modal-safe VBE compile path. The shared fixture now also covers a
two-module active project with a referenced project, referenced module/project
qualification, Public Const/Public variable ambiguity, referenced
`Option Private Module` hiding, reference precedence, and WithEvents source
visibility. The Public UDT/Public Enum name collision subset is now closed for
`bd-4ktq.36.3`: Excel reports `Ambiguous name detected: Payload`, and vm3 now
rejects the same unqualified type reference while preserving module-qualified
and project-qualified UDT references.

**Const/variable collision subset (bd-4ktq.36.2):** the Public Const/Public
module variable case is closed for the follow-up batch. The active fixture now
rejects unqualified `SharedName` as ambiguous and proves `Module.SharedName`
plus `Project.Module.SharedName` remain deterministic. Broader declaration-space
UDT/Enum declaration-space work is closed by `bd-4ktq.36.3`; broader
project/module/library namespace edges remain tracked under related PMR-NAME
rows.

**UDT/Enum naming subset (bd-4ktq.36.3):** the Public UDT/Public Enum
cross-module collision case is closed for the follow-up batch. The active
fixture now rejects unqualified `Payload` as ambiguous and proves
`Types.Payload` plus `VBAProject.Types.Payload` remain deterministic UDT type
references. Broader PMR-NAME-002 namespace-conflict coverage remains partial.

**Option Private cross-project subset (bd-4ktq.36.4):** the referenced
`Option Private Module` visibility case is closed for the follow-up batch. The
active fixture rejects unqualified and project-qualified external calls into
`LibProj.HiddenTools.HiddenValue`, proves the same module remains callable
inside its defining project, and proves a normal public referenced module in
the same project remains visible.

**Referenced-project precedence subset (bd-4ktq.36.5):** active-project names,
ordered referenced-project names, explicit project qualifiers, wrong project
qualifiers, and duplicate global names inside a referenced project now have
active vm3 coverage. `SurfaceProvider` now reports ambiguous unqualified names
inside a referenced project so the first ambiguous reference blocks fallback to
later references or the VBA library.

**WithEvents source visibility subset (bd-4ktq.36.6):** active-project and
referenced-project event source classes now have active vm3 coverage for
`WithEvents` handler-prefix routing. The fixture also proves a mismatched
handler prefix does not route, procedural-module `WithEvents` declarations are
rejected with a deterministic symbol diagnostic, and private/non-exposed
referenced event source classes remain inaccessible across project boundaries
both at declaration time and at construction/member-use sites. Broader event
graph ordering and reassignment semantics remain tracked outside this scoping
batch.

### Critic-added gaps (15)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☑|intra-project-private-not-enforced|High/SilentWrong|M|DONE (bead `bd-4ktq.9.2`): `ProjectProvider::MemberEntry` now carries `Visibility`; project-level unqualified lookup only publishes `Public` members, and `Module.Member` / `Project.Module.Member` qualified lookup uses a public-only owner-member resolver. Same-module private access remains available through the source scope chain before provider lookup. Oracle-backed vm3 fixtures flipped on for cross-module unqualified `Private` (`Sub or Function not defined`) and cross-module `Module.PrivateMember` (`Method or data member not found`). Checks: `cargo test -p oxvba-differential --test scoping_visibility_vm3`; `cargo test -p oxvba-symbol`; `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-symbol/src/providers/project.rs`; `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`|
|☑|ambiguous-name-not-detected|High/SilentWrong|M|DONE (bead `bd-4ktq.9.3`): duplicate `Public` members across distinct modules now stop unqualified active-project lookup with a binder-level `BIND-E-AMBIGUOUS-NAME` diagnostic (`ambiguous name detected: <name>`) instead of silently choosing the first scanned candidate or falling through to the VBA library. Qualified `Module.Member` lookup remains valid for each public owner. Oracle-backed vm3 fixture flipped on for duplicate `Dup()` across `Alpha` and `Beta`; resolver test pins that duplicate active-project `Len` blocks lower-priority library fallback. Checks: `cargo test -p oxvba-differential --test scoping_visibility_vm3`; `cargo test -p oxvba-symbol`; `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-symbol/src/provider.rs`; `crates/oxvba-symbol/src/providers/project.rs`; `crates/oxvba-bind/src/error.rs`; `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`|
|☑|date-arith-loses-date-type|High/SilentWrong|M|DONE (runtime-only, no binder change): oxvba-eval `widening_add`/new `widening_sub` re-tag the result `Date` per live oracle — `+` with ANY Date operand (incl. `Date+Date`) → Date; `-` with EXACTLY ONE Date operand → Date, `Date-Date` → Double; `*`/`/`/`\`/`Mod` stay numeric. Binder already types Date-arith as Variant→Widening so the runtime sees the real Date Variant (ranking Date would force Checked mode and strip the tag — left unranked deliberately). Helpers `is_date`/`as_date`. Tests `date_arithmetic_vm3.rs` (TypeName + CDbl vs oracle) + arith.rs unit test. Golden zero-drift. FOLLOW-UP (rare): unary `-Date` + Date-range overflow→6 not handled.|
|☑|currency-single-float-suffix-literals|Med/SilentWrong|S|DONE: `@`→Currency, `!`→Single, `#`/none→Double per the trailing type-decl char. Lexer already keeps the suffix in the token text (no lexer change). ONE shared `CoreConst::from_float_literal` (coreir.rs, next to from_vba_radix) called by BOTH the binder (expr.rs, type via `const_type`) AND `Const` folding (const_eval.rs); removed both dead `parse_float` helpers. Currency uses f64×10000 round_ties_even (consistent w/ CCur/Const-As-Currency; boundary-exactness is shared system behavior). Single is genuine f32. Live-verified TypeName 1.5@/100@=Currency, 1.5!/100!=Single, 1.5#/1.5=Double; CDbl(0.1!)=f32 rounding. Tests numeric_suffix_literals_vm3.rs. Golden zero-drift.|
|☑|implicit-string-to-boolean-13|Med/SilentWrong|S|DONE: implicit `b = "True"`/`"False"` now match explicit `CBool` via one shared recognizer (`oxvba_runtime::coerce::parse_bool_text`, strict case-insensitive, NO trim — live VBA errors on `"  False  "` for both paths). Numeric strings convert by non-zero. Also corrected `CBool`'s pre-existing over-trim. Tests: `implicit_bool_coercion_vm3.rs` (incl. padded→13) + unit tests in coerce.rs/arith.rs.|
|☑|currency-mul-f64-lossy|Med/SilentWrong|M|DONE (bead `bd-4ktq.8`): `Currency` `+`/`-`/`*` now uses an exact scaled-i64/i128 lane in `oxvba-eval::arith` for both `Checked(Currency)` typed arithmetic and Variant widening when a Currency operand combines with exact integer-compatible operands. Multiplication rounds half scaled units ties-to-even and reports Overflow (6) instead of falling through string errors. Currency-to-Currency coercion is now identity/exact, preventing store-time f64 re-rounding near the boundary. Tests: `cargo test -p oxvba-eval currency`; `cargo test -p oxvba-differential --test currency_arithmetic_vm3`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`. NOTE: a live-Excel retry was attempted first, but the first probe hit a compile modal (`Expected array`) because helper function `D` was shadowed by local variable `d`; UIA captured token `d` and line `"mul_near=" & d(a * b) & vbLf & _`, then the owned PID-scoped dialog/process was dismissed/stopped. Closure evidence here is the prior inventory live-oracle direction plus exact scaled tests/golden zero-drift.|oxvba-eval arith.rs exact Currency lane; `crates/oxvba-differential/tests/currency_arithmetic_vm3.rs`|
|☑|module-name-public-member-collision|Med/SilentWrong|M|DONE (bead `bd-4ktq.9.4`): a bare standard-module namespace used as a value/callee now reports binder diagnostic `BIND-E-EXPECTED-VARIABLE-OR-PROCEDURE-NOT-MODULE` (`expected variable or procedure, not module: <name>`) instead of drifting into a place/index fallback or silently selecting a colliding Public member. The oracle-backed `Clash` module / `Other.Clash` public function fixture is active and asserts the module diagnostic shape. Checks: `cargo test -p oxvba-differential --test scoping_visibility_vm3`; `cargo test -p oxvba-bind`; `cargo test -p oxvba-symbol`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-bind/src/error.rs`; `crates/oxvba-bind/src/lib.rs`; `crates/oxvba-bind/src/call.rs`; `crates/oxvba-bind/src/expr.rs`; `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`|
|☑|rgb-qbcolor-absent|Med/Absent|S|`RGB`/`QBColor` color functions absent (color *constants* exist)|Added `Rgb`/`QbColor` `NativeImplId`s (Information module): enum + `module()`/`library_member()`/`library_param_count()` arms + `vba_library` migrated-set guard + catalog `e`/`e_params` + lib dispatch + `pure::rgb`/`pure::qb_color`. RGB clamps each component to 0..=255 then packs `r+g*256+b*65536`; QBColor is the live-probed 16-entry palette, out-of-range → err 5. Live-verified values (`rgb_qbcolor_vm3.rs`, 3 tests) *(done)*|
|☐|format-number-family-absent|Med/Absent|M|`FormatNumber`/`FormatCurrency`/`FormatPercent`/`FormatDateTime` absent (only generic `Format`)|build on format.rs; named-format consts already exist (vba_library.rs:315-319) unconsumed|
|☑|financial-ipmt-ppmt-sln-syd-ddb-absent|Med/Absent|M|`IPmt`/`PPmt`/`SLN`/`SYD`/`DDB` absent (FV/PV/Pmt/NPV/IRR/MIRR/Rate/NPer exist)|Added 5 `NativeImplId`s (Financial module) + catalog + dispatch + pure bodies. Extracted shared `fv_value`/`pmt_value` f64 helpers (refactored FV/Pmt onto them). All formulas live-verified: SLN=`(cost-salvage)/life`; SYD=`(cost-salvage)*(life-per+1)*2/(life*(life+1))`; DDB=closed-form `book_start-book_end` with salvage floor (per5=296) + factor override; IPmt=`fv_value(rate,per-1,pmt,pv,t)*rate` with annuity-due `÷(1+rate)`/per1=0; PPmt=`Pmt-IPmt`. Guard `financial_vm3.rs`. Golden zero-drift *(done)*|
|☑|project-qualifier-ignored|Low/SilentWrong|S|DONE (bead `bd-4ktq.9.5`): active-project `Project.Module.Member` lookup now validates the project segment instead of discarding it. The binder recognizes active/referenced project names as namespace qualifiers, so valid `VBAProject.Lib.Pub()` reaches qualified resolution, while `WrongProject.Lib.Pub()` remains rejected. Referenced-project surface resolution already validates its project segment. Checks: `cargo test -p oxvba-differential --test scoping_visibility_vm3`; `cargo test -p oxvba-symbol`; `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-symbol/src/provider.rs`; `crates/oxvba-symbol/src/providers/project.rs`; `crates/oxvba-symbol/src/tests.rs`; `crates/oxvba-bind/src/call.rs`; `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`|
|☐|partition-absent|Low/Absent|S|`Partition(number,start,stop,interval)` absent|native.rs/catalog.rs add Partition(4,4) pure body|
|☐|getsetting-family-absent|Low/Absent|M|`GetSetting`/`SaveSetting`/`GetAllSettings`/`DeleteSetting` absent|route to a settings HAL facet (headless no-op) or HonestDecline|
|☐|vbmodal-vbmodeless-absent|Low/Absent|S|`vbModal`(1)/`vbModeless`(0) `Show`-modality constants absent (MsgBox modal consts exist)|vba_library.rs:294-295 add the two arms|
|☑|friend-on-standard-module|Low/SilentWrong|S|DONE (bead `bd-4ktq.9.6`): scanner rejects module-level `Friend` procedures/properties in standard/procedural modules with `SYM-E-FRIEND-ONLY-VALID-IN-OBJECT-MODULE`, while class-module `Friend` remains a distinct valid visibility. Oracle-backed vm3 fixture for standard-module `Friend Sub Helper` is active and rejects; class Friend baseline still runs. Checks: `cargo test -p oxvba-differential --test scoping_visibility_vm3`; `cargo test -p oxvba-symbol`; `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-symbol/src/model.rs`; `crates/oxvba-symbol/src/scanner.rs`; `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`|

## Tier 0 — user-named deferred beads (do first)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☑|redim-fixed-array-reject|Med/SilentWrong|S|`ReDim` of a fixed array silently re-dimensions instead of erroring|runtime guard in `array_redim` on `is_fixed_size()` → Fault 10 *(done; 6 corpus progs fixed to valid dynamic arrays)*|
|☑|erase-fixed-array-in-variant-element-type|Med/SilentWrong|M|`Erase` of a fixed array in a Variant slot re-defaults to Variant/Empty, flips element type|bind-site element unless Variant, then `array_element_type_for_vartype(element_vartype())` *(done)*|
|⊘|addressof-native-callback-thunk|Low/HonestDecline|L|`AddressOf`→native callback slot declines (vm3 lib.rs:3046)|**DECISION (keep the honest decline for now):** it is *correct*, just unimplemented — no in-scope corpus program exercises it, and it is the only capability that would need a large UB-adjacent unsafe facility (thunk pool + reentrant `*mut Vm3` re-entered from a C trampoline mid-FFI). Intra-VBA `AddressOf`/`CallProcRef` already works. Design when a real program needs it: substitute the thunk address as the `LongPtr` arg (HAL unchanged — it marshals `LongPtr` as a pointer-sized int), copy out `self.host` so the VM is unborrowed across the FFI, re-enter via `run_proc_with_values`. Sync-only (EnumWindows-style); async (SetTimer) additionally needs a message pump + thunk outliving the call.|
|☑|getobject-absent|Low/Absent|M|`GetObject` not bindable|catalog SpecialForm + `Native(GetObject)` route; `host::get_object` → `ComHal::get_object_variant`; HAL 3-mode dispatch (omitted→`GetActiveObject`, ""→`CreateObject`, path→`CoGetObject`); bridge `get_active_object`/`bind_file_object`. LIVE-verified (Dictionary new-instance + running Excel). *(done)* NOTE: the miss `Err.Number` is now correct (429/432) — `hal-errors-flattened-to-5` fixed|

## Tier 1 — Critical SilentWrong (data loss / everyday code)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☑|print-write-only-first-field|Crit/SilentWrong|M|`Print #`/`Write #` emit only the FIRST field|dedicated `bind_print_write` packs `[handle, sep-spec, fields…]`; `oxvba-lib` `assemble_print_record`/`assemble_write_record` emit every field + separators + terminator; HAL `print_line_variant` is now a verbatim text sink *(done)*|
|☑|input-stmt-no-writeback|Crit/SilentWrong|L|`Input #` never writes parsed fields back to targets|`bind_input` emits one `target = FileInput(handle, 1)` assignment per target *(done)*|
|☑|line-input-no-writeback|Crit/SilentWrong|M|`Line Input #` never writes the line back|`bind_line_input` emits `strvar = FileLineInput(handle)` *(done)*|
|☑|isdate-always-false|Crit/SilentWrong|S|`IsDate` False for all date strings|route strings through `cdate_from_string`; raw number not a date; validate calendar *(done)*|

## Tier 2 — High SilentWrong (common correctness bugs)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☑|date-to-string-emits-serial|High/SilentWrong|M|Date→String emits raw serial, not formatted|new `oxvba_runtime::vba_date` (canonical serial↔civil math, deduped from oxvba-lib) + `format_general_date` (M/D/YYYY h:mm:ss AM/PM, date-only/time-only variants); wired into `variant_to_vba_string`/`print_display_text`. `write_display_text` gets the `#YYYY-MM-DD HH:MM:SS#` Write form. Also fixed `TimeValue(<Date>)` (was round-tripping through the serial string → garbage; now extracts the time fraction directly) *(done)*|
|☑|datevalue-cdate-of-date-raises-13|High/SilentWrong|S|`DateValue`/`CDate` of Date/numeric raises 13|vtype dispatch in `date_value`; + calendar validation (Feb 30 → 13) *(done; CDate already dispatched)*|
|☑|redim-preserve-multidim-corrupt|High/SilentWrong|M|multi-dim `ReDim Preserve` flat-copies (corrupts)|coordinate-aware copy *(done w/ redim bead)*|
|☑|redim-preserve-no-dimension-guard|High/SilentWrong|M|`ReDim Preserve` doesn't enforce only-last-dim (no Err 9)|compare new vs old bounds *(done w/ redim bead)*|
|☑|option-base-1-ignored|High/SilentWrong|M|`Option Base 1` ignored (arrays always 0-based)|`module_option_base` threads the module's `Option Base` onto `ProcInfo.option_base` (mirrors `compare_mode`); `bind_array_bounds` uses it for a single-bound dim's lower (explicit `lo To hi` overrides); `Array()` carries it via a new `lower_bound` on `CoreValue/OxInst::ArrayLiteral` → vm3 builds the SafeArray with that base; `ParamArray` stays 0. Live-verified: Base1 `Dim a(3)`=1..3, `Array()`=1..3, explicit `2 To 5` & ParamArray=0-based *(done)*|
|☑|statement-call-paren-not-byval|High/SilentWrong|M|DONE (bead `bd-4ktq.10.2`): whitespace-separated statement-call parentheses such as `Inc (x)` now remain in the bare argument list as `ParenExpr`, causing binder call lowering to force ByVal and leave the caller unchanged. No-space compatibility forms such as `DispatchInvoke(...)` remain attached `IndexExpr` callees, and indexed receivers such as `obj(1).Inc (x)` keep their receiver index while splitting the terminal parenthesized argument. Oracle-backed vm3 fixture is active; golden re-bless audited the intended `vmr04_byref_expression_forms.bas` result change from `11` to `10`. Checks: `cargo test -p oxvba-syntax`; `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --test call_argument_binding_vm3`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-syntax/src/parser.rs`; `crates/oxvba-syntax/src/red.rs`; `crates/oxvba-bind/tests/bind_roundtrip.rs`; `crates/oxvba-differential/tests/call_argument_binding_vm3.rs`; `crates/oxvba-differential/vm3_golden.snap`|
|☑|byref-type-mismatch-accepted|High/SilentWrong|M|DONE (bead `bd-4ktq.10.3`): project-procedure ByRef aliasing now rejects mismatched l-value declared types with stable binder diagnostic `BIND-E-BYREF-TYPE-MISMATCH` / `ByRef argument type mismatch`. Parenthesized or otherwise non-aliased arguments are still passed through a coerced temporary, and the real-world array-to-`ByRef Variant` idiom remains accepted. Oracle-backed vm3 fixture is active; golden re-bless audited the intended `conformance/tests/byref_typed_mismatch_error.bas` change from wrong `ok[2]` to bind error. Checks: `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --test call_argument_binding_vm3`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-bind/src/call.rs`; `crates/oxvba-bind/src/error.rs`; `crates/oxvba-bind/tests/bind_roundtrip.rs`; `crates/oxvba-differential/tests/call_argument_binding_vm3.rs`; `crates/oxvba-differential/vm3_golden.snap`|
|☑|no-call-arity-validation|High/SilentWrong|S|DONE (bead `bd-4ktq.10.4`): project-procedure calls now validate arity during bind. Extra positional arguments without a `ParamArray` return stable binder diagnostic `BIND-E-WRONG-NUMBER-OF-ARGUMENTS` / `WrongNumberOfArgumentsOrInvalidPropertyAssignment`; omitted required parameters return `BIND-E-ARGUMENT-NOT-OPTIONAL` / `ArgumentNotOptional { parameter }`. Optional defaults and `ParamArray` tails stay valid. Project property put/set calls, indexed and unindexed, reserve the trailing RHS value slot so extra index arguments are rejected instead of overwritten and missing required index parameters are not filled by the RHS. Oracle-backed vm3 fixture is active; golden re-bless audited `vmr04_diag_missing_required.bas` and `vmr04_diag_too_many_args.bas` moving from wrong success to bind errors. Checks: `cargo test -p oxvba-bind`; `cargo test -p oxvba-differential --test call_argument_binding_vm3`; `cargo test -p oxvba-differential --lib vm3_golden_snapshot`.|`crates/oxvba-bind/src/call.rs`; `crates/oxvba-bind/src/error.rs`; `crates/oxvba-bind/src/stmt.rs`; `crates/oxvba-bind/tests/bind_roundtrip.rs`; `crates/oxvba-differential/tests/call_argument_binding_vm3.rs`; `crates/oxvba-differential/vm3_golden.snap`|
|☑|instr-leading-start-by-type|High/SilentWrong|S|`InStr` start detected by TYPE not arity|now arity-based: 2 args=(s1,s2), 3–4 args=(start,s1,s2,[compare]). Fixed 3 corpus progs (`InStr(12345,34)` was err 5, now 3) *(done)*|
|☑|instrrev-ignores-start-compare|High/SilentWrong|M|`InStrRev` ignores start, misreads compare|dedicated `instr_rev` with `(stringcheck,stringmatch,[start=-1],[compare])` layout: honours `start` (search region) + `compare` at arg 4 *(done)*|
|☑|split-ignores-limit-compare|High/SilentWrong|S|`Split` ignores limit+compare|`split_with_limit`: honours `limit` (last element = remainder; 0 → empty array) + `compare` (case-insensitive delimiter match, original case preserved) *(done)*|
|☑|option-compare-text-ignored-string-fns|High/SilentWrong|M|`Option Compare Text` ignored by InStr/StrComp/Replace/Filter/InStrRev|binder `inject_option_compare` (call.rs, on the `ExternMember` arm — these route to the "VBA" library bundle): under `Option Compare Text`, when the call omits `compare`, set it to `1` at the function's compare slot (StrComp@2, Filter/InStrRev@3, Replace@5; InStr promotes a 2-arg call to `InStr(1,s1,s2)` then @3) via `set_trailing_arg` (pads intermediate optionals with `Omitted`). Lib `arg_present` treats `Empty`/Missing-sentinel pads as absent so middle optionals still default. Live-verified truth table; golden no-drift. `Like` already respected compare (binary-op mode). *(done)*|
|☑|select-case-ignores-option-compare-text|High/SilentWrong|S|`Select Case` ignores Option Compare Text for strings|`CoreStmt::Select` gained a `compare_mode` (binder sets it from `self.info.compare_mode`); elaborate's `lower_select`→`lower_case_match`→`lower_case_clause` thread it into the `OxInst::Compare` mode (was hardcoded `Binary`), so `Select Case "a"`/`Case "A"` matches under Text. Live-verified; golden no-drift *(done)*|
|☑|mixed-string-numeric-compare-no-13|High/SilentWrong|M|String-vs-numeric compare returns value not Err 13|`cmp_order` guard: `String` (incl. numeric-looking) vs numeric/Boolean/Date → Err 13; `Empty`/`Null` exempt *(done; no golden drift)*|
|☑|and-or-imp-null-three-valued|High/SilentWrong|M|And/Or/Imp with Null always Null (no 3-valued logic)|`bitlogic` now evaluates the known operand against the unknown-as-0 vs unknown-as-(-1): agreeing bits survive (`False And Null`=False, `True Or Null`=True, Imp follows), else Null; Xor/Eqv always Null *(done; Not Null already Null)*|
|☑|null-not-propagated-string-fns|High/SilentWrong|M|DONE (bead `bd-s7cr`): string fns on Null raise 13 not Null (or 94 for `$`)|Unsuffixed value-returning string functions keep the shared `string_fn_propagates_null` policy at the `oxvba-lib::invoke` dispatch, so a `Null` arg → `Null` for Len/Left/Right/Mid/UCase/LCase/Trim*/StrReverse/Space/String/Chr*/Asc*/InStr*/Replace/StrComp/StrConv/Like. String-returning `$` aliases now preserve source spelling through synthetic `VBA` bundle import/export aliases (`Left$`, `Right$`, `Mid$`, `UCase$`, `LCase$`, `Trim$`, `LTrim$`, `RTrim$`, `Chr$`, `ChrW$`, `Space$`, `String$`, `Format$`) and vm3 raises error 94 before the shared unsuffixed body sees `Null`. Tests: `cargo test -p oxvba-differential --test null_string_fns_vm3`; `cargo test -p oxvba-bundle vba_library`; `cargo test -p oxvba-symbol unrelated_class_property_does_not_shadow_vba_left_intrinsic`; `cargo test -p oxvba-bind string_functions_left_mid_ucase`.|`NativeImplId::library_member_aliases`; `VbaLibraryProvider::resolve`; `Vm3::resolve_library_import`; `crates/oxvba-differential/tests/null_string_fns_vm3.rs`|
|☑|typeof-nothing-raises-91|High/SilentWrong|S|`TypeOf Nothing Is X` raises 91 not False|early `Ok(false)` for Nothing/Empty/Null (and an unset/`Set Nothing` object var) in `type_of_is` *(done)*|
|☑|for-counter-no-overflow|High/SilentWrong|M|`For` counter increment never overflows (Widening)|`lower_for_range` reads the counter's `OxTy` (via `lower_place_load`) and emits the step `Arith` in `NumericMode::Checked(target)` for a fixed-integer counter (Byte/Integer/Long/LongLong/LongPtr→Win64 LongLong), else `Widening`. So `For i As Integer = 32766 To 32767` runs the body for 32766/32767 then overflows (Err 6); a `Variant` counter promotes (Integer→Long, no error). Live-verified; golden no-drift *(done)*|
|☑|integer-literal-surfaces-as-long|High/SilentWrong|M|DONE (beads `bd-4ktq.11.1`..`bd-4ktq.11.3`): truth surface captured in `docs/evidence/conformance/vm3_integer_literal_oracle_20260701T1200Z/`. Decimal/radix Integer-width literals now surface as `2:Integer`; decimal Long-width and explicit `&` literals surface as Long; explicit `^` literals surface as LongLong; unsuffixed decimal beyond Long is Double; unsuffixed radix beyond Long width is rejected. Folded untyped `Const` values preserve Integer carriers, declared `Const As Long` and `Enum` members remain Long, and optional `Variant = 7` defaults preserve the Integer carrier for active-project and referenced-project calls. Tests: `crates/oxvba-differential/tests/integer_literal_carrier_vm3.rs` (11 rows), `cross_bundle_variant_optional_default_preserves_integer_carrier`, symbol metadata default test, hex/octal sign fixture, numeric suffix fixture, golden snapshot.|`CoreConst::I16`/`OxConst::I16`; parser/radix width diagnostics; vm3 `const_variant`; `DefaultValue::I16`; const/default audit|
|☑|vba-hex-oct-literal-sign|High/SilentWrong|M|`&HFFFFFFFF`=4294967295 not -1 (no width sign)|shared `oxvba_runtime::vba_radix` (magnitude→width→two's-complement, MS-VBAL §3.3.2; live-verified `&HFFFF`=-1, `&HFFFF&`=65535, `&O37777777777`=-1) via `CoreConst::from_vba_radix`; wired into the binder, const-eval, scanner, runtime `CInt`/`CLng`/`CDbl` string conversions, and `Val`'s leading-prefix scanner *(Val radix closed in bead `bd-4ktq.7`)*. The separate direct literal carrier gap is now handled through `integer-literal-surfaces-as-long` bead `bd-4ktq.11.2`.|
|☑|abs-int-fix-sgn-return-double|High/SilentWrong|M|Abs/Int/Fix/Sgn always Double; Sgn should be Integer; Abs overflow|`pure::{abs,int_floor,fix_trunc,sgn}` are type-aware (`math1_typed` dispatch): Abs/Int/Fix preserve the arg's subtype (incl. Currency/Decimal/Date), Abs **promotes** on overflow (Int→Long, Long/LongLong→Double — live shows promotion, NOT err 6), Sgn always→Integer (Null→err 94 since Integer can't hold Null), Bool→Integer, String→Double, Empty→Integer 0, Null→Null. Transcendentals stay Double. Verified live (full subtype matrix). Golden: 2 correct Double→Integer re-bless *(done)*|
|☑|seek-function-resets-position|High/SilentWrong|S|`Seek(n)` function resets position to 0|`seek_variant`: an omitted (Empty/Null) position READS the cursor (returns `entry.position`) without moving it; only `Seek #n, pos` repositions *(done)*|
|☑|reset-bare-close-error-5|High/SilentWrong|S|`Reset`/bare `Close` raise spurious Err 5|`bind_file_io` pushes a literal-0 handle for a `FileClose` with no file number (the close-all convention `close_variant` understands) *(done; `Reset` already parses as `CloseStmt`)*|
|☑|print-nonstring-truncates-to-long|High/SilentWrong|S|`Print #` non-strings truncate to Long|now routed via `print_display_text` in `assemble_print_record` *(done w/ the file-I/O cluster)*|
|☑|seek-loc-zero-based|High/SilentWrong|M|Seek/Loc 0-based; VBA Seek 1-based, Loc mode-dependent|`seek_report`/`loc_report` helpers: Seek FUNCTION = 1-based next byte (`cursor+1`) / next record (`cursor/reclen+1`); Seek STATEMENT = 1-based→0-based (`pos-1`, or `(pos-1)*reclen` for Random), rejects `pos<1`, and no longer extends the file on a bare seek; Loc = record# (Random) / cursor (Binary) / `pos\128` (sequential); Append reports a fresh cursor (Seek=1/Loc=0) while writes stay decoupled at EOF. All five live-Excel-verified end-to-end in `filesystem_statements.rs` *(done)*|

## Tier 3 — Medium SilentWrong / common BinderReject

~~collection-keynotfound-error-9-not-5(S)~~ *(DONE: bead `bd-4ktq.20`; vm3 built-in `Collection` missing keyed `Item`, default-member access, and `Remove` already raise error 9; added regression coverage in `collection_missing_key_vm3`)* · ~~foreach-com-failure-swallowed(M)~~ *(DONE: bead `bd-4ktq.31`; vm3 foreign COM `For Each` now maps `ComHal::enumerate_object` failures through `Fault::from_hal` instead of `unwrap_or_default`, so `On Error Resume Next` observes the host/COM `Err.Number`; covered by `foreach_over_foreign_object_surfaces_enumeration_failure` plus existing scalar/array/Collection controls.)* ·
~~foreach-scalar-non-object-empty(S)~~ *(DONE: bead `bd-4ktq.21`; vm3 `For Each` over scalar sources now raises error 13 instead of silently iterating zero times; covered by `foreach_scalar_source_vm3` plus array and `Collection` controls)* · ~~lbound-ubound-unallocated-error-13(S)~~ *(DONE: bead `bd-4ktq.19`; source-level never-allocated and erased dynamic arrays already raise error 13 for `LBound`/`UBound`; added regression coverage in `array_bounds_unallocated_vm3` plus allocated-array controls)* ·
~~coerce-null-numeric-no-94(S)~~ *(DONE: bead `bd-yd6d`; vm3 scalar numeric/date coercion now raises error 94 for `Null` through `arith::coerce_numeric`, and native explicit conversion helpers preserve 94 via `LibError::invalid_use_of_null`; covered by `null_coercion_vm3` for `C*` conversions plus implicit scalar assignment)* · ~~hex-oct-negative-width(M)~~ *(DONE: bead `bd-4ktq.14`; `Hex`/`Oct` preserve negative fixed-integer subtype width for Integer/Long/LongLong instead of widening every negative value to 64-bit; covered by `hex_oct_negative_width_vm3`)* · ~~trim-strips-all-whitespace(S)~~ *(DONE: bead `bd-4ktq.13`; `Trim`/`LTrim`/`RTrim` now remove U+0020 spaces only, preserving tabs and other non-space whitespace; covered by `trim_space_only_vm3`)* ·
~~string-charcode-mod256(S)~~ *(DONE: bead `bd-4ktq.12`; `String(number, numericCharacter)` folds numeric character codes greater than 255 with `Mod 256`, while string character arguments still repeat their first character; covered by `string_repeat_charcode_vm3`)* · ~~val-incomplete-parse(M)~~ *(DONE: bead `bd-4ktq.29`; `Val` now strips ASCII spaces/tabs/newlines before parsing, keeps the existing `&H`/`&O` radix sign-rule path, and parses the longest complete decimal token so `12-3` -> 12, `1.2.3` -> 1.2, incomplete `1e`/`1e+` -> 1, complete `E`/`D` exponents work, and nonnumeric starts still return 0. Evidence: `docs/evidence/conformance/vm3_val_oracle_20260701T1420Z/`; coverage: `val_incomplete_parse_vm3` plus oxvba-lib unit test.)* · ~~sqr-log-exp-nan-no-error(M)~~ *(DONE: bead `bd-4ktq.15`; `Sqr`/`Log` now raise error 5 for invalid domains and `Exp` raises overflow 6 instead of returning `NaN`/`Infinity`; covered by `math_domain_errors_vm3`)* ·
~~round-negative-digits-clamped(S)~~ *(DONE: bead `bd-4ktq.16`; `Round` now accepts negative `numdecimalplaces` and rounds at tens/hundreds/etc. with half-even behavior instead of clamping to integer rounding; covered by `round_negative_digits_vm3`)* · ~~vartype-typename-array-element(S)~~ *(DONE: bead `bd-4ktq.18`; `VarType`/`TypeName` now report typed SAFEARRAY element VARTYPE metadata, e.g. `Integer()` -> 8194/`Integer()`, while `Array(...)` remains 8204/`Variant()`; covered by `array_introspection_vm3`)* ·
~~nothing-represented-as-empty(M)~~ *(DONE: bead `bd-4ktq.35`; `Nothing` now materializes as a null `Object` Variant instead of `Empty`, so `VarType(Nothing)=9`, `TypeName(Nothing)="Nothing"`, `IsObject(Nothing)=True`, and `IsEmpty(Nothing)=False` while `Empty` remains `0/Empty/False/True`. `Set v = Nothing` for Variant stores the null object; `v = Nothing` under `On Error Resume Next` raises 91 and leaves `v` Empty. Unset/null object value-context coercion, arithmetic, and comparison now raise 91 instead of falling through as `Empty` or type mismatch. Evidence: `docs/evidence/conformance/vm3_nothing_oracle_20260701T151239Z/` plus existing conformance oracle row `operator_arithmetic_object_plus_error.bas`; coverage: `nothing_vs_empty_vm3` and vm3 golden rows.)* · ~~weekday-ignores-firstdayofweek(S)~~ *(DONE: bead `bd-4ktq.17`; `Weekday(date, firstdayofweek)` now rotates the returned 1-based day number relative to the requested first day instead of always using Sunday-first numbering; covered by `weekday_firstday_vm3`)* ·
~~now-date-time-utc-not-local(M)~~ *(DONE: bead `bd-4ktq.33`; native HAL `Date`, `Time`, `Now`, and `Timer` now use the host local civil clock instead of UTC day/modulo math; `FileDateTime` uses the same local serial conversion so fresh-file timestamps stay aligned with `Now`. Deterministic time constants are unchanged. Covered by `oxvba-hal` native/deterministic time tests and `filesystem_statements` FileDateTime regression.)* · ~~hal-errors-flattened-to-5(M)~~ *(DONE: `From<HalError> for LibError` now preserves `host_error_code` instead of hardcoding invalid_call(5); COM activation faults set it — `CreateObject` fail→429, `GetObject` running-instance/create→429, file-bind→432, all live-verified. The dispatch path already threaded it via `Fault::from_hal`. getobject_vm3 now asserts 429/432; golden no-drift)* · ~~resume-0-fails-elaboration(S)~~ *(DONE: bead `bd-4ktq.25`; binder maps `Resume 0` to the same `ErrorOp::Resume` path as bare `Resume`, preventing undefined-label elaboration; covered by `resume_zero_vm3` with bare `Resume` control)* ·
~~sparse-default-error-message(M)~~ *(DONE: bead `bd-4ktq.28`; vm3 `default_error_message` now includes the live-Excel/VBA default descriptions for the common runtime codes already surfaced by VM/library/HAL paths, including 7, 10, 14, file errors 52-76, object/Automation errors 380-463, Collection duplicate-key 457, and late search/temp errors 735/744/746; unmapped custom codes still fall back to `Application-defined or object-defined error`. Evidence: `docs/evidence/conformance/vm3_default_error_message_oracle_20260701T1410Z/`; coverage: `default_error_messages_vm3` exercises both `Error n` and omitted-description `Err.Raise n` plus fallback.)* · ~~err-properties-not-writable(M)~~ *(DONE: bead `bd-4ktq.32`; `Err.Number`, `Err.Description`, and `Err.Source` now bind/lower/execute as error-state writes, with `Err.Description`/`Err.Source` writes making omitted `Err.Raise` fields inherit even when `Err.Number` is 0; `Err.LastDllError` remains read-only per live compile modal. Evidence: `docs/evidence/conformance/vm3_err_property_writes_oracle_20260701T1442Z/`; coverage: `err_property_writes_vm3`.)* ·
~~stop-statement-fails-to-bind(S)~~ *(DONE: bead `bd-4ktq.22`; headless vm3 binds `Stop` as a no-op debugger suspend point, with no `Err` mutation; covered by `stop_statement_vm3`)* · ~~end-statement-misparsed(M)~~ *(DONE: bead `bd-4ktq.26`; bare `End` now parses as `EndStmt`, binds as `CoreStmt::End`, and lowers to vm3 `Halt`; block terminator detection still reserves same-statement `End Sub`/`End If`/etc. for closures; covered by `end_statement_vm3` direct, callee, and following-keyword halt cases)* · ~~redim-nonconstant-lower-rejected(M)~~ *(DONE: bead `bd-4ktq.34`; explicit `ReDim a(lo To hi)` lower bounds now stay as runtime expressions through CoreIR/OxIR/vm3 instead of binding through a constant-only `i32`; vm3 coerces lower and upper operands at resize time. Covered by `redim_negative_lower_vm3` runtime lower-expression and `Option Base` controls.)* ·
~~redim-negative-lower-rejected(S)~~ *(DONE: bead `bd-4ktq.24`; explicit negative constant lower bounds now fold through unary negation for both dynamic `ReDim` and fixed `Dim`; covered by `redim_negative_lower_vm3`)* · ~~cdec-absent(M)~~ *(DONE: bead `bd-4ktq.27`; `CDec` is now exported from the VBA Conversion module, binds through the native catalog, and returns the existing `Decimal96` Variant subtype; covered by `cdec_conversion_vm3`, `null_coercion_vm3`, and the intentional `conversion_extended_scalar_subset.bas` golden flip from unresolved bind error to Decimal success)* · ~~fixed-string-scalar-init-empty(S)~~ *(DONE: bead `bd-4ktq.23`; fixed-length scalar strings now default through the existing fixed-string store coercion, so local and module-level `String * N` slots initialize as `N` spaces; covered by `fixed_string_default_vm3` with assignment controls)*

## Tier 4 — Medium/Low (less common or larger)

for-start-step-not-coerced(M) · command-absent(S) · print-separators-zones(L — PARTIAL: `;` adjacency + `,` within-statement 14-col zones + trailing-separator newline suppression done with the file-I/O cluster; REMAINING: cross-statement print-column continuation after a suppressed newline, the leading sign space on numbers, and `Tab(n)`/`Spc(n)`) ·
input-no-date-null-parse(S) · predeclared-singleton-no-resurrection(M) · datediff-w-day-count(S) ·
datediff-datepart-ww-ignore-firstday(M) · negative-date-serial-floor(S) · date-range-not-validated(S) ·
date-string-parser-inconsistent(M) · cstar-null-error-13-not-94(S) · pow-negative-base-fractional-nan(S) ·
~~redim-multidim-count-overflow~~ *(done w/ redim bead — checked_mul → Err 7)* · object-default-member-index-get(M) · object-default-member-index-set(M) ·
left-right-mid-index-by-char(M) · ~~mid-start-less-than-1-clamped(S)~~ *(DONE: bead `bd-4ktq.30`; `Mid(string, start, ...)` and statement-form `Mid(target, start, ...) = value` now raise runtime error 5 for `start < 1` instead of clamping zero to the first character; positive start, overlarge function start, and valid statement replacement controls remain unchanged. Evidence: `docs/evidence/conformance/vm3_mid_start_oracle_20260701T1425Z/`; coverage: `mid_start_vm3` plus oxvba-lib unit test.)* · error-function-unsupported(M) ·
next-multivariable-unsupported(M) · line-number-labels-no-colon(M) · lset-rset-unrecognized(L) ·
fixed-string-udt-field-layout(L) · foreach-project-class-no-newenum(L) · paramarray-elements-byval(L) ·
sub-in-expression-accepted(S) · redim-undeclared-rejected(M) · class-terminate-not-synchronous(L,risky) ·
dim-as-new-no-resurrection(L,risky)

## Tier 5 — Low (rare / cosmetic / matches headless)

empty-plus-numeric-promotes-double · empty-plus-string-type-mismatch · is-operator-non-object-no-error ·
hms-round-crosses-boundary · array-byval-accepted-lost · array-assign-into-fixed-lhs ·
isarray-unallocated-false · raiseevent-fan-out-order · width-statement-no-wrap ·
erl-absent · err-helpfile-helpcontext-dropped · on-error-undefined-label-malformed ·
on-goto-out-of-range-no-5 · exit-do-in-while-accepted · numeric-string-parse-rust-f64 ·
leftb-rightb-byte-fns-absent · sendkeys-appactivate-absent · debug-assert-no-break ·
cc-constants-hardwired-64bit · sgn-nan-double · option-explicit-awareness ·
strconv-byte-modes-passthrough · shell-blocks-until-exit · resolve-library-import-dead-guard

## Risk notes (verify before/while implementing)

- **class-terminate-not-synchronous** and **dim-as-new-no-resurrection** are large object-model
  changes with tests/comments that PIN the current behavior as accepted residuals — confirm intent
  (and live-oracle behavior) before changing; they ripple through the golden.
- **integer-literal-surfaces-as-long**, **statement-call-paren-not-byval**, **byref-type-mismatch**,
  **no-call-arity-validation** change pervasive semantics → expect broad golden churn; re-bless
  carefully and sanity-check each changed line.
- Where a fix changes the golden, re-bless with `OXVBA_BLESS_GOLDEN=1` and confirm every changed
  line is an intended correctness improvement (not a regression).
