# HANDOVER → OxVba: vm3 dynamic-array element access is O(N) (array loops are O(N²))

Status: `PARTIAL — STILL OPEN` (OxVba master, 2026-06-30) · From: OxForms · To: OxVba · Date: 2026-06-29

> **Update 2026-06-30 (reopened):** the first fix removed the O(N) clone for arrays
> held in **module/local/temp slots** (those are now genuinely O(1) — ~13 µs/elem flat
> across N), a real ~5–6× win. But OxForms' min-of-trials still shows O(N) per element,
> and that is **confirmed**: arrays held as **class-instance fields** (`Private mX()` in a
> `.cls`, the OxForms shape) are STILL O(N) per access — they bypass the slot fast path and
> deep-clone the whole array on the field-load every iteration. The handover stays OPEN.
> See "Residual" below.
OxVba baseline exercised: master `2b817614` (vm3-only; pinned by OxForms via git rev).
Policy: per OxForms memory `oxvba-vm3-handover-policy`, perf pathologies vm3 exhibits under the
OxForms workload are handed to OxVba to **fix**, not worked around in OxForms — OxForms is the
workload that hardens OxVba. This is such a report.

## Summary

On the vm3 interpreter (`enable_jit=false`), **reading an element of a module-level dynamic array
inside a loop (`arr(i)`) scales O(N) in the array length N**, so a simple loop that indexes arrays is
**O(N²)**. A 100-element hit-test loop takes **130 ms**; a 400-element one takes **2.2 s**. The
equivalent native Rust loop is ~1 µs. This is a *general* pathology (it cripples any non-trivial VBA
loop over arrays), not specific to the hit-test code that surfaced it.

## Reproduction

Repo: OxForms. Files:
- `vba/smoke/fixtures/lex_hittest_bench_form.cls` — a class holding N control rects in four
  `Private` dynamic `Long` arrays (`mX/mY/mW/mH`), with `HitTestAll(X, Y)` looping `0..mN-1` and
  reading those arrays, and a `Noop()` for crossing-overhead isolation.
- `crates/oxforms-oxvba-adapter/tests/oxvba_lex_hittest_bench.rs` — drives a vm3 image session
  (`enable_jit=false`), calls `Setup(N)` then times N-scaled runs of `HitTestAll` vs `Noop` vs a
  native-Rust equivalent.

Run: `cargo test -p oxforms-oxvba-adapter --test oxvba_lex_hittest_bench -- --ignored --nocapture`

The hot loop (VBA):

```vba
For i = 0 To mN - 1
    If (X >= mX(i)) And (X < mX(i) + mW(i)) And _
       (Y >= mY(i)) And (Y < mY(i) + mH(i)) Then
        hit = i
    End If
Next i
```

## Measurements (per call, vm3 interpreter)

| N | native Rust | VBA `HitTestAll` | bare crossing (`Noop`) | VBA compute (`HitTestAll − Noop`) | per-element |
|---|---|---|---|---|---|
| 25 | 315 ns | 9.7 ms | 5.6 µs | 9.7 ms | 0.39 ms |
| 50 | 560 ns | 35.0 ms | 5.7 µs | 35.0 ms | 0.70 ms |
| 100 | 1.1 µs | 130.4 ms | 7.8 µs | 130.4 ms | 1.30 ms |
| 200 | 1.8 µs | 569.5 ms | 8.7 µs | 569.5 ms | 2.85 ms |
| 400 | 6.1 µs | 2226 ms | 6.7 µs | 2226 ms | 5.57 ms |

## Diagnosis

- **The boundary crossing is NOT the cause.** `Noop` (one `invoke_member_values` into an empty VBA
  function) is ~5–9 µs and flat across N. The cost is entirely inside the VBA loop.
- **The loop is O(N²).** Doubling N ~quadruples total time, and the **per-element** cost itself
  doubles with N (0.39 → 0.70 → 1.30 → 2.85 → 5.57 ms). A linear loop would have constant per-element
  cost.
