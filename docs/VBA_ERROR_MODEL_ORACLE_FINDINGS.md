# VBA Error / Control-Flow Model — Excel Oracle Findings

**Status:** living reference. Captured by running probes on **live Excel 16.0 VBA** (the
oracle), cross-referenced against **MS-VBAL** (the spec) and **vm2** (the current
interpreter, which we do *not* fix). The implementation rule for OxVBA is fixed:

> **Excel/VBA oracle > spec > vm2.** vm3 must be 100% oracle-compliant. Where the
> oracle deviates from the spec, the oracle wins and the deviation is flagged here as
> high-profile. Where vm2 deviates from the oracle, that is a known vm2 non-compliance
> (recorded, not fixed).

The harness, probes, and raw results live in [`oracle/`](../oracle): `run_oracle.ps1`
(injects a `.bas` of `Function PROBE_*() As String` probes into a throwaway workbook,
runs each, records the returned string), `probes.bas` (error handling), and
`probes_controlflow.bas` (GoTo / For / GoSub / Select / Do). Results:
`oracle/results/error_handling.json`, `oracle/results/controlflow.json`.

---

## 1. Running VBA as an oracle — hard-won harness notes

Driving Excel VBA from COM is full of traps; these cost real time, so they are recorded
for reproducibility (the harness now handles all of them automatically):

