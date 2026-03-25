# COM Activation Boundary Reconciliation — 2026-03-25

Purpose: reconcile `ODG-031` against the repaired implementation truth and the already-captured oracle evidence.

## Findings

1. Native Windows late-bound activation is the authoritative late-bound parity path.
   - `CreateObject("ProgID")` remains the live native activation path on Windows.
   - The remaining selector-era leak was in quoted `DispatchInvoke` member lowering, not in native activation itself.
   - That leak is now corrected: quoted external member names are preserved as string selectors on the real external COM lane, while deterministic token lowering remains confined to the bounded internal `OxVba.TestDispatch*` fixture lane.
   - The real registered late-bound oracle lane remains green after removing the temporary workaround names: `com_testeventserver_marshaling_oracle_20260325T231210Z` still matches Excel on `SumPair`, array-shape, self-object, scalar-array, and dispatch-array cases.

2. User-scope file-backed typelib interop is a real supported early-bound lane.
   - `com_testeventserver_oracle_20260325T221949Z` already proves Excel/OxVba agreement for:
     - `VBProject.References.AddFromFile(...)`,
     - early-bound `New TestEventServer` with `Ping() = 42`,
     - `WithEvents` callback payload preservation (`7`).
   - `com_testeventserver_versioned_typelib_probe_20260325T222709Z` already proves the versioned/broken-reference matrix for the same file-backed typelib lane.
   - Current registration now emits `OxVba.TestEventServer.tlb` on every run via `TlbExp.exe`, and a direct local Excel probe on 2026-03-25 confirmed that `AddFromFile` against that emitted `.tlb` succeeds and `New OxVba_TestEventServer.TestEventServer : Ping()` returns `42`.

3. Real imported `As New` support is closed for the supported initial-scope subset.
   - `com_early_oracle_20260325T145433Z` proves the real registered `Scripting.Dictionary` subset for `Dim obj As New Scripting.Dictionary` plus `Add` / `Exists` / `Count`.

4. Deterministic fallback/projection scaffolding still exists, but it is no longer part of the parity claim boundary.
   - It remains valid bounded test infrastructure.
   - It is not evidence for native Windows activation or for arbitrary real-library imported activation.

## Conclusion

`ODG-031` is now closed for the bounded initial-scope COM typelib interop claim.

What is closed:
- native late-bound Windows ProgID activation as a real path,
- user-scope file-backed typelib reference/import behavior,
- supported imported early-bound activation on the proved real-library subsets,
- versioned/broken-reference behavior for the file-backed typelib lane.

What is not being claimed:
- arbitrary real COM library activation parity beyond the bounded proved subsets,
- deterministic fallback/projection behavior as equivalent to native COM parity.

The remaining broader “arbitrary real-library COM parity” expansion is post-scope breadth work, not an active initial-scope blocker.
