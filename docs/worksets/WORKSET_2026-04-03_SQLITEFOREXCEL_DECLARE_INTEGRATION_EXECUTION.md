# Workset: SQLiteForExcel Declare Integration Execution

Date: 2026-04-03
Owner: Codex
Status: in-progress

## Purpose

Plan and execute a direct native/`Declare` integration exploration against the SQLiteForExcel VBA modules so OxVba can measure real Windows native-call compatibility instead of relying only on synthetic declare fixtures.

This lane is explicitly exploratory and evidence-first:
- set up reproducible external fixtures,
- run the real SQLiteForExcel declarations and sample code through OxVba CLI host modes,
- record exactly what works, what fails, and under which modality,
- and only then decide on implementation follow-on work.

It is not a “fix everything while testing” lane.

## Why This Exists

OxVba now has meaningful native/declare execution substrate, but we need a realistic outside-in integration probe that stresses:
- Windows `Declare` bindings,
- external DLL loading,
- pointer/handle passing,
- string marshalling,
- sample-suite execution flow,
- and the difference between interpreter/IL execution and CraneLift JIT.

`govert/SQLiteForExcel` is a strong probe because it contains:
- real VBA declare modules,
- a sample/demo module,
- both 32-bit and 64-bit variants,
- a companion `SQLite3_StdCall.dll` wrapper,
- and known SQLite binaries.

## Current Starting Facts

Public upstream:
- GitHub repo: `govert/SQLiteForExcel`
- repo URL: `https://github.com/govert/SQLiteForExcel`

Local material discovered on this machine:
- archive/worktree: `C:\Work\SQLiteForExcelArchive`
- this is not a Git clone
- it appears to be a Fossil-style checkout because it contains `_FOSSIL_`
- `fossil` is not currently installed on this machine, so direct Fossil remote-sync verification is not currently available

Local fixture content already present in that archive:
- `Source\SQLite3VBAModules\Sqlite3.bas`
- `Source\SQLite3VBAModules\Sqlite3Demo.bas`
- `Source\SQLite3VBAModules\Sqlite3_64.bas`
- `Source\SQLite3VBAModules\Sqlite3Demo_64.bas`
- `Distribution\SQLite3_StdCall.dll`
- `Distribution\sqlite3.dll`
- `Distribution\x64\sqlite3.dll`

Additional machine-wide SQLite discovery:
- `C:\Programs\SQLite\sqlite3.exe`
- `C:\Programs\SQLite\Old\sqlite3.exe`
- no `sqlite3.dll` was found under `C:\Programs` during the current sweep

## Governing Rules

1. This lane is evidence-first. Do not silently “fix and continue” when the integration probe exposes limitations.
2. External provenance must be explicit.
3. OxVba should control the test fixtures and dependency paths used for the integration runs.
4. The first-pass setup should favor local reproducibility over clever dynamic downloads.
5. Results must be split by modality, not collapsed into one pass/fail claim.

## Scope

This workset covers:
- provenance capture for SQLiteForExcel source/dependencies
- fixture import into this repo under a controlled external-artifacts area
- OxVba project setup for SQLiteForExcel declare/sample execution
- CLI-driven runs over the relevant execution modes
- positive and negative expectation matrix
- evidence capture and follow-on issue creation

This workset does not yet cover:
- implementation fixes in OxVba for failures found
- Office-hosted verification inside Excel
- COM automation around Excel itself
- packaging this as an end-user feature

## Fixture And Dependency Strategy

Preferred strategy:
- bring the required SQLiteForExcel test inputs into this repo under a controlled path such as `.external/sqliteforexcel/`
- include or reference:
  - exact source modules under test
  - exact SQLite DLLs and wrapper DLL
  - provenance note naming upstream source and local origin
- keep the imported set minimal and test-focused

Fallback strategy:
- if a binary should not be committed directly, track a public download source plus checksum/reference note in-repo and materialize it into `.external/` during setup

The goal is that the OxVba repo controls the integration fixture shape even if the original upstream repo changes.

## Execution Matrix

The matrix must separate at least:

### Positive-target modalities

- CLI-driven execution on Windows
- interpreter/IL path
- CraneLift JIT path
- correct 64-bit SQLiteForExcel module set
- correct wrapper/native DLL presence

### Negative-target modalities

- missing DLL / missing wrapper DLL
- wrong declare module variant for the current architecture
- direct `sqlite3.dll` versus `SQLite3_StdCall.dll` expectation mismatches where relevant
- any host policy / runtime mode that is expected not to support the scenario

Every matrix row should record:
- project/fixture
- host/runtime mode
- command used
- expected outcome
- actual outcome
- whether a limitation is an OxVba bug, an unsupported scenario, or an upstream/fixture mismatch

## Required Outcomes

1. SQLiteForExcel provenance and local-source state are documented honestly.
2. The required declare/sample artifacts are controlled by this repo or tracked with explicit reference metadata.
3. OxVba fixture projects exist for the relevant SQLiteForExcel module sets.
4. CLI-driven interpreter/IL and CraneLift runs are executed and recorded separately.
5. Expected failures are exercised and recorded separately.
6. The resulting evidence leaves a precise “works / fails / unsupported” boundary.
7. Follow-on bug/feature beads are created from the evidence instead of being fixed opportunistically in the same exploratory pass.

## Planned Execution Slices

1. verify upstream provenance and local-archive sync posture honestly
2. capture external fixture/dependency provenance in this repo
3. import the minimal SQLiteForExcel source/binary artifact set into `.external/`
4. create OxVba fixture projects for 64-bit declare/sample execution
5. create CLI harness commands and matrix rows for IL/interpreter and CraneLift
6. run the positive matrix
7. run the negative matrix
8. publish evidence and spawn follow-on implementation beads from actual findings

## Exit Condition

This workset is complete only when:
- the SQLiteForExcel integration fixture is reproducible from this repo,
- the modality matrix has been executed and recorded,
- and the results are captured as exact evidence with follow-on work split into separate beads rather than folded into the exploration lane.
