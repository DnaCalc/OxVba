# vm3 vs complete-VBA-spec gap inventory

Source: the `vm3-spec-gap-audit` workflow (14 spec-dimension finders → synthesis;
2026-06-29). Authority = **complete MS-VBAL spec + live Office VBA 7.1**, NOT vm2.
160 raw findings → **109 deduplicated items, 54 SilentWrong**. Several finders ran
empirical probes through the real vm3 pipeline; the completeness-critic stage died on a
connection error, so treat un-reverified claims with mild caution — **each bead re-confirms
the gap against the code before fixing**.

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

## Tier 0 — user-named deferred beads (do first)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☑|redim-fixed-array-reject|Med/SilentWrong|S|`ReDim` of a fixed array silently re-dimensions instead of erroring|runtime guard in `array_redim` on `is_fixed_size()` → Fault 10 *(done; 6 corpus progs fixed to valid dynamic arrays)*|
|☐|erase-fixed-array-in-variant-element-type|Med/SilentWrong|M|`Erase` of a fixed array in a Variant slot re-defaults to Variant/Empty, flips element type|drive reset from `SafeArray::element_vartype()` not bind-time element|
|☐|addressof-native-callback-thunk|Low/HonestDecline|L|`AddressOf`→native callback slot declines|VM-agnostic `CallbackRegistry` in oxvba-runtime + trampoline pool|
|☐|getobject-absent|Low/Absent|M|`GetObject` not bindable|mirror CreateObject pipeline; `ComHal::get_object_variant`; Err 429 on miss|

## Tier 1 — Critical SilentWrong (data loss / everyday code)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☐|print-write-only-first-field|Crit/SilentWrong|M|`Print #`/`Write #` emit only the FIRST field|`file_print`/`file_write` consume `args[1..]`; carry separators|
|☐|input-stmt-no-writeback|Crit/SilentWrong|L|`Input #` never writes parsed fields back to targets|dedicated binder emitting per-target `Assign = FileInput(handle)`|
|☐|line-input-no-writeback|Crit/SilentWrong|M|`Line Input #` never writes the line back|dedicated binder `strvar = FileLineInput(handle)`|
|☐|isdate-always-false|Crit/SilentWrong|S|`IsDate` False for all date strings|route through string-date parsers, not `coerce_to(..,Date)`|

## Tier 2 — High SilentWrong (common correctness bugs)

| # | id | sev/class | eff | gap | fix locus |
|---|----|-----------|-----|-----|-----------|
|☐|date-to-string-emits-serial|High/SilentWrong|M|Date→String emits raw serial, not formatted|General-Date formatter in `variant_to_vba_string`/`print_display_text`|
|☐|datevalue-cdate-of-date-raises-13|High/SilentWrong|S|`DateValue`/`CDate` of Date/numeric raises 13|vtype dispatch in `date_value`|
|☑|redim-preserve-multidim-corrupt|High/SilentWrong|M|multi-dim `ReDim Preserve` flat-copies (corrupts)|coordinate-aware copy *(done w/ redim bead)*|
|☑|redim-preserve-no-dimension-guard|High/SilentWrong|M|`ReDim Preserve` doesn't enforce only-last-dim (no Err 9)|compare new vs old bounds *(done w/ redim bead)*|
|☐|option-base-1-ignored|High/SilentWrong|M|`Option Base 1` ignored (arrays always 0-based)|thread module base into `bind_array_bounds`/Array()|
|☐|statement-call-paren-not-byval|High/SilentWrong|M|`Foo(x)` statement call doesn't force ByVal|carry paren-arg flag parser→`bind_one_arg`|
|☐|byref-type-mismatch-accepted|High/SilentWrong|M|`ByRef` type mismatch silently accepted+retypes caller|bind-time reject when ByRef param type ≠ l-value type|
|☐|no-call-arity-validation|High/SilentWrong|S|extra args dropped; missing→late wrong error|arity check in `bind_proc_args`|
|☐|instr-leading-start-by-type|High/SilentWrong|S|`InStr` start detected by TYPE not arity|disambiguate by arg count|
|☐|instrrev-ignores-start-compare|High/SilentWrong|M|`InStrRev` ignores start, misreads compare|own arg layout|
|☐|split-ignores-limit-compare|High/SilentWrong|S|`Split` ignores limit+compare|implement limit/compare|
|☐|option-compare-text-ignored-string-fns|High/SilentWrong|M|`Option Compare Text` ignored by InStr/StrComp/Replace/Filter/InStrRev|append synthesized compare-mode const in binder|
|☐|select-case-ignores-option-compare-text|High/SilentWrong|S|`Select Case` ignores Option Compare Text for strings|add mode to CaseClause, set from compare_mode|
|☐|mixed-string-numeric-compare-no-13|High/SilentWrong|M|String-vs-numeric compare returns value not Err 13|`cmp_order` mismatch guard (Empty exempt)|
|☐|and-or-imp-null-three-valued|High/SilentWrong|M|And/Or/Imp with Null always Null (no 3-valued logic)|special-case Null in and/or/imp|
|☐|null-not-propagated-string-fns|High/SilentWrong|M|string fns on Null raise 13 not Null (or 94 for `$`)|Null-propagation policy table|
|☐|typeof-nothing-raises-91|High/SilentWrong|S|`TypeOf Nothing Is X` raises 91 not False|early `Ok(false)` for Nothing/Empty/Null in `type_of_is`|
|☐|for-counter-no-overflow|High/SilentWrong|M|`For` counter increment never overflows (Widening)|Checked mode for fixed-type counters|
|☐|integer-literal-surfaces-as-long|High/SilentWrong|M|Integer literals are Long at runtime (VarType/TypeName)|OxConst::I16 carrier|
|☐|vba-hex-oct-literal-sign|High/SilentWrong|M|`&HFFFFFFFF`=4294967295 not -1 (no width sign)|width-based two's-complement in parse_radix (both copies)|
|☐|abs-int-fix-sgn-return-double|High/SilentWrong|M|Abs/Int/Fix/Sgn always Double; Sgn should be Integer; Abs overflow|type-aware math1|
|☐|seek-function-resets-position|High/SilentWrong|S|`Seek(n)` function resets position to 0|don't mutate when position arg omitted|
|☐|reset-bare-close-error-5|High/SilentWrong|S|`Reset`/bare `Close` raise spurious Err 5|push literal-0 handle for close-all|
|☐|print-nonstring-truncates-to-long|High/SilentWrong|S|`Print #` non-strings truncate to Long|route via `print_display_text`|
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

for-start-step-not-coerced(M) · command-absent(S) · print-separators-zones(L) ·
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
