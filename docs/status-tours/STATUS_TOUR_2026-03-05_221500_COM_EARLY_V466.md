# STATUS_TOUR_2026-03-05_221500_COM_EARLY_V466

## Scope

This tour summarizes where OxVba stands after COM early-binding/type-library ladder closure (`v407..v466`).

## What now works

1. PMR type-library identity supports deterministic hints (`importlib/libid/version/lcid`) with stable bind statuses.
2. HAL exposes a type-library surface (`resolve/load/invalidate`) and deterministic cache invalidation behavior.
3. Compiler project lowering supports constrained early-bound forms:
   - `Dim x As OxVba.TestDispatch`
   - `Dim x As New OxVba.TestDispatch`
   - `x.Count()` and `x.Exists(arg)` rewrite lanes.
4. Runtime executes this subset through controlled COM transport and policy-gated strategy selection.
5. Conformance lanes `E0..E6` run via scripts with machine-readable artifacts.

## Key code fragments

### Host policy strategy control

```rust
pub enum ComInvocationStrategy {
    DispatchOnly,
    PreferVtable,
}
```

File: `crates/oxvba-hal/src/model.rs`

### Runtime strategy branch

```rust
if self.policy.com_invocation_strategy != ComInvocationStrategy::PreferVtable {
    return Ok(None);
}
```

File: `crates/oxvba-hal/src/adapters/standard.rs`

### Early-bound E2E fixture shape

```vb
Dim obj As New OxVba.TestDispatch
countValue = obj.Count()
existsValue = obj.Exists(42)
```

File: `crates/oxvba-host/tests/com_early_project_end_to_end.rs`

### Conformance orchestration

```powershell
./scripts/run-com-early-conformance.ps1 -IncludeFormalLane
```

Files: `scripts/run-com-early-conformance.ps1`, `scripts/run-com-early-lane.ps1`

## Evidence outputs

- Conformance latest:
  - `docs/evidence/conformance/com_early/COM_EARLY_CONFORMANCE_LATEST.csv`
  - `docs/evidence/conformance/com_early/COM_EARLY_CONFORMANCE_LATEST.md`
- Perf latest:
  - `docs/evidence/perf/com_early/COM_EARLY_PERF_LATEST.csv`
  - `docs/evidence/perf/com_early/COM_EARLY_PERF_LATEST.md`
- Terminal closure:
  - `docs/evidence/profiles/v466/V466_COM_EARLY_CLOSURE_REPORT.md`

## Deferred/non-blocking items

- Oracle parity topics remain open:
  - `CCT-046` (`ODG-044`)
  - `CCT-047` (`ODG-045`)
  - `CCT-048` (`ODG-046`)
- Kani obligations for this ladder remain non-blocking when deferred and are tracked in formal registers.

## Bottom line

The COM early-binding/type-library ladder is closed for the implemented deterministic subset and is now backed by explicit scripts, diagnostics taxonomy, conformance/perf artifacts, and terminal-gate documentation.
