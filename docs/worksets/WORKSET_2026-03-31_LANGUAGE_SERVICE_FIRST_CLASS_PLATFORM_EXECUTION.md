# Workset: First-Class Language Service Platform Execution

Date: 2026-03-31  
Status: in-progress  
Scope: evolve OxVba from the current bounded internal language-service surface into a first-class editor-facing platform in the style of Roslyn and rust-analyzer, with direct Rust APIs first and LSP as a thin transport layer.

## 0. Current Truth

This workset starts from the current bounded, implemented internal service surface rather than from zero.

Current evidence-backed baseline:
1. `crates/oxvba-languageservice` exists and is exercised in `cargo test -p oxvba-languageservice`,
2. the current internal surface includes:
   - syntax tree,
   - semantic snapshot,
   - workspace invalidation,
   - diagnostics,
   - symbols,
   - completions,
   - signature help,
   - go-to-definition,
   - find-references,
   - hover,
3. `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md` is design-locked for the current internal architecture,
4. `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md` is the active successor contract for first-class platform execution,
5. `docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv` intentionally records this area as an `in-progress` bounded internal service surface, not as full LSP parity or a feature-complete editor platform.

Current execution progress:
1. Phase A contract lock is complete under `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`,
2. Phase B workspace/reference loading is complete for `ProjectManifest` ownership plus referenced-project and imported-typelib participation,
3. Phase B symbol identity/provenance is now present in the current bounded service surface for definition/reference query results,
4. Phase B invalidation/performance evidence is now present through explicit workspace stats and interactive query harness coverage in the LS test surface,
5. Phase C editor-capability tranche A is now the next execution slice.

This workset is therefore an extension and elevation track.
It must not widen current truth claims retroactively.

## 1. Purpose

OxVba already has the start of a language-service engine.
What it does not yet have is a first-class product surface for editor and host integration.

The goal of this workset is to turn the existing bounded internal service layer into a platform that can honestly support:
1. first-class in-process editor/host integration,
2. project-aware and reference-aware analysis across real OxVba projects,
3. richer semantic queries suitable for modern IDE features,
4. a thin external LSP adapter rather than an LSP-first architecture,
5. validation and documentation that describe the resulting capability ladder honestly.

The intended style target is:
1. Roslyn-like snapshot isolation and host-facing service contracts,
2. rust-analyzer-like typed syntax/semantic query ergonomics and editor feature shape,
3. OxVba-specific ownership of VBA, project, host, and imported-typelib semantics.

## 2. Product Direction

### 2.1 Primary product shape

The primary product shape is:
1. direct Rust API first,
2. host/editor embedding first,
3. LSP second as a thin transport wrapper over the direct service API.

This follows the already-recorded decision direction in the archived hosting/tooling proposal and remains the right architecture for low-latency embedded hosts.

### 2.2 Source-of-truth rule

The language-service source of truth must remain OxVba's real project and compiler model:
1. `ProjectManifest`,
2. module/class/document sources supplied by the host,
3. OxVba project references and imported typelibs,
4. explicit host/runtime policy boundaries where they affect analysis.

The language-service layer must not become a second parallel compiler.

### 2.3 Current bounded truth vs future first-class target

The current bounded truth is:
1. internal service infrastructure exists,
2. some core features are implemented,
3. the surface is intentionally not yet claimed as full LSP/editor parity.

The target first-class platform is:
1. host-usable and project-aware,
2. robust across multi-module and multi-reference workspaces,
3. explicit about semantic provenance and symbol identity,
4. capable of supporting a modern editor feature ladder,
5. externally exposable through a thin protocol adapter.

## 3. Non-Goals

This workset does not by itself claim:
1. complete Roslyn parity,
2. complete rust-analyzer parity,
3. full LSP feature parity,
4. forms/designer-aware Visual Basic IDE parity,
5. proof-complete formalization,
6. closure of every executable-language parity row in the main language matrices.

It is specifically about first-class language-service platform capability.

## 4. Required Outcomes

This workset is complete only when all of the following are true:
1. the repo has an explicit capability ladder for language services beyond the current bounded internal inventory,
2. the service layer has a stable project-aware workspace contract for real OxVba projects and references,
3. semantic snapshots expose stable identity/provenance suitable for editor features beyond hover/completion,
4. the first-class feature tranche is implemented and validated for:
   - document/workspace symbols,
   - richer completion/signature/hover context,
   - semantic classification/token surface,
   - rename-preparation/reference-update safety analysis,
   - diagnostics-driven code-action foundation,
5. a thin `oxvba-lsp` or equivalent transport adapter exists on top of the direct Rust API,
6. validation rows and user/developer docs describe the resulting supported subset honestly,
7. the bounded current `LSF-0001` / `LSF-0002` truth remains distinguishable from this broader execution track.

