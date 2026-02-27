# WORKSET_2026-02-27_JIT_OPT_HOTPATHS_V35.md

## Profile
- ID: mvp-jit-optimizer-hotpaths-v35
- Ladder step: v35

## Purpose
Execute and stabilize profile scope: JIT and optimizer hotspot parity/performance work.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-jit-optimizer-hotpaths-v35 -OutputDir docs/evidence/profiles/v35
./scripts/run-formal.ps1 -ProfileScope mvp-jit-optimizer-hotpaths-v35
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
