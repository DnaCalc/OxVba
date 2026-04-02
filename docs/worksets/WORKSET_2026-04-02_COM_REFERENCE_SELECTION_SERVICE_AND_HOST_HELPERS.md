# Workset: COM Reference Selection Service And Host Helpers

Date: 2026-04-02
Owner: Codex
Status: complete

## Purpose

Define and execute an OxVba-side COM reference selection service that can be consumed by:
- CLI tools for adding or repairing COM references in `.basproj`,
- OxIde for a first-class reference selection dialog,
- and later any other direct-embed or extension host that needs consistent COM reference semantics.

This workset exists to turn the earlier bounded planning bead about COM reference helper UX into a concrete execution ladder with research, design, and implementation slices.

## Why This Workset Exists

OxVba already has:
- `.basproj` `COMReference` items,
- CLI reference injection flags,
- and direct host-facing project helper/session surfaces for OxIde.

But OxVba does not yet have:
- a canonical COM library discovery/selection service,
- a typed model for available COM library candidates,
- a typed model for project-active COM selections,
- or a shared helper that both CLI and OxIde can consume.

That gap now matters because OxIde has a real direct-host path and needs a proper references workflow rather than ad hoc text editing.

## External Behavior Research

The Microsoft/VBA reference workflow confirms the shape we should copy selectively:

1. The VBA References dialog is a checkbox list of available references with ordering priority.
2. Reference order matters for type resolution.
3. The dialog supports Browse for additional library-bearing files.
4. Missing references are explicit and repairable.
5. VBA exposes programmatic add-by-GUID and add-by-file patterns.

Primary source anchors:
- References dialog box:
  - https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/references-dialog-box
- Check or add an object library reference:
  - https://learn.microsoft.com/en-us/office/vba/Language/How-to/check-or-add-an-object-library-reference
- References.AddFromGUID / AddFromGuid:
  - https://learn.microsoft.com/en-us/office/vba/api/Access.References.AddFromGuid
  - https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/addfromguid-method-vba-add-in-object-model
- AddFromFile:
  - https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/addfromfile-method-vba-add-in-object-model
- Missing-reference repair behavior:
  - https://learn.microsoft.com/en-us/office/vba/Language/Reference/User-Interface-Help/can-t-find-project-or-library

Important facts from those sources:
- the References dialog lists available references with check boxes,
- the order is user-visible and semantically meaningful,
- Browse supports object libraries and executable/library files,
- standalone type libraries may be `.olb` or `.tlb`,
- type libraries may also be embedded in `.dll` or `.exe`,
- missing references are shown as missing and can be repaired.

## OxVba-Specific Policy

OxVba should not copy VBA blindly.

The governing OxVba rules should be:

1. `.basproj` remains the durable source of truth.
2. Selection/discovery helpers may infer candidate references, but they do not become project truth until serialized explicitly.
3. ProgID lookup is a convenience discovery input, not the sole canonical identity.
4. The service must expose stable typed identities for candidates and current selections.
5. CLI and OxIde should consume the same underlying service.
6. OxIde owns the user interface; OxVba owns reference semantics and selection state.

## Intended User/Host Flows

### CLI flow

Examples:
- add a COM reference by friendly name
- add a COM reference by ProgID
- list matching COM libraries
- browse/resolve a `.tlb`, `.dll`, `.ocx`, `.exe`, or `.xll` candidate into a canonical `COMReference`
- show active COM references for a project
- repair or replace a missing/unresolved COM reference deterministically

### OxIde flow

OxIde should be able to:
- query the current project’s active COM reference list,
- query available COM library candidates from the machine and from user-specified files,
- filter/search by friendly name, library name, GUID, version, and ProgID where available,
- present selection state in a dialog,
- return a typed edit/apply plan back into the project model.

## Scope Shape

The whole job is too large for one execution bead. It naturally breaks into:

1. research and boundary confirmation
2. candidate identity/data model
3. discovery backends
4. project-active selection state model
5. CLI surface
6. OxIde-facing service surface
7. edit/apply and repair flows
8. docs and validation

## Proposed Core Types

Illustrative only:

- `ComSelectionCandidate`
- `ComSelectionIdentity`
- `ComSelectionSource`
- `ComSelectionMatch`
- `ComProjectSelectionState`
- `ComSelectionQuery`
- `ComSelectionService`
- `ComSelectionEditPlan`

Likely fields:
- friendly/library name
- GUID
- major/minor version
- LCID if known
- import library path if known
- discovered file path if any
- embedded-type-library carrier kind
- candidate ProgIDs if discoverable
- current project-active / missing / resolved state

## Discovery Lanes

### A. Registered library discovery

Discover registered type libraries and related identities from the host machine.

Potential search keys:
- library/friendly name
- GUID
- version
- ProgID

### B. File-backed discovery

Resolve candidate references from:
- `.tlb`
- `.olb`
- `.dll`
- `.ocx`
- `.exe`
- `.xll` when it contains an embedded type library resource

The `.xll` lane should remain capability-detected rather than assumed.

### C. Project-active state

Track:
- current project COM references
- whether each resolves cleanly
- which discovered candidate matches each active entry
- whether repair/replacement is deterministic or ambiguous

## Non-Goals

This workset does not initially claim:
- a full Excel/VBA clone of the UI,
- automatic silent mutation of project references from inference alone,
- global machine mutation such as COM registration,
- or universal typelib extraction from every arbitrary binary format.

## Exit Condition

This workset is complete only when:
- OxVba has a typed COM selection service and project-selection state model,
- CLI can list/add/repair COM references through that service,
- OxIde has a direct service surface it can bind a dialog to,
- and the earlier blocked COM helper planning lane is fully replaced by executed slices.

Status evidence:
- typed COM selection and active-selection models landed in `oxvba-project::com_selection`
- direct host-facing `ComSelectionService` and workspace/project COM state inspection landed
- canonical `.basproj` apply helpers landed via `apply_host_project_edits_to_basproj*`
- CLI `oxvba com-ref list|add|repair` landed over the shared service
