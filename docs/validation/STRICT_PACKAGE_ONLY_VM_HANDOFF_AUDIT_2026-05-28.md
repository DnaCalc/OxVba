# Strict Package-Only VM Handoff Audit

Status: `handoff-passed`
Date: 2026-05-28
Workset:
[`../worksets/WORKSET_2026-05-28_STRICT_PACKAGE_ONLY_VM_EXECUTION.md`](../worksets/WORKSET_2026-05-28_STRICT_PACKAGE_ONLY_VM_EXECUTION.md)
Terminal bead: `bd-embl.10`

## Scope

This audit is the terminal review for the strict package-only VM execution
workset. It verifies that VM execution entry points no longer expose bare
`Bytecode` as the public execution unit, that legacy bundle backfill and
public/environment typed-fastpath toggles are gone, and that the VM and
`ProcLoweringIr` entry share `VmPackageSupportReport` for unsupported package
facts.

This is not executable JIT evidence. It is the package/JIT handoff gate for the
next JIT layer: `ProcLoweringIr` may consume package facts, but it must reject
or classify every path still represented by unsupported VM-consumption evidence
or tracer-matrix blockers.

## Automated Child Closure Audit

Command:

```powershell
$ids = 1..9 | ForEach-Object { "bd-embl.$_" }
$issues = br show @ids --json | ConvertFrom-Json
$failures = @()
foreach ($issue in $issues) {
    if ($issue.status -ne 'closed') { $failures += "$($issue.id):status=$($issue.status)" }
    if ([string]::IsNullOrWhiteSpace($issue.close_reason)) { $failures += "$($issue.id):missing-close-reason" }
    $checkHits = ([regex]::Matches($issue.close_reason, 'cargo|test|scripts|meta-check|check-governance|git diff|clippy')).Count
    if ($checkHits -lt 3) { $failures += "$($issue.id):insufficient-check-citations=$checkHits" }
}
if ($failures.Count) { $failures; exit 1 }
$issues | ForEach-Object {
    "AUDIT_CHILD_CLOSURE $($_.id) status=$($_.status) close_reason_checks=" +
        ([regex]::Matches($_.close_reason, 'cargo|test|scripts|meta-check|check-governance|git diff|clippy')).Count
}
```

Result:

```text
AUDIT_CHILD_CLOSURE bd-embl.1 status=closed close_reason_checks=3
AUDIT_CHILD_CLOSURE bd-embl.2 status=closed close_reason_checks=16
AUDIT_CHILD_CLOSURE bd-embl.3 status=closed close_reason_checks=31
AUDIT_CHILD_CLOSURE bd-embl.4 status=closed close_reason_checks=3
AUDIT_CHILD_CLOSURE bd-embl.5 status=closed close_reason_checks=14
AUDIT_CHILD_CLOSURE bd-embl.6 status=closed close_reason_checks=17
AUDIT_CHILD_CLOSURE bd-embl.7 status=closed close_reason_checks=16
AUDIT_CHILD_CLOSURE bd-embl.8 status=closed close_reason_checks=17
AUDIT_CHILD_CLOSURE bd-embl.9 status=closed close_reason_checks=21
```

## Truth Surface Reconciliation

| Surface | Audit result |
|---|---|
| VM public execution | `oxvba-vm` public helpers accept `VmExecutionPackage` or `OxBundle`. The VM instruction loop still receives package bytecode internally after package metadata is loaded. |
| Host/project/session execution | Project, callable session, bundle, and debug preparation paths build or receive `OxBundle`-backed packages. Source-snippet snapshot execution remains a bounded non-strict evidence harness; strict support reporting classifies `VmPackageOrigin::InMemory` as `PACKAGE-INMEMORY-NOT-STRICT`. |
| Legacy bundle format | `FORMAT_VERSION = 16`; serialized versions 1 through 15 reject with `unsupported legacy bundle version ...`, and current-version bundles missing strict sections reject with `BUNDLE-STRICT-MISSING-SECTIONS`. |
| Fastpath selection | Active crate scan finds no `execute_with_typed_fastpaths`, `OXVBA_DISABLE_TYPED_FASTPATH`, public typed-fastpath selector, raw bytecode runner, or `execute_bytecode` symbol. Optimized VM paths are selected from package descriptors and recorded as `descriptor-selected-fastpaths`. |
| VM/JIT support query | `VmExecutionPackage::support_report_for_vm_execution` and `support_report_for_proc_lowering_ir` both call `package_support_report`; strict VM execution calls `ensure_supported_for_vm_execution`; `ProcLoweringIr` rows block through the same support-report data. |
| VM-consumption ledger | `VBA_VM_CONSUMPTION_EVIDENCE_SEED_TABLE_V1.csv` has no deferred boundary-consumption row. Selected rows remain supported warnings; unsupported rows reject strict VM execution and block `ProcLoweringIr`. |
| Tracer matrix | TB01 through TB09 remain VM/package evidence seeds with explicit JIT-entry blockers. No row permits lowering to invent package-absent facts. |
| JIT entry specs | `JIT_V2_PROC_LOWERING_IR_V1.md` and the JIT planning workset require package-owned facts and the shared support report before lowering. |

## Residual Classification

