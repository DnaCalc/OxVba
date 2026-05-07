# Wrapper Library Runner Producer Evidence

Date: 2026-05-07
Bead: `bd-9xmu.5.9`

## Outcome

The native-ready runner CLI now has a real wrapper-library producer:

```text
oxvba native-ready-runner ... --wrapper-library [--wrapper-library-out <dll|so|dylib>]
```

The producer:

- compiles the loaded project,
- validates declared `<NativeExport>` descriptors,
- builds a wrapper DLL/shared-library artifact with `oxvba_build::dll::generate_dll_shim`,
- records `artifact_path` and `artifact_size_bytes`,
- loads the generated library through the existing native FFI bridge,
- invokes a supported exported function/sub with deterministic smoke arguments,
- emits a `backend=wrapper-library`, `artifact_kind=wrapper-library`, `result_kind=exported-call` row under `NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1`.

This is wrapper-host-over-OXB evidence. It is not direct native PE/ELF codegen evidence.

## Validation

Focused parser checks:

```text
cargo test -p oxvba-cli parse_native_ready_runner_args_supports_wrapper_library --quiet
cargo check -p oxvba-cli --all-targets
```

Real wrapper-library smoke command executed over a temp project with:

```vb
Public Function NativeReadyValue() As Long
    NativeReadyValue = 42
End Function
```

and a matching `<NativeExport Include="NativeReadyValue">` descriptor:

```text
cargo run -p oxvba-cli --quiet -- native-ready-runner target/native-ready/wrapper-library-smoke/WrapperLibrarySmoke.basproj \
  --run-id-prefix nr-cli-wrapper-library-smoke-001 \
  --timestamp-utc 2026-05-07T00:00:00Z \
  --workload-id NR-WRAPPER-LIB-001 \
  --workload-name "Wrapper library smoke" \
  --source-path target/native-ready/wrapper-library-smoke/Module1.bas \
  --iterations 1 \
  --out target/native-ready/wrapper-library-smoke/native-ready-wrapper-library.csv \
  --wrapper-library \
  --wrapper-library-out target/native-ready/wrapper-library-smoke/WrapperLibrarySmoke.dll
```

Observed wrapper-library row shape:

```text
nr-cli-wrapper-library-smoke-001-wrapper-library,...,wrapper-library,wrapper-library,target/native-ready/wrapper-library-smoke/WrapperLibrarySmoke.dll,2207232,correctness,1,0,...,0,,false,not-applicable,exported-call,fnv1a64:63d73df59f98c366,"Wrapper library artifact built and invoked by native-ready runner via exported NativeExport call; wrapper host over OXB, not direct native PE/ELF evidence"
```
