# Workset: Array, Date, and Pointer Residual Compliance Execution

Date: 2026-04-05
Owner: Codex
Status: in-progress

## Purpose

Turn the remaining bounded generic gaps exposed during the SQLiteForExcel lane
into explicit compiler/runtime delivery work, ordered toward broader VBA
compliance without pretending those residuals are already closed.

This workset is not a generic audit. It is an execution owner for the next real
delivery slices after the completed SQLite integration lane.

## Current Residual Families

### Arrays

The scoped array residual family for this workset is now closed. This workset
delivered the remaining dynamic-array/runtime-array slices that were still open
after the SQLite lane:

- runtime-expression one-dimensional `ReDim Preserve`
- broader runtime-array index/bounds behavior over the base-slot model
- multi-dimensional runtime-sized arrays
- explicit lower bounds and `Option Base` defaults in the dynamic runtime-array
  lane
- `LBound` / `UBound` truth over runtime arrays that carry non-zero lower-bound
  metadata

### Dates

The scoped date/time residual family for this workset is now closed. This
workset delivered the remaining bounded carrier-cleanup slices that were still
open after the first Date runtime-value move:

- `TimeSerial` / `TimeValue` now materialize real Date-subtyped runtime values
  rather than raw integer-second carriers
- host `Date()` / `Time()` / `Now()` now surface honest Date-subtyped runtime
  values
- host `Timer()` now surfaces a numeric time-of-day value rather than an
  arbitrary legacy token
- `IsDate` now follows real date coercion rather than the earlier 8-digit
  integer heuristic
- packed-digit compatibility is retained only as an explicit compatibility lane,
  not as the primary runtime carrier

### Pointer Helpers

The pointer-helper lane is materially stronger now, but still bounded. Residuals
currently include:

- broader `Variant` container coverage
- honest `ObjPtr` support beyond the currently narrow object categories
- explicit unsupported/writeback matrix cleanup for the remaining helper shapes

## Priority Order

1. Arrays
2. Dates
3. Pointer helpers

That ordering is intentional: the next array and date slices move core runtime
truth forward fastest and reduce the largest remaining mismatch between bounded
delivery and broader compliance expectations.

## Bead Map

- `bd-cmpr1` parent execution epic for this workset
- `bd-cmpr1.1` rollout/support bead that creates the concrete residual closure
  families
- `bd-cmpr1.2` dynamic-array residual closure epic
- `bd-cmpr1.2.1` deliver runtime-bound one-dimensional `ReDim Preserve`
- `bd-cmpr1.2.2` deliver broader runtime-array bounds/index semantics
- `bd-cmpr1.2.3` deliver multi-dimensional runtime-sized array support
- `bd-cmpr1.2.3.1` repair stale fixed-subset multidimensional evidence after
  base-slot retention
- `bd-cmpr1.2.3.2` deliver dynamic multi-dimensional `ReDim`, lower-bound, and
  `Option Base` support
- `bd-cmpr1.3` date/runtime-value residual closure epic
- `bd-cmpr1.3.1` promote `DateSerial` and core date-part/date-arithmetic lanes
  to real Date-subtyped runtime values
- `bd-cmpr1.3.2` complete remaining date/time intrinsic carrier cleanup
- `bd-cmpr1.4` pointer-helper residual closure epic
- `bd-cmpr1.4.1` widen `VarPtr`/`ObjPtr` honest boundary coverage
- `bd-cmpr1.5` repair COM dynamic-name projection fallback and retire stale
  token-era assertions

## Current Execution Intent

Delivered in this workset so far:

- `bd-cmpr1.2.1`
  - runtime-expression one-dimensional `ReDim Preserve` now lowers and executes
    over existing runtime-array values in compiler, VM, and JIT
