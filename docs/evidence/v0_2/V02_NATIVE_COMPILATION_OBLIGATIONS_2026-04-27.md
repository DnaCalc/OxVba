# V0.2 Native Compilation Obligation Evidence

Date: 2026-04-27

Bead: `bd-bqm8.10.3`

## Result

Published the V0.2 ABI, packaging, platform, artifact, and deployment
obligations for the selected wrapper-hosted native path.

Artifacts:

- [V02_NATIVE_COMPILATION_OBLIGATIONS_V1.md](/C:/Work/DnaCalc/OxVba/docs/validation/V02_NATIVE_COMPILATION_OBLIGATIONS_V1.md)
- [V02_NATIVE_COMPILATION_OBLIGATIONS_V1.csv](/C:/Work/DnaCalc/OxVba/docs/validation/V02_NATIVE_COMPILATION_OBLIGATIONS_V1.csv)

## Coverage

The matrix covers these obligation areas:

- bundle artifact embedding
- host runtime execution
- Cranelift accelerator and VM fallback boundary
- DLL export ABI shape
- COM server entry point and class factory skeleton boundaries
- XLL entry point and registration metadata boundaries
- external value ABI materialization
- platform target and non-Windows boundaries
- registration and deployment separation
- artifact provenance

## Residual Boundary

This bead defines the obligations but does not yet provide the executable
scaffold. The next ready bead is `bd-bqm8.10.4`, which must add the runnable
native validation scaffold and tie it back to this matrix.
