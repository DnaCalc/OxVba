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

### Critic-added gaps (15)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☐|intra-project-private-not-enforced|High/SilentWrong|M|`Private` members of one module resolve (unqualified + `Module.Foo`) from other modules same project|`MemberEntry` carries no visibility (oxvba-symbol providers/project.rs:17-20,53,136-150,90-99); thread `member.visibility`, exclude Private from the unqualified `public` map + `resolve_owner_member` unless requesting module is the declarer|
|☐|ambiguous-name-not-detected|High/SilentWrong|M|duplicate `Public` members across modules silently resolve to the first scanned (no "Ambiguous name detected")|project.rs:104 `candidates.first()`; when the candidate Vec spans >1 module, surface AmbiguousName|
|☐|date-arith-loses-date-type|High/SilentWrong|M|`Date + number` / `Date - number` yield Double/Variant (Date subtype + TypeName lost); `Date - Date` is Double|types.rs:225-243 (numeric_rank None for Date → Variant), arithmetic.rs:50-63; add Date to the +/- lattice + runtime lane (compute on serial, re-tag Date)|
|☐|currency-single-float-suffix-literals|Med/SilentWrong|S|`@`/`!` float-suffix literals typed Double (Currency/Single subtype + Currency exactness lost)|lexer.rs:339-340 collapses !/#/@→FloatLiteral; expr.rs:63-66 all→F64; emit CoreConst::Currency for `@`, Single for `!`|
|☐|implicit-string-to-boolean-13|Med/SilentWrong|S|`Dim b As Boolean: b = "True"` raises 13 (explicit `CBool` works) — implicit/explicit diverge|oxvba-eval arith.rs:483-488 coerce_numeric Boolean arm → num() f64::parse; special-case "true"/"false" (share pure::cbool)|
|☐|currency-mul-f64-lossy|Med/SilentWrong|M|`Currency * Currency` computes in f64, diverging near the ±922,337,203,685,477.5807 boundary|oxvba-eval arith.rs:170-199,523-530,114-127; exact scaled-i64/i128 lane for Currency * and +/-|
|☐|module-name-public-member-collision|Med/SilentWrong|M|a name used both as a module name and a project-level Public member is never diagnosed (VBA: ambiguous)|project.rs:90-97; detect module-name == unqualified Public member at env build|
|☐|rgb-qbcolor-absent|Med/Absent|S|`RGB`/`QBColor` color functions absent (color *constants* exist)|native.rs/catalog.rs add Rgb(3,3)/QbColor(1,1) pure bodies|
|☐|format-number-family-absent|Med/Absent|M|`FormatNumber`/`FormatCurrency`/`FormatPercent`/`FormatDateTime` absent (only generic `Format`)|build on format.rs; named-format consts already exist (vba_library.rs:315-319) unconsumed|
|☐|financial-ipmt-ppmt-sln-syd-ddb-absent|Med/Absent|M|`IPmt`/`PPmt`/`SLN`/`SYD`/`DDB` absent (FV/PV/Pmt/NPV/IRR/MIRR/Rate/NPer exist)|native.rs:139-147 + catalog.rs:180-187 + pure bodies|
|☐|project-qualifier-ignored|Low/SilentWrong|S|`Project1.Module1.Foo` ignores a wrong/nonexistent project segment|project.rs:85 discards `_project`; validate vs active/referenced project names|
|☐|partition-absent|Low/Absent|S|`Partition(number,start,stop,interval)` absent|native.rs/catalog.rs add Partition(4,4) pure body|
|☐|getsetting-family-absent|Low/Absent|M|`GetSetting`/`SaveSetting`/`GetAllSettings`/`DeleteSetting` absent|route to a settings HAL facet (headless no-op) or HonestDecline|
|☐|vbmodal-vbmodeless-absent|Low/Absent|S|`vbModal`(1)/`vbModeless`(0) `Show`-modality constants absent (MsgBox modal consts exist)|vba_library.rs:294-295 add the two arms|
|☐|friend-on-standard-module|Low/SilentWrong|S|`Friend` accepted on standard-module members (VBA: class-only; compile error otherwise)|scanner.rs:641-646; reject Friend when module_kind is Procedural|

