# Error Codes

Catalog of current OxVba error-code families. The clean pipeline is
`oxvba-bind` → `oxvba-oxir` → VM3/JIT; structured diagnostics are carried
through `oxvba-diagnostics`. This catalog does not itself prove that every
producer supplies complete source locations or VBA-compatible timing.

## Scope

This file catalogs stable error and diagnostic code families that appear in
current source. It does not catalog HAL clause IDs (`HAL-TIME-002` etc.),
profile/workset identifiers, or generic Rust error strings with no stable
prefix.

## Status Register

| Family | Source status | Authority | Notes |
|---|---|---|---|
| `SYN-E-*` | emitted | `crates/oxvba-syntax/src/parser.rs` (`ParseError::to_diagnostic`) | Syntax parse diagnostics with byte offsets and, when source text is available, line/column/snippet rendering. |
| `SYM-E-*` | emitted | `crates/oxvba-symbol/src/model.rs` (`SymbolModelError::to_diagnostic`) | Symbol-model failures such as duplicate declarations, unknown internal scopes, and preprocessing failures. Parse failures preserved by the symbol layer keep their `SYN-E-PARSE` code. |
| `BIND-E-*` | emitted | `crates/oxvba-bind/src/error.rs` (`BindError::to_diagnostic`) | Binder failures: unresolved names, invalid assignments, malformed CST shapes, unsupported lowered constructs, and symbol-model handoff failures. |
| `ELAB-E-*` | emitted | `crates/oxvba-host/src/engine.rs`, `crates/oxvba-oxir` | Core IR-to-OxIR elaboration/internal compiler failures. |
| `PROJ-E-*` | emitted | `crates/oxvba-project/src/error.rs` (`BasProjError::to_diagnostic`) | `.basproj`/`.vbp` project loading, discovery, entry-point, reference, export, and COM-server project failures. |
| `RUN-E-*` | emitted | `crates/oxvba-host/src/engine.rs`, `crates/oxvba-jit/src/lib.rs` | Backend admission, compile, unsupported, fault and runtime orchestration diagnostics. VBA runtime errors still carry numeric VBA error state structurally. |
| `VM3-E-*` | emitted | `crates/oxvba-vm3`, `crates/oxvba-host/src/engine.rs` | VM3 image, link, malformed, unimplemented and runtime/fault diagnostics. |
| `BUILD-E-*` | emitted | `crates/oxvba-build`, `crates/oxvba-cli` | Wrapper/build planning and artifact-emission diagnostics. |
| `HOST-E-*` | emitted | `crates/oxvba-cli/src/main.rs` | CLI/host orchestration failures such as runner bootstrap resolution. |
| VBA runtime error numbers | emitted subset | `crates/oxvba-runtime`, `crates/oxvba-rt-abi`, `crates/oxvba-vm3`, `crates/oxvba-jit` | Numeric VBA codes and Err state carried structurally. Full line/Erl, metadata and propagation parity remains part of the core workset. |
| `HAL-E-*` | emitted | `crates/oxvba-hal/src/error.rs`, `crates/oxvba-hal/src/project.rs` | Stable HAL capability/policy/adapter/profile/project-boundary failures. Core factories emit `HAL-E-CAP-UNAVAILABLE`, `HAL-E-POLICY-DENIED`, `HAL-E-ADAPTER-FAULT`, `HAL-E-UNSUPPORTED-PROFILE`; project catalog/reference callbacks also emit stable `HAL-E-PROJ-*` codes. |
| `COM-E-*` | emitted subset | `crates/oxvba-com`, `crates/oxvba-hal/src/adapters/standard` | COM activation, typelib, event, value, lifecycle and dispatch failures for current routes. |
| `VBP-E-*` | emitted subset | `crates/oxvba-project` | Explicit diagnostics for the bounded supported `.vbp` adapter and rejected VB6-only forms. |

## Removed With The Legacy Stack

`PMR-E-*`, `PMR-I-*`, and the old legacy-stack `BIND-E-*` meanings were emitted by the deleted
`oxvba-compiler`/legacy host and no longer exist in source (see git history
and `docs/archive/`). Equivalent project-model and early-binding legality
checks now live in `oxvba-symbol`/`oxvba-bind`. New clean-stack `BIND-E-*`
codes are owned by `oxvba-bind` and must not be treated as resurrection of the
legacy PMR diagnostic catalog.

## Non-Families Often Confused With Error Codes

Real identifiers that are not error-code families:
- `HAL-TIME-*`, `HAL-DES-*`, `HAL-GEN-*`, `HAL-DYN-*` (HAL conformance clause IDs)
- profile IDs such as `v506`
- evidence/workset/run IDs
