# OxVba

OxVba is a clean-room Rust implementation of the VBA 7 language, runtime and project model. It is building toward a full compiler, verified executable artifact, complete VM3 interpreter, complete Cranelift JIT, Windows COM/native compatibility and compiler-backed language services.

The implementation is broad but still in progress. It should not yet be presented as a fully conforming completed VBA toolchain. See the [current architecture](docs/ARCHITECTURE.md) and [2026-07-10 status review](docs/OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md).

## Architecture

```text
project/source
  -> target-aware preprocessing
  -> lossless CST
  -> compiler analysis and Core IR
  -> typed OxIR / OxImage (.oxi)
       |-- VM3 reference interpreter
       \-- Cranelift JIT
```

The retired VM2/Bundle/`.oxb` execution path is not part of the current product architecture. `oxvba-bundle` remains the home of Core IR and bounded synthetic VBA-library metadata while that metadata migrates to the current typed contracts.

The durable destination is defined by the [OxVba System Contract](docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md). Current realization and gaps are defined by [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Current capabilities

- lossless VBA lexer/parser and CST;
- target-aware conditional compilation;
- project/reference loading for `.basproj` and a bounded `.vbp` subset;
- provider-based symbol resolution and typed Core IR binding;
- typed CFG OxIR and serialized `.oxi` project-closure artifacts;
- broad VM3 language/runtime/library execution;
- broad platform-neutral Cranelift JIT execution without VM fallback;
- exact-layout-oriented Variant, BSTR, SAFEARRAY, ObjectRef and record carriers;
- host capability/profile/policy abstractions;
- substantial VM-backed Windows COM and Declare infrastructure;
- bounded WrappedComServer build/host infrastructure;
- extensive unit, differential and captured Excel/VBA oracle evidence.

Important open areas include verified artifact admission, several compiler/type/reference edges, complete base-library proof, VM3/JIT structural parity, JIT errors/recursion/sessions/cache, real JIT COM/native support, native DLL/EXE output and the clean-stack language service.

## Build from source

Requirements:

- current stable Rust toolchain;
- Git;
- PowerShell 7 for repository scripts;
- Windows SDK/Office only for Windows COM/native/Excel lanes.

```powershell
git clone https://github.com/DnaCalc/OxVba.git
cd OxVba
cargo build --workspace
```

Run all ordinary tests:

```powershell
cargo test --workspace
```

The 2026-07-10 review records known red workspace/Clippy/governance issues that the current core workset owns. A local green subset is not a full compatibility claim.

## Run source

Create `hello.bas`:

```vb
Sub Main()
    Debug.Print "Hello from OxVba"
End Sub
```

Run through VM3:

```powershell
cargo run -p oxvba-cli -- run hello.bas --backend vm3
```

Run through the JIT:

```powershell
cargo run -p oxvba-cli -- run hello.bas --backend jit
```

The JIT has a hard whole-program decline boundary and does not silently execute unsupported code through VM3.

Useful options:

```text
--dump-values
--diagnostic-format human|json
--backend vm3|jit
--profile <id>
--policy <preset>
--dump-bootstrap
```

Run `cargo run -p oxvba-cli -- --help` for the current complete option list.

## Run a project

```powershell
cargo run -p oxvba-cli -- run-project path\to\project.basproj --backend vm3
```

or:

```powershell
cargo run -p oxvba-cli -- run-project path\to\project.vbp --backend jit
```

The loader constructs the transitive referenced-project closure. An entry can be selected with:

```text
--entry Module.Procedure
```

`.vbp` support is intentionally a bounded import adapter, not a complete VB6 project-system claim.

## Build a wrapped COM server

The current build command is Windows-specific and emits a VM-backed WrappedComServer artifact set including `.oxi`, COM metadata and type-library artifacts:

```powershell
cargo run -p oxvba-cli -- build project.basproj --target WrappedComServer --out-dir out
```

This is a runtime-backed wrapper, not a genuine native per-program DLL. JIT-backed wrappers and native DLL/EXE outputs are target capabilities in the current Windows workset.

## Compatibility and evidence

OxVba targets real VBA compile-time and run-time behavior. Compatibility authority is limited to:

- public specifications and documentation;
- the real VBA type library;
- published research;
- reproducible black-box Office/VBA observations.

Existing OxVba behavior and historical fallbacks are not compatibility targets. Where specifications and observed Excel behavior differ, the discrepancy remains explicit until resolved.

VM3 is the permanent JIT reference interpreter, but VM3 is not itself VBA authority. VM3 behavior is validated against public specifications and Excel/VBA, and corrected divergences become permanent regression evidence.

## Current work program

The active architecture-led worksets are:

- [Ideal Core Toolchain and Dual-Runtime Realization](docs/worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md)
- [Ideal Windows Interop and Native Tooling Realization](docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md)
- [Ideal Language-Service and IDE Foundation Realization](docs/worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md)

Together they target the core, Windows compatibility, IDE foundation and standalone-tooling profiles in the system contract. Forms, debugger, broader security, browser/WASM and non-Windows COM remain explicit extended profiles rather than hidden omissions.

## Documentation

Start with [docs/README.md](docs/README.md).

Primary authority order:

1. [CHARTER.md](CHARTER.md)
2. [OPERATIONS.md](OPERATIONS.md)
3. [OxVba System Contract](docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md)
4. [Current Architecture](docs/ARCHITECTURE.md)
5. current subsystem specifications
6. accepted worksets and canonical validation/evidence artifacts

Superseded specifications and guidance are classified in the [deprecation ledger](docs/spec/DEPRECATION_LEDGER_2026-07-10.md). Historical documents remain useful provenance but are not present-tense implementation authority.

## Contributing

Read [AGENTS.md](AGENTS.md), [OPERATIONS.md](OPERATIONS.md) and [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md). Compatibility work requires explicit subset boundaries, relevant tests/evidence and current documentation. Partial behavior remains `in-progress`; support/docs work alone does not close a capability.

## License

See the repository license.
