# HAL Contract Assertion Hardening (Scaffold)

Status: `working-draft`  
Scope: build-gated pre/post assertion strategy for HAL adapter implementations.

## Goal

Establish lightweight, low-friction contract assertions now, while preserving a clear path to stronger governance and possible release-grade guards later.

This document complements:
- `HAL_CONTRACT_CLAUSE_CATALOG_V1.md`
- `HAL_FORMALIZATION_PROGRAM.md`
- `HAL_CONFORMANCE_SUITE.md`

## Build Modes

- `debug` builds:
  - assertions enabled by default via `debug_assertions`.
- `checked` builds:
  - assertions enabled by turning on crate feature `hal_contract_checks`.
  - intended for CI and deep conformance runs in non-debug profiles.
- release builds:
  - assertions disabled by default.
  - no maturity governance enforcement yet.

## Initial Scaffold (L0)

Current initial assertions focus on deterministic scaffolding domains where we already have stable behavior:
- file handle-state invariants in `standard.rs` (`open`, `close`, `seek`, `free_file`);
- UI virtualization branch postconditions (`msg_box`, `input_box`).

These checks are intentionally light and do not replace conformance tests.

## Hardening Ladder

1. L1: clause-linked assertions
   - annotate each assertion site with relevant clause IDs from `HAL_CONTRACT_CLAUSE_CATALOG_V1.md`.
   - require new stateful logic to include at least one local invariant assertion in checked/debug builds.

2. L2: policy + error-shape assertions
   - assert policy-denial precedence over adapter-specific behavior.
   - assert stable error category/shape at adapter boundaries before VM/host mapping.

3. L3: executable drift guards
   - couple machine-readable clause catalog to assertion presence checks and conformance coverage reports.
   - fail checked CI when clause coverage regresses.

4. L4: optional production guardrails (opt-in only)
   - keep default release path lean.
   - allow deployment profiles to enable selected invariant checks where safety priority justifies runtime cost.

## Notes

- This is scaffolding, not final governance policy.
- Contract/maturity enforcement remains advisory during exploratory HAL phase.
- Any assertion-induced behavior change must still be reflected in:
  - clause catalog status,
  - conformance expectations,
  - implementation-defined/uncertainty registers where applicable.