- **By elimination, `arr(i)` is O(N).** Every operation in the loop body is an O(1) `Long`
  comparison / `And` / add, **except** the dynamic-array element reads (`mX(i)`, `mW(i)`, `mY(i)`,
  `mH(i)`). The only operation whose cost can grow with N is the array element access — so element
  access is O(N), not O(1). **Confirmed directly by isolation — see below.**

## Isolation — the array read is the O(N) op (confirmed)

`array_access_isolation_on_vm3` (same test file) runs two identical fixed-length loops
(`Reps = 1000`) that differ ONLY in whether each iteration reads one array element, while varying the
array length N:

| N | `ScanPlain` (no array) | `ScanArray` (1 read/iter) | per-read = (arr − plain)/Reps |
|---|---|---|---|
| 25 | 9.6 ms | 80.4 ms | 71 µs |
| 50 | 9.2 ms | 118.5 ms | 109 µs |
| 100 | 8.8 ms | 215.4 ms | 207 µs |
| 200 | 9.1 ms | 546.3 ms | 537 µs |
| 400 | 11.7 ms | 1119 ms | 1107 µs |

- **`ScanPlain` (pure arithmetic, no array) is FLAT across N** (~9–12 ms per 1000 iterations ≈
  ~9 µs/iter) — the interpreter's ordinary per-iteration overhead, independent of array length. This
  is *not* the bug.
- **A single `arr(idx)` read is O(N).** per-read grows ~linearly with the array length (71 µs at
  N=25 → 1.1 ms at N=400; indices sweep the whole array). Reading one `Long` from a 400-element array
  costs ~1.1 ms. This isolates the entire O(N²)-loop pathology to element access.

## Suspected mechanism (for OxVba to confirm)

Something in the vm3 element-access path is linear in array length or in surrounding state. Candidates:
- dynamic-array element access walking a descriptor / SafeArray / non-O(1) storage per index;
- module-field (`Private mX()`) resolution being non-constant per access inside a loop;
- no caching / strength-reduction of repeated `arr(i)` within an interpreted loop iteration.

## Ask

Make dynamic-array element access **O(1)** so array loops are **O(N)**. Target: a 100-element,
6-array-read loop should be sub-millisecond on the interpreter (it is ~µs natively), not 130 ms.

## Why this matters / impact

This gates OxForms's move of the control reducer/interaction layer into VBA (memory
`oxforms-reducer-home-decision`) and the hit-test architecture decision
(`docs/specs/HIT_TEST_AND_MESSAGE_LOOP_OFFLOAD.md`): with O(N²) array loops, *no* VBA-side hit-test
(even a spatial-index one) is viable, and more broadly any VBA hot path that touches arrays is
unusable. It almost certainly affects other vm3 workloads too. OxForms will **re-run this benchmark
after the fix** and report the new curve here; the VBA-vs-Rust hit-test decision is deferred until
then.

---

## OxVba-side intake (this repo)

Received into the OxVba work list 2026-06-29. Tracked by bead **bd-us4v** (vm3 perf;
independent of the `bd-4ktq` spec-gap-closure correctness sweep — this is a performance, not a
correctness, gap). Mirror of `OxForms/docs/handovers/HANDOVER_OxVba_vm3_dynamic_array_access_perf.md`.

First place to look on the OxVba side (to confirm the suspected mechanism before fixing):
- the vm3 array element read/write path in `crates/oxvba-vm3/src/lib.rs` (the `OxInst` array-index
  load/store handlers) and the underlying `oxvba-runtime` `SafeArray` element accessor
  (`crates/oxvba-runtime/src/safe_array.rs`) — verify the per-element read is O(1) (direct flat
  index into contiguous storage) and not re-resolving/cloning the whole array or walking a
  descriptor per access;
- the module-field resolution for a `Private mX()` read inside a loop — confirm the place lookup is
  O(1) and the array Variant is not deep-cloned per access (a per-access `SafeArray` clone of an
  N-element backing store would be exactly O(N) per read → O(N²) per loop).

