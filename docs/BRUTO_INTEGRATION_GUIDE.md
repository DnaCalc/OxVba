# Bruto Integration Guide

This guide covers the bounded in-repo Bruto/OxVba integration.

## Crates

The Bruto integration currently lives in:
- `crates/oxvba-bruto-lang`
- `crates/oxvba-bruto`

The integration uses direct OxVba APIs. It does not route through `oxvba-lsp`.

## Supported Slice

The current bounded slice supports:
1. Bruto language registration as `OxVba`
2. `.bas` as the primary file extension
3. a minimal OxVba sample program
4. lexical syntax highlighting for core VBA categories
5. one-file compile/build through direct OxVba project and compiler surfaces
6. bounded run execution through the Bruto build contract, with console output captured by direct host callbacks

The design boundary for this slice is in:
- `docs/BRUTO_INTEGRATION_BOUNDARY.md`

## Build

### Windows x64

Native-host build:

```powershell
./scripts/build-bruto.ps1
```

Equivalent direct cargo command:

```powershell
cargo build --release -p oxvba-bruto --target x86_64-pc-windows-msvc
```

Artifact:

```text
target/x86_64-pc-windows-msvc/release/oxvba-bruto.exe
```

### Linux x64

Build this natively on a Linux x64 host:

```bash
./scripts/build-bruto.sh
```

Equivalent direct cargo command:

```bash
cargo build --release -p oxvba-bruto --target x86_64-unknown-linux-gnu
```

Artifact:

```text
target/x86_64-unknown-linux-gnu/release/oxvba-bruto
```

Notes:
- this repo now includes the target selection path and script, but this workset does not claim a Linux binary produced from the current Windows machine
- use a normal Linux Rust toolchain and native system libraries on the Linux host

## Run

After building, launch the Bruto host binary:

```text
target/<target-triple>/release/oxvba-bruto[.exe]
```

## Build Evidence

Windows x64 release build was produced on 2026-04-01 with:
- command: `cargo build --release -p oxvba-bruto --target x86_64-pc-windows-msvc`
- artifact: `target/x86_64-pc-windows-msvc/release/oxvba-bruto.exe`
- artifact size: `2477568` bytes

Linux x64 evidence in this workset is the native-host source build path:
- script: `scripts/build-bruto.sh`
- command: `cargo build --release -p oxvba-bruto --target x86_64-unknown-linux-gnu`
- no claim is made here that a Linux binary was cross-produced on Windows

## Current Limits

This integration does not yet claim:
1. full multi-file `.basproj` authoring inside Bruto
2. class-module editing parity
3. semantic IDE parity with the direct language-service surface
4. debugger parity
5. full project/reference management inside Bruto
