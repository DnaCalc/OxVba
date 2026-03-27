# WORKSET: Console StdIO HAL Refactor Plan

**Date:** 2026-03-27
**Status:** Completed for the bounded first-slice console/stdIO HAL landing. ODG-043 overall remains in-progress pending broader startup oracle coverage.
**Area:** ODG-043 follow-on host/runtime functionality

---

## Objective

Introduce a clean, first-class console/stdIO host surface for CLI execution without distorting real VBA file I/O, UI interaction, or diagnostics semantics.

This workset exists to support QBasic-like console behavior for CLI-hosted OxVba projects:

- plain `Print` writes to console output
- plain `Input` / `Line Input` read from console input
- `Print #` / `Input #` / `Line Input #` remain file I/O
- `Debug.Print` remains diagnostic output and should continue to flow to `stderr`

The architectural goal is to keep Windows process-hosted DLL / GUI-hosted VBA behavior closer to real VBA, while allowing CLI/stdIO hosts to expose honest console capabilities.

---

## Problem Statement

Current OxVba host surfaces are split across:

- `UiInteraction`
- `FileSystemIo`
- `DiagnosticsTelemetry`
- runtime profiles / runtime classes such as `LinuxStdio`

But there is no dedicated console/stdIO capability. That creates pressure to overload one of:

- file I/O
- `InputBox` / UI virtualization
- diagnostics output

That would be architecturally dishonest and would blur real VBA semantics with CLI-host-specific behavior.

---

## Architectural Decision

OxVba should add a dedicated console/stdIO HAL domain instead of reusing file I/O, UI, or diagnostics.

Binding rules for this workset:

- Console/stdIO is a host capability, not a VBA project kind.
- Console/stdIO is distinct from file handles and `#`-qualified file statements.
- Console/stdIO is distinct from `MsgBox` / `InputBox`.
- Console/stdIO is distinct from diagnostics; `Debug.Print` remains diagnostics.
- Runtime profiles should model both OS profile and host shape.
- StdIO host behavior should be available on both Windows and Linux.

---

## Target Architecture

### 1. HAL Capability

Add a dedicated `ConsoleIo` capability to the HAL model.

Planned contract surface:

- `print_line(value)`
- `input_fields(count)`
- `line_input()`

This surface is intentionally narrow for the first slice.

### 2. HostServices Split

Extend `HostServices` with a `console()` domain in parallel with:

- `ui()`
- `fs()`
- `diag()`

This keeps host semantics explicit:

- UI prompts remain UI
- file handles remain file handles
- diagnostics remain diagnostics
- console remains console

### 3. Runtime Class Factoring

The current `LinuxStdio` path should be generalized into a cross-platform stdIO host style.

Planned shape:

- Windows stdIO runtime class/profile
- Linux stdIO runtime class/profile

Capability factoring doctrine:

- profile decides platform affordances and OS boundary behavior
- runtime class decides host shape such as GUI, headless, stdIO

### 4. Compiler / VM Semantics

Plain console statements become their own host-backed execution lane:

- `Print`
- `Input`
- `Line Input`

File forms remain separate:

- `Print #`
- `Input #`
- `Line Input #`

### 5. CLI Host Semantics

The `oxvba` CLI should run loaded projects under a stdIO runtime profile that exposes the console HAL.

Output policy:

- console `Print` -> `stdout`
- `Debug.Print` -> `stderr`

Input policy:

- console `Input` / `Line Input` -> stdin or host callback override

---

## Non-Goals For First Slice

The initial console HAL landing should not attempt to close every BASIC console formatting edge.

Explicitly deferred unless needed:

- full comma/semicolon `Print` zone semantics
- `Input "prompt"; var` prompt formatting surface
- richer terminal control features
- CLI-only helper APIs beyond the base console lane
- host-project injection for basic `Print` / `Input`

Host-project injection may still be useful later for richer CLI-specific helpers, but it is not the foundation for this slice.

---

## Deliverables