Reproduce on the OxVba side with a self-contained `oxvba-differential`/`oxvba-vm3` micro-benchmark
(no OxForms dependency): a module-level `Dim a() As Long` / `ReDim a(N)` filled, then a timed
`For i … a(i) …` loop scaled over N — assert per-element cost is flat in N once fixed.

## Resolution (OxVba master, 2026-06-30)

**Root cause (confirmed):** the vm3 index path read the array place **by value** — `arr(i)` ran
`operand(array)` *and* `array_of(array)`, each cloning the array `Variant`, and `SafeArray::clone`
/ `Variant::as_safearray` **deep-copy every element** (rebuild the whole SAFEARRAY). So each
`arr(i)` did ~4 O(N) full-array copies → O(N) per access → O(N²) per loop. `variant_element`
itself was already O(1); the cost was entirely the per-access whole-array clone. The write path
(`arr(i) = v`) had the same defect (clone, mutate one element, write the whole array back).

**Fix:** O(1) element access straight through the SAFEARRAY descriptor, with no whole-array clone:
- `oxvba-runtime` `SafeArray::raw_safearray_variant_element` + `raw_safearray_bounds_len`
  (borrow-the-raw-descriptor-without-owning, mirroring the existing raw element *write*), surfaced
  on `Variant` as `safearray_element` / `safearray_bounds_len`.
- `oxvba-vm3` `array_get_fast` / `array_set_fast`: borrow the array's slot in place
  (`read_loc_ref` / `read_loc_mut`), bounds-check and read/write the single element, no clone.
  `ArrayGet`/`ArraySet` take this fast path for any place-resident array and fall back to the old
  general path only for object default-member / run-time-resolved receivers.

**Result (slot-held arrays only):** a self-contained guard
(`crates/oxvba-differential/tests/array_access_perf_vm3.rs`) fills then reads a **2000-element
module-level** dynamic `Long` array; it now completes in **~70 ms** (debug) where the O(N²)
defect was tens of seconds, and asserts both the correct sum and a <5 s ceiling. Per-element
cost is flat in N **for module/local/temp-slot arrays**.

## Residual — class-instance-field arrays are STILL O(N) (the part OxForms hits)

The first fix only intercepts an array whose receiver place is a **slot** (Global/Local/Temp):
`array_get_fast`/`array_set_fast` bail to the cloning general path for any other receiver. The
OxForms workload reads arrays held as **class-instance fields** (`Private mX()` in
`lex_hittest_bench_form.cls`), and that path was **not** covered — the field-held array is still
deep-cloned on every access, so the loop stays O(N²).

Confirmed by a module-vs-field scaling diagnostic
(`crates/oxvba-differential/tests/array_perf_diagnose.rs`,
`cargo test -p oxvba-differential --test array_perf_diagnose -- --ignored --nocapture`):

| N | module-level (slot) | class-instance-field (`.cls`) |
|---|---|---|
| 250 | 13.1 µs/elem | 270 µs/elem |
| 500 | 13.5 µs/elem | 486 µs/elem |
| 1000 | 14.9 µs/elem | 961 µs/elem |
| 2000 | 11.0 µs/elem | 1920 µs/elem |

Module-level is flat (**O(1)**); class-field doubles with N (**O(N)** → loops O(N²)). This matches
OxForms' min-of-trials.

**Direction for the revisit (OxVba side):** make field-held array element access O(1) too. Trace how
`obj.field(i)` / `Me.field(i)` lowers in OxIR (`crates/oxvba-oxir/src/elaborate/`) — almost certainly a
record/field load that clones the whole field `Variant` before the `ArrayGet`, executed once per loop
iteration. Options: (a) extend `array_get_fast`/`array_set_fast` to recognise a field-of-object/record
receiver and borrow the field's array `Variant` in place (reuse the `Variant::safearray_element` /
`safearray_bounds_len` descriptor-borrow primitives from the first fix); or (b) introduce a fused
"index a field array" lowering that never materialises the whole field. Also check UDT-field arrays
(`a.items(i)` where `a` is a `Type` record) — same shape, same likely fix. Re-run the diagnostic above
(both rows should go flat) plus the OxForms bench.
