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
