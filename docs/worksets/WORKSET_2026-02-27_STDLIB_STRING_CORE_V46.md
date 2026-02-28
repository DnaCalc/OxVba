# WORKSET_2026-02-27_STDLIB_STRING_CORE_V46.md

## Purpose
Execute profile `v46` (`mvp-stdlib-string-core-v46`) for core string intrinsic subset in current runtime model.

## Scope
- Add expression-level intrinsic support for `Len`, `Left`, `Right`, `Mid`, `InStr`, `LCase`, `UCase`.
- Extend bytecode + VM interpreter with intrinsic ops for this subset.
- Preserve JIT safety by using existing unsupported-op fallback path where needed.

## Implementation Notes
- Current semantics are intentionally scoped to decimal-string-over-int projection.
- This profile establishes intrinsic execution plumbing and deterministic behavior before full BSTR/variant string surface expansion.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-string-core-v46
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-string-core-v46 -OutputDir docs/evidence/profiles/v46
./scripts/meta-check.ps1 -Fast -Conformance -Formal
```

## Completion Signal
`v46` closes when string-core subset fixtures are green and `FO-V46-*` obligations are green (or formally logged under non-blocking policy).
