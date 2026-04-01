# COM Reference Selection Service V1

This spec defines the intended OxVba-side shape for COM reference discovery and selection.

## Objective

Provide one canonical OxVba service that:
- discovers COM/type-library candidates,
- models active project COM selections,
- supports add/list/repair flows,
- and can be consumed by both CLI and OxIde.

## Design Principles

1. Project truth is explicit.
   - Discovery is advisory.
   - `.basproj` serialization is the durable commit point.

2. Identity is richer than a display name.
   - Friendly name alone is not enough.
   - ProgID alone is not enough.
   - Canonical identity should prefer typelib/library identity:
     - GUID
     - version
     - LCID
     - import library / file path when relevant

3. Selection state is first-class.
   - The service should model both available candidates and current project-active entries.
   - Missing/unresolved references should remain explicit.

4. UI stays outside OxVba.
   - OxIde renders dialogs and search UX.
   - OxVba provides typed data and edit/apply plans.

5. CLI and OxIde consume the same service.
   - No duplicate COM helper logic.

## Candidate Inputs

The service should support discovery from:
- library/friendly name
- GUID
- ProgID
- file path

File path inputs should support:
- `.tlb`
- `.olb`
- `.dll`
- `.ocx`
- `.exe`
- `.xll` if an embedded typelib is actually present

## Candidate Identity

Illustrative canonical identity:

- `guid: Option<String>`
- `version_major: Option<u16>`
- `version_minor: Option<u16>`
- `lcid: Option<u32>`
- `library_name: String`
- `import_lib: Option<String>`
- `carrier_path: Option<PathBuf>`

Supplemental discovery fields:
- `friendly_description`
- `prog_ids`
- `source_kind`
- `resolution_confidence`

Current implementation anchor:
- `oxvba_project::com_selection`
- exported convenience surface from `oxvba_project`:
  - `ComSelectionIdentity`
  - `ComSelectionCandidate`
  - `ComProjectSelection`
  - `ComProjectSelectionStatus`
  - `ComProjectEditPlan`
  - `ComProjectEditPlanKind`
  - `HostComProjectSelectionSurface`
  - `ComSelectionService`
  - `FileBackedComSelectionQuery`
  - `RegisteredComSelectionQuery`
  - `basproj_reference_from_candidate`
  - `candidate_from_catalog_entry`
  - `candidate_from_project_reference`
  - `assess_project_com_selections`
  - `discover_file_backed_com_candidates`
  - `discover_registered_com_candidates`
  - `discover_prog_id_com_candidates`
  - `plan_add_com_candidate`
  - `plan_replace_com_reference`
  - `plan_repair_project_selection`
  - `plan_remove_com_reference`

## Project Selection State

The service should expose current project COM references as:
- active and resolved
- active but missing/unresolved
- active but ambiguously matched

This state should be comparable against discovered candidates to support:
- add
- replace
- repair
- remove

Current matching rule:
- deterministic matching prefers exact GUID/version/LCID or GUID/importlib matches
- then strong GUID/name, importlib/name, or name/version matches
- then weak name-only matches
- ambiguous candidate lists must be ordered deterministically rather than by backend enumeration order

## Output Surface

The service should be able to produce:
- candidate lists for search dialogs and CLI listing
- active selection state for the current project
- typed edit/apply plans for `.basproj` mutation

## Immediate Follow-On Implementation Lanes

1. machine discovery over registered libraries and ProgID lookup
2. file-backed discovery over `.tlb` / `.dll`-style carriers
3. project-active selection state
4. CLI command surface
5. OxIde-facing service surface

Status:
- lane 1 now exists in bounded form:
  - registered-library lookup over known identities plus Windows registry typelib discovery by friendly library name
  - ProgID lookup over the shared COM typelib identity resolver
- lane 2 now exists in bounded form:
  - explicit file-backed candidate discovery over `LoadTypeLibEx`-style importlib carriers
  - candidate carrier typing for `.tlb`, `.olb`, `.dll`, `.ocx`, `.exe`, and capability-detected `.xll`
- lane 3 now exists in bounded form:
  - active project COM selection assessment over discovered candidates
  - deterministic typed add/replace/repair/remove edit plans without implicit project mutation
- lane 5 now exists in bounded form:
  - direct OxIde-facing `ComSelectionService`
  - typed workspace/project COM state surface via `inspect_workspace_com_project_state`
- the remaining delivery slice is the CLI command surface
