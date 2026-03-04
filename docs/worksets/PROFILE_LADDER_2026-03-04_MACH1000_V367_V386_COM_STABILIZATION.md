# PROFILE_LADDER_2026-03-04_MACH1000_V367_V386_COM_STABILIZATION

## Range

- Ladder span: `v367..v386`
- Focus: COM client/server stabilization, deferred-gate hygiene, and terminal integrated closure.

## Steps

| Step | Focus | Deliverables |
|---|---|---|
| `v367` | stabilization baseline lock | residual issues classified |
| `v368` | regression sweep I | workspace regression pass |
| `v369` | regression sweep II | COM-focused regression pass |
| `v370` | regression sweep III | host policy + mode matrix sweep |
| `v371` | diagnostics normalization | consistent COM diagnostic wording |
| `v372` | conformance lane hardening I | lane scripts/probes hardened |
| `v373` | conformance lane hardening II | artifact schema hardened |
| `v374` | clause coverage audit | COM/HAL/PMR clause coverage cross-check |
| `v375` | docs hardening I | scope/conformance docs refined |
| `v376` | docs hardening II | evidence and index hygiene |
| `v377` | formal topic refresh | formal obligations and deferred-gate sync |
| `v378` | formal async kickoff | long-running formal jobs scheduled/deferred |
| `v379` | perf baseline capture I | COM-path micro-baseline capture |
| `v380` | perf baseline capture II | host-backed overhead snapshot |
| `v381` | integration suite touchpoint | integration suite COM scenario updates |
| `v382` | oracle backlog refresh | deferred-oracle topics reconciled |
| `v383` | gate dry-run | dry-run integrated checks |
| `v384` | gate stabilization | fixups from dry-run |
| `v385` | terminal gate prep | final artifact rollup |
| `v386` | terminal closure gate | final integrated gate + profile closure |

## Exit Criteria (`v386`)

1. COM client native lane is executable in Windows host-backed mode with deterministic fallback behavior preserved.
2. COM server scaffolding contracts and harness surfaces are documented and checked in reproducible scripts.
3. Integrated gate evidence for `v386` exists and reports `PASS`.
4. AutoRun control docs point to terminal gate `v386`.
