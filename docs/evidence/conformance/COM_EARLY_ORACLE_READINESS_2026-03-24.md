# COM Early Oracle Readiness 2026-03-24

## Purpose

Record the current machine-readiness and the newly discovered blocker shape for the remaining COM early oracle gates (`ODG-044..046`).

This note is a readiness/blocker artifact, not closure evidence.

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

## Newly discovered blocker

While preparing a real Office-backed anchor for `ODG-044`, exploratory local repro work showed that the matching OxVba-side early-bound path is not yet an honest parity anchor for a real registered typelib target:

- target used: `Scripting.Dictionary` (`scrrun.dll`)
- exploratory source shape:
  - `Dim obj As New Scripting.Dictionary`
  - `countValue = obj.Count()` / `existsValue = obj.Exists(42)`
- observed OxVba behavior during local repro:
  - the path does not yet provide a trustworthy real-registered external baseline,
  - one exploratory route hit adapter/member-shape faults,
  - another returned the controlled test baseline (`Count = 7`) instead of the empty external dictionary baseline (`Count = 0`).

Interpretation:

- `ODG-044` is not just waiting on Excel execution.
- A real registered early-bound external lane still needs to be made honest on the OxVba side before side-by-side oracle closure can be claimed.

## Gate implications

- `ODG-044`
  - blocked by missing trustworthy OxVba-side real-registered early-bind anchor for `scrrun` / `Scripting.Dictionary`
- `ODG-045`
  - still needs a mixed-server / dual-interface oracle harness; Excel availability alone does not answer transport-policy parity
- `ODG-046`
  - still needs versioned-typelib / broken-reference mutation harness; Excel availability alone does not answer version-selection or repair parity

## Recommended next steps

1. Build a reproducible OxVba host lane for real registered early-bound `Scripting.Dictionary` execution.
2. Use that lane to close the honest semantic floor for `ODG-044`.
3. Only then schedule/fold the side-by-side Excel oracle capture for `ODG-044`.
4. Treat `ODG-045` and `ODG-046` as distinct harness-construction tasks, not mere calendar items.
