# Class_Terminate Timing Oracle (Excel)

Captured from Excel 16 (`AccessVBOM=1`) on 2026-05-31 via `scripts/run-class-terminate-timing-oracle.ps1`
(single session: a class module `Foo` whose `Class_Terminate` appends `"T"` to a module log, plus
helpers that append ordered markers). This is the ground-truth timing target for project-object
`Class_Terminate` (Phase 2 of the per-instance object lifetime).

## Probes and results

| Probe | VBA source | Markers | Excel result |
|---|---|---|---|
| A | `Set a = New Foo : Append"1" : Set a = Nothing : Append"2"` | `1`,`T`,`2` | **`1T2`** |
| B | `s = MakeFoo().Tag & Mark()` | `g`=Tag read, `M`=Mark called, `T`=Terminate | **`gMT`** (`after=x`) |

## Derived rules (the timing target)

1. **`Class_Terminate` fires at the statement that releases the last reference, before the next
   statement.** Probe A: `T` lands between `1` and `2`, i.e. during the `Set a = Nothing`
   statement; the following statement (`Append"2"`) sees it already done.
2. **Expression temporaries are held until the end of the statement, then terminated** — not
   released after their last sub-use. Probe B: the temp `Foo` from `MakeFoo()` is read by `.Tag`
   (`g`), yet its `Terminate` (`T`) fires only *after* the later `Mark()` (`M`) and the whole
   assignment complete → order `gMT`. An intra-statement (release-after-last-use) model would have
   produced `gTM`; VBA does **not** do that.
3. Therefore `Class_Terminate` timing is **statement-granular**: terminations are drained at the
   statement boundary (after the statement that caused the release fully completes, before the
   next), and at the procedure epilogue for locals. VBA never interleaves `Class_Terminate` between
   the sub-operations of a single statement.

## OxVba implementation target

- `compat_release` at refcount 0 enqueues the instance (id + route key) on a pending-termination
  queue; the interpreter drains it **at statement boundaries** (and the procedure epilogue, after
  releasing the callee's local object slots), running `Class_Terminate` with the instance as `Me`,
  clearing its field state, looping for cascades. Reference cycles never reach 0 → leak (VBA-consistent).
- Regression target: a `1T2`-shaped test (Set Nothing terminates at that statement) and a
  `gMT`-shaped test (temp terminates at statement end), reproduced in OxVba via run-project snapshots.

## Implementation status (landed 2026-05-31)

Per-instance `Class_Terminate` is implemented end-to-end:

- **Runtime queue** (`oxvba-runtime/object_ref.rs`): `RuntimeObjectIdentity.terminates_on_release`
  marks instances of classes that define `Class_Terminate`. At refcount 0, `compat_release`
  parks the original object box and enqueues `(instance_id, route_key)` on a thread-local
  pending-termination queue, so teardown runs with `Me` bound to the same field-owning object.
  APIs: `has_pending_terminations`, `take_pending_terminations`,
  `retained_parked_termination_object`, `finish_pending_termination`, `reset_pending_terminations`.
- **Route** (`oxvba-compiler` `ProjectDynamicObjectRoute.class_terminate`): each class route carries
  its `Class_Terminate` member (captured regardless of `Private` visibility) with its `entry_pc` and
  hidden-`Me` `param_slots`.
- **VM drain** (`oxvba-vm/interpreter.rs`): at every statement boundary (a pc in any procedure's
  `statement_entry_pcs`) and at each procedure epilogue (`Return` / entry `Halt`), the VM (1) releases
  that scope's terminating-object **temporaries** (statement boundary) and **locals + temporaries**
  (epilogue), then (2) drains the queue, running each `Class_Terminate` against the original parked
  object. If `Me` is not resurrected, the runtime clears the object's field store after terminate;
  cascades accumulate and are drained by the surrounding loop.
- **Legacy retired**: the module-entry-exit `Class_Terminate` hook (which predated the hidden `Me`
  param and never ran for `New`-ed instances) was removed from `emit.rs`.

Releasing **expression temporaries at the statement boundary** is what makes the basic cases work:
the `New Foo` result lives in a temporary, so without end-of-statement temp release the instance
never reaches refcount 0 during execution.

Tests (`oxvba-host/tests/com_early_project_end_to_end.rs`): `1T2` (terminate at the statement that
drops the last reference), `inTafter` (local terminates at the procedure epilogue), the
no-`Class_Terminate` control (`12`), and a route-carries-`Class_Terminate` compiler check — all pass.

Follow-ups landed since:
- **`Set <objvar> = Nothing`** now compiles and clears the reference (the canonical idiom); the
  `1T2` test uses it. (`Nothing` → `__nothing` intrinsic typed Object, emits runtime 0; the runtime
  Set guard accepts the null-object value.)
- **Class with `Class_Initialize`** now terminates: a class member's hidden ByVal `Me` is released
  at the procedure epilogue, so the constructor no longer pins the instance (test `i1T2`).
- **Object-typed instance fields** can be assigned with `Set field = New X` (per-instance state
  write).
- **Regular project class fields** now live on the project instance object, not in the VM's
  WithEvents binding map. Field route metadata records ordinary field tokens; the VM routes
  `__oxvba_withevents_get/set` for those tokens to same-box per-instance storage. Dropping an owner
  now releases its object fields after `Class_Terminate`, so regular field cascades terminate
  children (test `pure_oxvba_class_terminate_cascades_through_object_field`).
- **Single-thread runtime shape**: live `ObjectRef` no longer carries broad `Send`/`Sync`; COM/shared
  callback state stores raw identity tokens and reconstructs projected handles at the edge.

- **Expression-level member access** now exists: `BoundExpr::Member { receiver, member, args }`,
  bound for non-bare-variable receivers (call results / chains), lowered to the late-bound
  dispatch (`IntrinsicDispatchInvokeHost`). `MakeFoo().Tag` and `MakeBox().Add(2,3)` dispatch and
  return correctly (tests `member_access_on_function_call_result_dispatches` and the args variant).

Still deferred — activation/return-slot lifetime work parked under `bd-xkwq`:
- The `gMT` probe's trailing `T`: the temporary `Foo` from `MakeFoo()` is retained by the
  **function's return slot** (the returned object lingers there until the next call), so it does not
  terminate at end of statement. Member access (the feature) works; only this terminate timing is
  pending. Correct by construction once return-slot retention is resolved.
