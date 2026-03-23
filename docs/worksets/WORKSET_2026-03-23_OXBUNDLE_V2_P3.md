# WORKSET: Phase 3 — OxBundle v2

**Date:** 2026-03-23
**Phase:** 3
**Status:** Planned
**Depends on:** Phase 1 (complete)

---

## Objective

Extend the `OxBundle` serialization format to carry project metadata, export inventories, source hashes, event dispatch bindings, and dynamic object routes. Increment format version to 2. Maintain backward-compatible deserialization of v1 bundles.

---

## Deliverables

### 1. Extended `OxBundle` struct in `crates/oxvba-compiler/src/bundle.rs`

```rust
pub struct OxBundle {
    pub bytecode: Bytecode,
    pub procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    // v2 extensions (all Option<T> for backward compat):
    pub manifest_snapshot: Option<ManifestSnapshot>,
    pub export_inventory: Option<ExportInventory>,
    pub source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    pub toolchain_fingerprint: Option<ToolchainFingerprint>,
    pub event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    pub dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
}
```

### 2. New structs (all with `rkyv::Archive + Serialize + Deserialize`)

- `ManifestSnapshot { project_name, project_kind, module_names, reference_names }`
- `ExportInventory { host_exports: Vec<HostProcedureExport>, native_exports: Vec<SerializableNativeExport>, com_class_exports: Vec<ComClassExportEntry> }`
- `ToolchainFingerprint { oxvba_version: String, build_profile: String }`
- `ComClassExportEntry { class_name, prog_id, instancing, clsid, description }` — for COM server output, records which classes are externally creatable and their registration metadata

### 3. Add rkyv derives to existing types that lack them

- `HostProcedureExport`, `ExportKind`
- `ProjectEventDispatchBinding`
- `ProjectDynamicObjectRoute`, `ProjectDynamicMemberRoute`, `ProjectDynamicMemberKind`
- `ObjectHandle` in `oxvba-runtime` (check if already derived)

### 4. Format version bump: `FORMAT_VERSION = 1 → 2`

- `deserialize_from_bytes`: accept version 1 (old fields only) OR version 2 (all fields)
- v1 bundles deserialize with all new fields as `None`

### 5. `CompiledProject → OxBundle` builder helper

- `OxBundle::from_compiled_project(compiled: &CompiledProject, manifest: &ProjectManifest) → OxBundle`
- Populates all v2 sections from compiled project data

---

## Key Existing Code

- `crates/oxvba-compiler/src/bundle.rs` — Current OxBundle: `{ bytecode, procedure_metadata }`, wire format `OXVB` + version(1) + length + rkyv payload
- `crates/oxvba-compiler/src/project.rs:167-175` — `CompiledProject` has all the data the extended bundle needs
- Types needing rkyv: `HostProcedureExport` (line 128), `ExportKind` (line 40), `ProjectEventDispatchBinding` (line 136), `ProjectDynamicObjectRoute` (line 159), `ProjectDynamicMemberRoute` (line 146), `ProjectDynamicMemberKind` (line 46)

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-compiler/src/bundle.rs` | Extend OxBundle struct, add ManifestSnapshot/ExportInventory/ToolchainFingerprint, bump version, backward-compat deser |
| `crates/oxvba-compiler/src/project.rs` | Add `#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]` to HostProcedureExport, ExportKind, ProjectEventDispatchBinding, ProjectDynamic* types |
| `crates/oxvba-runtime/src/lib.rs` | Verify ObjectHandle has rkyv derives (add if missing) |
| `crates/oxvba-compiler/src/lib.rs` | Re-export new bundle types |
| `crates/oxvba-compiler/tests/bundle_v2_tests.rs` (new) | v1 backward-compat, v2 round-trip, from_compiled_project |

---

## Execution Steps

1. Add rkyv derives to the six types in `project.rs` that lack them
2. Define `ManifestSnapshot`, `ExportInventory`, `ToolchainFingerprint`, `ComClassExportEntry` in `bundle.rs` with rkyv derives
3. Extend `OxBundle` with `Option<T>` fields
4. Implement dual-version deserialization: read version from header, deserialize v1 payload into extended struct with None fields, deserialize v2 payload normally
5. Add `OxBundle::from_compiled_project` builder
6. Write tests: serialize v2 → deserialize v2 round-trip; manually construct v1-format bytes → deserialize → verify new fields are None; from_compiled_project populates all sections

---

## Closure Conditions

1. v1 bundles produced by existing code still deserialize successfully (backward compat)
2. v2 bundles carry all six new sections
3. `OxBundle::from_compiled_project` populates ManifestSnapshot, ExportInventory, source hashes, event bindings, dynamic routes from `CompiledProject`
4. Existing 2017+ tests still pass (no regressions from rkyv derive additions)
