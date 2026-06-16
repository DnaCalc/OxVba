# Diagnostics

OxVba uses `oxvba-diagnostics` as the shared diagnostic contract across the
compiler, project loader, runtime, HAL, COM bridge, host, and CLI.

## Shape

Every diagnostic has:
- a stable code such as `SYN-E-PARSE`, `BIND-E-UNRESOLVED-NAME`, or
  `COM-E-EVENT-ADVISE-FAILED`;
- a severity, phase, and human message;
- optional source/module span data;
- optional notes, help text, cause diagnostics, metadata, and VBA
  `Err.Number`.

The error-producing crate owns the semantic error and exposes a local
`to_diagnostic()` conversion. The diagnostics crate is intentionally a leaf
crate; it does not know compiler, project, COM, or HAL internals.

## CLI Output

The CLI supports:

```text
--diagnostic-format human
--diagnostic-format json
```

`human` is the default and prints a readable code-bearing diagnostic. `json`
prints a deterministic `DiagnosticReport` with the same code, phase, message,
source, metadata, and VBA error-number fields.

## Code Families

Current code families are registered in `docs/ERROR_CODES.md`.

Clean-stack compiler/project codes are:
- `SYN-E-*` for syntax parsing;
- `SYM-E-*` for symbol model failures;
- `BIND-E-*` for binder/lowering failures;
- `BUND-E-*` for defensive Core IR linearization failures;
- `PROJ-E-*` for project loading and closure failures.

Runtime and boundary codes are:
- `RUN-E-*` for runtime infrastructure failures;
- VBA numeric runtime errors for VBA-visible run-time failures;
- `HAL-E-*` for HAL capability, policy, adapter, profile, and host-project
  boundary failures;
- `COM-E-*` for COM bridge and COM transport failures;
- `HOST-E-*` for host/CLI orchestration failures.

Do not reintroduce deleted `PMR-E-*` or `PMR-I-*` codes. New clean-stack
diagnostics should be added in the crate that owns the corresponding error type
and documented in `docs/ERROR_CODES.md`.
