# Workset: OxIde Host Surface And VS Code Alternate Editor Execution

Date: 2026-04-01
Owner: Codex
Status: in-progress

## Purpose

Turn the current OxVba language-service and project-helper layers into a clearer, first-class editor substrate for OxIde, while keeping VS Code as an alternate integration lane over a thin LSP transport.

This workset is intentionally OxVba-side first:
- tighten the public direct host-facing interface consumed by OxIde,
- update guidance so hosts know which OxVba surfaces are canonical,
- and plan the separate VS Code extension lane over `oxvba-lsp`.

## Why This Workset Exists

OxVba already has:
- a direct language-service core in `oxvba-languageservice`,
- canonical project loading and host project-helper APIs in `oxvba-project`,
- and a thin `oxvba-lsp` transport shell.

OxIde already exists as a local host with the right top-level seams:
- `ProjectSession`,
- `DocumentSession`,
- `EditorSurface`,
- and an `OxVbaServices` seam.

But the current OxIde seam is still too thin and CLI-oriented:
- it shells out for build/run,
- it does not yet consume a first-class direct OxVba editor/session API,
- and OxVba’s public host guidance is still more implicit than explicit.

So the next OxVba-side step is not “invent another host.”
It is:
- make OxIde the showcase direct-embed consumer,
- make the direct Rust API the primary editor story,
- and treat VS Code as an alternate editor integration over the same semantics.

## Governing Policy

1. `oxvba-languageservice` and `oxvba-project` remain the semantic/project source of truth.
2. OxIde should prefer direct typed Rust APIs, not CLI parsing and not LSP.
3. `oxvba-lsp` remains a thin transport for VS Code-class hosts.
4. Project authoring stays outside LSP.
5. No second project model, no second parser, no transport-local naming or reference rules.

## Current Honest Starting Point

Current OxVba direct-host strengths:
- project-aware semantic analysis,
- workspace/document identity,
- diagnostics, symbols, completion, hover, definition, references,
- rename preparation and reference-update analysis,
- bounded diagnostics-driven code-action planning,
- canonical project-loading policy,
- typed module/reference helper plans,
- validated project-edit planning and apply flow for `.basproj` host authoring.

Current OxVba direct-host gaps:
- no explicit high-level IDE session facade over the current query surface,
- no broader typed project/session API for roster and authoring workflows,
- build/run is still more CLI-shaped than direct-host-shaped,
- public guidance does not yet present OxIde as the reference direct-embed consumer clearly enough.

Updated current status:
- the first direct IDE session API is now implemented
- the project/helper and COM helper surfaces are now implemented
- the first embedded build/run direct-host substrate is now implemented
- OxVba-side direct-host showcase boundary docs now exist
- the remaining showcase gap is real OxIde-side consumption evidence over that stack

Current VS Code gaps:
- `oxvba-lsp` still exposes only a bounded subset of the direct semantic ladder,
- there is no OxVba VS Code extension package yet,
- project authoring commands for VS Code are still only conceptual.

## Intended OxVba-Side Outcome

After this workset, OxVba should present a cleaner public editor integration model:

### For OxIde

- direct typed host/session APIs over the existing language-service and project layers,
- clear project/module/reference authoring helpers,
- typed build/run requests and results suitable for embedding,
- docs that present OxIde as the reference direct host.

### For VS Code

- a documented separation between:
  - direct semantic source of truth in OxVba,
  - thin LSP transport in `oxvba-lsp`,
  - and extension commands for project authoring flows.
- a concrete execution lane for an OxVba VS Code extension package.

## Proposed OxVba-Side Additions

### A. Public Direct Host Interface Tightening

Clarify and stabilize the public host-facing story around:
- `oxvba-languageservice` query/result types,
- `oxvba-project` workspace target loading,
- `oxvba-project::host_helpers`,
- and the recommended ownership split between host UI/session state and OxVba semantic/project state.

### B. IDE Session Facade

Add a direct host-facing facade that reduces orchestration burden for OxIde.

Candidate responsibilities:
- load/reload workspace target,
- open/update/close documents,
- expose diagnostics and semantic queries by document identity,
- preserve real project-model identity instead of detached file guesses.

This facade should be thin and typed.
It should not introduce a second semantic model.

### C. Broader Project Authoring Helpers

Expand the helper surface as OxIde needs it:
- module roster inspection,
- add/remove module and class flows,
- reference listing and edit application planning,
- project create/open/save flows as needed,
- file-name / logical-name / `Attribute VB_Name` reconciliation.

### D. Typed Build/Run Embedding Surface

Define a typed direct-host build/run contract for embedded IDE use.

This should:
- preserve the CLI as the end-user shell,
- but avoid requiring OxIde to parse CLI-shaped strings for core workflows.

### E. VS Code Alternate Integration

Plan a minimal VS Code extension lane that uses:
- `oxvba-lsp` for language features,
- extension commands for project authoring,
- and the same OxVba project/helper semantics as the source of truth.

## Execution Slices

1. guidance/public-interface update
2. OxIde-facing direct-host facade design
3. first typed IDE session API implementation
4. OxIde showcase/debug harness and docs
5. VS Code extension planning and initial scaffold

## Non-Goals

This workset does not claim:
- full VS Code extension completion in one pass,
- full LSP parity immediately,
- OxIde UI/editor implementation inside the OxVba repo,
- a second editor-specific project model,
- or a transport-owned project authoring story.

## Exit Condition

This workset is complete only when:
- OxVba guidance clearly presents the direct host surface for OxIde,
- the public host-facing interface is explicit rather than implied,
- the next executable OxIde-facing OxVba API slices are tracked as beads,
- and the VS Code alternate integration lane is explicitly tracked rather than hand-waved.

## Follow-On Split

This workset remains the parent policy and ownership document for editor-facing host integration, but execution now splits into more concrete follow-on lanes:

- OxIde direct-host continuation remains under this workset and the `bd-oxi1` bead tree.
- Embedded build/run direct-host delivery is split into:
  - `docs/worksets/WORKSET_2026-04-03_EMBEDDED_BUILD_RUN_DIRECT_HOST_EXECUTION.md`
- VS Code extension delivery is split into:
  - `docs/worksets/WORKSET_2026-04-02_VSCODE_EXTENSION_AND_LSP_FEATURE_LADDER_EXECUTION.md`
- shared OxVba-code debugging is split into:
  - `docs/worksets/WORKSET_2026-04-02_OXVBA_DEBUGGING_SERVICE_AND_HOST_INTEGRATION.md`

That split reflects the intended architecture:
- OxIde consumes direct Rust host APIs.
- VS Code consumes `oxvba-lsp` plus an extension package.
- Debugging should share one semantic OxVba-side debugger core, then project into OxIde directly and VS Code through DAP later.
