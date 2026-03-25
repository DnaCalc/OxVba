# COM Early Oracle Readiness 2026-03-24

## Purpose

Record the current machine-readiness and the newly discovered blocker shape for the remaining COM early oracle gates (`ODG-044..046`).

This note is a readiness/blocker artifact, not closure evidence.

Update 2026-03-25:

- `ODG-044` supported-subset oracle capture is now complete.
- The external `OxVba.TestEventServer` user-scope typelib path is now also proven.
- Evidence:
  - `docs/evidence/conformance/oracle_captures/com_early_oracle_20260325T145433Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_early_oracle_20260325T145433Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_typelib_probe_20260325T204228Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_typelib_probe_20260325T204228Z/results.csv`
- Result:
  - Excel VBA and OxVba both produced `True,1` for the supported `Dim obj As New Scripting.Dictionary` plus `Add` / `Exists` / `Count` subset.
  - Excel also now accepts the exported `OxVba.TestEventServer.tlb` through `AddFromFile`, and the user-scope external fixture lane proves both `New TestEventServer` (`42`) and `WithEvents` callback ingress (`7`).
  - The same runner now also captures a first `ODG-046` baseline: when a saved workbook's file-backed `OxVba.TestEventServer.tlb` is removed before reopen, no matching reference entry remains in `VBProject.References` for that path.
- Remaining open COM-early oracle items are `ODG-045` and `ODG-046`, plus the broader imported activation-authority question under `ODG-031`.

## Local host readiness

- Excel COM automation is available locally:
  - version: `16.0`
  - path: `C:\Program Files\Microsoft Office\Root\Office16`
- `AccessVBOM` is enabled locally:
  - registry: `HKCU\Software\Microsoft\Office\16.0\Excel\Security`
  - value: `1`
- Direct Excel/VBA probe is runnable:
  - workbook created through COM automation,
  - `Microsoft Scripting Runtime` reference added via `AddFromGuid`,
  - VBA module injected through `VBProject`,
  - probe `Dim d As New Scripting.Dictionary : d.Add "a", 1` executed successfully,
  - observed VBA result: `True,1`.

## What this changes

- `ODG-044..046` are no longer blocked by lack of an Excel host on this machine.
- The earlier description that these items are only waiting for scheduling is no longer fully accurate.
- The earlier user-scope external typelib-path blocker is now resolved for the baseline `OxVba.TestEventServer` fixture.

## Newly discovered blocker

While preparing a real Office-backed anchor for `ODG-044`, exploratory local repro work first showed that the matching OxVba-side early-bound path was not yet an honest parity anchor for a real registered typelib target. That specific core defect is now fixed.

- target used: `Scripting.Dictionary` (`scrrun.dll`)
- exploratory source shape:
  - `Dim obj As New Scripting.Dictionary`
  - `countValue = obj.Count()` / `existsValue = obj.Exists(42)`
- observed OxVba behavior during local repro and subsequent core repair:
  - the real registered early-bound lane is now reproducible in-repo for the supported subset:
    - `Dim obj As New Scripting.Dictionary`
    - `Call obj.Add("a", 1)`
    - `countValue = obj.Count`
    - `existsValue = obj.Exists("a")`
    - observed OxVba result: object handle bound on the native registered lane, `Count = 1`, `Exists("a") = True`
  - root cause of the earlier failure:
    - hardcoded `scrrun.dll` metadata in `oxvba-com` incorrectly exposed a fake `Exists` event, which made normal dictionary member traffic look like a projected event trigger.

Interpretation:

- `ODG-044` is no longer blocked by the earlier OxVba-side callback-transport fault on the supported registered `Scripting.Dictionary` subset.
- That supported subset is now oracle-captured and folded via `com_early_oracle_20260325T145433Z`.
- The broader real-library activation-model question remains open under `ODG-031`; that is a different closure item.

## Gate implications

- `ODG-044`
  - no longer blocked by total absence of a real-registered early-bind anchor for `scrrun` / `Scripting.Dictionary`
  - no longer blocked by the earlier `Add` / `Exists` callback-transport defect on the supported subset
  - side-by-side Excel oracle capture and foldback for the supported subset is now complete
- `ODG-045`
  - still needs a mixed-server / dual-interface oracle harness; the baseline external user-scope typelib lane now exists, but Excel availability alone does not answer transport-policy parity
- `ODG-046`
  - still needs versioned-typelib / broken-reference mutation harness; the baseline external user-scope typelib lane now exists, but Excel availability alone does not answer version-selection or repair parity

## Recommended next steps

1. Keep the new registered early-bound `Scripting.Dictionary` `As New` / `Add` / `Exists` / `Count` lane as the permanent supported anchor for the closed `ODG-044` subset.
2. Keep the new `OxVba.TestEventServer` user-scope `.tlb` probe as the permanent baseline external oracle harness anchor.
3. Keep the broader activation-model review under `ODG-031` separate from the now-closed `ODG-044` callback-transport issue.
4. Treat `ODG-045` and `ODG-046` as distinct harness-construction tasks, not mere calendar items.