## Tier 0 — user-named deferred beads (do first)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☑|redim-fixed-array-reject|Med/SilentWrong|S|`ReDim` of a fixed array silently re-dimensions instead of erroring|runtime guard in `array_redim` on `is_fixed_size()` → Fault 10 *(done; 6 corpus progs fixed to valid dynamic arrays)*|
|☑|erase-fixed-array-in-variant-element-type|Med/SilentWrong|M|`Erase` of a fixed array in a Variant slot re-defaults to Variant/Empty, flips element type|bind-site element unless Variant, then `array_element_type_for_vartype(element_vartype())` *(done)*|
|⊘|addressof-native-callback-thunk|Low/HonestDecline|L|`AddressOf`→native callback slot declines (vm3 lib.rs:3046)|**DECISION (keep the honest decline for now):** it is *correct*, just unimplemented — no in-scope corpus program exercises it, and it is the only capability that would need a large UB-adjacent unsafe facility (thunk pool + reentrant `*mut Vm3` re-entered from a C trampoline mid-FFI). Intra-VBA `AddressOf`/`CallProcRef` already works. Design when a real program needs it: substitute the thunk address as the `LongPtr` arg (HAL unchanged — it marshals `LongPtr` as a pointer-sized int), copy out `self.host` so the VM is unborrowed across the FFI, re-enter via `run_proc_with_values`. Sync-only (EnumWindows-style); async (SetTimer) additionally needs a message pump + thunk outliving the call.|
|☑|getobject-absent|Low/Absent|M|`GetObject` not bindable|catalog SpecialForm + `Native(GetObject)` route; `host::get_object` → `ComHal::get_object_variant`; HAL 3-mode dispatch (omitted→`GetActiveObject`, ""→`CreateObject`, path→`CoGetObject`); bridge `get_active_object`/`bind_file_object`. LIVE-verified (Dictionary new-instance + running Excel). *(done)* NOTE: miss surfaces as Err 5 not 429 pending `hal-errors-flattened-to-5` (HRESULT preserved in the fault message)|

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
|☐|date-to-string-emits-serial|High/SilentWrong|M|Date→String emits raw serial, not formatted|General-Date formatter in `variant_to_vba_string`/`print_display_text`|
|☑|datevalue-cdate-of-date-raises-13|High/SilentWrong|S|`DateValue`/`CDate` of Date/numeric raises 13|vtype dispatch in `date_value`; + calendar validation (Feb 30 → 13) *(done; CDate already dispatched)*|
|☑|redim-preserve-multidim-corrupt|High/SilentWrong|M|multi-dim `ReDim Preserve` flat-copies (corrupts)|coordinate-aware copy *(done w/ redim bead)*|
|☑|redim-preserve-no-dimension-guard|High/SilentWrong|M|`ReDim Preserve` doesn't enforce only-last-dim (no Err 9)|compare new vs old bounds *(done w/ redim bead)*|
|☐|option-base-1-ignored|High/SilentWrong|M|`Option Base 1` ignored (arrays always 0-based)|thread module base into `bind_array_bounds`/Array()|
|☐|statement-call-paren-not-byval|High/SilentWrong|M|`Foo(x)` statement call doesn't force ByVal|carry paren-arg flag parser→`bind_one_arg`|
|☐|byref-type-mismatch-accepted|High/SilentWrong|M|`ByRef` type mismatch silently accepted+retypes caller|bind-time reject when ByRef param type ≠ l-value type|
|☐|no-call-arity-validation|High/SilentWrong|S|extra args dropped; missing→late wrong error|arity check in `bind_proc_args`|
|☑|instr-leading-start-by-type|High/SilentWrong|S|`InStr` start detected by TYPE not arity|now arity-based: 2 args=(s1,s2), 3–4 args=(start,s1,s2,[compare]). Fixed 3 corpus progs (`InStr(12345,34)` was err 5, now 3) *(done)*|
|☑|instrrev-ignores-start-compare|High/SilentWrong|M|`InStrRev` ignores start, misreads compare|dedicated `instr_rev` with `(stringcheck,stringmatch,[start=-1],[compare])` layout: honours `start` (search region) + `compare` at arg 4 *(done)*|
|☐|split-ignores-limit-compare|High/SilentWrong|S|`Split` ignores limit+compare|implement limit/compare|
|☐|option-compare-text-ignored-string-fns|High/SilentWrong|M|`Option Compare Text` ignored by InStr/StrComp/Replace/Filter/InStrRev|append synthesized compare-mode const in binder|
|☐|select-case-ignores-option-compare-text|High/SilentWrong|S|`Select Case` ignores Option Compare Text for strings|add mode to CaseClause, set from compare_mode|
|☑|mixed-string-numeric-compare-no-13|High/SilentWrong|M|String-vs-numeric compare returns value not Err 13|`cmp_order` guard: `String` (incl. numeric-looking) vs numeric/Boolean/Date → Err 13; `Empty`/`Null` exempt *(done; no golden drift)*|
|☑|and-or-imp-null-three-valued|High/SilentWrong|M|And/Or/Imp with Null always Null (no 3-valued logic)|`bitlogic` now evaluates the known operand against the unknown-as-0 vs unknown-as-(-1): agreeing bits survive (`False And Null`=False, `True Or Null`=True, Imp follows), else Null; Xor/Eqv always Null *(done; Not Null already Null)*|
|☐|null-not-propagated-string-fns|High/SilentWrong|M|string fns on Null raise 13 not Null (or 94 for `$`)|Null-propagation policy table|
|☑|typeof-nothing-raises-91|High/SilentWrong|S|`TypeOf Nothing Is X` raises 91 not False|early `Ok(false)` for Nothing/Empty/Null (and an unset/`Set Nothing` object var) in `type_of_is` *(done)*|
|☐|for-counter-no-overflow|High/SilentWrong|M|`For` counter increment never overflows (Widening)|Checked mode for fixed-type counters|
|☐|integer-literal-surfaces-as-long|High/SilentWrong|M|Integer literals are Long at runtime (VarType/TypeName)|OxConst::I16 carrier|
|☐|vba-hex-oct-literal-sign|High/SilentWrong|M|`&HFFFFFFFF`=4294967295 not -1 (no width sign)|width-based two's-complement in parse_radix (both copies)|
|☐|abs-int-fix-sgn-return-double|High/SilentWrong|M|Abs/Int/Fix/Sgn always Double; Sgn should be Integer; Abs overflow|type-aware math1|
|☐|seek-function-resets-position|High/SilentWrong|S|`Seek(n)` function resets position to 0|don't mutate when position arg omitted|
|☐|reset-bare-close-error-5|High/SilentWrong|S|`Reset`/bare `Close` raise spurious Err 5|push literal-0 handle for close-all|
|☑|print-nonstring-truncates-to-long|High/SilentWrong|S|`Print #` non-strings truncate to Long|now routed via `print_display_text` in `assemble_print_record` *(done w/ the file-I/O cluster)*|
|☐|seek-loc-zero-based|High/SilentWrong|M|Seek/Loc 0-based; VBA Seek 1-based, Loc mode-dependent|1-based logical position|

