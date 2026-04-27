# V0.2 Native Compilation Path Decision Evidence

Date: 2026-04-27

Bead: `bd-bqm8.10.2`

## Result

Published the V0.2 native-compilation path decision:

- Canonical decision:
  [V02_NATIVE_COMPILATION_PATH_DECISION_V1.md](/C:/Work/DnaCalc/OxVba/docs/validation/V02_NATIVE_COMPILATION_PATH_DECISION_V1.md)
- Selected direction: staged hybrid.
- Primary V0.2 product target: wrapper-hosted compiled artifact around an
  embedded `.oxb` bundle.
- Execution accelerator: Cranelift inside the OxVba runtime host for supported
  bytecode, with VM fallback retained as the correctness boundary.
- Explicit non-claim: V0.2 does not claim direct standalone native-image or
  full AOT native-code parity.

## Evidence Reviewed

Code surfaces checked:

- `crates/oxvba-build/src/lib.rs` exposes wrapper-generation modules for
  `compile`, `exe`, `dll`, `comserver`, `comserver_exe`, and `xll`.
- `crates/oxvba-build/src/exe.rs` generates executable shim source that embeds
  an `.oxb` bundle and executes it through `oxvba_host::Engine`.
- `crates/oxvba-build/src/dll.rs` generates native-export DLL shim source and
  marshals export calls through an OxVba runtime session.
- `crates/oxvba-build/src/comserver.rs` generates Windows COM server entry
  point and class-factory source skeletons, while leaving dispatch invocation
  and registration completion as explicit future obligations.
- `crates/oxvba-build/src/xll.rs` generates XLL entry point and registration
  source skeletons.
- `crates/oxvba-jit/src/lib.rs` attempts RtSlot Cranelift execution first,
  then the legacy Cranelift subset, then VM interpreter fallback.

## Residual Boundary

This bead closes only the path decision. It does not close the native
compilation capability lane. The next ready bead is `bd-bqm8.10.3`, which must
publish ABI, packaging, platform, artifact, and deployment obligations for the
selected wrapper-hosted path.