## 5. Main Execution Lanes

### 5.1 Scope and contract lock

Before broad implementation, OxVba needs a first-class capability ladder and service contract update that answers:
1. what "first-class language services" means in OxVba,
2. which features belong in tranche A vs later tranches,
3. how the direct Rust API maps onto any later LSP surface,
4. how project manifests, references, imported typelibs, and host-supplied sources enter the service pipeline.

### 5.2 Workspace and semantic database hardening

The current workspace model is useful but still too light for a first-class editor platform.

This lane must harden:
1. project-aware loading,
2. reference-aware cross-module and cross-project analysis,
3. stable symbol identity,
4. semantic provenance,
5. invalidation and caching strategy,
6. performance measurement for interactive use.

### 5.3 Editor capability tranche A

This lane delivers the first explicit first-class editor tranche:
1. document symbols,
2. workspace symbols,
3. semantic classification / semantic-token surface,
4. richer context-aware completions,
5. richer signature help and hover detail,
6. rename preparation and safe reference-update analysis,
7. diagnostics-driven code-action foundation.

### 5.4 Transport and host embedding

Once the in-process API is strong enough, OxVba should add:
1. a thin transport adapter,
2. document/workspace synchronization behavior,
3. a host-facing sample or debug harness proving the service is consumable outside unit tests.

### 5.5 Validation, performance, and showcase

The resulting feature tranche must be validated and documented as a real capability lane, not as an implicit side effect of internal compiler work.

This lane must produce:
1. expanded validation rows,
2. multi-module and reference-aware regression coverage,
3. latency/performance evidence for interactive use,
4. a clear showcase/demo boundary and honest user-facing docs.

## 6. Phase Plan

### Phase A. Capability Ladder and Service Contract V2

1. expand `docs/spec/LANGUAGE_SERVICE_SPEC_V1.md` or publish a successor spec,
2. define the first-class capability ladder,
3. define tranche boundaries,
4. define project/workspace source contracts,
5. define the direct Rust API vs thin LSP boundary.

### Phase B. Workspace / Snapshot / Identity Hardening

1. make the workspace project-aware,
2. make it reference-aware,
3. carry imported-typelib and project provenance,
4. introduce stable symbol identity/query handles,
5. add explicit invalidation and performance harnesses.

### Phase C. Editor Capability Tranche A

1. document/workspace symbols,
2. semantic classification/token surface,
3. richer completion/signature/hover context,
4. rename preparation and safety checks,
5. code-action foundation tied to diagnostics.

### Phase D. Transport and Host Boundary

1. implement a thin LSP adapter crate or equivalent transport shell,
2. define document open/change/close and workspace synchronization behavior,
3. provide a host embedding sample or CLI debug surface.

### Phase E. Validation and Showcase Closure

1. expand the matrix coverage for the new language-service surfaces,
2. add focused regression and performance tests,
3. publish the bounded showcase and user/developer docs,
4. keep the current bounded truth and the new tranche truth separate and honest.

## 7. Architecture Constraints

The workset must preserve these constraints:
1. direct Rust API remains the primary architectural boundary,
2. LSP remains an adapter layer, not the core design driver,
3. service analysis must stay aligned with OxVba compiler/project semantics,
4. project references and imported typelibs must be handled through the real OxVba model rather than editor-only shortcuts,
5. the capability ladder must avoid overclaiming by naming the implemented tranche explicitly.

## 8. Acceptance Tests

This workset should ultimately support an honest statement like:
1. an embedded host can open an OxVba project/workspace,
2. receive diagnostics/symbols/completions/hover/signature help/go-to-definition/find-references from the direct Rust API,
3. receive document/workspace symbols, semantic classification, rename-preparation, and diagnostics-driven code actions from the same service core,
4. and optionally expose those same queries through a thin LSP adapter with no separate semantic implementation.

If the resulting platform still relies on ad hoc per-feature editor glue rather than a coherent project-aware service core, the workset is not complete.

## 9. Current First Slice

The first executable slice under this workset is:
1. lock the capability ladder and service contract update,
2. harden the workspace/source contract around real projects and references,
3. make the next editor tranche explicit in the tracker before broad code changes begin.

## 10. Bead Root

Execution for this workset proceeds through the bead subtree rooted at `bd-ls1`.

Initial epic shape:
1. `bd-ls1.2` scope and contract lock,
2. `bd-ls1.3` workspace and semantic database hardening,
3. `bd-ls1.4` editor capability tranche A,
4. `bd-ls1.5` transport and host embedding,
5. `bd-ls1.6` validation, performance, and showcase closure.

The workset is intentionally broader than the current bounded `LSF-0001` matrix row.
It is a forward execution plan for first-class language-service capability, not a rewrite of current validation truth.
