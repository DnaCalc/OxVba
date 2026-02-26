# FORMAL.md

## Scope
Formal artifacts in OxVBA are currently scaffolded for staged adoption.

## Lean scaffold
Location: `formal/lean/`

Current files:
- `OxVba/VarType.lean`
- `OxVba/Coerce.lean`
- `OxVba/Arithmetic.lean`
- `OxVba/RefCount.lean`

## Kani scaffold
Kani harness placeholders are introduced in runtime and VM code under `#[cfg(kani)]` blocks and expanded as unsafe-heavy paths mature.
