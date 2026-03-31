# OxVBA v0.1.0

This release provides prebuilt Windows x64 binaries for running OxVBA without installing Rust.

Artifacts:
- `oxvba-v0.1.0-windows-x64.zip`
  - `oxvba.exe`: the main OxVBA CLI
  - `oxvba-run.exe`: launcher for compiled `.oxb` bundles
  - `README.txt`: short local usage note

Quick start:

```powershell
oxvba.exe run hello.bas
oxvba.exe build .\demo-app -o .\dist\DemoApp.oxb
oxvba-run.exe .\dist\DemoApp.oxb
```

Notes:
- These release binaries are intended to run directly on Windows x64 with no Rust toolchain installed.
- `.oxb` is the current stable compiled OxVBA bundle format.
- `OutputType=Exe` means executable startup semantics for OxVBA projects; the stable build artifact emitted by the CLI today is still `.oxb`, not a native project-specific Windows `.exe`.

For the full user guide, project formats, references, parity boundaries, and runtime/host options, see the repository README:
- https://github.com/DnaCalc/OxVba#readme
