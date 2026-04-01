# Workset: LSP Transport Discipline and Host Project Helper Cleanup

Date: 2026-04-01

## 1. Purpose

This workset follows the first-class language-service tranche completed under:
- `docs/worksets/WORKSET_2026-03-31_LANGUAGE_SERVICE_FIRST_CLASS_PLATFORM_EXECUTION.md`

That tranche delivered:
- a rich direct Rust language-service API,
- a thin `oxvba-lsp` bootstrap transport,
- workspace synchronization,
- a debug harness,
- and initial validation/performance evidence.

The recent design review showed that the next cleanup step is not “more features first.”
It is to tighten transport discipline and separate host-authoring helpers from LSP transport concerns.

This workset therefore owns:
1. cleanup of recent `oxvba-lsp` layering drift,
2. lock-in of the transport discipline that should govern all future LSP work,
3. definition and introduction of explicit host-side project helper APIs for OxIde/direct-embed hosts.

## 2. Problem Statement

The direct language-service core is currently in the right architectural position.
The recent `oxvba-lsp` slices were useful, but they also exposed a few design risks:

1. workspace reload currently replaces the direct service state while leaving URI mappings alive,
2. initialization currently swallows workspace-load failures,
3. URI sync can fabricate detached shadow `DocumentId` values when a file cannot be mapped back to the real project model,
4. project discovery/loading policy is starting to be reimplemented inside the transport crate,
5. host/editor project-authoring operations are still implicit or host-local rather than explicit OxVba helper APIs.

Those are not reasons to reverse the transport work.
They are reasons to tighten the boundary now, before broader LSP feature mapping continues.

## 3. Design Policy

### 3.1 Direct API first

The direct Rust API remains the primary language-service boundary.

That API should own:
1. workspace identity,
2. document identity,
3. project loading,
4. semantic snapshots,
5. diagnostics and semantic queries,
6. rename/reference/code-action planning,
7. explicit project-authoring helper operations.

The direct API should be clean enough that:
1. `oxvba-lsp` can be a thin adapter over it,
2. OxIde can embed it directly,
3. direct-embed hosts can embed it directly,
4. future non-LSP transports can reuse it unchanged.

### 3.2 Thin transport discipline

`oxvba-lsp` may own:
1. protocol/session lifetime,
2. workspace-folder/session notifications,
3. JSON-RPC/LSP marshaling,
4. text synchronization,
5. protocol capability advertisement,
6. protocol-shaped error/reporting behavior.

`oxvba-lsp` must not own:
1. a second parser,
2. a second semantic model,
3. project-discovery policy drift,
4. shadow document creation that bypasses the real project model,
5. editor-only reinterpretation of module/reference ownership.

### 3.3 Honest workspace model

The workspace must be derived from the real OxVba project model.

That means:
1. `.basproj`, bounded `.vbp`, and convention directories are loaded through shared OxVba project-loading policy,
2. project references and imported typelibs enter via the real OxVba model,
3. editor buffer state is an overlay on real workspace documents,
4. unresolved host/editor file identity mismatches should be explicit failures or explicit helper flows, not silent shadow documents.

### 3.4 Multi-root policy

OxVba does not yet have a real multi-root workspace model.

So the honest near-term policy is:
1. support one loaded workspace/project per `oxvba-lsp` session,
2. reject or explicitly ignore extra workspace folders with deterministic reporting,
3. only add real multi-root behavior once the direct API itself has an explicit multi-workspace/session model.

Pretending multi-root support exists by repeatedly replacing a single workspace is incorrect.

### 3.5 Initialization/load error policy

Workspace-load failures must be visible.

Allowed behavior:
1. fail initialization deterministically, or
2. initialize in an explicitly degraded/no-workspace state while surfacing the failure to the host.

Disallowed behavior:
1. swallow load failures silently,
2. appear initialized while the actual workspace was not loaded.

### 3.6 Host project-helper policy

Project-authoring helpers are not an LSP concern.

They should exist as explicit OxVba helper APIs or CLI-backed host helpers for operations such as:
1. create project,
2. add/remove `.bas` module,
3. add/remove `.cls` class module,
4. inspect effective module/class identity,
5. rename logical module name,
6. reconcile file name with logical module name,
7. determine when `Attribute VB_Name` is redundant or required,
8. add/remove project or COM references.

The host may expose those operations through UI.
But the semantics should live in OxVba, not as host-local heuristics.

## 4. Desired End State

This workset is complete when:
1. the transport is honest about single-root vs multi-root behavior,
2. initialization and reload failure behavior is explicit and deterministic,
3. URI/document synchronization no longer fabricates detached project shadows,
4. project discovery/loading policy is shared rather than duplicated in `oxvba-lsp`,
5. a host-facing project-helper API exists for module/class/reference authoring and logical-name inspection,
6. the docs describe the boundary clearly for VS Code-style hosts and direct-embed hosts like OxIde.

## 5. Execution Plan

### Phase A. Policy and design lock

1. publish this workset,
2. encode the cleanup/fix graph in beads,
3. make the next cleanup slice explicit before further feature mapping.

### Phase B. Transport correctness cleanup

1. make workspace/session reload semantics deterministic,
2. clear or rebuild transport-side URI mappings on reload,
3. make initialization/reporting honest on load failure,
4. remove shadow-document fallback behavior.

### Phase C. Shared project-loading cleanup

1. move project discovery/loading policy out of `oxvba-lsp`,
2. reuse shared helper entry points from a non-transport layer,
3. keep convention/basproj/vbp policy defined once.

### Phase D. Host project-helper API

1. define a host-facing helper surface,
2. add initial module/class/reference helper operations,
3. add logical-name and `VB_Name` inspection/update helpers,
4. prove the helpers in direct API tests or a small host harness.

### Phase E. Boundary docs

1. describe what VS Code-class hosts require from LSP,
2. describe what direct-embed hosts require from the direct API,
3. make clear which responsibilities belong to transport vs helper APIs.

## 6. Bead Root

Execution proceeds through a new bead subtree rooted at `bd-ls2`.

Initial intended shape:
1. `bd-ls2.1` policy and design lock,
2. `bd-ls2.2` transport reload and initialization correctness,
3. `bd-ls2.3` remove shadow-document fallback and make file-to-module mismatch explicit,
4. `bd-ls2.4` extract shared project-loading/discovery policy out of `oxvba-lsp`,
5. `bd-ls2.5` define and land host project-helper APIs,
6. `bd-ls2.6` boundary/showcase docs for LSP hosts vs direct-embed hosts.

## 7. Acceptance Statement

At the end of this workset, OxVba should be able to say:

1. the direct language-service API is the canonical semantic/editor surface,
2. `oxvba-lsp` is a thin and honest transport over that surface,
3. workspace/project loading semantics are shared across hosts instead of drifting by transport,
4. hosts that need project-authoring operations have explicit OxVba helper APIs instead of inventing editor-local behavior.
