# WIN-14 Excel/VBA Oracle Supervisor Authority Evidence

Date: 2026-07-14

Profile: Windows x64 development/oracle host

Bead: `bd-59co.3.15.6`

Status: current harness implementation evidence; noncertifying

## Scope and credit

This evidence covers the offline authority, containment, protocol, and fixture checks for the Excel/VBA oracle supervisor. The repair pass did not launch Excel or the VBE. It therefore claims no live five-case success, Excel/VBA behavioral result, canonical matrix row, release, certification, or capability credit.

The later live run must still use the characterized Windows x64 / 64-bit Excel development-oracle environment with `-NoMatrixUpdate`, capture the selected default five cases, and run the bounded `runtime-unhandled-modal` diagnostic when that transcript is needed. Release certification remains assigned to the clean pinned certification VM.

## Authority boundaries implemented

- Fallback process cleanup opens one `SafeProcessHandle` and uses that same retained handle for executable/start-time identity, termination, and wait. A PID is never reopened between validation and action. Missing, reused, and same-instance-conflict identities fail closed; only the exact retained process can be terminated.
- Each live run uses a GUID-based default run ID and an atomic `FileMode.CreateNew`, `FileShare.None` claim held through the run. A top-level `finally` releases only that exact claim on every post-claim success or failure path while preserving failed evidence. Reused run directories and concurrent equal run IDs are rejected; different IDs remain isolated. The run directory is not created through `-Force`.
- The supervisor assigns the waiting worker to a kill-on-close Job and positively queries membership before publishing containment authority. The worker validates the exact Boolean membership assertion before any case mutation.
- VBE command ID 578 execution is enclosed by exact active project, active module, active code-pane, and injected-source-hash snapshots immediately before and after `Execute()`.
- Active project/module/code-pane values inspected by those snapshots are borrowed RCW aliases. The helper never final-releases them; explicit COM release remains at the case-owner boundary so authority inspection cannot invalidate the live project between snapshots or before runtime measurement.
- Compile-error acceptance uses only the immutable guardian observation written before dismissal. The expected token and exact injected source line are case-bound. Post-dismiss COM selection is retained as diagnostic data only and cannot repair the observation.
- Guardian controls use a strict v2 schema with exact run/case/operation identity, positive sequence, phase, JSON Boolean dismissal authority, and timestamp. Invalid controls are durably reported and never arm an operation or authorize dismissal. Compile, run, and cleanup use separate dialog-classification allowlists.
- Every operation requires a guardian arm acknowledgement and a strictly sequenced heartbeat after the invocation completes. Evidence flush fails if the heartbeat does not span the invocation, including the ready/benign-then-hang shape.
- Job tests cover explicit termination, dispose-only handle closure, abrupt supervisor death, and children created before their ownership ledger entry. In every case the kill-on-close Job removes the contained worker/child.
- Runtime adjudication records measured VBOM access, invoked-entry existence, nested macro target existence, and configured AutomationSecurity separately. Runnable entry is true only after an observed qualified return, case-specific return sentinel, or owned unhandled-runtime modal; configuration is never used as a substitute. The default caught full-`Err` case remains. A separate live diagnostic case raises an unhandled VBA error so an owned runtime-error modal transcript can be captured without weakening the default self-test.
- Ownership, guardian, result, and plan evidence is bound to the selected case set. Case identities are unique. Ledger validation checks JSON scalar types, timestamps, phases, sequence monotonicity, capture-before-dismiss ordering, requested buttons, the actual dismissed button, and exact case-set/count binding where one record per case is required.

## Offline verification

Command:

```powershell
./scripts/test-excel-vba-oracle.ps1
```

Result on 2026-07-14:

```text
test-excel-vba-oracle: PASS
```

The focused suite completed in approximately eight seconds. It parsed every supervisor script and exercised:

- exact retained-handle termination plus adversarial same-PID/path rejection, production worker cleanup coverage, and a mutation that reopens the PID;
- atomic equal-run rejection, distinct-run isolation, and forced post-claim failure proving claim release while stale evidence remains fail-closed;
- compile/run cross-phase dialog rejection and string/numeric JSON Boolean mutations;
- immutable token/line, dismissal-button, case, PID-type, schema, duplicate, malformed, and timestamp/event-ledger mutations;
- operation coverage failure for an arm plus only a pre-completion heartbeat;
- Job containment for pre-ledger children, dispose-only closure, and abrupt supervisor death;
- plan-only selection of the five default cases and declaration of the bounded unhandled-runtime diagnostic;
- source-order checks for Job membership before authority, arm acknowledgement before invocation, exact compile snapshots around command execution, and evidence-gated acceptance.

Post-test inspection found no `oxvba-oracle-*` temporary directories and no surviving test worker/child process. The repository diff passed `git diff --check`.

An independent fresh-eyes follow-up review was clean after the exception-safe claim release, observed-entry adjudication, retained worker cleanup, and borrowed-RCW ownership repairs. Its independent focused rerun also passed, with zero temporary roots and zero Excel processes observed.

## Residual live work

This evidence does not establish that a particular Office build exposes the exact expected UIA token/line or runtime dialog shapes. Before closing the supervisor bead:

1. run the default five-case harness against the characterized development/oracle host with Excel/VBE modal interception active;
2. inspect the compile-failure and intrinsic-shadow token/line captures, ambiguous macro transcript, full `Err` state, lifecycle ordering, and zero owned residuals;
3. run the `runtime-unhandled-modal` diagnostic and retain its owned runtime-error modal transcript;
4. record the live run separately without advancing certification or canonical capability rows.
