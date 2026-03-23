# WORKSET: Phase 2 — Reference Resolution

**Date:** 2026-03-23
**Phase:** 2
**Status:** Planned
**Depends on:** Phase 1 (complete)

---

## Objective

Wire up `<ProjectReference>`, `<COMReference>`, and `<NativeReference>` resolution in `oxvba-project`, producing fully-populated `ProjectManifest.reference_projects` and integrated `TypeLibraryCatalogEntry` binding. Add cycle detection for recursive project references.

---

## Phase 1 Addendum: COM Server Project Model Extensions

Before proceeding to reference resolution, the existing `oxvba-project` crate needs minor additions to support COM server output. These are small enough to fold into Phase 2 work.

**Additions to `crates/oxvba-project/src/model.rs`:**
- Add `OutputType::ComServer` — in-process COM server DLL (ActiveX DLL in VB6 terms)
- Add `Instancing` enum to `BasProjModule` for class modules:

```rust
pub enum Instancing {
    Private,              // Not externally creatable
    PublicNotCreatable,   // Accessible but only created internally
    MultiUse,             // External creation via CoCreateInstance (default for COM server)
    GlobalMultiUse,       // Like MultiUse + members accessible without explicit instance
    SingleUse,            // One instance per process
    GlobalSingleUse,      // Like SingleUse + global access
}
```

**`.basproj` XML for COM server class:**
```xml
<ClassModule Include="Calculator.cls">
  <VBExposed>True</VBExposed>
  <VBCreatable>True</VBCreatable>
  <Instancing>MultiUse</Instancing>
  <ProgId>MyLib.Calculator</ProgId>
  <Description>Calculator COM class</Description>
</ClassModule>
```

**Additions to `crates/oxvba-project/src/parse.rs`:** Parse `<Instancing>` and `<ProgId>` metadata on ClassModule items.

**Additions to `BASPROJ_SPEC_V1.md`:** Document `OutputType=ComServer`, `Instancing` metadata, `ProgId` metadata.

**VB6 context:** VB6 `Type=OleDll` projects exposed class modules as COM coclasses. Each class had an `Instancing` property (set in the VB6 IDE Properties window) that controlled whether external clients could create instances. The project produced a DLL with `DllGetClassObject`/`DllCanUnloadNow`/`DllRegisterServer`/`DllUnregisterServer` exports, an embedded type library, and an `IClassFactory` per creatable class. Each class implemented `IDispatch` for late-bound access. The runtime managed STA threading, reference counting, and teardown.

---

## Deliverables

### 1. `resolve_project_references` in `crates/oxvba-project/src/resolve.rs` (new file)

- Recursively loads referenced `.basproj` files via `load_basproj`
- Populates `ProjectManifest.reference_projects: Vec<ReferencedProjectManifest>` with public module units from each referenced project
- Cycle detection via path-set accumulator (`HashSet<PathBuf>`) — returns `BasProjError::CyclicProjectReference` on revisit
- Precedence index assignment: declaration order in `.basproj` (top-to-bottom across `<ItemGroup>` elements)

### 2. COMReference → TypeLibraryCatalogEntry integration

- `LoadedProject.type_library_catalog` (already populated in Phase 1) feeds into `ProjectGraph.resolve_type_library_references()` at the host layer
- Add `resolve_com_references` helper that bridges `BasProjComReference` → host `ProjectReference` with `importlib_hint`, `libid_hint`, version hints, `lcid_hint`
- Cross-platform behavior: on non-Windows, COM references produce `ReferenceBindingState::Failed` unless portable metadata blobs are provided

### 3. NativeReference path resolution

- Validate `<NativeReference>` path exists relative to project directory
- Store resolved paths for `ExternalCallDescriptor.library` feed-in during compilation

### 4. Error types in `crates/oxvba-project/src/error.rs`

- `CyclicProjectReference { path: String, cycle: Vec<String> }`
- `ProjectReferenceNotFound { include: String }`
- `NativeReferenceNotFound { include: String, resolved_path: String }`

---

## Key Existing Code

- `oxvba-compiler/src/project.rs:106-115` — `ProjectReference` (simple: name + kind) and `ReferencedProjectManifest` (name + modules)
- `oxvba-host/src/project.rs:162-172` — Host-layer `ProjectReference` (richer: precedence_index, binding_state, importlib/libid hints)
- `oxvba-host/src/project.rs:527-651` — `ProjectNode::resolve_type_library_references()` — existing binding resolution using catalog
- `oxvba-project/src/load.rs:169` — `reference_projects: Vec::new()` — the gap this phase fills

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-project/src/resolve.rs` (new) | `resolve_project_references`, `resolve_com_references`, cycle detection |
| `crates/oxvba-project/src/load.rs` | Call `resolve_project_references` from `load_basproj`, populate `reference_projects` |
| `crates/oxvba-project/src/error.rs` | Add cyclic/not-found error variants |
| `crates/oxvba-project/src/model.rs` | Add `OutputType::ComServer`, `Instancing` enum, `ProgId` field |
| `crates/oxvba-project/src/parse.rs` | Parse `<Instancing>` and `<ProgId>` metadata |
| `crates/oxvba-project/src/lib.rs` | Add `pub mod resolve;` and re-exports |
| `crates/oxvba-project/tests/resolve_tests.rs` (new) | Multi-project fixture tests |
| `docs/spec/BASPROJ_SPEC_V1.md` | Document `OutputType=ComServer`, `Instancing`, `ProgId` |

---

## Execution Steps

1. Add `OutputType::ComServer`, `Instancing` enum, and `ProgId` field to `model.rs`
2. Update `parse.rs` to parse `<Instancing>` and `<ProgId>` metadata on ClassModule items
3. Update `BASPROJ_SPEC_V1.md` with COM server documentation
4. Create `resolve.rs` with `resolve_project_references(basproj: &BasProj, project_dir: &Path, visited: &mut HashSet<PathBuf>) → Result<Vec<ReferencedProjectManifest>, BasProjError>`
5. For each `BasProjProjectReference`, resolve the include path, check cycle set, recursively `load_basproj`, extract public modules into `ReferencedProjectManifest`
6. Create `resolve_com_references` that maps `Vec<BasProjComReference>` → enriched `Vec<ProjectReference>` with binding hints
7. Wire into `build_loaded_project` — call resolve when `project_dir` is available
8. Write tests: two-project chain (A→B), three-project diamond (A→B, A→C, B→C), cycle detection (A→B→A), missing reference

---

## Closure Conditions

1. `ProjectManifest.reference_projects` populated with modules from referenced projects
2. Cycle detection rejects A→B→A with clear diagnostic
3. COMReference entries produce `TypeLibraryCatalogEntry` values that can be fed to existing `resolve_type_library_references`
4. All 21 existing Phase 1 tests still pass
5. New resolve tests pass
6. `OutputType::ComServer` variant added with `Instancing` and `ProgId` metadata parsing
