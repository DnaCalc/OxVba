# Strict Package-Only VM Execution Workset

Status: `in-progress`
Date: 2026-05-28
Scope owner: OxVBA compiler/VM/package/JIT-entry

## Purpose

Make the executable semantic package the only accepted VM execution input.
Bytecode remains the package instruction stream, but bare `Bytecode` execution
must not remain a production VM path. Every supported behavior must be driven
by package descriptors, and any missing or unsupported package fact must reject
deterministically before VM execution or `ProcLoweringIr` entry.

This workset follows the typed VM metadata bundle completion workset:
[`WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md`](WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md).
That workset completed the package handoff gate, but it intentionally left raw
bytecode baselines, selected-only VM descriptor consumption, legacy bundle
compatibility, and deferred package facts as explicit residuals. This workset
owns removing that split.

## Target End State

1. All production VM execution, invocation, debug/session, host, launcher,
   callable, CLI, and test harness paths execute through `VmExecutionPackage`
   or `OxBundle`.
2. Public raw `Bytecode` VM execution APIs are removed rather than deprecated.
3. Old serialized bundle versions are rejected; the current bundle version must
   contain the strict package sections needed for execution.
4. Supported VM behavior consumes package descriptors. Unsupported or missing
   facts return deterministic support diagnostics instead of silently falling
   back to old VM semantics.
5. Typed fastpaths are no longer selected by public flags or environment
   toggles. Any optimized path is selected by package descriptors and recorded
   in VM/package evidence.
6. `ProcLoweringIr` and VM execution share the same package support query, so
   JIT lowering cannot infer semantics from bytecode patterns, snapshots,
   helper names, or VM internals.

## Current Split Catalog

The current repository is package-capable but not package-only.

- `VmExecutionPackage` and `OxBundle` package execution APIs exist, but raw
  public bytecode execution APIs also exist in `oxvba-vm`.
- Raw bytecode baselines are intentionally preserved in
  `crates/oxvba-vm/tests/package_identity_fixtures.rs`.
- Descriptor-driven execution is selected only for:
  - VMR06 direct `Long` argument to declared `Double ByVal` call entry;
  - VMR06 rank-1 fixed/static `LBound`/`UBound`;
  - VMR06 UDT owning-field cleanup as package evidence, not explicit cleanup
    stack execution.
- `docs/validation/VBA_VM_CONSUMPTION_EVIDENCE_SEED_TABLE_V1.csv` still
  classifies these gaps:
  - Optional `Variant` missing runtime materialization;
  - error/deopt cleanup consumption;
  - COM/native/export boundary consumption;
  - host-policy behavior-driving descriptor consumption.
- `oxvba-vm` still has public typed-fastpath selection APIs and environment
  defaulting.
- `bd-embl.4` removed the legacy bundle reader/backfill path: strict bundle
  format v15 is now the only accepted serialized package format.

Current progress:

- `bd-embl.1` published this workset and bead rollout.
- `bd-embl.2` has introduced the first `VmPackageSupportReport` surface. The
  strict VM gate rejects incomplete in-memory packages immediately. Existing
  deferred VM-consumption rows remain VM-execution warnings until the relevant
  delivery beads convert them into descriptor-driven behavior or hard rejects.
  The same rows are already `ProcLoweringIr` blockers.
- `bd-embl.3` removed public raw `Bytecode` execution from VM/JIT/launcher/debug
  surfaces. VM free functions, JIT stubs, launcher execution, debugger stepping,
  and package identity fixtures now enter through `OxBundle` or
  `VmExecutionPackage`; raw bytecode remains only as the package instruction
  stream and internal VM loop input.
- `bd-embl.4` bumps the bundle format to strict version 15. Current bundle
  serialization and deserialization require procedure metadata, manifest,
  export inventory, descriptor inventory, and project context sections. Versions
  1 through 14 reject deterministically instead of backfilling missing facts.

## Execution Beads

Parent bead: `bd-embl` (`Strict package-only VM execution`).

| Bead | Type | Outcome |
| --- | --- | --- |
| `bd-embl.1` | support | Publish this strict package-only contract and split catalog. |
| `bd-embl.2` | delivery | Add VM package support report and deterministic rejection gate. |
| `bd-embl.3` | delivery | Remove raw public VM execution APIs and update callers. |
| `bd-embl.4` | delivery | Reject legacy bundle versions under strict package completeness. |
| `bd-embl.5` | delivery | Replace typed-fastpath toggles with descriptor-selected execution. |
| `bd-embl.6` | delivery | Close or reject call and value-state descriptor consumption gaps. |
| `bd-embl.7` | delivery | Close or reject array, UDT, string, and cleanup descriptor gaps. |
| `bd-embl.8` | delivery | Close or reject error, deopt, cleanup, and host-policy gaps. |
| `bd-embl.9` | delivery | Close or reject COM, native, and exported-callable boundary gaps. |
| `bd-embl.10` | support | Terminal review and package/JIT handoff audit. |

The dependency path is sequential: `bd-embl.1` through `bd-embl.10`.

## Required Checks

Every bead closure requires:

```text
cargo fmt --check
./scripts/check-governance.ps1
git diff --check
```

Behavior-affecting beads must also run the impacted crate tests plus:

```text
cargo test -p oxvba-vm --test package_identity_fixtures --quiet
./scripts/run-jit-v2-tracer-fixtures.ps1
br dep cycles
```

Bundle-format changes must run compiler bundle tests. Host/launcher/debug
changes must run their impacted host or launcher tests. Before terminal closure,
run `./scripts/meta-check.ps1 -Fast -NoArtifacts`.

## Completion Definition

This workset is complete only when:

- no production VM, host, launcher, CLI, callable, debug, or session path can
  execute bare `Bytecode`;
- all execution paths construct or receive a strict current-version executable
  semantic package;
- old bundle versions reject with deterministic diagnostics;
- typed-fastpath public/env toggles are gone;
- support-query diagnostics cover every unsupported descriptor gap that remains;
- VM and `ProcLoweringIr` entry use the same support report;
- remaining unsupported behavior is rejected or classified, not inferred;
- docs, completion map, VM contract, tracer matrix, and bead state agree.
