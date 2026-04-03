# Debug.Print Host Ownership V1

Date: 2026-04-04
Status: accepted

## Rule

`Debug.Print` is a host-supplied diagnostics capability, not a special built-in library function.

OxVba owns the syntax, lowering, and runtime instruction shape for `Debug.Print`, but the observable behavior is provided through the runtime host diagnostics surface.

## Intended layering

- parser/compiler:
  - recognize `Debug.Print`
  - lower it to the host diagnostic instruction path
- VM/JIT runtime:
  - route execution through the host diagnostics capability
- HAL/host:
  - implement the diagnostics sink for the active runtime profile
- callbacks/hosts:
  - optionally capture or redirect the diagnostic text

## Canonical host surface

The canonical semantic boundary is:

- `CapabilityId::DiagnosticsTelemetry`
- `DiagnosticsHal::debug_print(...)`
- host callbacks `on_debug_print(...)`

This means:

- interpreter and JIT must both use the same host diagnostics contract
- CLI/stdIO hosts may route `Debug.Print` to `stderr`
- GUI/IDE hosts may surface it in an Immediate/Debug/Output pane
- null/unsupported hosts must reject it through the normal host-capability path rather than panicking

## Non-goals

- `Debug.Print` is not modeled as part of a built-in VBA standard-library object
- OxVba should not require hosts to emulate a fake `Debug` object implementation just to support `Debug.Print`

## Consistency requirement

Every execution backend must agree on the same ownership model:

- parser/lowering
- VM interpreter
- JIT helper registration
- HAL adapters
- host callback surfaces
- public docs and examples

If a runtime profile supports diagnostics, `Debug.Print` must work through the diagnostics host surface in both VM and JIT modes.
