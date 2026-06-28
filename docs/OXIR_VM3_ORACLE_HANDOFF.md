# OxIR / vm3 — Oracle Handoff (M3 complete)

**Status: M3 complete.** vm3 (the typed-CFG interpreter of OxIR, crate `oxvba-vm3`) is now
OxVBA's **differential oracle** — the executable specification the Cranelift JIT (M4+) is
differentialed against. The legacy `oxvba-vm2` bundle interpreter is **frozen**: kept buildable
as a transitional cross-check, no longer extended, and no longer the definition of correct.

Authority order (unchanged): **live Windows Office VBA 7.1 (oracle) > MS-VBAL spec > clean typed
design**; vm2 is a transitional cross-check only. Where vm2 deviates from real VBA it is a
documented divergence (allowlisted), never chased — vm3 is the more-correct side.

## What "handoff" means

- **vm3 is the reference executor.** The differential harness (`oxvba-differential`) runs the
  full corpus under vm2 and vm3 and asserts they agree (mismatches == 0) on all observable axes;
  M4's JIT becomes a third executor compared against **vm3** (a fixed target, never moving).
- **The handoff invariant is enforced in CI.** `vm3_matches_vm2_across_the_corpus_subset`
  asserts `in-scope-skipped == 0` (every in-scope corpus program runs on vm3), `mismatches == 0`,
  and a stale-allowlist guard over the out-of-scope deferrals. `oracle_conformance.rs`
  (`vm3_is_oracle_compliant`) asserts vm3 is 100% compliant on the live-Excel error/control-flow
  probe corpus (`KNOWN_VM3_GAPS` is empty).

## Coverage (what vm3 executes)

The full OxIR vocabulary the elaborator emits: the scalar/string/Boolean value core, all
arithmetic + the typed-narrowing coerce-on-store, control flow, compiled procedure calls with
true ByRef aliasing, the complete error/`Resume` model (per-activation handler policy + active-
error latch), cross-bundle/builtin-library dispatch (`CallExtern`), arrays + `For Each` + `Bound`,
records/UDT value semantics, objects + lifecycle + `Class_Initialize`/`Class_Terminate` drain
timing + `Is`/`TypeOf`, events (`RaiseEvent` + the `WithEvents` family), native interop (pointer
helpers, `AddressOf`/`CallProcRef`, `Declare Lib` via the dynamic-link HAL), and **typed COM** —
late-bound by-name dispatch (`ComCallLate`/`CallByName`/`CreateObject`/`GetObject`) and early-bound
descriptor-typed dispatch (`ComCallEarly`) with rich `HRESULT`/`EXCEPINFO` → `Err.Number` fidelity
(never flattened to 5) and vtable/IDispatch transport parity with vm2.

The six differential axes are green: (1) return values, (2) `Err` state (number / raised-vs-
completed / `LastDllError`), (3) host side-effects, (4) terminate timing, (5) COM transport
counts, (6) COM typed-arg + `[out]` writeback + specific `DISP_E_*` numbers. Live COM is proven
on real Excel/Scripting (`oxvba-host` `com_matrix_vm3` tests: value, dup-key 457, remove 32811,
late-bound transport `(5,2)==(5,2)`, early-bound transport `(7,0)==(7,0)`).

## Documented out-of-scope residuals (filed follow-ups, not in-scope gaps)

Each is an explicit `Unimplemented`/`Malformed` in vm3 (never a silent wrong value), and none is
reached by an in-scope corpus program:

1. **Built-in `Collection`** (`New Collection` / `.Add`/`.Item`/`.Count`/`.Remove`,
   `For Each` over a `Collection`). Needs the cross-bundle native-backed-object mechanism vm3's
   deliberately single-`OxProgram` object model defers; it is the subject of its own approved
   builtins-as-library program (Collection = phase P1). The one corpus carve-out
   (`object_identity_is_same_and_different.bas`) is the allowlisted `KNOWN_VM3_DEFERRED_SKIPS`
   entry.
2. **Default-member indexing on an object receiver** (`x(i)` / `x(i) = v` where `x` is a runtime
   object in a statically-bare `Variant`/`Object` slot) and **`For Each` over a COM
   `IEnumVARIANT`**. Gated behind object-receiver guards no corpus program hits; partly blocked on
   (1).
3. **`AddressOf` proc marshaled into a native callback slot** (a `ByVal LongPtr` `Declare`
   parameter, e.g. a `SetTimer` `TimerProc`). The right fix is a VM-agnostic shared callback-thunk
   facility in `oxvba-runtime` (not a mirror of vm2's `*mut Vm`-coupled 32-slot table).
4. **True multi-`OxProgram` cross-project linking** (project A → project B, both VBA-bodied).
   M3-1 covers the library-bundle case; true cross-project link is a named `Unimplemented`.
5. **`GetObject`** — absent product-wide (also absent in vm2); a feature gap, not a vm3 defect.

## Accepted vm3-vs-vm2 divergences (vm3 is the more-correct side)

- Early-bound `ComCallEarly` uses the descriptor's member name in its dispatch selector where vm2
  carries the call-site name; the two differ only for early-bound *default-member* access, the
  difference is dormant on the live Windows COM bridge (dispatch-by-dispid ignores the name), and
  on the portable adapter vm3 invokes the real member rather than vm2's receiver-label. Kept.
- vm2's three `Resume`-residual-`Err` staleness files (`KNOWN_VM2_DIVERGENCES`): vm3 correctly
  clears `Err` on `Resume` per the oracle; vm2 leaves a stale number. Kept (vm2 is the wrong side).
