# WORKSET_2026-02-27_BATCH_V27_V36.md

## Purpose
Define the next long execution batch after `v26` with emphasis on:
- formal reliability and Kani operational maturity,
- full language coverage closure,
- performance work on proven-correct hot paths.

## Batch Span
- Profiles: `v27` through `v36`
- Source of truth: `docs/worksets/PROFILE_LADDER_2026-02-27_MACH1000.md`

## Batch Highlights
1. `v27-v31`: formal lane reliability, Kani unblock/capacity, COM `VARIANT` conformance, boundary marshalling.
2. `v32-v34`: explicit language coverage audit and closure milestones.
3. `v35-v36`: JIT/optimizer hotspot expansion with parity and performance gates.

## Operational Pattern
For long formal steps, use async orchestration:
- `./scripts/run-formal-kani-async.ps1 -Action Start -Name <run-name> -ProfileScope <profile>`
- `./scripts/run-formal-kani-async.ps1 -Action Status -Name <run-name>`
- `./scripts/run-formal-kani-async.ps1 -Action Tail -Name <run-name>`
- `./scripts/run-formal-kani-async.ps1 -Action Wait -Name <run-name>`

## Completion Signal
Batch completes when profile `v36` gate criteria are satisfied and evidence artifacts are updated.
