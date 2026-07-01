# OxIR / vm3 error-handling model (On Error · Resume · Err · GoSub)

> Status: M2-c. This is the reference for how vm3 (and, later, the Cranelift JIT)
> execute VBA run-time error handling. It is **spec-faithful to MS-VBAL** (= real
> Office VBA 7.1); where the legacy oracle **vm2 diverges from the spec, vm3 follows
> the spec** and the divergence is recorded in [§7](#7-confirmed-vm2-vs-spec-divergences).
> Spec citations are to `[MS-VBAL]-250520` (mirrored in `../Foundation`).

The whole point of M2-c is that error handling is the trickiest corner of VBA and we
want it *perfectly right* — so this document is written against the spec, not against
vm2's behaviour.

---

## 1. The shape of the model: block-structured, not pc-arithmetic

vm2 models errors over a flat program counter: a fault calls `route_fault`, which uses
`statement_bounds(pc)` to binary-search a `statement_starts` table for the resume
targets. OxIR is **block-structured**, which makes the model simpler and the resume
targets *static*:

- Each VBA statement lowers to its own **start block** plus a **landing-pad block**.
- A statement's fallible instructions (and a fallible terminator — `Raise`/`RaiseValue`)
  carry a `fault_target` edge to that statement's pad.
- The pad is `OxTerminator::FaultDispatch { resume, resume_next }`, which **carries the
  two resume seeds as block ids**: `resume` = the faulting statement's own start block
  (where `Resume` re-enters), `resume_next` = the following statement's start block
  (where `Resume Next` continues).

Consequence: vm3 needs **no** `statement_starts` table and **no** pc arithmetic. When a
handler catches, it captures `(resume, resume_next)` straight off the pad. Cross-call
`Resume` also falls out for free — see [§5](#5-cross-call-propagation).

---

## 2. Runtime state (per VM and per activation)

| State | Scope | Meaning |
|---|---|---|
| `error_mode: ErrorMode` | per-activation (VM cell, saved/restored per frame) | the active **handler policy**: `None` (Default), `ResumeNext`, `Goto(BlockId)` |
| `active_error: Option<ResumePoint>` | per-activation (VM cell, saved/restored per frame) | the spec's **"active error"** latch + the captured `(resume, resume_next)` seeds. `Some` ⇔ an error is currently being handled in this activation |
| `err: ErrState` | global (the singleton `Err` object) | `number`, `description`, `source` (+ `helpfile`/`helpcontext` once M2-c-2 lands); `last_dll_error` |
| `gosub_stack: Vec<BlockId>` | per-frame | the **GoSub Resumption List** (LIFO) |

`ErrorMode` is the spec's *handler policy*; `active_error` is the spec's *active error /
fault statement*. **vm2 conflates the two (it has only the policy), which is the root
cause of most divergences in [§7](#7-confirmed-vm2-vs-spec-divergences).**

Per-activation cells are saved on the callee `Frame` at call time and restored on
return / unwind (alongside `error_mode`), so each procedure has its own handler + active
error (MS-VBAL §5.4.4 ll. 2793–2794: each invocation has its own policy, born Default).

---

## 3. The OxIR contract (what the elaboration emits)

| Construct | Form | Notes |
|---|---|---|
| `OxInst::SetErrorHandler(ErrorHandler)` | `ResumeNext \| Goto0 \| GotoLabel(BlockId)` *(+ `ClearActiveError` for `On Error GoTo -1`, M2-c-4)* | sets `error_mode`; **also resets `Err`** (Rule R5) |
| `OxTerminator::FaultDispatch { resume, resume_next }` | the landing pad | dispatches on `error_mode` (Rule R4) |
| `OxTerminator::Resume` / `ResumeNext` / `ResumeLabel(BlockId)` | terminators | the three `Resume` forms (Rules R6–R8) |
| `OxTerminator::Raise { code }` / `RaiseValue(op)` | terminators | `Err.Raise` / `Error n`; route through the block's `fault_target` so `On Error` catches them |
| `OxInst::ErrFieldGet { dst, field }` | `Number/Description/Source/LastDllError` | read `Err` |
| `OxInst::ClearErr` | — | `Err.Clear` → reset `Err` |
| `OxTerminator::GoSub { target, ret }` / `GoSubReturn` | terminators | LIFO resumption list (Rules R12) |
| `OxInst::StmtBoundary { stmt }` | statement index | finalization timing (M3); **not used by the error model** (resume seeds live on the pad) |

A block ending in `Raise`/`RaiseValue` is given a `fault_target` because
`OxTerminator::is_fallible()` is true for them and `finish_to` honours it (committed in
M2-c prep `478df517`); the verifier requires it.

---

## 4. Execution rules (spec-grounded)

Each rule cites its MS-VBAL basis. "R*" ids are referenced from the code.

- **R1 — Per-activation policy, born Default.** Each activation's `error_mode` starts
  `None` (Default) and is saved/restored across calls. The base frame surfacing a
  `Fault` out of the run **is** the host "Terminate" boundary. *(§5.4.4 ll. 2793–2794.)*

- **R2 — Default propagation across calls.** An unhandled fault discards the activation,
  re-applies the *caller's* policy, and re-seats the fault at the **call site**: a later
  `Resume` re-runs the calling statement, `Resume Next` the statement after the call.
  *(§5.4.4 l. 2799; ll. 2801, 2832.)*

- **R3 — The `active_error` latch is the spec's "active error".** It is set when a live
  fault is routed into a handler (`ResumeNext` continuation or `Goto` handler), and
  cleared by a successful `Resume`/`Resume Next`/`Resume <label>`, by `Exit
  Sub/Function/Property`, and by `On Error GoTo -1`. *(§5.4.4 ll. 2803, 2805, 2807.)*

- **R4 — `FaultDispatch` dispatch.** With `error_mode`:
  - `None` (Default) → **propagate** the fault to the caller (early unwind).
  - `ResumeNext` → set `active_error = Some((resume, resume_next))`, then continue at
    `resume_next`.
  - `Goto(h)` → **demote `error_mode` to `None`** (Rule R9), set `active_error =
    Some((resume, resume_next))`, then transfer to `h`.

- **R5 — Every `On Error` resets `Err`.** All four forms reset the `Err` object
  *unconditionally*, before applying their policy change. *(§5.4.4.1 l. 2821; §6.1.3.2.1.1
  l. 9052.)*

- **R6/R7/R8 — `Resume` semantics.** First, **if `active_error` is empty → raise
  runtime error 20 ("Resume without error")** (R7-precondition, §5.4.4.2 l. 2830). Else
  reset `Err` and clear `active_error`, then transfer: `Resume` → `resume`; `Resume Next`
  → `resume_next`; `Resume <label>` → the label block. *(§5.4.4.2 ll. 2830–2832;
  §6.1.3.2.1.1 l. 9050.)*

- **R9 — `On Error GoTo <label>` is single-shot.** When the handler **fires**, the
  activation policy is demoted to Default *before* control reaches the handler label. A
  re-raise inside the handler therefore **propagates to the caller**, not back into the
  same handler. The handler is re-armed only by an explicit `On Error` inside it, or by
  `Resume*`/`On Error GoTo -1` clearing the active error. *(§5.4.4 l. 2803.)*

- **R10 — `Exit Sub/Function/Property` resets `Err`** (and clears `active_error`). A
  *normal* `End Sub` / fall-off-the-end does **not**. *(§6.1.3.2.1.1 ll. 9049–9051.)*

- **R11 — `Err.Raise(Number[, Source][, Description][, HelpFile][, HelpContext])`.** All
  supplied args honoured; omitted args inherit the *un-cleared* `Err` fields; omitted
  `Source` defaults to the project name; omitted `Description` is mapped from `Number`
  (else "Application-defined or object-defined error"). `Error <n>` ≡ `Err.Raise <n>`.
  *(§6.1.3.2.1.2 ll. 9055–9071; §5.4.4.3.)* **(M2-c-2/3.)**

- **R12 — GoSub is a per-activation LIFO list.** `GoSub` pushes its `ret`; `Return` pops
  the most recent and branches there; `Return` on an empty list raises runtime error 3
  ("Return without GoSub"). *(§5.4.2.14 l. 2523; §5.4.2.15 ll. 2527–2528.)*

- **R13 — `On Error GoTo -1`** clears the `active_error` latch and resets `Err`, but
  leaves the handler label/policy intact (so the same handler can re-catch). The spec
  *grammar* admits `GoTo -1` (§5.4.4.1 l. 2815) but the *runtime prose is silent*; the
  effect here is documented Office behaviour and **must be confirmed by a live oracle
  run before locking** (see [§8](#8-needs-live-excel-oracle)). **(M2-c-4.)**

- **R14 — `End`** terminates the whole run immediately with **no** `Class_Terminate`
  drain (it closes files / clears variables, but does not fire finalizers). vm3's `Halt`
  (truncate to base, no finalization) is spec-consistent. *(§5.4.2.22.)*

---

## 5. Cross-call propagation

A callee fault with no enabled handler unwinds one activation (`propagate_fault`),
restores the caller's `error_mode`/`active_error`, and routes at the caller's call-site
block. Because that block's `FaultDispatch` was seeded by the elaboration with the
*call-site* statement's `(resume, resume_next)`, a `Resume` in the caller's handler
re-runs the calling statement and `Resume Next` continues after the call — Rule R2,
**for free, with no special cross-frame logic**. The `active_error` latch is the
*caller's* (set when the caller's pad catches the propagated fault), never leaked from
the unwound callee.

---

## 6. Implementation map (vm3)

`crates/oxvba-vm3/src/lib.rs`:
- `Vm3` cells: `error_mode`, `active_error: Option<ResumePoint>`, `err: ErrState`,
  `last_dll_error`.
- `Frame`: `saved_error_mode`, `saved_active_error`, `gosub_stack`.
- `run_loop` terminator arms: `FaultDispatch` (R4/R5/R9), `Resume*` (R6/R7/R8),
  `Raise`/`RaiseValue` (route via `fault_target`, R11-runtime), `GoSub`/`GoSubReturn`
  (R12).
- `exec`: `SetErrorHandler` (R5), `ErrFieldGet`, `ClearErr`.
- `call_proc`/`do_return`/`propagate_fault`: save/restore `active_error` (R3) +
  per-frame `gosub_stack`.

Elaboration (`crates/oxvba-oxir`): `Raise`/`RaiseValue` fault edge (done, `478df517`);
`Exit` path emits a `ClearErr` (R10); `On Error GoTo -1` → `ErrorHandler::ClearActiveError`
(R13, M2-c-4).

---

## 7. Confirmed vm2-vs-spec divergences

These are real bugs in the legacy oracle (vm2). vm3 follows the spec; the vm3-vs-vm2
differential allowlists the affected programs as known vm2 deviations (with the spec
citation), exactly as we did for the duplicate-label case. **The single root cause is
that vm2 has no "active error" concept — only a handler policy.**

| # | Divergence | Spec | vm2 | Sev |
|---|---|---|---|---|
| D1 | `On Error GoTo <h>` is **not demoted** on fire, so a re-raise inside the handler **re-enters the same handler** (can loop) instead of propagating to the caller | §5.4.4 l. 2803 | `lib.rs:960-963` leaves `error_mode=Goto(h)` | high |
| D2 | `Err.Raise` **drops Source/Description/HelpFile/HelpContext** (keeps only Number; always `default_error_message`); no §9071 inheritance | §6.1.3.2.1.2 ll. 9055-9071 | `stmt.rs:1609-1626` reads only arg 0; `set_err` forces `source=""` | high |
| D3 | **No `Err` auto-reset** on `On Error`/`Resume`/`Exit *` — only `Err.Clear` resets | §5.4.4.1 l. 2821; §5.4.4.2 l. 2831; §6.1.3.2.1.1 ll. 9047-9052 | only `set_err` (raise) + `Op::ClearErr` touch `self.err` | med |
| D4 | `Resume*` with **no active error** does not raise **error 20** — vm2 jumps to a stale `{0,0}` seed | §5.4.4.2 l. 2830 | `lib.rs:2466-2468` unconditional `next_pc = resume.*` | med |
| D5 | `Error <n>` statement **fails to parse** (no parser/binder arm) | §5.4.4.3 | `parser.rs` has no `KwError` statement arm | med |
| D6 | `Return` on an empty GoSub list does not raise **error 3** ("Return without GoSub") | §5.4.2.15 l. 2528 | `linearize.rs:285` trampoline falls through to `Op::Return` | low |
| D7 | GoSub is a **single overwriting slot**, not a LIFO stack — nested GoSub loses outer return points | §5.4.2.14 l. 2523 | `linearize.rs` single `gosub_ret_slot` | low |
| D8 | `On Error GoTo -1` **cannot compile** (parser rejects the `-1`) — a valid VBA construct | §5.4.4.1 l. 2815 | `parse_label_ref` rejects `Minus`; `bind_on_error` → `Malformed` | high |

---

## 8. Needs live Excel oracle

Spec-silent or spec-ambiguous points to confirm against real Office before locking
(snippets print to the Immediate window; capture each value):

1. **`On Error GoTo -1` exact effect** — clears the active error + resets `Err`, keeps
   the handler armed; re-arming `On Error GoTo H2` after `GoTo -1` works.
2. **Re-arming a handler while an error is still active** (no `GoTo -1`/`Resume`) — is it
   rejected or does it silently re-arm?
3. **Default `Err.Source`** for an omitted Source — confirm it is the VBA project name;
   capture the literal.
4. **Default `Err.Description`** text — mapped (e.g. 11 → "Division by zero") vs unmapped
   (513 → "Application-defined or object-defined error").
5. **`Err.Raise` un-cleared-field inheritance (§9071)** — a second `Raise` omitting
   Source/Description inherits the previously-set un-cleared fields.
6. **Re-raise inside an active `Goto` handler propagates to the caller** (confirms D1/R9).

(Each item carries an exact VBA snippet in the M2-c research record / memory.)

---

## 9. Open questions

- `On Error GoTo -1` vs `Resume`: both clear the active error, but `GoTo -1` must **not**
  transfer control or consume the resume seed — confirm.
- `Err` properties are **writable** in real VBA (`Err.Number = …`, `Err.Source = …`);
  vm2 has no Err-write path. Needed to make §9071 inheritance observable — scope for
  M2-c-2 or defer.
- Distinct Office `Err.Description` text for codes 3 and 20 (tie to the
  `default_error_message` table extension).

---

## 10. Delivery phases

- **M2-c-1 (vm3 core, spec-clear, no front-end, no oracle):** `active_error` cell;
  `SetErrorHandler` (R5); `FaultDispatch` arms with demotion (R4/R9); `Resume*` with
  error-20 + Err-reset (R6/R7/R8); `Raise`/`RaiseValue` routing (R11-runtime, single-arg);
  `ErrFieldGet`; `GoSub`/`GoSubReturn` LIFO + error 3 (R12); `Exit` Err-reset (R10).
  Gate: the On-Error/Resume corpus programs run + match (vm2-bug edge cases allowlisted).
- **M2-c-2 (richer `Err.Raise`/`Err`):** Source/Description/HelpFile/HelpContext through
  binder → OxIR → `ErrState`; §9071 inheritance; Err-property writes. **Needs oracle (§8.3–8.5).**
- **M2-c-3 (`Error <n>` statement):** parser + binder arm → `Raise`/`RaiseValue`.
- **M2-c-4 (`On Error GoTo -1`):** parser + binder + `ErrorOp`/`ErrorHandler` variant +
  vm3 handler. **Needs oracle (§8.1–8.2).**
