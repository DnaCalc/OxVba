# OxVBA

OxVBA is a full-fidelity VBA 7 runtime engine in Rust, built for compatibility, correctness, and high performance.

## Core Documents
- `CHARTER.md` — project mission, values, scope, and clean-room rule.
- `OPERATIONS.md` — execution and development doctrine for this repo.
- `MACH1000_PLAN.md` — detailed architecture and phased implementation plan.
- `docs/IMPLEMENTATION_LOG.md` — rolling implementation progress log.

## Top-Level Layout
- `crates/` — Rust workspace crates (runtime, compiler, VM, JIT, host, etc.).
- `docs/` — supporting documentation and archived planning inputs.
- `synthesis/` — synthesis workflow docs and run artifacts.
- `scripts/` — local automation (`meta-check`, `docs-check`).
- `formal/` — formal methods assets (Lean/Kani) as they are introduced.
- `conformance/` — conformance tests, harnesses, and golden outputs as they are introduced.
- `tables/` — decision-table artifacts (coercion/arithmetic/comparison) as they are introduced.

## Context
OxVBA is part of the broader DNA Calc program and aligns with Foundation doctrine, while remaining an independent project with its own charter and operations model.

## Quick Verification
```powershell
./scripts/meta-check.ps1 -Fast
./scripts/run-smoke.ps1
./scripts/run-conformance.ps1
./scripts/run-matrix.ps1
```

Optional:
```powershell
./scripts/meta-check.ps1 -Fast -Conformance
./scripts/meta-check.ps1 -Fast -Matrix
```
