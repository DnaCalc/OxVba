# DNA OxIde After-W370 Editor Version Provenance Audit

Date: 2026-05-20
Bead: `bd-94av.3.1`
Workset: `docs/worksets/WORKSET_2026-05-19_DNAOXIDE_AFTER_W370_DIRECT_HOST_ROUNDOUT.md`

## Scope

This audit covers direct editor-facing language-service and host-session
responses that OxIde can consume without LSP transport. The question is whether
each response carries enough document/workspace version identity for a host to
reject stale results after an edit.

The audit is not a delivery claim. It identifies existing provenance and leaves
delivery beads for missing response-level version DTOs and tests.

## Existing Version Truth

Available version truth:

- `Document` carries `version`; every edit creates a new document version.
- `Document::semantic_provenance()` returns `SemanticProvenance` with
  `document_id`, `project_name`, `snapshot_version`, and provenance kind.
- `SymbolInfo` carries `SymbolIdentity` and `SemanticProvenance`.
- `HostWorkspaceRoster` carries `snapshot_revision`, and each
  `HostWorkspaceModuleRosterEntry` carries `document_version` and
  `has_workspace_overlay`.
- `HostWorkspaceSession::semantic_provenance(document)` can return the current
  provenance for a single document.

Important limitation:

- Most editor-facing `HostWorkspaceSession` methods return the raw
  language-service payload (`Vec<T>`, `Option<T>`, or `String`) after checking
  that the document exists. They do not include a response envelope with the
  request document version, current document version, or workspace revision.
  A host can separately query provenance, but that is not atomic with the
  response.

## Response-Family Findings

| Response family | Current exposed version shape | Gap | Delivery owner |
| --- | --- | --- | --- |
| Workspace roster | `HostWorkspaceRoster::snapshot_revision`; module rows carry `document_version` and `has_workspace_overlay`. | Covered for roster/module list purposes. | none |
| Document list | `HostWorkspaceDocument` has document ID, project name, and provenance kind. | No document version; hosts cannot detect stale document-list rows without a separate roster/provenance query. | `bd-7bvk` |
| Document source | `HostWorkspaceSession::document_source` returns `String`. | No document version on the returned text. | `bd-7bvk` |
| Diagnostics | `SpannedDiagnostic` carries span/message/severity only. `HostWorkspaceSession::diagnostics` returns `Vec<SpannedDiagnostic>`. | No document ID/version on diagnostic rows or response envelope. | `bd-7bvk` |
| Document symbols | `DocumentSymbol` carries `SymbolIdentity` and `SemanticProvenance`. | Symbol rows include provenance, but the response has no queried-document version or workspace revision. Empty results cannot carry freshness. | `bd-7bvk` |
| Workspace symbols | `WorkspaceSymbol` wraps `DocumentSymbol`, so symbol rows carry semantic provenance. | No workspace revision for the query result set; empty result sets cannot carry freshness. | `bd-7bvk` |
| Completions | Symbol completions may carry `source_document` and `SemanticProvenance`; keyword/intrinsic completions may not. | No response-level request document version; keyword-only results cannot carry freshness. | `bd-7bvk` |
| Hover | `HoverInfo` may carry `SymbolIdentity` and `SemanticProvenance`. | `None` and keyword/basic hovers have no freshness; no response-level queried-document version. | `bd-7bvk` |
| Go to definition | `Location` carries document, span, optional symbol identity, and optional semantic provenance. | `None` results have no freshness; no response-level queried-document version. | `bd-7bvk` |
| Find references | `Location` rows carry optional semantic provenance. | Empty result sets have no freshness; no response-level queried-document/workspace revision. | `bd-7bvk` |
| Signature help | `SignatureHelp` carries optional source document and provenance. | Direct `HostWorkspaceSession` does not expose a signature-help method; if exposed later, it needs the same response envelope. | future if surfaced |
| Rename preparation / reference analysis | `RenamePreparation` and `ReferenceUpdateAnalysis` carry symbol/location provenance. | Direct `HostWorkspaceSession` does not expose these methods yet; if exposed later, they need response-level freshness. | future if surfaced |
| Code actions | `CodeActionPlan` carries document and diagnostic. | Direct `HostWorkspaceSession` does not expose code actions yet; if exposed later, actions need document version and stale-apply guards. | future if surfaced |

## Delivery Split

The missing work should be delivered as a direct-host response wrapper layer
rather than by requiring OxIde to pair every query with a separate provenance
call. The wrapper should include:

- queried document ID and document version for document-scoped requests;
- workspace snapshot revision for workspace-scoped result sets;
- result payload unchanged enough that existing callers can keep using the raw
  methods during migration;
- tests proving the response version changes after `set_document_text` and that
  empty result sets still carry freshness.

## Non-Claims

This audit does not claim:

- stale-response rejection is implemented;
- every semantic row has document version provenance;
- the existing raw `LanguageService` API is an editor freshness contract;
- future rename/code-action apply paths are protected by version preconditions.

Those remain delivery work under the follow-up bead(s) created from this audit.

## Review Notes

Fresh-eyes review points before closing `bd-94av.3.1`:

- Row-level semantic provenance is distinguished from response-level freshness.
- Empty result sets are treated as first-class stale-response risks.
- Existing roster version coverage is not broadened into a claim for query
  responses.
- Future-only surfaces are called out without making them current delivery
  blockers.