## Tier 3 — Medium SilentWrong / common BinderReject

collection-keynotfound-error-9-not-5(S) · foreach-com-failure-swallowed(M) ·
foreach-scalar-non-object-empty(S) · lbound-ubound-unallocated-error-13(S) ·
coerce-null-numeric-no-94(S) · hex-oct-negative-width(M) · trim-strips-all-whitespace(S) ·
string-charcode-mod256(S) · val-incomplete-parse(M) · sqr-log-exp-nan-no-error(M) ·
round-negative-digits-clamped(S) · vartype-typename-array-element(S) ·
nothing-represented-as-empty(M) · weekday-ignores-firstdayofweek(S) ·
now-date-time-utc-not-local(M) · hal-errors-flattened-to-5(M) · resume-0-fails-elaboration(S) ·
sparse-default-error-message(M) · err-properties-not-writable(M) · stop-statement-fails-to-bind(S) ·
end-statement-misparsed(M) · redim-nonconstant-lower-rejected(M) · redim-negative-lower-rejected(S) ·
cdec-absent(M) · fixed-string-scalar-init-empty(S)

## Tier 4 — Medium/Low (less common or larger)

for-start-step-not-coerced(M) · command-absent(S) · print-separators-zones(L — PARTIAL: `;` adjacency + `,` within-statement 14-col zones + trailing-separator newline suppression done with the file-I/O cluster; REMAINING: cross-statement print-column continuation after a suppressed newline, the leading sign space on numbers, and `Tab(n)`/`Spc(n)`) ·
input-no-date-null-parse(S) · predeclared-singleton-no-resurrection(M) · datediff-w-day-count(S) ·
datediff-datepart-ww-ignore-firstday(M) · negative-date-serial-floor(S) · date-range-not-validated(S) ·
date-string-parser-inconsistent(M) · cstar-null-error-13-not-94(S) · pow-negative-base-fractional-nan(S) ·
~~redim-multidim-count-overflow~~ *(done w/ redim bead — checked_mul → Err 7)* · object-default-member-index-get(M) · object-default-member-index-set(M) ·
left-right-mid-index-by-char(M) · mid-start-less-than-1-clamped(S) · error-function-unsupported(M) ·
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