- `crates/oxvba-hal/src/model.rs`
- `crates/oxvba-hal/src/traits.rs`
- `crates/oxvba-hal/src/lib.rs`
- `crates/oxvba-hal/src/callbacks.rs`
- `crates/oxvba-hal/src/adapters/standard/mod.rs`
- `crates/oxvba-hal/src/adapters/standard/descriptor.rs`
- `crates/oxvba-hal/src/adapters/standard/console.rs`
- `crates/oxvba-hal/src/adapters/null.rs`
- `crates/oxvba-hal/src/adapters/replay.rs`
- `crates/oxvba-hal/src/adapters/recording.rs`
- `crates/oxvba-hal/src/conformance.rs`
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-jit/src/runtime_helpers.rs`
- `crates/oxvba-host/src/runner.rs`
- `crates/oxvba-cli/src/main.rs`
- host / HAL / compiler / VM regression coverage for the new lane

Optional later deliverables:

- CLI-oriented host-injected helper project/module for richer console helpers
- docs/spec note for console/stdIO host semantics

---

## Implementation Phases

### Phase 1. HAL Contract Refactor

- add `CapabilityId::ConsoleIo`
- add `ConsoleHal`
- add `HostServices::console()`
- export the new trait in `oxvba-hal`
- update descriptor matrices
- update null / replay / recording / standard adapters

### Phase 2. Runtime Profile and Capability Factoring

- add or rename runtime profiles so stdIO exists on both Windows and Linux
- keep GUI/headless behavior distinct
- ensure `ConsoleIo` support is true only for stdIO runtime classes
- leave `Debug.Print` on diagnostics

### Phase 3. Compiler Surface

- add bound statements for plain:
  - `Print`
  - `Input`
  - `Line Input`
- keep `#`-qualified file statements unchanged
- add bytecode instructions for console host intrinsics
- emit them from the compiler

### Phase 4. VM / JIT Execution

- dispatch console intrinsics through `host_services.console()`
- update capability-to-code mappings in VM/JIT helper layers
- preserve clear host capability errors when console is unavailable

### Phase 5. CLI / Host Integration

- wire stdIO runtime profile selection in the CLI to the new console lane
- ensure console output goes to `stdout`
- ensure diagnostic output remains on `stderr`
- add callback overrides for deterministic tests and embeddings

### Phase 6. Validation and Docs

- HAL tests for capability reporting and adapter behavior
- compiler tests for plain statement lowering
- VM tests for execution and denial behavior
- CLI/host tests for stdIO projects on both Windows and Linux profiles
- doc/status update after landing

---

## Design Constraints

- Do not describe this work as complete until stdIO console semantics are genuinely available cross-platform for the declared first-slice behavior.
- Do not repurpose `FileSystemIo` to mean console.
- Do not repurpose `UiInteraction` to mean console.
- Do not repurpose `DiagnosticsTelemetry` to mean console.
- Do not regress Windows GUI / headless parity by broadening stdIO assumptions into non-stdio runtime classes.

---

## Initial Success Criteria

This workset first reaches a meaningful landing point when all of the following are true:

1. Plain `Print`, `Input`, and `Line Input` are real host-backed statements, not unsupported placeholders.
2. Those statements execute through a dedicated console HAL path.
3. StdIO runtime classes exist and expose `ConsoleIo` on both Windows and Linux.
4. `Print #`, `Input #`, and `Line Input #` still execute through file I/O.
5. `Debug.Print` still routes to diagnostics and remains suitable for `stderr`.
6. CLI project execution can use console I/O without changing `.basproj` startup semantics.

---

## Follow-On Work

After this HAL refactor lands, a second workset may add a CLI-host injected helper project/module for richer console affordances such as:

- explicit `StdErr`
- command-line argument helpers
- terminal-oriented helper functions

That follow-on should build on top of the new console HAL, not replace it.

---

## Landing Outcome

The bounded first slice described in this workset is now landed:

- `ConsoleIo` is a first-class HAL capability.
- `HostServices` now exposes a dedicated `console()` domain.
- StdIO runtime classes now exist on both Windows and Linux (`windows-stdio`, `linux-stdio`).
- Plain `Print`, `Input`, and `Line Input` now execute through console host intrinsics.
- `Print #`, `Input #`, and `Line Input #` remain on the file-I/O lane.
- `Debug.Print` remains on diagnostics and is suitable for `stderr`.
- CLI/host execution can use console I/O without changing loaded `.basproj` startup semantics.

This workset does not close `ODG-043` as a whole. The remaining open scope is still startup/entrypoint oracle breadth across wider project configurations.

## Verification

Executed during landing:

- `cargo test -p oxvba-compiler compile_console_io_statements_emit_console_host_instructions --quiet`
- `cargo test -p oxvba-host --test console_stdio_end_to_end --quiet`
- `cargo test -p oxvba-hal --quiet`
- `cargo test -p oxvba-jit --quiet`
- `cargo test -p oxvba-host --quiet`
