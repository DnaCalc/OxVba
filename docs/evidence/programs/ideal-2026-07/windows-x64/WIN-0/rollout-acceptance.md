# WIN-0 Executable Rollout Acceptance

Date: 2026-07-11
Rollout bead: `bd-59co.3.1.1`
Status: accepted.

## Outcome and support-only boundary

WIN-0 now has a bounded support graph for x64 matrix authority, controlled
fixture recipes/hashes, owned Windows resources, current-stack/historical
residual classification, immutable environment capture, downstream handoff and
terminal queue certification. WIN-0 does not implement or certify COM, native,
VM3 or JIT capability.

The six canonical matrices retain 57 required Windows rows. Only
`WIN-ABI-CARRIER/WAC-TARGET-DEV-ENV` is owned by WIN-0 and may transition to
verified here. Its evidence/residual owner is `bd-59co.3.1.2`. The clean release
VM stays planned-blocking under `bd-59co.3.15.3`; the other 56 required rows
retain their existing delivery owners.

## Executable graph

| ID | Outcome | Commands/evidence | Downstream truth |
|---|---|---|---|
| `.3.1.3` | lock x64-only six-matrix stewardship | Windows control validator plus ownership/clause/trace validators; `x64-matrix-stewardship.md` | no capability transition |
| `.3.1.4` | canonical 57-row fixture manifest with distinct recipe/artifact/environment hashes | fixture sync/check validators; `controlled-fixture-inventory.md` | pending fixtures retain producers |
| `.3.1.5` | PID/registry/file/UIA/apartment/connection/callback owned-resource policy | owned-resource policy self-test; `owned-resource-policy.md` | no capability transition |
| `.3.1.6` | current-stack and historical residual ledger | residual/legacy/trace/truth validators; `current-stack-residual-migration.md` | every row keeps an exact owner |
| `.3.1.2` | implement generic immutable capture and characterize the dev oracle host | capture self-test, real dev capture, environment validator; `dev-oracle-environment.md` | only `WAC-TARGET-DEV-ENV` may verify; release flag false |
| `.3.1.7` | reconcile environment, fixture and downstream-owner handoffs | all new validators plus truth/governance; `environment-and-owner-handoff.md` | other rows preserve their then-current evidence-backed states and downstream owners |
| `.3.1.8` | terminal control-plane and successor certification | truth/governance/path/lint/cycles; `terminal.md` | exact current-only queue |

The first post-rollout Windows queue is intentionally `.3`, `.4` and `.5` in
parallel. Residual characterization follows matrix lock; the dev-host capture
follows matrix, fixture and resource policy; handoff and terminal work serialize
the final truth update.

## Repaired ownership routes

The rollout's five capability anchors no longer point at the dev-host bead:

- `WCC-PLAN-LATE` -> `bd-59co.3.4`;
- `WCE-PLAN-INCOMING` -> `bd-59co.3.6.4`;
- `WCS-LATE-INPROC` -> `bd-59co.3.7`;
- `WNI-PLAN-DECLARE` -> `bd-59co.3.10`; and
- `WNE-WRAPPER-EXE` -> `bd-59co.3.12`.

`bd-59co.3.1.2` owns only `WAC-TARGET-DEV-ENV`. Source/recipe hashes,
built-artifact hashes and environment hashes are separate truth fields.
Historical captures and controlled fixture self-tests cannot substitute for
current VM3/JIT/Excel/native product evidence.

The durable producer outputs are fixed before implementation:

- `docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv`;
- `docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md`;
- `docs/validation/IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv`; and
- `docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json`.

## Observable and cleanup policy

Applicable support evidence records result, full Err, effects, lifecycle/order,
transport and balance. All Windows mutation is journaled and owned: exact HKCU
subtrees, owned PIDs/dialogs/files/connections/callbacks only. Blanket registry,
dialog or process cleanup is prohibited. The current host remains useful for
development/oracle work but noncertifying for release.

## Acceptance record

The final independent WIN-0 review is clean. It rechecked all 57 row routes,
six-matrix ownership, x64-only scope, environment authority, fixture and
owned-cleanup boundaries, historical-evidence noncredit and the successor
queue. The full truth, governance, path, documentation, graph, lint, cycle and
negative-fixture gates pass; no capability is credited by this support epic.
