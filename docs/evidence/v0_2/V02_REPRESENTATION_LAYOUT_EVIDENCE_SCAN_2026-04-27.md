# V0.2 Representation/Layout Evidence Scan

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.5.3`
Parent: `bd-bqm8.5`
Status: complete

## Scope

This bead scanned implementation and documentation surfaces for
representation/layout claims after the accepted doctrine in
`OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md`.

## Scan Result

No scanned runtime, COM, HAL, VM, host, CLI, or product-doc surface requires
reopening the core value model as raw OLE Automation wire structs.

Classified surfaces:

- Runtime `Variant`, `BStr`, `SafeArray`, and `ObjectRef` are canonical
  semantic carriers, with targeted VARIANT/BSTR/SAFEARRAY/object-pointer layout
  support where boundary materialization requires it.
- COM and HAL compatibility projection remains an explicit adapter boundary,
  matching the completed `bd-bqm8.2` compat-slot doctrine.
- Pointer helpers intentionally materialize honest BSTR payloads, VARIANT cells,
  SAFEARRAY cells, and object pointers for supported native interop windows.
- JIT slots retain `oxvba_runtime::Variant` as their internal ABI carrier. A
  stale comment that called this the canonical `VARIANT` carrier was corrected
  to avoid confusing the retained runtime carrier with raw COM wire ownership.

## Remaining Risk Surfaces

These are downstream boundary risks, not blockers for `bd-bqm8.5`:

- `bd-bqm8.6`: malformed or unsupported boundary cells need hardening evidence,
  especially around native VARIANT/SAFEARRAY/pointer helper shapes.
- `bd-bqm8.7`: broader Excel and Access/JET COM corpus work must continue to
  prove `oxvba-com` translation fidelity for real external dependencies.
- `bd-bqm8.10`: native compilation and wrapper lanes must preserve semantic
  internal values and materialize ABI shapes only at declared external
  boundaries.
- Historical docs may still describe older token-era or partial HAL boundary
  states; they are acceptable as historical records when active docs point to
  the accepted doctrine.

## Verification

Passed:

- implementation scan for `RuntimeValue::I32`, `compat_slot`, `from_compat`,
  `to_runtime_value`, `to_runtime_token`, `VARIANT`, `BSTR`, `SAFEARRAY`,
  `DISPPARAMS`, `RawIDispatch`, `RawIUnknown`, `ObjectRef`, `VT_DATE`,
  `VT_ARRAY`, and `VT_VARIANT`
- docs scan for `wire structs`, `canonical`, `boundary`, `BSTR`, `VARIANT`,
  `SAFEARRAY`, `DISPPARAMS`, `ObjectRef`, `compat-slot`, and `RuntimeValue`
- `cargo test -p oxvba-runtime pointer --lib`
- `cargo test -p oxvba-runtime variant --lib`
- `cargo test -p oxvba-com com_value --lib`

## Follow-Up

The next ready bead is `bd-bqm8.5.4`, the final representation/layout doctrine
checklist. It can close `bd-bqm8.5` if the doctrine, evidence scan, and
downstream path remain explicit.
