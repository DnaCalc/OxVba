# V0.2 Language-Service Product Matrix

Date: 2026-04-27

Bead: `bd-bqm8.8.2`

## Scope Rule

The V0.2 language-service claim is direct-API-first. OxIde-class hosts consume
`oxvba-languageservice`, `oxvba-project`, and host/runtime crates directly.
`oxvba-lsp` is a thin transport over the same semantics for VS Code-class hosts.
No row below claims full IDE, full LSP, Roslyn/rust-analyzer, designer/forms, or
multi-root parity.

## Product Matrix

| Row | Area | Scenario | V0.2 status | Evidence / command anchor | Owner bead |
| --- | --- | --- | --- | --- | --- |
| `LS-V02-001` | Direct API | Workspace/document identity, open/change/close, project-backed overlays. | supported-active | `crates/oxvba-languageservice/src/workspace.rs`; `HostWorkspaceSession` docs/tests | `bd-bqm8.8.3` |
| `LS-V02-002` | Direct API | Diagnostics from parse/typecheck/resolution with spans. | supported-active | `LanguageService::diagnostics`; semantic snapshot tests | `bd-bqm8.8.3` |
| `LS-V02-003` | Direct API | Document symbols and workspace symbols with provenance. | supported-active | `document_symbols`; `workspace_symbols` | `bd-bqm8.8.3` |
| `LS-V02-004` | Direct API | Hover, go-to-definition, and find-references over known source symbols. | supported-active | `hover`; `go_to_definition`; `find_references` | `bd-bqm8.8.3` |
| `LS-V02-005` | Direct API | Completions and signature help for bounded keyword, symbol, and intrinsic surfaces. | supported-active | `completions`; `signature_help`; intrinsic spec support | `bd-bqm8.8.3` |
| `LS-V02-006` | Direct API | Rename preparation and safe reference-update analysis with provenance blockers. | supported-active | `prepare_rename`; `ReferenceUpdateAnalysis` | `bd-bqm8.8.3` |
| `LS-V02-007` | Direct API | Diagnostics-driven quick-fix/code-action planning. | supported-active | `code_actions`; `CodeActionPlan` | `bd-bqm8.8.3` |
| `LS-V02-008` | Direct API | Semantic classifications / semantic-token source data. | supported-active | `semantic_classifications` | `bd-bqm8.8.3` |
| `LS-V02-009` | LSP transport | Advertise bounded capabilities for hover, definition, references, symbols, completion, signature help, rename prepare, code actions, and semantic tokens. | supported-active | `server_capabilities()` | `bd-bqm8.8.4` |
| `LS-V02-010` | LSP transport | Single-root workspace load/reload, URI mapping, full-text document sync, diagnostics substrate. | supported-active | `LspCore::load_workspace_path`; open/change/close tests | `bd-bqm8.8.4` |
| `LS-V02-011` | LSP transport | Transport-neutral query parity for exposed direct-service rows. | supported-active | `LspCore` query wrappers | `bd-bqm8.8.4` |
| `LS-V02-012` | OxIde | Direct host-consumption guidance for language service, project helpers, embedded build/run, and no CLI parsing for semantic features. | supported-guidance | `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md`; `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md` | `bd-bqm8.8.5` |
| `LS-V02-013` | VS Code | Alternate-editor path through `oxvba-lsp` plus extension/CLI/direct-helper commands for project authoring. | supported-guidance | `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md`; LSP docs/tests | `bd-bqm8.8.5` |
| `LS-V02-014` | Project authoring | LSP-owned project creation/module/reference authoring methods. | unsupported-v02 | Authoring belongs to CLI/direct helper commands, not LSP methods. | `bd-bqm8.8.5` |
| `LS-V02-015` | Workspace model | Multi-root LSP/workspace semantics. | unsupported-v02 | Single loaded workspace/project is the honest V0.2 transport boundary. | `bd-bqm8.8.6` |
| `LS-V02-016` | IDE parity | Full VS Code extension package, full VBIDE parity, designer/forms editing, complete refactoring suite. | out-of-scope-v02 | Explicit non-scope; do not count docs as capability closure. | `bd-bqm8.8.6` |

## Evidence Inventory

- Direct implementation: `crates/oxvba-languageservice/src/service.rs`,
  `semantic.rs`, `workspace.rs`, `document.rs`, and `host_session.rs`.
- LSP implementation: `crates/oxvba-lsp/src/lib.rs` and `main.rs`.
- Public docs: `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md`,
  `docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md`, and
  `docs/LANGUAGE_SERVICE_SHOWCASE.md`.
- Platform specs: `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md` and
  `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`.

## Checks Run

- `Get-Content docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md | Select-Object -First 220`
- `Get-Content docs/LANGUAGE_SERVICE_HOST_BOUNDARIES.md | Select-Object -First 220`
- `Get-Content docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md | Select-Object -First 220`
- `Get-Content crates/oxvba-lsp/src/lib.rs | Select-Object -Skip 330 -First 55`

## Result

`bd-bqm8.8.2` is complete as a matrix bead. The language-service lane remains
in-progress until the direct-service tests, LSP transport tests, host guidance,
and final checklist beads complete.
