# V0.2 Date-String Parsing Rollout

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.4.1`
Parent: `bd-bqm8.4`
Status: complete

## Purpose

`bd-bqm8.4` is a capability lane, not a single documentation task. This
rollout splits the date-string parsing/coercion work into delivery beads so the
lane closes only after the accepted grammar is documented, parser gaps are
implemented or explicitly rejected, executable evidence is refreshed, and the
final checklist passes.

## Child Beads

- `bd-bqm8.4.1`: audit and roll out date-string child beads.
- `bd-bqm8.4.2`: publish accepted grammar, locale policy, and unsupported
  ambiguity boundaries for `DateValue`, `CDate`, and `IsDate`.
- `bd-bqm8.4.3`: implement or reconcile accepted parser/coercion gaps.
- `bd-bqm8.4.4`: refresh VM/JIT/host/conformance evidence for accepted rows.
- `bd-bqm8.4.5`: run final date-string parsing checklist and close the parent
  only if accepted rows are green and unsupported ambiguity remains explicit.

## Initial Code Surface

- `crates/oxvba-vm/src/semantics.rs` owns the centralized date/time parsing and
  coercion helpers.
- `crates/oxvba-vm/src/interpreter.rs` executes `DateValue`, `CDate`, and
  `IsDate` intrinsics against retained `Variant` slots.
- `crates/oxvba-host/src/engine.rs` contains focused formal host evidence for
  current month-name `DateValue`/`CDate` rows and `IsDate` behavior.

## Current Known Boundary

The active parser still reports `DateValue string format is not yet supported`
and `CDate string format is not yet supported` for shapes outside the current
month-name and compatibility numeric subset. The next bead must make the
accepted V0.2 grammar explicit before broadening implementation.