1. **Macro execution must be enabled.** `HKCU\…\Office\16.0\Excel\Security\VBAWarnings`
   must be `1` (enable all) — the default `2` ("disable with notification") blocks
   injected macros under automation (error: *"Cannot run the macro … macros may be
   disabled"*). `AccessVBOM=1` (already set here) only permits *injection*, not
   *execution*. The harness sets `VBAWarnings=1` per run (restore to the original at the
   end of an oracle session).
2. **A crash poisons the next launch.** A VBA crash leaves Excel in post-crash safe mode
   with `…\Excel\Resiliency\DisabledItems` / `DocumentRecovery` keys, after which macros
   silently won't run. The harness deletes those keys + kills stray `EXCEL.EXE` before
   every run.
3. **`Resume` / `Resume Next` after a CROSS-CALL propagated error crashes Excel when the
   handler is the frame `Application.Run` invokes directly** (the COM top frame) — RPC
   failure, not a dialog. This is a **COM-invocation artifact, not VBA behaviour**: the
   identical code one frame down runs fine. The harness runs every such probe via an
   inner `Private Function` (one frame below the COM entry).
4. **Bare `Resume` (re-run) after a cross-call propagated error genuinely wedges/crashes
   Excel even one frame down** — see §5 (degenerate, excluded). (`Resume Next` is fine.)
5. **A compile error surfaces as "macro not available" with no dialog** via
   `Application.Run` (the VBE compile-error dialog only appears in interactive use). To
   read a live VBE dialog, `oracle/read_live_dialog.ps1` uses UI Automation (technique
   from govert's gist) to read the message + dismiss it before COM cleanup (an open
   dialog during `Quit` causes RPC failures).
6. **`Fix` is a VBA intrinsic** — a parameter/variable named `fix` makes the enclosing
   procedure fail to compile, and *callers* then report "Sub or Function not defined".
   (General trap: avoid intrinsic names — `Fix`, `Date`, `Error`, `Err`, `Left`, … — as
   identifiers in probes.)

---

## 2. Error-handling oracle results

| Probe | Oracle result | Meaning |
|---|---|---|
| `oe_resume_next` | `errnum=11;desc=Division by zero` | `On Error Resume Next` catches `1/0`; `Err.Number=11`, `Err.Description="Division by zero"`. |
| `oe_goto_label` | `handler;errnum=13` | `On Error GoTo H` transfers to the handler with `Err` populated. |
| `oe_goto0_disables` | `caught=7` | `On Error GoTo 0` in a callee disables its handler → the fault propagates to the caller. |
| `oe_resume_next_resets_err` | `after_raise=9;after_rearm=0` | An `On Error Resume Next` statement **resets `Err`** (9 → 0). |
| `err_reset_on_onerror0` | `after_raise=5;after_onerror0=0` | `On Error GoTo 0` **resets `Err`** (5 → 0). |
| `oe_goto_minus1` | `H1(11);H2(20);H3(20);H4(20);GUARD` | `On Error GoTo -1` **clears the active error**; the subsequent `Resume Next` then has no active error → **error 20** (re-caught, hence the loop until the guard). |
| `resume_without_error` | `errnum=20` | `Resume` with no active error → **run-time error 20** ("Resume without error"). |
| `resume_reruns_faulting_stmt` | `x=50;n=1` | `Resume` re-runs **only the faulting statement** (`x = 100\d`), not the preceding one (`n` stays 1). |
| `resume_next_continues` | `before;handler;after;errnum=0` | `Resume Next` continues at the statement **after** the fault; **`Err` is cleared** (0). |
| `resume_label` | `before;handler;errnum=0` | `Resume <label>` jumps to the label (skips intervening statements); `Err` cleared. |
| `resume_clears_err` | `in_handler=5;after_resume_next=0` | **An explicit `Resume`/`Resume Next` clears `Err`** (5 in the handler → 0 after). |
| `err_raise_full` | `n=5;src=MySrc;desc=MyDesc` | `Err.Raise 5,"MySrc","MyDesc"` **honors all arguments**. |
| `err_description_derivation` | `desc=Division by zero` | `Err.Raise 11` with no description → `Err.Description` **derived from the number**. |
| `err_source_default` | `src=[VBAProject]` | `Err.Raise 5` with no source → `Err.Source` **defaults to the project name** `"VBAProject"`. |
| `err_raise_omitted_inherit` | `n=6;src=VBAProject;desc=Overflow` | (An intervening `On Error` reset `Err`, so omitted args fell to defaults; see note below.) |
| `err_clear` | `before=5;after_clear=0` | `Err.Clear` zeroes `Err`. |
| `err_persists_after_clean_stmt` | `errnum=5` | `Err` **persists across a non-faulting statement** (only cleared by the rules above). |
| `prop_callee_to_caller` | `caller-caught;errnum=6` | An unhandled callee fault **propagates to the caller's handler**. |
| `reraise_in_handler_propagates` | `caller-caught;errnum=5` | **A re-raise inside an active handler propagates to the caller** (it does not re-enter the same handler). |
| `resume_next_after_prop` | `before;handler;after;errnum=0` | `Resume Next` after a cross-call propagated error continues after the **call** statement in the caller; `Err` cleared. |
| `nested_onerror_restore` | `caller-caught;errnum=42` | A callee's `On Error` does **not** leak; the caller's handler is **restored on return**. |
| `exit_sub_clears_err` | `after_call_errnum=5` | **`Exit Sub` does NOT clear the `Err` object** (5 persists). See §4 — likely spec deviation. |
| `end_sub_err_persists` | `after_call_errnum=5` | Normal `End Sub` does not clear `Err` either (5 persists). |

> Note on `err_raise_omitted_inherit`: the probe placed `On Error Resume Next` between
> the two raises, which **resets `Err`** (per `oe_resume_next_resets_err`), so the second
> `Err.Raise 6` had nothing to inherit and fell to defaults (`src=VBAProject`,
> `desc=Overflow`-derived). The MS-VBAL §6.1.3.2.1.2 "omitted args reuse un-cleared `Err`
> fields" rule needs a probe **without** an intervening `On Error`/`Resume`/`Clear` —
> a TODO refinement.

---

## 3. Control-flow oracle results

| Probe | Oracle result | Meaning |
|---|---|---|
| `cf_goto_forward` | `a;b;` | `GoTo` skips the intervening statement. |
| `cf_goto_backward` | `1;2;3;` | Backward `GoTo` loops (guarded by a counter). |
| `cf_goto_out_of_for` | `s=1;2;3;i=3` | `GoTo` out of a `For` leaves the counter at its current value (3). |
| `cf_for_counter_after` | `loop=1;2;3;final_i=4` | After `For i=1 To 3`, the counter is **end+step = 4**. |
| `cf_for_step` | `loop=1;4;7;10;final_i=13` | `Step 3`: iterates 1,4,7,10; final `i = 10+3 = 13`. |
| `cf_for_step_neg` | `loop=5;3;1;final_i=-1` | `Step -2`: 5,3,1; final `i = 1+(-2) = -1`. |
| `cf_for_never_runs` | `s=x;final_i=5` | `For i=5 To 1` (step 1) runs **0 times**; the counter is set to `start` (5) and left there. |
| `cf_for_bound_evaluated_once` | `loop=1;2;3;final_i=4` | The limit is **evaluated once** at entry; mutating `n` inside does not extend the loop. |
| `cf_for_modify_counter` | `loop=2;4;6;final_i=7` | Mutating the counter inside the body **is respected** (1→2,3→4,5→6, then 7>5 exits). |
| `cf_exit_for` | `s=1;2;final_i=3` | `Exit For` leaves the counter at the exit value (3). |
| `cf_nested_for_exit` | `s=1.1;2.1;i=3;j=2` | `Exit For` exits the **inner** loop only; outer continues. |
| `cf_gosub_return` | `a;b;c;` | `GoSub`/`Return` runs the subroutine and resumes after the `GoSub`. |
| `cf_gosub_twice` | `x;x;` | Two `GoSub`s to the same target. |
| `cf_on_n_goto` | `L2;` | `On 2 GoTo L1,L2,L3` branches to the **2nd** label. |
| `cf_on_n_goto_zero` | `fell;` | `On 0 GoTo …` takes **no** branch (falls through). |
| `cf_on_n_gosub` | `S2;end` | `On 2 GoSub …` calls the 2nd target and returns. |
| `cf_select_case` | `r=mid` | `Case 4 To 9` matches `7`; ranges + `Is >` + `Case Else` work. |
| `cf_select_first_match_wins` | `r=A` | The **first** matching `Case` wins (overlapping cases). |
| `cf_do_while` | `s=1;2;3;i=3` | `Do While` pre-tested loop. |
| `cf_do_loop_until` | `s=1;2;3;i=3` | `Do … Loop Until` post-tested loop. |
| `cf_exit_do` | `s=1;2;i=3` | `Exit Do` breaks the loop. |
| `cf_while_wend` | `s=1;2;3;i=3` | `While … Wend`. |
| `cf_for_each_array` | `s=10;20;30;` | `For Each` over `Array(10,20,30)`. |

---

## 4. Oracle vs SPEC deviations (HIGH-PROFILE — implementation follows the oracle)

These are the most interesting findings: where **live Excel diverges from MS-VBAL / the
commonly-cited VBA documentation**. vm3 follows the **oracle**.

- **`Exit Sub` / `Exit Function` do NOT clear the `Err` object.** Microsoft documentation
  commonly states `Err.Clear` is invoked automatically by `Exit Sub`/`Exit Function`/
  `Exit Property`. The oracle disagrees: after a sub raises (and its own
  `On Error Resume Next` catches) an error and then `Exit Sub`s, the caller still observes
  `Err.Number = 5` (`exit_sub_clears_err`). So **the `Err` object survives `Exit Sub`**.
  → vm3: do **not** clear `Err` on `Exit`. (Possible reconciliation: the spec text may
  intend `Exit` to clear the *active-error latch*, not the *`Err` object*; the
  active-error/`Err`-object distinction matters — see §6. Needs exact-spec-text
  cross-check, but the observable is settled.)
- **Only an *explicit* `Resume`/`Resume Next`/`Resume <label>` clears `Err`; the implicit
  error-skip of `On Error Resume Next` does NOT.** `resume_clears_err` shows the explicit
  statement clears `Err`; `oe_resume_next` / `exit_sub_clears_err` show that when
  `On Error Resume Next` *skips* a faulting statement, `Err` **remains set** (the whole
  point — you test `Err.Number` after). vm3 must implement this asymmetry.

(Both are subtle enough that they are easy to get wrong; they are the headline reasons
this oracle pass exists.)

---

## 5. Oracle vs vm2 — known vm2 non-compliances (NOT fixed; vm3 follows the oracle)

vm2 is the legacy interpreter; we do not fix it. The differential/conformance harness
should flag these as expected vm2 divergences:

| Behaviour | Oracle (→ vm3) | vm2 |
|---|---|---|
| `Resume` with no active error | **error 20** | silent (uses stale resume target {0,0}) |
| `On Error …` statement | **resets `Err`** | does not reset |
| `Resume`/`Resume Next`/`Resume label` | **resets `Err`** | does not reset |
| `On Error GoTo -1` | **clears the active error** | unsupported |
| `Err.Raise n, Source, Description` | **honors Source + Description** | keeps only `Number` |
| `Err.Source` when omitted | **`"VBAProject"`** (project name) | `""` |
| `Err.Description` when omitted | derived from the number | `default_error_message` table (partial) |
| Re-raise inside an active handler | **propagates to the caller** | risks re-entering the same handler (route_fault keeps `Goto` armed) |

---

## 6. Degenerate / excluded cases

- **Bare `Resume` (re-run) after a CROSS-CALL propagated error reproducibly
  wedges/crashes Excel itself**, even when the handler is one frame down. This is Excel
  instability, not a behaviour to mirror — excluded from the runnable corpus. (Same-frame
  `Resume` re-run is fine: `resume_reruns_faulting_stmt`. `Resume Next` after cross-call
  propagation is fine: `resume_next_after_prop`.) vm3 should behave *defined* here (it
  will simply re-run the call-site statement), never crash.

---

## 7. The model vm3 must implement (oracle contract)

State per procedure activation: an **error-handling policy** (`None` / `ResumeNext` /
`Goto(label)`), an **active-error latch** (the faulting statement's resume/resume-next
targets, set when a handler catches), saved/restored across calls. Plus a **global `Err`
object** (`Number`/`Description`/`Source`/…).

1. `On Error GoTo L` / `Resume Next` / `GoTo 0` / `GoTo -1` set the policy; **every
   `On Error` statement resets the `Err` object**. `GoTo -1` additionally clears the
   active-error latch but keeps the handler.
2. A fault routes to the enclosing statement's landing pad, which populates `Err`
   (Number; Description derived-from-number or as-supplied; **Source = project name when
   omitted**) and dispatches on the policy: `None` → propagate to the caller; `ResumeNext`
   → continue at the next statement; `Goto(L)` → jump to `L`. On a `Goto` catch the
   policy demotes so a re-raise in the handler propagates to the caller (not re-enter).
3. `Resume` re-runs the faulting statement; `Resume Next` continues after it; `Resume L`
   jumps to `L`. **All three clear the `Err` object and the active-error latch.** With no
   active error → **error 20**.
4. `Err.Raise` honors all supplied args; omitted Source → project name; omitted
   Description → derived from Number. `Err.Clear` zeroes `Err`.
5. `Err` is **not** auto-cleared by `Exit Sub`/`Exit Function`, by a normal procedure
   end, by a non-faulting statement, or by the *implicit* skip of `On Error Resume Next`.
6. Control flow: `For` counter ends at `last+step`; a zero-iteration `For` leaves the
   counter at `start`; the limit and step are evaluated once; counter mutation in the body
   is honored; `Exit For`/`GoTo` out leave the counter at its current value. `On n GoTo`
   is 1-based (0 / out-of-range falls through). `Select Case` takes the first matching
   case.
