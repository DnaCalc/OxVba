# WORKSET: Phase 6 — CLI Integration

**Date:** 2026-03-23
**Phase:** 6
**Status:** Planned
**Depends on:** Phase 2 (reference resolution), Phase 3 (OxBundle v2)

---

## Objective

Add `build`, `run-project`, `init`, and `import-vbp` subcommands to the `oxvba` CLI, backed by the `oxvba-project` crate.

---

## Deliverables

### 1. `oxvba build <path.basproj>`

Compile a `.basproj` project to `.oxb` bundle:

- Loads `.basproj` via `load_basproj`
- Compiles `ProjectManifest` via `compile_project`
- Serializes to `.oxb` via `OxBundle::from_compiled_project` + `serialize_to_bytes`
- Output path: `<ProjectName>.oxb` in project dir (or `-o <path>`)

### 2. `oxvba run-project [PATH]`

Load, compile, and execute a project:

- PATH resolution: `.basproj` file, directory with `.basproj`, directory convention mode, or `.vbp` file
- Uses `Engine::execute_project_with_snapshot_phased`
- Supports existing flags: `--jit`, `--profile`, `--policy`, `--dump-values`, `--dump-slots`

### 3. `oxvba init [DIR]`

Scaffold a new project:

- Creates `<dirname>.basproj` with `OutputType=Exe` and empty `Module1.bas`
- Minimal XML with SDK attribute

### 4. `oxvba import-vbp <file.vbp> [--out <path.basproj>]`

Convert `.vbp` to `.basproj`:

- Parse VBP-S0 subset keys (`Type`, `Name`, `Startup`, `Module`, `Class`, `Reference`)
- Generate `.basproj` XML via `generate_basproj_xml`
- Warn on unsupported keys with `VBP-E-UNSUPPORTED-*` diagnostics

---

## Key Existing Code

- `crates/oxvba-cli/src/main.rs` — Current CLI: manual arg parsing, `compile` and `run` subcommands, ~395 lines
- `crates/oxvba-project/src/load.rs` — `load_basproj`, `load_basproj_from_str`
- `crates/oxvba-project/src/generate.rs` — `generate_basproj_xml`
- `crates/oxvba-host/src/engine.rs` — `Engine::execute_project_with_snapshot_phased`

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-cli/src/main.rs` | Add `build`, `run-project`, `init`, `import-vbp` subcommands |
| `crates/oxvba-cli/Cargo.toml` | Add `oxvba-project` dependency |
| `crates/oxvba-project/src/vbp.rs` (new) | VBP-S0 parser for `import-vbp` |
| `crates/oxvba-project/src/lib.rs` | Add `pub mod vbp;` |

---

## Execution Steps

1. Add `oxvba-project` dependency to `oxvba-cli/Cargo.toml`
2. Refactor `main()` match to include `build`, `run-project`, `init`, `import-vbp`
3. Implement `run_build`: parse args, load_basproj, compile_project, serialize OxBundle, write .oxb
4. Implement `run_project`: path discovery (basproj/dir/vbp), load, configure Engine, execute, output
5. Implement `run_init`: create directory, write minimal .basproj + Module1.bas
6. Implement VBP-S0 parser in `vbp.rs`: line-oriented key=value parser, map to BasProj
7. Implement `run_import_vbp`: parse .vbp, generate .basproj XML, write

---

## Closure Conditions

1. `oxvba build MyProject.basproj` produces `MyProject.oxb`
2. `oxvba run-project .` discovers and executes a project
3. `oxvba init myproject` creates a runnable scaffold
4. `oxvba import-vbp legacy.vbp` produces a valid `.basproj`
5. Existing `oxvba compile` and `oxvba run` subcommands still work
