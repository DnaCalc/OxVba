# WORKSET_2026-02-27_JIT_CALLS_V24.md

## Profile
- ID: mvp-jit-calls-v24
- Ladder step: v24

## Purpose
Execute and stabilize profile scope: JIT call-flow parity subset.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-jit-calls-v24 -OutputDir docs/evidence/profiles/v24
./scripts/run-formal.ps1 -ProfileScope mvp-jit-calls-v24
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
