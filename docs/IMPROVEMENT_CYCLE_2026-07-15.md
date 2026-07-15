# OxVba Improvement Cycle — 2026-07-15

Purpose: a focused bug-fix / hole-fill / docs-and-tests improvement cycle to move the
compiler, VM3 interpreter, JIT, base library and support surfaces toward usable, complete
and maintainable long-term shape. This is a **distinct engineering track** from the
`ideal-2026-07` / `bd-59co` certification program (whose ready leaves are all blocked on
physical Windows/Excel certification infrastructure).

Method: each item was surfaced by a 13-finder fan-out survey and **adversarially verified at
`file:line` against the actual code** (survey run `wf_09d327f1-901`, 2026-07-15), plus one
item confirmed independently (lexer non-ASCII panic). Baseline at start: workspace builds
clean, full non-ignored test suite green (exit 0).

Execution discipline (per bead): re-confirm the defect in code → implement → add/adjust
tests → run the affected suites → fresh-eyes review for blunders/omissions → rework until
clean → commit. Label: `improve-2026-07`.

Legend: ☐ open · ◐ in progress · ☑ done

## Tier 1 — Robustness / host-crash (a guest VBA program must never abort the host)

- ☑ **B1 lexer-nonascii-panic** (syntax, HIGH) — `crates/oxvba-syntax/src/lexer.rs`
  fallthrough byte-slices non-ASCII chars → `i += 1` splits a UTF-8 char → `&source[start..i]`
  panics ("byte index not a char boundary"). Any non-ASCII char outside a string/comment/
  bracket — incl. **legal VBA Unicode identifiers** — crashes the compiler/host. Fix: accept
  Unicode letters as identifier chars; advance by full char width in the unknown-char arm.