- `bd-cmpr1.2.2`
  - dynamic arrays now stay on the runtime-array model under literal
    one-dimensional `ReDim`
  - ordinary runtime-array element assignment/read semantics now work across
    later one-dimensional `ReDim Preserve`
  - residual moved explicitly to `bd-cmpr1.2.3`: multi-dimensional
    runtime-sized arrays
- `bd-cmpr1.2.3.1`
  - stale fixed-bounds multidimensional evidence was repaired after base-slot
    retention moved declaration ordering
  - explicit lower bounds and `Option Base` coverage remain evidenced in the
    older fixed-bounds slot-expanded subset, without claiming that this alone
    closed the runtime-sized residual
- `bd-cmpr1.2.3.2`
  - dynamic runtime-sized arrays now support multi-dimensional `ReDim` /
    `ReDim Preserve` in compiler, VM, and JIT
  - runtime indexed read/write over dynamic multi-dimensional arrays is now a
    first-class compiler/runtime path
  - explicit lower bounds and `Option Base` defaults are honored in the
    runtime-sized lane
  - `LBound` / `UBound` now follow runtime-array lower-bound metadata rather
    than leaking zero-based legacy tags
- `bd-cmpr1.3.1`
  - `DateSerial`, core day-based `DateAdd`/`DateDiff`, and
    `Year`/`Month`/`Day`/`Weekday` now use shared Date-subtyped runtime-value
    semantics
  - residual moved explicitly to `bd-cmpr1.3.2`: remaining date/time carrier
    cleanup and packed-date bridge retirement
- `bd-cmpr1.3.2`
  - `TimeSerial` / `TimeValue` now use Date-subtyped runtime values in VM/JIT
    rather than raw second-count carriers
  - host `Date()` / `Time()` / `Now()` now use Date-subtyped runtime values and
    `Now()` combines host date plus host time honestly
  - host `Timer()` now uses a numeric time-of-day carrier
  - `IsDate` now follows real date coercion semantics instead of the earlier
    digit-range heuristic
  - deterministic host and recorder/replay evidence now preserve those typed
    host time values
- `bd-cmpr1.4.1`
  - Windows `VarPtr(v As Variant)` now widens honest container coverage to
    include Decimal-valued `Variant` cells in addition to the earlier
    scalar/string subset
  - `ObjPtr` now has end-to-end evidence for object-valued `Variant`
    expressions, not only plain object variables
  - the still-untruthful object-valued and array-valued `Variant` container
    cases now have explicit rejection evidence instead of remaining a fuzzy
    unsupported matrix
  - the temporary moved frontier in the full pointer-helper end-to-end file was
    closed under `bd-lfa1.16`; the pointer-helper family itself remains
    delivered and the full pointer-helper host-backed file is green again
- `bd-cmpr1.5`
  - deterministic/projection COM fallback now keeps honest binding metadata on
    synthetic object handles instead of dropping back to anonymous
    `projection:<handle>` descriptors
  - dynamic name-based COM dispatch now resolves through the current
    metadata-backed binding path before any fallback to the old sentinel-token
    lowering
  - stale COM tests were repaired to match current truth:
    `Scripting.Dictionary.Count` now uses the explicit missing-arg token,
    native dictionary metadata is asserted by member name rather than fixture
    DISPIDs, and the live typelib metadata row now expects `NewEnum`

Current ready delivery bead:

- none in this workset; the array/date/pointer residual family is delivered

Current array truth:

- fixed-bounds multidimensional indexing plus explicit lower bounds and
  `Option Base` defaults remain covered in the older slot-expanded subset
- dynamic runtime-sized arrays now also cover:
  - multi-dimensional `ReDim` / `ReDim Preserve`
  - explicit lower bounds
  - `Option Base` defaults for omitted lower bounds
  - lower-bound-aware `LBound` / `UBound`
  - VM/JIT parity evidence for runtime multi-dimensional indexing and preserve

Queued follow-on after the active array bead:

- none in this workset

Current state:

- complete; the array/date/pointer residual family under `bd-cmpr1` is closed.
