# OxVBA Language-Service Platform Spec V2

**Status:** active successor spec  
**Date:** 2026-04-01  
**Supersedes for first-class platform planning:** `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md`  
**Retains:** `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md` as the design-locked authority for the current bounded internal surface tracked by `LSF-0001`

---

## 1. Purpose

This spec defines the first-class language-service platform target for OxVba.

It does not rewrite current truth retroactively.

Current truth remains:
1. the bounded internal service surface described by `LANGUAGE_SERVICE_SPEC_V1.md`,
2. the current validated inventory tracked in `LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv`,
3. no current claim of full Roslyn parity, rust-analyzer parity, or full LSP/editor parity.

This V2 spec exists so forward execution can be explicit about:
1. the capability ladder,
2. the project/workspace source contract,
3. the direct Rust API vs thin transport boundary,
4. tranche-A feature ownership,
5. the honesty boundary between the current bounded internal surface and the broader platform target.

---

## 2. Relationship to V1

`LANGUAGE_SERVICE_SPEC_V1.md` remains authoritative for the currently implemented internal design:
1. lossless syntax,
2. semantic snapshots,
3. workspace invalidation,
4. diagnostics,
5. symbols,
6. completions,
7. signature help,
8. go-to-definition,
9. find-references,
10. hover.

This V2 spec adds the next-layer platform contract.

The two specs therefore have different jobs:
1. `V1` says what the current bounded internal service layer is.
2. `V2` says what the first-class platform execution track must become.

No statement in this document upgrades the current validation truth on its own.

---

## 3. Capability Ladder

OxVba language-service capability is described through explicit levels.

### 3.1 LS-B0: bounded internal service surface

This is the current implemented and validated baseline.

Included:
1. syntax tree,
2. semantic snapshot,
3. workspace invalidation,
4. diagnostics,
5. symbols,
6. completions,
7. signature help,
8. go-to-definition,
9. find-references,
10. hover.

Excluded:
1. project-aware first-class workspace claims,
2. stable symbol identity across snapshots,
3. rename/code-action guarantees,
4. semantic-token/classification surface,
5. thin external transport guarantee.

This level is what `LSF-0001` tracks today.

### 3.2 LS-P1: project-aware service core

This is the first platform step beyond the bounded inventory.

Required:
1. real `ProjectManifest` ownership in the workspace,
2. real multi-module semantics,
3. project-reference awareness,
4. imported-typelib awareness through the real OxVba projection path,
5. explicit source/provenance ownership,
6. deterministic invalidation and cache semantics.

This level is about correctness of the analysis substrate, not new editor surface area.

### 3.3 LS-P2: first-class editor tranche A

This is the first user-visible first-class editor tranche.

Required:
1. document symbols,
2. workspace symbols,
3. semantic classification / semantic-token surface,
4. richer completion context,
5. richer signature-help context,
6. richer hover context,
7. rename preparation,
8. safe reference-update analysis,
9. diagnostics-driven code-action foundation.

This tranche may still be intentionally bounded.
It does not imply full IDE parity.

### 3.4 LS-P3: transport and embedding boundary

This level exists when the same service core can be consumed:
1. directly through the Rust API,
2. by an embedding/debug harness,
3. by a thin LSP adapter that does not own separate semantic logic.

### 3.5 LS-P4: validated showcase and performance boundary

This level exists when:
1. the capability ladder is reflected in validation rows,
2. performance/invalidation evidence exists for interactive use,
3. user/developer docs can describe the supported subset honestly,
4. showcase claims remain tranche-bounded.

---

## 4. Tranche-A Deliverables

The first first-class editor tranche is explicitly bounded to:
1. document symbols,
2. workspace symbols,
3. semantic classification/token surface,
4. richer completion/signature/hover context,
5. rename preparation and safe reference-update analysis,
6. diagnostics-driven code-action foundation.

Tranche A does not claim:
1. full refactoring parity,
2. full code-action families,
3. semantic-tokens parity with every modern LSP client,
4. designer/forms IDE parity,
5. full macro-host UX parity.

---

## 5. Non-Goals and Honesty Boundary

This platform work does not by itself claim:
1. complete Roslyn parity,
2. complete rust-analyzer parity,
3. full LSP parity,
4. full Visual Basic IDE parity,
5. proof-complete formalization,
6. closure of all language/workspace/editor futures in one tranche.

