# WORKSET_2026-02-27_LANGUAGE_COVERAGE_AUDIT_V32.md

## Profile
- ID: mvp-language-coverage-audit-v32
- Ladder step: v32

## Purpose
Execute and stabilize profile scope: Language coverage index and audit tooling.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-language-coverage-audit-v32 -OutputDir docs/evidence/profiles/v32
./scripts/run-formal.ps1 -ProfileScope mvp-language-coverage-audit-v32
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
