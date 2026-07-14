# WIN-0 Current-Stack Residual Migration

Date: 2026-07-14
Bead: `bd-59co.3.1.6`
Profile: Windows x64
Result: support characterization complete; capability delivery remains `planned`

## Outcome

The current repository implementation, test sources, and historical evidence have been characterized against all 57 required rows in the six Windows x64 matrices. The authoritative row-by-row result is [IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv](../../../../../validation/IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv).

This work does not advance a capability state. Every canonical row and every ledger row remains `planned`. Current VM3, HAL, COM, runtime-carrier, comhost, and build assets are recorded only as implementation or test subsets. Historical evidence is recorded only as provenance. In particular, neither kind of asset is credited as current JIT completion.

## Review method and state vocabulary

The pass inspected the current code paths and test sources for VM3, JIT, COM client/server/events, native imports/callbacks, wrapper output, runtime carriers, typelib metadata, and the Windows build host. It then mapped the historical Windows/Excel evidence packs to the rows they inform without treating those packs as certification of the current stack.

The ledger uses closed vocabularies enforced by `scripts/validate-windows-current-stack-residuals.ps1`:

- Code: `current-subset`, `current-divergence`, `absent`, or `n/a`.
- Tests: `current-subset`, `historical-only`, `absent`, or `n/a`.
- Historical evidence: `provenance-only` or `none`.
- Gap: missing implementation, backend divergence, known blocker, missing controlled fixture, missing current evidence, environment pending, or aggregate pending.

`current-subset` for tests means that a present-stack test source resolves and exercises or characterizes part of the row. Some Windows integration tests are ignored or operator-run; this label does not mean that the row has passed release certification. `historical-only` means that only an earlier evidence pack is available for the assessed behavior.

## Inventory result

| Matrix | Rows | Current code characterization | Current test characterization | Residual shape |
|---|---:|---|---|---|
| `WIN-COM-CLIENT` | 9 | 8 subsets; 1 not applicable control row | 8 subsets; 1 historical-only | 8 VM3/JIT divergences; 1 current-evidence gap |
| `WIN-COM-EVENTS` | 7 | 6 subsets; 1 divergence | 7 subsets | 1 known blocker; 6 missing implementation/proof gaps |
| `WIN-COM-SERVER` | 7 | 7 subsets | 4 subsets; 3 historical-only | 7 VM3-backed/JIT-serving divergences |
| `WIN-NATIVE-IMPORT` | 8 | 8 subsets | 8 subsets | 6 VM3/JIT divergences; 2 retained/nested callback implementation gaps |
| `WIN-NATIVE-EXPORT` | 8 | 3 subsets; 4 absent; 1 terminal row | 3 subsets; 4 historical-only; 1 terminal row | 3 wrapped-output/JIT divergences; 4 genuine native-output gaps; terminal pending |
| `WIN-ABI-CARRIER` | 18 | 11 subsets; 1 absent; 6 control rows | 10 subsets; 4 historical-only; 3 absent; 1 terminal row | 5 fixture gaps; 5 implementation gaps; 5 evidence gaps; 2 environments and terminal pending |

Across the profile, the code assessment is 43 current subsets, one current divergence, five absent implementations, and eight control/not-applicable rows. The gap assessment is 24 backend divergences, 17 missing-current-implementation gaps, six missing-current-evidence gaps, five missing-controlled-fixture gaps, one known blocker, two environment-pending rows, and two aggregate terminals.

## Material architectural findings

The current JIT still rejects projects with external calls or COM interfaces and leaves the relevant OxIR operations unsupported. Consequently:

- VM3 late-bound and early-vtable COM routes are useful substrate, not dual-runtime completion.
- VM3 `Declare`, pointer-helper, and synchronous callback routes are useful substrate, not JIT native-import completion.
- The wrapped COM server and build paths are VM3-backed subsets, not JIT COM serving or genuine native DLL/EXE output.
- There is no owning verified x64 interop plan shared by VM3 and JIT. Existing backend-specific descriptors cannot substitute for that sealed producer contract.

The most concrete current divergence is synchronous incoming COM event writeback. The connection-point path converts native arguments to owned callback values and the runtime state queues those values. It does not preserve mutable source slots through callback execution and write changed ByRef values back before the native call returns. This remains the explicit blocker for `WCE-PLAN-INCOMING`.

The carrier implementation is substantial but not release evidence. BSTR, Variant, SafeArray, object identity, numeric/LongPtr, interface-array, and record code needs controlled x64 fixture and lifecycle proof. Historical VBA 7.1/Excel fact packs provide target provenance only.

## Imported residual preservation

Two precise legacy delivery routes remain active and are cross-checked against both the legacy migration ledger and exact matrix traceability:

- `bd-aprs.8.8.9` remains the delivery route for `WIN-COM-EVENTS/WCE-PLAN-INCOMING` under `bd-59co.3.6`.
- `bd-9sed.17` remains the delivery route for `WIN-NATIVE-IMPORT/WNI-PLAN-CALLBACK` under `bd-59co.3.11`.

The residual ledger deliberately routes these capability rows to their open delivery epics, not to support-only rollout or evidence beads. The same rule applies to all capability rows. Only the target-development environment, clean-certification environment, and profile-terminal control rows use exact support owners.

## Fail-closed controls

The validator requires:

- the exact 57-row canonical identity set and exact 15-column schema;
- exact canonical claim keys, owner epics, and `planned` truth state;
- the reviewed code/test/history/gap classification for each row;
- every pipe-separated code, test, and historical anchor to resolve to a repository file;
- historical evidence to remain under `docs/evidence/` and never appear as a current test anchor;
- backend-divergence rows to expose the current JIT boundary;
- the synchronous ByRef event row to remain a named blocker;
- every capability residual to have an active Windows x64 delivery owner in the correct epic ancestry;
- exact preservation of the two imported legacy routes.

`scripts/test-windows-current-stack-residuals.ps1` supplies a positive case and nine mutations. The mutations prove rejection of a missing row, capability-state advancement, JIT credit sourced only from VM3/historical material, historical evidence credited as a current test, a support-only capability owner, a removed imported route, an unresolved anchor, a hidden ByRef event blocker, and a closed imported callback route.

## Commands and evidence boundary

The support artifact is accepted by:

```powershell
./scripts/validate-windows-current-stack-residuals.ps1
./scripts/test-windows-current-stack-residuals.ps1
./scripts/validate-ideal-legacy-migration.ps1
./scripts/validate-bead-traceability.ps1
./scripts/run-truth-reconciliation.ps1
```

No registry mutation, Excel/VBE automation, native fixture execution, or release certification was performed by this bead. Those activities remain with their Windows delivery and certification owners. The current ledger therefore provides migration truth and a fail-closed starting point; it is not Windows capability evidence.
