# HANDOVER → OxVba: vm3 dynamic-array element access is O(N) (array loops are O(N²))

Status: `OPEN` · From: OxForms · To: OxVba · Date: 2026-06-29
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
