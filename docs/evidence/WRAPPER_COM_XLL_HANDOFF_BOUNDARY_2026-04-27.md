# Wrapper COM/XLL Handoff Boundary

Date: 2026-04-27
Beads: `bd-wrap1.6`, `bd-wrap1`

## Purpose

Publish the wrapper-side handoff boundary to downstream COM server and XLL lanes
without claiming those lanes are complete.

## Handoff Surface

The wrapper substrate now exposes these generated-source lanes over canonical
`.oxb` bundles:

- `crates/oxvba-build/src/exe.rs`: wrapper executable shim.
- `crates/oxvba-build/src/dll.rs`: native DLL/shared-library shim with bounded
  native export metadata marshaling.
- `crates/oxvba-build/src/comserver.rs`: Windows in-process COM server source
  skeleton with `DllGetClassObject`, class factory, dispatch instance, and
  registration entry points.
- `crates/oxvba-build/src/comserver_exe.rs`: out-of-process COM server source
  skeleton.
- `crates/oxvba-build/src/xll.rs`: XLL entry-point source with `xlAutoOpen`,
  `xlAutoClose`, `xlAutoFree12`, and registration type-string derivation.
- `crates/oxvba-build/src/xloper.rs`: bounded XLOPER12 type-string support for
  exported function metadata.

## Validation

Commands:

```powershell
cargo fmt --check -p oxvba-build
cargo test -p oxvba-build --lib -- --nocapture
./scripts/check-governance.ps1
git diff --check
```

Results:

- `cargo fmt --check -p oxvba-build`: pass
- `cargo test -p oxvba-build --lib -- --nocapture`: pass, 31/31
- `./scripts/check-governance.ps1`: pass
- `git diff --check`: pass with CRLF conversion warnings only

Relevant regression rows:

- `comserver::tests::com_server_shim_structure`
- `comserver::tests::com_server_has_class_factory`
- `comserver::tests::com_server_has_dispatch_instance`
- `comserver::tests::com_server_multiple_classes`
- `comserver_exe::tests::com_exe_shim_structure`
- `xll::tests::xll_shim_has_required_entry_points`
- `xll::tests::xll_registration_type_string`
- `xloper::tests::type_string_for_double_function`
- `xloper::tests::type_string_for_long_sub`
- `xloper::tests::type_string_for_string_function`

## Boundary

This closes the wrapper substrate workset. It does not claim:

- registered COM server parity,
- Excel-loaded XLL parity,
- end-user installer/package behavior,
- or Office-facing deployment closure.

Those remain owned by downstream COM server and XLL worksets. The wrapper lane
is complete only as the generated-source and metadata handoff substrate that
unblocks those downstream lanes.
