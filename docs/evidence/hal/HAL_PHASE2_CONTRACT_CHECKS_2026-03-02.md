# HAL Phase-2 Contract Checks (2026-03-02)

Status: `phase-2-increment`  
Scope: executable clause-mapped checks for HAL v1 contract catalog

## 1. Objective

Strengthen HAL formal verification lane by mapping executable checks to clause IDs and expanding direct clause tests for previously partial domains.

## 2. Implemented Additions

1. Clause-mapped conformance reporting:
- conformance probes now carry clause IDs;
- descriptor checks produce clause-level pass/fail records;
- report includes clause coverage aggregation.

Code:
- `crates/oxvba-hal/src/conformance.rs`
- `crates/oxvba-hal/src/bin/hal-conformance.rs`

2. Expanded clause tests in adapter suite:
- UI virtualization mode outcomes (`ScriptedResponses`, `Disabled`, `FailOnPrompt`);
- event pump support/unsupported behavior;
- process env deterministic mapping (`shell`, `environ`, `dir`);
- COM dispatch deterministic projection;
- dynamic link policy denial;
- diagnostics emit deterministic behavior;
- time/locale deterministic values;
- filesystem invariants (reuse, allocation progression, negative seek error).

Code:
- `crates/oxvba-hal/src/adapters/standard.rs`

3. Clause catalog status promotion updates:
- promoted multiple domain clauses from `implemented-partial` to `implemented-verified`.

Doc:
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md`

## 3. Verification Runs

- `cargo test -p oxvba-hal` (pass)
- `cargo test -p oxvba-host` (pass)
- `./scripts/run-hal-conformance.ps1 -SkipTests` (pass)

Generated artifact set:
- `docs/evidence/hal/HAL_CONFORMANCE_1772431514.md`
- `docs/evidence/hal/HAL_CONFORMANCE_1772431514.jsonl`

## 4. Remaining Gaps for Next Phase

1. Clause-level property tests for broader state-space exploration (beyond deterministic unit checks).
2. Event-pump fairness and queue semantics formal clauses.
3. Rich boundary-value model migration beyond `ValueToken = i32`.
4. Native Windows behavior clauses and verification strategy (H2 track).
