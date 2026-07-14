# WIN-14 Excel/VBA Oracle Supervisor Authority Evidence

Date: 2026-07-14

Profile: Windows x64 development/oracle host

Bead: `bd-59co.3.15.6`

Status: current harness implementation evidence; noncertifying

## Scope and credit

This evidence covers the authority, containment, protocol, fixture, and attachment repair checks for the Excel/VBA oracle supervisor. One bounded, targeted `success` probe launched Excel after containment was established. It failed before VBE access or workbook mutation and therefore claims no live case success, Excel/VBA behavioral result, canonical matrix row, release, certification, or capability credit.

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
- Ownership, guardian, result, and plan evidence is bound to the selected case set. Case identities are unique. Ledger validation checks JSON scalar types, timestamps, phases, sequence monotonicity, capture-before-dismiss ordering, requested buttons, the actual dismissed button, and exact case order/count where one record per case is required.
- Excel attachment enumerates top-level windows for the exact directly launched PID and their exact-PID descendants. Enumeration has a separate 512-window authority bound: native truncation, API failure, foreign PID, or malformed enumeration fails before attachment. The diagnostic retains at most 128 last-window descriptions and 256 HRESULT observations without limiting the complete admitted candidate set. `OBJID_NATIVEOM` is accepted only from `EXCEL7`, and returned `Application.Hwnd` must resolve back to the launch PID. PID/start/path, exact launch argv, bootstrap identity, HWND/class/title/visibility, HRESULT, enumeration outcome, and result are recorded. Visible owned startup/modal surfaces fail closed without broad dismissal.
- Each case creates a byte-deterministic, macro-free, five-part OpenXML `.xlsx` bootstrap package. The bootstrap validator pins the part order/set, XML and content types, root/workbook relationship identities and target closure, macro absence, and recorded hash. `ProcessStartInfo.ArgumentList` must contain exactly `/x` and the bootstrap path—no string-concatenated command and no `/n`. The attached process must open exactly that workbook, `Workbooks.Add` is not used, and missing or modified bootstrap bytes fail before launch or after close-without-save.
- A pure post-cleanup validator binds the result document to exact run, worker PID, containment token and authority, diagnostic mode, case schema and JSON types, selected-case and ownership-ledger order, aggregate status, durable PID records, cleanup status, and exit envelope. Process exit alone never bypasses ledger binding. Its only partial-result disposition is exactly the first selected case failing with `harness-error`, no durable owned PID, nonempty transport, both ledgers exactly empty, and exit code 1 after successful Job cleanup. A durable-record failure stops further launches even if an exact PID was already observed; that one process remains under the supervisor Job.

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
- deterministic OpenXML bootstrap generation, exact ZIP part order/set, XML well-formedness, ordinary `.xlsx` content types, absence of macro parts, hash-consistent broken-relationship rejection, missing/modified package rejection, exact two-argument launch/no-`/n`, no `Workbooks.Add`, exact-PID native window enumeration, truncation rejection, separately bounded diagnostics, result-PID verification, durable-record early stop, and explicit empty-ledger binding;
- behavioral post-cleanup envelopes for complete success and five-case first-case transport, plus foreign worker/token, string Boolean, case schema/type, result/ledger order, aggregate, exit, partial-result, nonempty-ledger, missing/modified bootstrap, and process-exit-only mutations.

Post-test inspection found no `oxvba-oracle-*` temporary directories and no surviving test worker/child process. The repository diff passed `git diff --check`.

An earlier independent fresh-eyes follow-up review was clean after the exception-safe claim release, observed-entry adjudication, retained worker cleanup, and borrowed-RCW ownership repairs. The subsequent attachment repair review requested changes: the five-case early-stop result was unreachable through all-ID validation, process exit could bypass ledger binding, early stop used an observed PID instead of a durable record, result authority and window truncation were under-bound, and bootstrap persistence/OPC closure were incomplete. The implementation and behavioral tests above address each finding. A fresh independent re-review is still required before another targeted live probe.

## Bounded negative attachment probe

The single authorized targeted probe used:

```powershell
./scripts/run-excel-vba-oracle.ps1 -Suite HarnessSelfTest -EnvironmentId win-x64-dev-oracle-2026-07 -NoMatrixUpdate -DiagnosticCaseId success
```

Run `excel_vba_oracle_d50ce53d005c42ceb89decb099a521a3` failed safely after 35.8 seconds. The exact PID 18980 reached a visible `XLMAIN`/`FullpageUIHost` no-workbook start surface. Across 195 retries, the final exact-PID snapshot contained 58 windows, including two `XLMAIN` and two `XLDESK` windows, but no `EXCEL7` window. Consequently no owned window exposed the Excel native object model. The Job terminated the unledgered Excel child, the run claim was released, and post-run inspection found zero Excel or owned-helper residuals.

The full local failed-run directory remains under `artifacts/windows-x64/excel-vba-oracle/`; it is intentionally excluded from the commit. The bounded exact summary is retained in `attachment-negative-2026-07-14.json`. That summary also records two defects found by the probe: the early `Process.Path` sample was empty, and the supervisor replaced the valid case transport with an empty-ledger case-binding error. The repair now records the exact launch executable input, supplies the deterministic workbook bootstrap, and preserves case failure through cleanup. Per the one-probe limit, Excel was not launched again after this failure.

## Residual live work

This evidence does not establish that a particular Office build exposes the exact expected UIA token/line or runtime dialog shapes. Before closing the supervisor bead:

1. independently review the offline bootstrap/attachment repair, then run one targeted `success` probe against the characterized development/oracle host;
2. only if that targeted probe passes with an exact `EXCEL7`/`OBJID_NATIVEOM` attachment and zero residuals, run the default five-case harness with Excel/VBE modal interception active;
3. inspect the compile-failure and intrinsic-shadow token/line captures, ambiguous macro transcript, full `Err` state, lifecycle ordering, and zero owned residuals;
4. run the `runtime-unhandled-modal` diagnostic and retain its owned runtime-error modal transcript;
5. record the live run separately without advancing certification or canonical capability rows.
