# `_legacy_harvest/` — reference code for the clean-stack rebuild

This directory holds **non-main-path code on equal footing as reference material**.
None of it is a workspace member; none of it builds. It is kept so the clean stack
(`source → oxvba-syntax → oxvba-symbol → oxvba-bind → oxvba-bundle → oxvba-vm2`,
plus `oxvba-lib`/`oxvba-runtime`/`oxvba-hal`/`oxvba-com`/`oxvba-project`/`oxvba-host`/
`oxvba-cli`) can be **rebuilt by re-implementing against this as a reference** — not
by patching the old code. Full history is also in git; this tree is the convenient
working reference.

## The clean stack (the ONLY thing the workspace builds)

`oxvba-hal · oxvba-syntax · oxvba-runtime · oxvba-com · oxvba-bundle · oxvba-lib ·
oxvba-vm2 · oxvba-symbol · oxvba-bind · oxvba-project · oxvba-host · oxvba-cli`

## What's here, and the rebuild intent

### Frontend / tooling crates — rebuild on the clean stack (oxvba-symbol semantic model)
| Crate | What it did | Rebuild note |
|---|---|---|
| `oxvba-languageservice` | semantic model / analysis queries over the legacy frontend (HIR, query DB) | re-implement over `oxvba-symbol` resolution + `oxvba-syntax` CST |
| `oxvba-lsp` | LSP server on top of `oxvba-languageservice` | re-point at the rebuilt language service |
| `oxvba-debug` | debugger (DAP) over the legacy VM | re-point at `oxvba-vm2` (needs a vm2 debug surface) |
| `oxvba-launcher` | run/launch entry (legacy VM + JIT) | fold into the clean `oxvba-host`/`oxvba-cli` run path |
| `oxvba-web-host` | web/wasm host over the legacy frontend | re-point at the clean stack + `oxvba-hal` wasm adapter |
| `oxvba-web-shell` | web shell UI over `oxvba-web-host`/`oxvba-languageservice` | re-point at the rebuilt web host |

### Native-export build — re-target at the clean bundle
| Crate | What it did | Rebuild note |
|---|---|---|
| `oxvba-build` | DLL/EXE/COM-server/`.tlb`/XLL shims embedding a serialized `OxBundle` + reflection-driven signature emit | re-target bundle-embed + reflection at the `oxvba-bundle` `Bundle`. Reusable as-is: `registration.rs` (COM HKCR/CLSID/TypeLib registry writes), `deffile.rs` (`.def`), `compile.rs` (rustc invoke), `idl.rs`/`typelib_gen.rs` (IDL/`.tlb` gen) |

### Legacy execution guts — superseded by `oxvba-vm2`; reference only
| Crate | What it did |
|---|---|
| `oxvba-compiler` | legacy frontend + HIR lowering + bytecode + `OxBundle` + `compile_project` + reflection descriptors. NOTE: `module_unit_from_source` + the `ModuleUnit`/`ModuleAttributes`/`ModuleKind`/`ProjectKind` structs were rehomed into `oxvba-project` before the move. |
| `oxvba-vm` | legacy register-window VM (`Vm::execute_package`, `VmExecutionPackage`, package-identity evidence) |
| `oxvba-jit` | a 61-line stub (`JIT_NOT_IMPLEMENTED_MESSAGE`) |

## High-value reference points (lessons / fragments / tests)

- **Host-sensitivity compile-time gate** — `oxvba-host/.../engine.rs::preflight_host_sensitive_support`
  (in git history at the pre-cleanup commit; matches HAL capability + host policy
  against host-sensitive intrinsics over legacy `Bytecode`). The clean `vm2` path has
  **no equivalent** — re-express it over `oxvba-bundle` ops. (Deferred review item M1.)
- **VBA-semantics test corpora** (re-point at the clean path; assertions + VBA source
  are reusable, only the harness is legacy):
  - `oxvba-vm/tests/vm_feature_coverage.rs` — the broad scalar/string/array/UDT/control-flow/error matrix (highest value).
  - `oxvba-host/tests/com_early_project_end_to_end.rs`, `com_client_end_to_end.rs`,
    `invoke_procedure_tests.rs`, `file_io_host_backed_end_to_end.rs`,
    `pointer_helpers_end_to_end.rs`, `startup_entry_end_to_end.rs`,
    `native_declare_string_marshalling_end_to_end.rs`, `imported_collection_newenum_regression.rs`.
- **`.basproj` loader + entry-shim / top-level-mainline rewrite** — already rehomed
  into `oxvba-project` (the clean path needs it); see `oxvba-project/src/load.rs`.