- ☑ **B2 vm3-array-bounds** (vm3, HIGH — folds survey #1 + #10) — `crates/oxvba-vm3/src/lib.rs`
  (follow-up **B28** created: bring JIT subscript/ReDim to parity with the corrected vm3 oracle)
  (1) `ReDim v(0 To 2000000000)` → `(0..count).map(default_array_element).collect()`
  unbounded alloc → allocator abort (VBA raises catchable err 7); false doc at 2394-2396.
  (2) subscripts/bounds narrowed with bare `as i32` at ~2448/2458/2406/2411 → index ≥ 2^32
  wraps (SilentWrong element) / `ReDim a(2^32)` truncates to 0. Fix: range-check i64 against
  Long range → err 6/9; cap element count/bytes against a budget + `try_reserve` → Fault 7.
- ☑ **B3 oxir-array-elem-panic** (bundle, MED) — `crates/oxvba-bundle/src/array_runtime.rs:122/124`
  `default_array_element` `expect()`s record layout/alloc for a Record element; guest UDTs
  (>64 KiB, zero-sized field) `Err` → `ReDim arr(..) As BigUDT` aborts host. Sibling `NewRecord`
  maps identical failures to Fault(13). Fix: return `Result`, propagate `?` at resize_with sites.
- ☑ **B4 oxir-verify-loadpath** (oxir, MED — survey #13) — `crates/oxvba-oxir/src/image.rs:88`,
  `verify.rs:697` `verify_program` is dead outside tests; `from_bytes` runs only `validate()`,
  `link` only `validate_links()`; verifier omits `program.entry`/`global_initializer` bounds →
  a header-valid `.oxi` with an OOB entry/proc FuncId panics (OOB) at `new_frame_in`
  (vm3 lib.rs:900/940). Fix: run `verify_program` on load/link → `Malformed`; add entry bounds.
- ☐ **B5 jit-recursion-guard** (jit, HIGH, L — survey #2) — `crates/oxvba-jit/src/lib.rs:6254`
  every proc call lowers to a native `call`; deep self-recursion overflows the host thread
  stack (hardware overflow, not a Rust unwind — `catch_unwind` can't catch) → host abort; vm3
  raises err 28. Fix: bounded-guard execution thread converting overflow to seated err 28 (or
  iterative trampoline). Add a differential recursion test asserting both raise 28.

## Tier 2 — Correctness bugs (mostly SilentWrong)

- ☑ **B6 dateadd-clamp** (lib, HIGH, S — #8) — `crates/oxvba-lib/src/pure.rs:1363` DateAdd
  (also fixed the adjacent time-of-day drop in the same branch)
  "m"/"yyyy"/"q" reuses original day, no clamp → `DateAdd("m",1,#1/31/2021#)`→3/3/2021 (VBA
  2/28). Fix: clamp day to days-in-month before `ymd_to_serial`.
- ☑ **B7 single-string-precision** (runtime, HIGH, S — #6) — `crates/oxvba-runtime/src/coerce.rs:314`
  (shared 7-sig-digit `format_vba_single` for CStr + display paths; residual: large-magnitude
  scientific notation shares the B19 gap)
  Single arm does `format_vba_f64(f64::from(as_f32()))` → `CStr(CSng(0.1))`→"0.10000000149011612".
  Correct sibling `print_display_text:429` uses `as_f32().to_string()`. Fix: format from f32 directly.
- ✗ **B8 null-propagation** (lib, MED, S — #16) — **REFUTED, no change.** Survey #16 over-generalized
  from Abs/Int/Fix. `Sgn(Null)`=94 is *verified against live Office VBA 7.1* (existing
  `abs_int_fix_sgn_vm3` test), and `Round`/`Sgn` coerce their argument to a numeric type
  (Null→94), unlike the Variant-preserving Abs/Int/Fix (Null→Null). Added a documenting
  regression guard (`round_sgn_null_vm3`) so the false positive isn't re-applied.
- ☑ **B9 str-leading-zero** (lib, MED, S — #17) — `crates/oxvba-lib/src/pure.rs:1666` `Str(0.5)`→
  " 0.5" (VBA " .5"). Fix: strip a leading "0" before "." in the Str-specific path only (CStr unchanged).
- ☑ **B10 comhost-errnumber** (comhost, HIGH, M — #9) — `crates/oxvba-comhost/src/lib.rs:4017` &
  `engine.rs vm3_runtime_diagnostic ~174-185` hardcode EXCEPINFO scode 5; true `fault.code`
  discarded. Fix: thread `fault.code` through the diagnostic → `RuntimeCallError::new(code,…)`.
- ☑ **B11 format-string-operand** (lib, HIGH, M — #5) — `crates/oxvba-lib/src/format.rs:75/82-84`
  `num()`/`serial_of` coerce via Double (no String→numeric arm) → `Format("123.5","0.00")`→"0.00".
  Fix: parse String operands with `parse_vba_numeric_string`/date parser before defaulting to 0.
- ☑ **B12 lexer-datelit-overscan** (syntax, HIGH, M — #3) — `crates/oxvba-syntax/src/lexer.rs:263`
  `looks_like_date` treats any `#` with a later `#` on the line as a date → `Close #1: Close #2`,
  `Print #1, amount#` mis-lex a span as one DateLiteral. Fix: constrain interior (bail on `,` /
  non-date alpha); add a token-kind regression test.
- ☑ **B13 debugprint-semicolon** (syntax/bind, HIGH, M — #4) — `crates/oxvba-syntax/src/parser.rs:782`
  (two layers: `parse_bare_arg_list` dropped post-`;` items AND `arg_items` mis-segmented pre-`;`;
  residual: `;`-no-space vs `,`-tab rendering still deferred, shared with file Print)
  Debug.Print via `parse_bare_arg_list` only continues across Comma; `;` ends the list →
  `Debug.Print "x="; x` prints only "x=". Fix: accept `;` (and `,`) as print-item separators on
  the Debug.Print path; add a semicolon test.
- ☑ **B14 exact-carrier-compare** (eval, HIGH, M — #7) — `crates/oxvba-eval/src/arith.rs:640/649`
  (integer+Currency exact via i128@4dp; Decimal-vs-Decimal exact; residual: Decimal-vs-other-carrier
  and float mixes stay on f64)
  `cmp_order` compares via `read_f64` → LongLong/Currency/Decimal lose precision >2^53
  (`CLngLng(2^53+1)=CLngLng(2^53)`→True); inconsistent with exact i128 add/sub/mul. Fix: exact
  same-carrier fast paths before the f64 fallback.
- ◐ **B15 constfold-parity** (symbol, MED, M — #15) — `crates/oxvba-symbol/src/const_eval.rs`
  DONE 4/5: banker's rounding, Boolean preservation, Div/Pow non-finite guard, CC-Empty→0.
  Sub-item 5 (Long*Long overflow-widening) DEFERRED to **B29** — VBA's const-overflow semantics
  are unverifiable without a live oracle (same caution as B8).
  const folder diverges from runtime: `\`/`Mod`/bitwise round half-away vs `round_ties_even`
  (1596/1600); `True And True`→Long not Boolean (1622-1626); `1#/0#`→+Inf for Variant const (1618);
  overflowing Long*Long silently widens (1612-1614); undefined-CC `=0` folds Empty→false (1592, VBA
  Empty=0 True). Fix: mirror runtime int() (round_ties_even), preserve Boolean, guard Div/Pow,
  range-check Checked regime, coerce Empty→0 for CC compares.
- ☑ **B16 set-nonobject-424** (vm3, LOW, S — #18) — `crates/oxvba-vm3/src/lib.rs:2946`
  (surgical: gated `is_object` on the actual Object type rather than touching `is_nothing`'s 8 other callers) (is_nothing
  4992-4998) treats numeric 0 as Nothing → `Set o = 0` silently stores scalar (VBA err 424). Fix:
  drop the numeric-zero clause / gate the Set legality check.
- ☑ **B17 collection-index-subtypes** (eval, LOW, S — #19) — `crates/oxvba-eval/src/collection.rs:70`
  `variant_selector` tries only i16/i32/i64/f64 then `unwrap_or(0)` → `c.Item(CByte(1))` maps to
  index 0. Fix: extend to u8/i8/u16/u32/u64/f32 (or shared coercion) before defaulting.
- ☑ **B18 jit-single-overflow** (jit, MED, M — #11) — `crates/oxvba-jit/src/lib.rs:1791` two-Single
  (finite-check → error 6 via new `emit_overflow_if_not_finite`. Review caught: vm3 does NOT raise
  on **Double** overflow (yields Inf), so the JIT must match — extending to Double was reverted;
  the vm3 Double gap is tracked as **B30**.)
  fast path emits raw fadd/fsub/fmul, stores f32 with no finite check → `2E38+2E38`→+Inf silently;
  vm3 raises err 6. Fix: route checked Single through coerce_numeric(Single) / finite check; diff test.
- ⊘ **B19 format-scientific** (runtime, MED, M — #12) — **DEFERRED (blocked, needs oracle).**
  `format_vba_f64` never emits scientific notation, but the exact large/small thresholds and
  E-format need live-VBA pinning I can't do here; implementing unverified thresholds risks
  regressing currently-accidentally-correct values. `crates/oxvba-runtime/src/coerce.rs:378`
  `format_vba_f64` never emits exponent form → `CStr(1E20)`→"1000…0" (VBA "1E+20"). Fix: emit E+/E-
  for |x|≥~1E16 and small nonzero <~1E-4 with ~15-sig-digit cap; pin thresholds against live VBA.
- ☐ **B20 com-variant-subtype** (com, LOW, M — #20) — `crates/oxvba-com/src/windows_variant.rs:2036/2021`
  VT_I2/UI1/I1/UI2 flatten to I32 (TypeName "Long" vs "Integer"); VT_BYREF scalar match misses
  DATE/DECIMAL/UI1/I8/ERROR → spurious COM error. Fix: add small-int carriers; mirror byref arms.
- ◐ **B21 resumenext-latch** (vm3/rt-abi, MED, doc — #21) — Surfaced + cross-referenced the
  code(None)-vs-spec-R4(Some) discrepancy in BOTH `oxvba-rt-abi` and `OXIR_VM3_ERROR_MODEL.md`
  (a JIT author is now warned, not silently diverging). The actual resolution needs a live probe
  → **B31**. `crates/oxvba-rt-abi/src/lib.rs:3148`
  ResumeNext sets `active_error=None`, opposite to `docs/OXIR_VM3_ERROR_MODEL.md` R3/R4
  (=Some). Neither is test-covered. Resolve from spec+oracle-findings; align code or doc; add test.
  (Live-Excel probe unavailable here — resolve conservatively from existing evidence docs.)

## Tier 3 — Docs accuracy

- ☑ **B22 docs-jit-runtime** (docs, MED — #22) — `docs/BUILDING.md:14-17,36`, `CONFORMANCE.md:153`,
  `TESTING.md:53-54` describe the JIT as a "disabled skeleton" and the runtime as the deleted
  bytecode compiler/VM. Fix: rewrite to the real vm3/jit backends + oxir/vm3/differential surface.
- ☑ **B23 docs-vm2-refs** (docs, LOW — #23) — POST_CLEANUP.md superseded banner + pure.rs
  stale-Collection claim fixed + vm3 header note de-dangling its 67 refs + consumer/caller-list
  corrections (eval/runtime/bundle/hal). Residual ~23 "mirrors vm2's X" design-history breadcrumbs
  in oxir/bind/com are now contextualized repo-wide (a full prose de-attribution would be low-value
  churn — they accurately describe conventions; only the dead `vm2` name remains, now explained).
  Stale `vm2`/`oxvba-vm2` references across code
  comments (`oxvba-vm3` lib.rs ~68, bind, oxir, com, lib) + `POST_CLEANUP.md` present a deleted
  crate as live. Fix: sweep-replace with the actual current consumer; banner POST_CLEANUP.md.

## Tier 4 — Maintainability / tests

- ☐ **B24 descriptor-dedup** (code-shape, MED — #24) — `oxvba-vm3/src/lib.rs:83-330,531-621` vs
  `oxvba-rt-abi/src/lib.rs:237-614` duplicate ~15 runtime_* descriptor builders (already drifted)
  on the exact vm3-vs-JIT seam. Fix: promote rt-abi's cluster to `pub`, vm3 calls it, reconcile.
- ◐ **B25 cli-project-diag** (host-cli, LOW — #27) — DONE: ambiguous-project-dir now reports a
  distinct "ambiguous" diagnostic (was swallowed as "not found"), with tests. Residual (lower
  value, separate features): missing-file usage-banner→I/O diagnostic, convention-dir support via
  the canonical loader, recursive `<Import>`. `oxvba-cli/src/main.rs:257/260/406`,
  `oxvba-project/src/{load.rs:912,parse.rs:20/78}`: ambiguous dir reported as "no project file";
  missing file → usage banner+exit 2; convention dirs unsupported (dead canonical loader); single-
  level `<Import>`. Fix: propagate ambiguity Err; separate path-parse from load; recursive imports.
- ☐ **B26 comhost-tests** (test, MED, L — #25) — `crates/oxvba-comhost/src/lib.rs` 4339-line cdylib
  with zero tests. Fix: in-process tests for host-side logic (DISPPARAMS round-trips, descriptor→
  registration mapping, vtable-slot assignment) needing no COM apartment.
- ☐ **B27 module-splits** (code-shape, L — #26) — `oxvba-jit/src/lib.rs` (35,348 lines),
  `oxvba-vm3` exec ~975-line fn, `oxvba-differential` ~26k-line inline test mod, `oxvba-rt-abi`
  mixed concerns, CLI vs host dup parsers. Fix: behavior-preserving splits by concern + single
  source of truth for paired tables. (Deferred to last — high-churn, low functional payoff.)