Honesty rules:
1. `LSF-0001` continues to describe the current bounded internal surface until new validation rows are added.
2. This spec is a forward platform contract, not evidence that later levels are already implemented.
3. Crate/module comments must not describe the current implementation as already first-class when validation still records a bounded internal subset.

---

## 6. Project-Aware Workspace Source Contract

The language-service workspace must be built from the real OxVba model, not an editor-only parallel model.

### 6.1 Source-of-truth inputs

The workspace source of truth is:
1. a real `ProjectManifest`,
2. real module/class/document sources,
3. real project references,
4. imported typelibs through OxVba's projected imported-reference path,
5. host-supplied edits layered onto those sources,
6. explicit generated modules/shims where OxVba itself introduces them.

### 6.2 Contract rules

1. The workspace must not invent a second compiler model just for editor consumption.
2. Project references must enter analysis through the same semantic ownership model that compilation uses.
3. Imported typelibs must enter analysis through the real projected reference seam, with provenance preserved as imported-typelib origin rather than flattened into anonymous source modules.
4. Host-supplied edits are source overlays, not semantic shortcuts.
5. Generated/project-shaped inputs remain valid workspace members only if their provenance is explicit.
6. Every externally consumable query must be answerable against an immutable semantic snapshot.

### 6.3 Identity and provenance expectations

The workspace contract must carry enough provenance to answer:
1. which project a symbol comes from,
2. which document/module declared it,
3. whether it originated from source, generated shim, project reference, or imported typelib projection,
4. which snapshot/version produced the answer.

Name-only matching is acceptable only for the bounded current surface.
It is not sufficient for the first-class platform target.

### 6.4 Editing model

The host/editor may:
1. open a document,
2. replace document text,
3. close a document,
4. reload a workspace/project,
5. override project-supplied source text in memory.

The host/editor may not:
1. bypass project/reference semantics,
2. inject editor-only semantic tables as a replacement for OxVba project ownership,
3. treat the transport protocol as the semantic source of truth.

---

## 7. Direct Rust API and Thin Transport Boundary

OxVba language services are direct-API-first.

### 7.1 Direct API responsibilities

The direct Rust API owns:
1. workspace loading,
2. snapshot construction,
3. diagnostics,
4. symbol queries,
5. definition/reference queries,
6. completion/signature/hover queries,
7. semantic classification,
8. rename-preparation safety analysis,
9. code-action planning inputs tied to diagnostics.

### 7.2 Thin transport responsibilities

A later `oxvba-lsp` or equivalent thin transport may own:
1. document open/change/close notifications,
2. workspace folder/session plumbing,
3. JSON-RPC/LSP marshaling,
4. client capability negotiation,
5. protocol-specific formatting and batching.

### 7.3 Thin transport non-responsibilities

The transport must not own:
1. a separate parser,
2. a separate semantic model,
3. a separate reference-resolution model,
4. editor-only reinterpretation of project/reference semantics,
5. a second symbol graph independent from the direct API.

### 7.4 Transport-neutral query model

The direct API should expose transport-neutral concepts:
1. document identity,
2. workspace identity,
3. position/span,
4. semantic snapshot/version,
5. symbol identity/provenance,
6. diagnostics,
7. query result records suitable for both embedding and wire adaptation.

If a query shape only makes sense as an LSP packet and not as an OxVba semantic concept, it does not belong in the core service layer.

---

## 8. Acceptance Shape for Contract Lock

The contract-lock phase is complete when:
1. the capability ladder is explicit,
2. tranche-A deliverables are explicit,
3. non-goals and honesty boundaries are explicit,
4. the project-aware workspace source contract is explicit,
5. the direct-API-first and thin-transport boundary is explicit,
6. the next implementation lane can proceed without reopening architectural ambiguity.

That is the closure target for `bd-ls1.2`.

---

## 9. Execution Mapping

This spec maps to the active bead subtree as follows:
1. `bd-ls1.2.1`: capability ladder and tranche boundary,
2. `bd-ls1.2.2`: project-aware workspace source contract,
3. `bd-ls1.2.3`: direct Rust API and thin transport boundary,
4. `bd-ls1.3+`: implementation work over the now-locked contract.

The next implementation bead after this contract lock should therefore begin at workspace hardening, not at further architecture debate.
