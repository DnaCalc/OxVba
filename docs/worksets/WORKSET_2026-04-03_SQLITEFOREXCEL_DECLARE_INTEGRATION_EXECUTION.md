# Workset: SQLiteForExcel Declare Integration Execution

Date: 2026-04-03
Owner: Codex
Status: closed

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
- fresh Git clone now exists at `C:\Work\SqliteForExcel`
- fresh clone HEAD: `8aae8bd5c69a9083a67a295fcbcfde838c755f4f`

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

Delivered so far in this lane:
- public upstream identity pinned
- fresh Git clone created and verified against current remote HEAD
- local archive versus fresh clone comparison completed
- archive treated as stale for the declare/sample artifacts under test
- controlled fixture import created under `.external/sqliteforexcel/`
- provenance recorded in `docs/evidence/SQLITEFOREXCEL_PROVENANCE_AND_SYNC_2026-04-03.md`
- tiny host-environment reference shim created under `.external/sqliteforexcel/fixtures/HostEnvironment/`
- first raw 64-bit fixture probe created under `.external/sqliteforexcel/fixtures/Demo64/`
- raw `_64` source ingestion is no longer blocked after the `VB_Name`-wins
  external-identity fix; raw and normalized demo fixtures now both reach the
  same later runtime-sized `ReDim` compile boundary
- first mutable host-shim attempt intentionally reduced after a setup finding:
  direct helper call `HostEnv.SetWorkbookPath` did not resolve across the
  current project-reference boundary, so the tiny shim now only supplies the
  exact upstream host-ish names under test
- standalone host-environment probe now compiles and runs cleanly, so the tiny
  `ThisWorkbook.Path` / `Debug.Print` reference-project shim is usable as a
  separate probe surface
- normalized core-only SQLite probe now moves past the earlier
  `thisworkbook_path` boundary, the later `Private Declare` / `LoadLibrary`
  binding boundary, the built-in `Win64` conditional-selection boundary, and the
  later multi-expression `Debug.Print` boundary;
  it currently fails later at compile time with
  `PMR-E-BACKEND-COMPILE: type error: unsupported statement: ReDim with runtime expression bounds is not yet supported for array 'buf': buf(length - 1)`
- normalized full demo probe currently fails at compile time with
  `PMR-E-BACKEND-COMPILE: type error: unsupported statement: ReDim with runtime expression bounds is not yet supported for array 'buf': buf(length - 1)`
- both current compile-time limitations are now pinned under automated host and
  compiler regressions; the SQLite core and demo rows now both reproduce the
  later runtime-sized `ReDim` boundary after the `sqlite3open` duplicate-name
  fix
- current implementation finding: this `ReDim` boundary is not a small parser
  issue. Static arrays still lower to compile-time alias slots, while the
  SQLite helper lane needs a bounded runtime-sized dynamic-array substrate for
  `ReDim buf(bSize)` plus `VarPtr(buf(0))`
- host-only `--jit` probe now runs cleanly after the diagnostics-host and
  console-host helper consistency follow-ons
- follow-on ownership decision: `Debug.Print` is treated as a host-supplied
  diagnostics capability routed through the runtime diagnostics surface, not as
  a special built-in library function
- Excel ground truth now captured separately:
  - shipped workbook compiles/runs through SQLite initialization and `TestVersion`
    before first observed runtime failure at `TestOpenClose` (`Err 53`)
  - raw upstream `_64` source files import successfully into Excel, confirming a
    real Excel-vs-OxVba ingestion-policy difference for filename vs `VB_Name`
- the earlier `Private Const` host-shim suspicion is not currently reproduced by
  the minimal direct compiler probe, so it remains a narrower follow-on question
  rather than a proven active blocker
- 2026-04-04 execution update:
  - the bounded runtime-sized one-dimensional `ReDim` lane is now implemented
    far enough for the normalized core fixture to compile and execute in VM and
    JIT
  - the demo fixture also now moves past the earlier `ReDim`, `Kill`, named
    `Select Case` constant-label, `Beep`, and comma/equals-inside-SQL-string
    assignment parsing boundaries
  - the comparison-valued-expression frontier in
    `Debug.Print "Long String is the same: " & (myStringResult = myLongString)`
    is now delivered, together with the follow-on array-parameter and
    fixed-array whole-value passing slices needed to keep the demo moving
  - direct compile for the normalized demo manifest now succeeds; the frontier
    has moved out of backend compile and into runtime/native execution
  - current front edge:
    - host demo row: runtime phase reached after compile succeeds
    - CLI VM and CLI JIT rows with `--allow-filesystem-mutation true`: native
      `STATUS_ACCESS_VIOLATION` after execution begins
    - JIT-specific helper gap `missing helper: oxrt_array_resize_1d` was exposed
      and fixed in-cycle; JIT now also reaches the shared native crash frontier
  - current interpretation: SQLite is now blocked on a real runtime/native
    crash during actual execution, not on the previous compile-time
    expression/array barriers

