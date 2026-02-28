# WORKSET_2026-02-27_STDLIB_MATH_FINANCIAL_CORE_V49.md

## Purpose
Execute profile 49 ($(System.Collections.Hashtable.Id)) for declared ladder scope.

## Scope
- Math/financial intrinsic subset: Abs, Int, Fix, Sgn, Round, Sqr, Sin, Cos, Log, Exp, FV, PV, PMT.
- Expand conformance corpus and formal obligations for this profile.
- Preserve VM/JIT parity via fallback for unsupported JIT instructions.

## Validation Commands
`powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-vm
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-math-financial-core-v49
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-math-financial-core-v49 -OutputDir docs/evidence/profiles/v49
./scripts/meta-check.ps1 -Fast -Conformance -Formal
`",
    ",
    
49 closes when scope fixtures are green and FO-V49-* obligations are green (or logged under non-blocking formal policy).