The audit intentionally does not remove every `metadata-missing`,
`VM-limitation`, `interop-limitation`, `oracle-required`, or `test-shortcoming`
label. Those labels remain the JIT-entry safety rails.

Remaining strict support blockers are explicit, not silent fallbacks:

| Row | Classification |
|---|---|
| `CALL-OPTIONAL-MISSING-VARIANT` | Strict VM rejection until missing-state runtime/oracle behavior is descriptor-consumed. |
| `CALL-BYVAL-COERCION-UNSUPPORTED` | Strict VM rejection outside the selected direct `Long` to declared `Double ByVal` call-entry slice. |
| `ARRAY-DESCRIPTOR-UNSUPPORTED` | Strict VM rejection for multi-rank, incomplete-bound, and owning-element cleanup shapes. |
| `UDT-LAYOUT-CLEANUP-UNSUPPORTED` | Strict VM rejection for executable UDT layout/copy/drop/cleanup beyond the selected evidence-only cleanup map. |
| `STRING-CLEANUP-UNSUPPORTED` | Strict VM rejection for BSTR/string cleanup and lifetime shapes not yet package-owned. |
| `ERROR-CLEANUP-DEOPT-UNSUPPORTED` | Strict VM rejection for non-selected error/resume/fallible-helper/deopt cleanup maps. |
| `BOUNDARY-CONSUMPTION-UNSUPPORTED` | Strict VM rejection for COM/native boundary ABI result/writeback/cleanup/error-policy execution. |
| `HOST-POLICY-CONSUMPTION-UNSUPPORTED` | Strict VM rejection for behavior-driving host-policy descriptor evaluation. |

Selected supported metadata slices remain valid VM evidence but are not broad
JIT permission: VMR06 call-entry coercion, VMR06 static array bounds, VMR06 UDT
owning-field cleanup evidence, VMR08 Err reset, VMR08 call-frame deopt, VMR09
native descriptor identity, VMR09 early-bound COM selector identity, and VMR09
exported-callable descriptor identity.

## Scan Record

Active-code negative scan:

```powershell
rg -n "execute_with_typed_fastpaths|OXVBA_DISABLE_TYPED_FASTPATH|pub fn execute\(|pub fn execute_with_|pub fn invoke_procedure_with_i32_args\(|run_bytecode|execute_bytecode" crates -g"*.rs"
```

Result: no active crate matches.

Bounded in-memory package scan:

```powershell
rg -n "VmExecutionPackage::new\(" crates -g"*.rs"
```

Result: matches are limited to VM unit/identity evidence and the host
single-source snapshot harness. The strict support query rejects that package
origin through `PACKAGE-INMEMORY-NOT-STRICT`; project/session/bundle/debug/JIT
paths enter through `OxBundle` or `VmExecutionPackage::from_bundle`.

Legacy bundle scan:

```powershell
rg -n "FORMAT_VERSION: u32 = 16|unsupported legacy bundle version|BUNDLE-STRICT-MISSING-SECTIONS" crates/oxvba-compiler/src/bundle.rs docs -g"*.rs" -g"*.md"
```

Result: current strict format and deterministic rejection diagnostics are the
only active bundle-version paths.

## Final Check Record

The terminal state was checked with:

```text
cargo fmt --check
cargo test -p oxvba-host package_identity --lib -- --nocapture
cargo test -p oxvba-host --lib
cargo test -p oxvba-host --test jit_v2_tracer_vm_seed
cargo test -p oxvba-vm --lib strict_package -- --nocapture
cargo test -p oxvba-vm --test package_identity_fixtures
cargo check -p oxvba-vm -p oxvba-host -p oxvba-jit -p oxvba-launcher -p oxvba-debug --all-targets
cargo clippy -p oxvba-vm -p oxvba-host -p oxvba-jit -p oxvba-launcher -p oxvba-debug --all-targets -- -D warnings
./scripts/run-jit-v2-tracer-fixtures.ps1
./scripts/run-formal.ps1 -ProfileScope mvp-typed-execution-fastpaths-v85
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
cargo test -p oxvba-host --test com_early_project_end_to_end mixed_bound_project_executes_registered_access_jet_ado_database_subset -- --nocapture
br dep cycles
br lint bd-embl bd-embl.10
git diff --check
```

The formal lane reported 242 pass, 4 non-blocking Kani skips, and 0 failures.
The first `meta-check` pass stopped on
`mixed_bound_project_executes_registered_access_jet_ado_database_subset` with a
native Access/ADO provider COM aggregation exception; the exact failed test
passed on immediate rerun. A second full `meta-check` pass
(`run-id=20260528T102711Z`) completed successfully, including the previously
failing Access/ADO lane. All package-specific checks listed above passed.

## Handoff Decision

The strict package-only VM execution workset passes the package/JIT handoff
gate.

Rules for the next phase:

- VM execution and JIT entry must consume the executable semantic package as
  the semantic fact source; raw bytecode remains only the package instruction
  stream.
- `ProcLoweringIr` entry must call the shared support-report surface and reject
  or classify unsupported rows before lowering.
- Remaining tracer rows are not executable JIT permission until their named
  package, VM/oracle, verifier, and differential evidence gates pass.
