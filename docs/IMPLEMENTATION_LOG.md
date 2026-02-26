# Implementation Log

## 2026-02-26
- Bootstrapped full Cargo workspace and crate boundaries.
- Added baseline CI, meta-check scripts, and core docs.
- Implemented first executable runtime primitives:
  - 16-byte `Variant` container with typed helpers.
  - Basic coercion and arithmetic paths.
- Added baseline unit tests across syntax/compiler/host/runtime.
- Added initial decision-table CSV scaffolds in `tables/`.
- Upgraded lexer tokenization (keywords, identifiers, numbers, strings, comments, trivia).
- Added IR lowering consistency tests (HIR -> MIR -> CFG).
- Added host root-object registration API scaffolding.
- Hardened scripts so native command failures stop the pipeline.
- Added architecture companion docs (`ARCHITECTURE`, `IR_DESIGN`, `BYTECODE_FORMAT`, `VM_ARCHITECTURE`).
- Added smoke execution assets and script (`conformance/tests/smoke.bas`, `scripts/run-smoke.ps1`).
