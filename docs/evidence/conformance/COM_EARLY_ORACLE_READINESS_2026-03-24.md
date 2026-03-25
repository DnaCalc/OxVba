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
  - the narrow activation floor is now reproducible in-repo:
    - `Dim obj As New Scripting.Dictionary`
    - `countValue = obj.Count`
    - observed OxVba result: object handle bound on the native registered lane and `Count = 0`
  - broader exploratory member traffic is still not closure-ready:
    - `Call obj.Add("a", 1)` / `obj.Exists("a")` hit `COM-E-VALUE-TRANSPORT-UNSUPPORTED` via projected event-trigger callback transport.

Interpretation:

- `ODG-044` is not just waiting on Excel execution.
- The missing trustworthy OxVba-side anchor is now narrowed, not absent:
  - real registered early-bound activation plus `Count` baseline exists,
  - richer `Scripting.Dictionary` member traffic still needs transport/model correction before side-by-side oracle closure can be claimed.

## Gate implications

- `ODG-044`
  - no longer blocked by total absence of a real-registered early-bind anchor for `scrrun` / `Scripting.Dictionary`
  - still blocked by richer member/event transport correctness beyond the new activation-plus-Count floor
- `ODG-045`
  - still needs a mixed-server / dual-interface oracle harness; Excel availability alone does not answer transport-policy parity
- `ODG-046`
  - still needs versioned-typelib / broken-reference mutation harness; Excel availability alone does not answer version-selection or repair parity

## Recommended next steps

1. Keep the new registered early-bound `Scripting.Dictionary` activation-plus-Count lane as the honest minimum anchor for `ODG-044`.
2. Fix the richer member/event transport fault surfaced by `Add` / `Exists` on that same registered early-bound path.
3. Only then schedule/fold the side-by-side Excel oracle capture for `ODG-044`.
4. Treat `ODG-045` and `ODG-046` as distinct harness-construction tasks, not mere calendar items.
