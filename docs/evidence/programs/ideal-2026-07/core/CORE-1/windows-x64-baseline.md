# CORE-1 Windows x64 Development Baseline

Date: 2026-07-18

Bead: `bd-59co.2.2.10`

Effect: support

Clause: `CONF-QUALITY-001`, `RUNTIME-VALUE-001`, `SEC-BOUNDARY-001`

Canonical row: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and truth boundary

The complete Windows x64 development baseline passes through the canonical
versioned runner on the current development host at source `5fb85b47` (clean
checkout). Five gates of `core-profile-portable-gates-v1` passed in one run —
environment ledgers, meta-fast governance and strict Rust gates,
default-parallel and single-thread differential lanes, and truth
reconciliation — while the Linux-only gate was explicitly `not-applicable` on
Windows. The run produced a hashed command/result manifest
(`run-manifest.json` + `run-manifest.sha256`) with zero balance drift
reported by the balance fixtures. The standalone truth reconciliation command
also passes.

The bead text's acceptance spelling `run-core-profile-gates.ps1 -Profile
WindowsX64 -Mode Development` predates the versioned runner; the executed
equivalent is the platform-adaptive 6-gate plan
`run-core-profile-gates.ps1 -Mode NoArtifacts -RunId windows-x64-baseline-2026-07-18`,
which runs exactly the Windows x64 gate set on this host.

This is a development-host support transcript. It is **not** release
certification: it does not certify the pinned Linux x64 CI image, does not
close any of the five CORE-1 canonical rows, and makes no release claim.
Pinned Linux execution remains `bd-59co.2.2.11`; terminal reconciliation of
both platform transcripts remains `bd-59co.2.2.12`.

## Gate results (run `windows-x64-baseline-2026-07-18`)

| # | Gate | Result | Duration | Notes |
|---|---|---|---:|---|
| 1 | linux-runtime-environment | not-applicable | — | explicit N/A on Windows |
| 2 | windows-environment-ledger | passed | 3.4 s | environment ledger verified |
| 3 | meta-fast-no-artifacts | passed | 666.1 s | fmt, strict Clippy, workspace tests, governance battery |
| 4 | differential-default-parallel | passed | 660.4 s | 1,642 passed / 0 failed; ambient residual recorded (below) |
| 5 | differential-single-thread | passed | 486.1 s | 1,642 passed / 0 failed; lanes agree exactly |
| 6 | truth-reconciliation | passed | 30.1 s | validator battery green |

Run identities:

| Surface | SHA-256 |
|---|---|
| manifest (`ci/core-profile/gates-v1.json`) | `208547ecf7bef2f736bf48e9eb3a2f077fd6b6f2b1c6cca33054a8ea4ca3ef2a` |
| plan | `8f5e17feb96ce2e86907188cc3fe4383706898219af0df776063ea24041b8459` |
| run manifest | `a33a2d8244a07962ce6b3a900978ddbd89d13f13e0790ea77d4003cede0d4c5c` |
| summary | `af37bff3b4b2e1776276d0c4692f919f710a2a050e5687a7072317108fd6c8e1` |

Evidence bytes live under
`temp/no-artifacts/core-profile-gates/windows-x64-baseline-2026-07-18/` (not
committed by design; the committed identities above are the reference).

Standalone acceptance command:

```text
pwsh -NoProfile -File scripts/run-truth-reconciliation.ps1
run-truth-reconciliation: ok
```

## Gate-4 ambient descendant record

Gate 4 completed with one manifest-declared ambient residual, recorded per
`bd-59co.2.2.39` and terminated by kill-on-close cleanup
(`tree_cleanup=complete`):

```text
54680:C:\Program Files\Microsoft Visual Studio\18\Insiders\VC\Tools\MSVC\14.51.36231\bin\Hostx64\x64\vctip.exe
```

## Delivery path (failed attempts and routed successors)

Nine failed attempts (one of them an instrumented rerun) preceded the green
tenth run. Every failure was routed through an exact successor before the
next attempt:

| Attempt | Failure | Routed fix |
|---|---|---|
| 1 | untracked oracle artifact drift (clean-checkout check) | parked `artifacts/windows-x64/excel-vba-oracle` to ignored `temp/` |
| 2, 5 | stale evidence root from the previous aborted attempt | removed before relaunch |
| 3 | stale fixture source digests (ivaha commits changed fixture sources) | `bd-ivaha.32` re-sync |
| 4 | WIN-14 embedded manifest digest cascade | `bd-ivaha.33` re-sync |
| preflight | paused improvement track left 9 non-program items in global ready | `bd-ivaha.34` deferred to 2026-12-31 (reversible) |
| preflight | `cargo fmt` drift across 11 files | `bd-ivaha.35` + digest re-sync `bd-ivaha.36` |
| 6 | `descendant-processes-remained-after-direct-exit` after a green gate 4; out-of-order proxy/toolchain teardown past the 100 ms grace | `bd-59co.2.2.38` bounded 3,000 ms drain window |
| 7 | gate 3 red on `.38` truth surfaces (bead traceability acceptance, rollout residual wording, derived summary) | record updates + summary regen (no code change) |
| 8, 9 | same trip even with the drain window; instrumented rerun 9 captured the survivor: `vctip.exe` MSVC telemetry (+ `conhost.exe` console host), ambient and long-lived | `bd-59co.2.2.39` manifest-declared ambient descendant judgment |
| 10 | — | **all gates green** |

The `bd-ivaha.34` deferral is a queue-truth repair, not a product change; the
paused improvement items resume with `br update <id> --defer ""`.

## Six-axis evidence

| Axis | Observation |
|---|---|
| result | All six gates passed; both differential lanes report 1,642 passed / 0 failed and agree exactly. |
| full Err | Balance fixtures report complete `FinalErr` shapes (policy fixture retains error 5 with full fields); no flattened error observed. |
| side effects | Only the declared evidence root was written; the one ambient toolchain residual (`vctip.exe`) was recorded and terminated; no other process, file, registry or environment mutation persisted. |
| lifecycle/order | Every executed gate ran in plan order under Windows Job containment with `tree_cleanup=complete`; the ambient residual's identity was recorded before termination. |
| transport | Windows Job-object transport `job-object-v3` with identity-bound inputs; cargo lock serialization observed across cargo gates. |
| balance | Balance fixture protocol reports all four carrier counters (BSTR, object, SAFEARRAY, record) at zero drift inside the green meta and differential lanes. |

## Review and residuals

Fresh-eyes non-author review (explore subagent, 2026-07-18) verified every
recorded identity against the run manifest — gate statuses, durations, all
four SHA-256 values (recomputed), the exact 1,642/1,642 lane agreement, the
truth-reconciliation transcript, the gate-4 ambient residual, the
routed-successor bead states, and the no-release-claim boundary — and
returned changes-required with five findings, all repaired in this committed
revision: the delivery-path table regrouped attempts 8/9 under
`bd-59co.2.2.39`; this review record was added; gate-1 prose now states five
passes plus an explicit not-applicable; the balance axis names the four
carrier counters; the lifecycle axis scopes to executed gates. No finding
remains open.

Residuals (all owned, none hidden):

- The five CORE-1 canonical rows remain `planned`; this transcript is
  development-host support evidence only.
- Pinned Linux x64 CI execution: `bd-59co.2.2.11`.
- Terminal cross-platform reconciliation: `bd-59co.2.2.12`.
- The runner changes (`bd-59co.2.2.38`, `.39`) carry Linux code parity until
  `.11` executes.
- `BLK-BASELINE-001` in `CURRENT_BLOCKERS.md` stays open until `.10`-`.12`
  complete; this bead discharges the `.10` leg.
