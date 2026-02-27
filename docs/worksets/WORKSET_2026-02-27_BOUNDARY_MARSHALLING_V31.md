# WORKSET_2026-02-27_BOUNDARY_MARSHALLING_V31.md

## Profile
- ID: mvp-boundary-marshalling-v31
- Ladder step: v31

## Purpose
Execute and stabilize profile scope: Boundary marshalling roundtrip and failure-surface checks.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-boundary-marshalling-v31 -OutputDir docs/evidence/profiles/v31
./scripts/run-formal.ps1 -ProfileScope mvp-boundary-marshalling-v31
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
