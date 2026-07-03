# JIT M4-1 IR-Prep Evidence

Date: 2026-07-03

Scope: `bd-h4oh.2` / `M4-1` IR-prep passes.

## Landed

- `OxFunc.temps: Vec<OxTy>` is populated by elaboration and carried by `.oxi` image version 2.
- Escape analysis lives in `oxvba-oxir`, updates `OxLocal.escaped`, and exposes escaped-temp facts as analysis-only data.
- Assign normalization is always-on before VM/JIT consumption; representation-changing `Assign` sites now go through `Box`, `Unbox`, or `Coerce`, with vm3 execution for `Box`/`Unbox`.
- Static fixed-array declarations lower to `ArrayShape::Fixed { rank }`; inline UDT fixed-array fields remain the existing conservative `Variant` path until record-layout typing work owns them.
- The verifier now checks local/global/temp references, `StmtBoundary` temp floors, recomputed escape flags, representation-preserving `Assign`, terminator operands, `FaultDispatch`/`GoSub` successor domains, and COM/table references.
- ParamArray alias sufficiency is confirmed for caller-side copy-out through `ArrayLiteral { aliases }`; no IR extension was needed.

## Compatibility Boundaries

- Raw `Assign` from `Const Empty` is preserved as a VM-visible reset/finalization
  sentinel, notably for `For Each` exhaustion. Other representation-changing
  assignments must route through explicit `Box`, `Unbox`, or `Coerce`.
- `Object(Untyped) <- Variant` remains a bounded representation-preserving exception
  only for unresolved UDT record carriers while OxIR lacks full record-layout identity
  threading. Follow-up bead `bd-h4oh.9.1` owns removing this exception under M4-7.

## Checks

- `cargo fmt`
- `cargo test -p oxvba-oxir -- --nocapture`
- `cargo test -p oxvba-vm3 -- --nocapture`
- `cargo test -p oxvba-differential --test call_argument_binding_vm3 paramarray -- --nocapture`
- `cargo test -p oxvba-differential --test option_base_vm3 paramarray_stays_zero_based_under_option_base_one -- --nocapture`
- `cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`
- `cargo test --workspace --no-run`

## Non-Blocking Formal Lane

`pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet -NoArtifacts`
could not start because this Linux environment has no `pwsh` or `powershell` executable on
`PATH`. The formal lane remains non-blocking under the current ladder policy and is tracked in
`docs/evidence/formal/EXTENDED_TODO.md`.