## Governing Rules

1. This lane is evidence-first. Do not silently “fix and continue” when the integration probe exposes limitations.
2. External provenance must be explicit.
3. OxVba should control the test fixtures and dependency paths used for the integration runs.
4. The first-pass setup should favor local reproducibility over clever dynamic downloads.
5. Results must be split by modality, not collapsed into one pass/fail claim.

## Current Next Delivery Slice

- `bd-sql1.16.1` design the bounded runtime-sized dynamic-array substrate needed
  for SQLite native buffer helpers
- `bd-sql1.16.2` implement one-dimensional non-`Preserve` runtime `ReDim`
  allocation into the base array slot
  - complete on 2026-04-27 with evidence in
    [SQLITE_RUNTIME_REDIM_BASE_ARRAY_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/SQLITE_RUNTIME_REDIM_BASE_ARRAY_2026-04-27.md)
- `bd-sql1.16.3` bridge `VarPtr(buf(0))` and array return/assignment over that
  runtime array payload
- `bd-sql1.16.4` rerun the SQLite fixture matrix and publish the moved boundary
- `bd-sql1.17` comparison-valued expressions in value position, first observed
  through the demo line
  `Debug.Print "Long String is the same: " & (myStringResult = myLongString)`
- `bd-sql1.18` post-compile runtime/native crash isolation in the real SQLite
  execution lane
  - delivered: the first shared VM/JIT `STATUS_ACCESS_VIOLATION` was isolated to
    `SQLite3Open` / `sqlite3_open16` inside `TestOpenClose`
  - delivered: native declare `ByRef` scalar/LongPtr container-cell marshaling
    and caller-slot writeback for the m1 host-backed lane
  - moved boundary: the demo now reaches `TestOpenCloseV2` in both VM and JIT
- current next implementation frontier after that delivery:
  - `bd-sql1.19` string-versus-`Empty` comparison in `SQLite3OpenV2`
    is delivered; the demo now moves through `TestOpenCloseV2`, `TestError`,
    `TestInsert`, and `TestSelect` in both VM and JIT
  - `bd-sql1.20` is delivered; string-valued `DateValue("1 Jan 2000")` no
    longer traps on the legacy i32 lane
  - `bd-sql1.21` is delivered; writable `StrPtr` / `VarPtr` sync is now
    expression-shape and boundary-kind driven rather than keyed off Windows API
    names
  - `bd-sql1.22` isolated the moved blob frontier honestly
  - `bd-sql1.23` is delivered; runtime-sized byte-array element reads now work
    in expression position, which moved SQLite through `TestBlob`
  - `bd-sql1.24` is delivered; Windows x64 native `Double` arguments/returns
    now use an ABI-aware call path, `DateValue` / `CDate` materialize real
    Date-subtyped runtime values with packed-date compatibility, and `As Date`
    procedure parameters coerce at procedure entry
  - current evidence after those deliveries:
    - the reduced full-order SQLite harness now completes in both VM and JIT
      through `TestBindingReduced`, `TestDates`, `TestStrings`, `TestBackup`,
      `TestBlob`, and the readonly checks
    - a tracked bounded normalized fixture now exists under
      `.external/sqliteforexcel/fixtures/Demo64NormalizedBounded/` so the full
      upstream call order can be exercised with reduced binding-loop counts
      while the non-reduced normalized fixture remains the terminal evidence row
    - the real normalized fixture now completes end-to-end in CLI VM and CLI
      JIT with `--allow-filesystem-mutation true`, including the non-reduced
      `TestBinding` row over the full 100k loop and terminal
      `----- All Tests Complete -----` evidence
    - concurrent JIT/VM runs can still collide on the shared temp database path
      and fail in `Kill ...` with Windows `os error 32`; that is evidence noise,
      not a language/runtime frontier
  - terminal evidence:
    - `bd-sql1.25.1` delivered a tracked bounded-loop support row so the same
      full call order completes quickly in automated VM/JIT evidence without
      editing the real normalized fixture
    - `bd-sql1.25` is delivered; the non-reduced normalized fixture now has
      terminal completion evidence in both CLI VM and CLI JIT, so no fresh
      runtime or semantic frontier remained underneath the final 100k loop row

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
4. create OxVba fixture projects for 64-bit declare/sample execution, including a
   deliberately tiny host-environment shim for `ThisWorkbook.Path` and
   `Debug.Print`
5. create CLI harness commands and matrix rows for IL/interpreter and CraneLift
6. run the positive matrix
7. run the negative matrix
8. publish evidence and spawn follow-on implementation beads from actual findings

## Exit Condition

This workset is complete only when:
- the SQLiteForExcel integration fixture is reproducible from this repo,
- the modality matrix has been executed and recorded,
- and the results are captured as exact evidence with follow-on work split into separate beads rather than folded into the exploration lane.
