# WORKSET_2026-02-27_STRING_BSTR_V14.md

## Profile
- ID: mvp-string-bstr-v14
- Ladder step: v14

## Purpose
Execute and stabilize profile scope: BSTR/string semantics subset evidence.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-string-bstr-v14 -OutputDir docs/evidence/profiles/v14
./scripts/run-formal.ps1 -ProfileScope mvp-string-bstr-v14
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
