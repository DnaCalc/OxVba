# V0.2 VM/JIT Hardening Rollout

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.6.1`
Parent: `bd-bqm8.6`
Status: complete

## Scope

`bd-bqm8.6` is the VM/JIT hardening and security pass for the bounded V0.2
surface. It must produce an explicit hardening matrix, executable bad-input
tests, and refreshed evidence for safety-sensitive runtime/JIT assumptions.

This rollout does not close the parent lane. It creates the delivery and
validation path required before the hardening epic can close.

## Initial Audit Inputs

The first scan covered:

- invalid and malformed runtime inputs
- panic/unwrap/expect references in runtime, VM, JIT, and host surfaces
- boundary-cell terms from the representation/layout doctrine: `Variant`,
  `BSTR`, `SAFEARRAY`, and object/interface pointer materialization
- existing hardening/formal evidence references

The broad scan shows the lane needs scoped child delivery rather than a single
documentation outcome.

## Child Beads

- `bd-bqm8.6.1`: audit and roll out VM/JIT hardening child beads.
- `bd-bqm8.6.2`: publish the V0.2 VM/JIT hardening matrix and scan baseline.
- `bd-bqm8.6.3`: harden malformed retained-Variant/JIT-slot boundary handling.
- `bd-bqm8.6.4`: add malformed bytecode/project/runtime-input regression tests.
- `bd-bqm8.6.5`: refresh formal/security evidence and classify any residual
  non-blocking formal failures.
- `bd-bqm8.6.6`: run the final VM/JIT hardening checklist and close `bd-bqm8.6`
  only if the matrix, delivery tests, and residual classification are explicit.

## Ready Path

The next ready bead is `bd-bqm8.6.2`, which will publish the hardening matrix
and baseline scan so subsequent delivery beads have concrete rows to advance.
