# WORKSET_2026-02-27_STDLIB_STRING_ADVANCED_V47.md

## Purpose
Execute profile `v47` (`mvp-stdlib-string-advanced-v47`) for advanced string intrinsic subset.

## Scope
- Add intrinsic expression support for `Split`, `Join`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp`.
- Extend bytecode + VM execution ops for advanced string subset under current runtime model.
- Capture conformance/formal evidence for advanced string behaviors.

## Implementation Notes
- Semantics continue to use decimal-string-over-int projection in this profile stage.
- JIT continues to use existing unsupported-op fallback path for newly introduced intrinsic instructions.

## Validation Commands
```powershell
cargo test -p oxvba-compiler
cargo test -p oxvba-host
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-formal.ps1 -ProfileScope mvp-stdlib-string-advanced-v47
./scripts/run-matrix.ps1 -ProfileScope mvp-stdlib-string-advanced-v47 -OutputDir docs/evidence/profiles/v47
./scripts/meta-check.ps1 -Fast -Conformance -Formal
```

## Completion Signal
`v47` closes when advanced string subset fixtures are green and `FO-V47-*` obligations are green (or formally logged under non-blocking policy).
