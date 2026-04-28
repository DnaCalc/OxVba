# Workset: XLL Excel Host Validation Execution

Date: 2026-04-28
Owner: Codex
Status: complete

## Purpose

Validate the generated OxVba XLL package inside a real Excel host, slowly and
with enough instrumentation to separate fixture mistakes, Excel registration
mistakes, ABI/marshaling mistakes, and runtime invocation mistakes.

This is a child workset of
`WORKSET_2026-04-02_XLL_ADDIN_REALIZATION_EXECUTION.md` and owns the remaining
Excel-loaded validation gap for `bd-xll1.5`.

## Operating Rules

1. Do not close the parent XLL lane on local compile evidence alone.
2. Keep each Excel attempt small, recorded, and repeatable.
3. Treat a failing Excel-host test as an implementation finding unless the
   evidence proves the fixture or host environment is invalid.
4. Do not delete or weaken failing tests to make the lane pass.
5. Fix compiler, runtime, generated XLL source, packaging, or harness code when
   the failure belongs there.
6. Keep public docs aligned with the exact supported XLL subset after each
   successful or failed host attempt.

## Scope

In scope:

- a deterministic Addin `.basproj` fixture that exports a small scalar function
  set,
- a repeatable local build/staging script for the generated `.xll`,
- Excel-host load/unload validation,
- `xlAutoOpen` / registration evidence,
- worksheet invocation evidence for the bounded scalar lanes,
- focused implementation fixes found by those host runs,
- final evidence that states pass/fail status and any remaining boundaries.

Out of scope:

- broad Excel automation parity,
- RTD, async, macro-command, custom UI, or menu integration,
- macOS Excel parity,
- non-scalar array/object return coverage unless required to fix the scoped
  scalar path.

## Validation Fixture

The first fixture should be intentionally small and boring. It should cover the
scalar lanes already claimed by generated XLL marshaling:

- `AddDouble(ByVal x As Double, ByVal y As Double) As Double`
- `EchoText(ByVal s As String) As String`
- `NotFlag(ByVal b As Boolean) As Boolean`
- `IncLong(ByVal n As Long) As Long`

The fixture must build through the normal `OutputType=Addin` path and produce a
staged `.xll` artifact without special one-off commands.

## Execution Phases

### Phase 0: Planning and Bead Tree

Publish this workset, attach it to the parent XLL workset, and create the bead
subtree under `bd-xll1.5`.

Exit evidence:

- this workset exists,
- `CURRENT_BLOCKERS.md` points to this staged path,
- `.beads/issues.jsonl` has the child bead sequence.

### Phase 1: Fixture and Artifact Staging

Create the deterministic Addin fixture and a script or testable command path
that builds and stages:

- source fixture,
- generated `.oxb` if retained as an intermediate,
- generated XLL shim source if emitted for inspection,
- final `.xll`,
- build transcript.

Exit evidence:

- local build passes,
- staged artifact paths are deterministic,
- artifact size and timestamp are captured,
- no Excel-host claims are made yet.

### Phase 2: Excel Load/Unload Harness

Add the smallest repeatable Excel-host harness that can load the staged `.xll`
and unload it cleanly. Prefer PowerShell COM automation when Excel is installed.
If host automation is unavailable, leave a manual script and record the exact
environment blocker rather than converting this into a local-only test.

Exit evidence:

- Excel version and bitness are captured when available,
- add-in load attempt result is captured,
- load/unload does not crash Excel, or the crash/failure is recorded with logs.

### Phase 3: Registration Evidence

Prove whether `xlAutoOpen` reaches the registration path and whether
`xlfRegister` succeeds for each exported function. Add generated-XLL diagnostic
logging if Excel object-model observation is not enough.

Exit evidence:

- per-function registration status is recorded,
- registration failures include return codes or generated-source trace output,
- any required implementation fixes become delivery work, not documentation
  edits only.

### Phase 4: Worksheet Invocation Evidence

Invoke the registered functions from a workbook and compare observed cell
results with expected values.

Exit evidence:

- `AddDouble(2.5, 3.25)` returns `5.75`,
- `EchoText("abc")` returns `abc`,
- `NotFlag(TRUE)` returns `FALSE`,
- `IncLong(41)` returns `42`,
- failures include worksheet formula text, observed result, and relevant XLL
  diagnostic trace.

### Phase 5: Triage and Fix Loop

For each host finding, identify the owner:

- fixture,
- CLI/package staging,
- generated XLL registration source,
- XLOPER12 ABI or lifetime,
- runtime invocation bridge,
- compiler/runtime scalar semantics,
- Excel host environment.

Implementation-owned findings must be fixed in code and covered by focused
tests before rerunning the host lane.

Exit evidence:

- each finding is classified,
- code-owned findings have remediation beads or are fixed under the active
  bead,
- stale docs are updated after the implementation truth changes.

### Phase 6: Evidence Closure

Publish the final host validation evidence and update the parent XLL workset.
Only close `bd-xll1.5` when the scoped Excel-host registration and invocation
matrix passes, or when the remaining blocker is external and documented with
exact unblocking steps.

## Bead Map

- `bd-xll1.5.2` - complete: publish staged host-validation workset and bead tree
- `bd-xll1.5.3` - complete: create deterministic Addin scalar fixture
- `bd-xll1.5.4` - complete: add repeatable XLL artifact staging path
- `bd-xll1.5.5` - complete: add Excel load/unload host harness
- `bd-xll1.5.6` - complete: prove or instrument `xlAutoOpen` / `xlfRegister`
- `bd-xll1.5.7` - complete: prove worksheet invocation for the scalar fixture
- `bd-xll1.5.8` - complete: triage and fix implementation-owned host findings
- `bd-xll1.5.9` - complete: publish final Excel-host evidence and update parent lane

Final evidence:

- [XLL_EXCEL_REGISTRATION_TRACE_2026-04-28.md](/C:/Work/DnaCalc/OxVba/docs/evidence/XLL_EXCEL_REGISTRATION_TRACE_2026-04-28.md)
- [XLL_EXCEL_WORKSHEET_INVOCATION_2026-04-28.md](/C:/Work/DnaCalc/OxVba/docs/evidence/XLL_EXCEL_WORKSHEET_INVOCATION_2026-04-28.md)

## Exit Condition

This workset is complete: the generated scalar fixture `.xll` loads in Excel,
registers the scoped exported functions, executes the scalar worksheet
invocation matrix, and leaves reproducible staging, registration, and worksheet
evidence under `target/xll-host-validation/`.
