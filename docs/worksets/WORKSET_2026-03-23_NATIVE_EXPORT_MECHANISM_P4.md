# WORKSET: Phase 4 — Native Export Mechanism

**Date:** 2026-03-23
**Phase:** 4
**Status:** Planned
**Depends on:** Phase 1 (complete)

---

## Objective

Integrate `NativeExportDescriptor` into the compiler pipeline. Validate declared exports against compiled procedure signatures. Derive `DeclareParamType` vectors from procedure `BoundParam`/`BoundType` for each export. Additionally, validate COM server class exports: for `OutputType=ComServer`, enumerate creatable class modules and build `ComClassExportDescriptor` entries with IDispatch member inventories.

---

## Deliverables

### 1. Extended `NativeExportDescriptor` in `crates/oxvba-project/src/model.rs`

Add type-info fields populated post-compilation:

```rust
pub struct NativeExportDescriptor {
    pub exported_name: String,
    pub module_name: String,
    pub procedure_name: String,
    pub calling_convention: CallingConvention,
    pub ordinal: Option<u16>,
    // Populated post-compilation:
    pub kind: Option<ExportKind>,                   // Sub | Function
    pub param_types: Option<Vec<DeclareParamType>>,
    pub return_type: Option<Option<DeclareParamType>>,
}
```

### 2. `validate_native_exports` function in `crates/oxvba-project/src/validate.rs` (new file)

- Input: `Vec<NativeExportDescriptor>` + `CompiledProject`
- For each export:
  a. Find `Module.Procedure` in `compiled.host_exports` — error if missing
  b. Verify module is `Procedural` (not class) — reuse `PMR-E-HOST-EXPORT-MODULE-KIND` pattern
  c. Verify procedure is `Public`
  d. Look up `ProcedureRuntimeMetadata` by lowered key `"module.procedure"`
  e. Derive `param_types` and `return_type` using `bound_type_to_declare_param_type` mapping
- Return enriched `Vec<NativeExportDescriptor>` with type info populated

### 3. Expose `bound_type_to_declare_param_type`

Currently `fn` (private) in `crates/oxvba-compiler/src/emit.rs:1730`. Either:
- Make it `pub` and re-export from `oxvba-compiler`, OR
- Create a parallel mapping in `oxvba-project` that maps procedure metadata to DeclareParamType (simpler: avoids coupling to internal BoundType)

### 4. COM class export validation for `OutputType=ComServer`

- `validate_com_class_exports(manifest, compiled) → Result<Vec<ComClassExportDescriptor>, BasProjError>`
- For each class module with `vb_exposed=true` and `vb_creatable=true`:
  a. Verify `Instancing` is set (default to `MultiUse` if omitted)
  b. Generate a deterministic CLSID from project name + class name (or accept explicit CLSID from `.basproj`)
  c. Build ProgId: `<ProjectName>.<ClassName>` (or explicit from `.basproj`)
  d. Collect public member inventory from `ProjectDynamicObjectRoute` for IDispatch generation

```rust
pub struct ComClassExportDescriptor {
    pub class_name: String,
    pub prog_id: String,
    pub clsid: String,        // GUID string
    pub instancing: Instancing,
    pub description: Option<String>,
    pub dispatch_members: Vec<DispatchMemberInfo>,  // name, dispid, kind, param types, return type
}
```

### 5. Validation error types

- `ExportProcedureNotFound { exported_name, module_name, procedure_name }`
- `ExportModuleNotProcedural { exported_name, module_name }`
- `ExportProcedureNotPublic { exported_name, module_name, procedure_name }`
- `ComClassNotExposed { class_name }` — class lacks `VBExposed=True`
- `ComServerNoCreatableClasses` — `OutputType=ComServer` with no creatable classes

---

## Key Existing Code

- `crates/oxvba-compiler/src/emit.rs:1730-1747` — `bound_type_to_declare_param_type` maps 15 BoundType → 13 DeclareParamType
- `crates/oxvba-compiler/src/project.rs:5825-5866` — `collect_host_exports` filters Public + Procedural modules
- `crates/oxvba-compiler/src/bytecode.rs:11-25` — `DeclareParamType` enum (already has rkyv derives)
- `crates/oxvba-compiler/src/resolve.rs:279-286` — `BoundParam { name, by_ref, param_array, optional, default_value, ty: BoundType }`

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-project/src/validate.rs` (new) | `validate_native_exports`, `validate_com_class_exports` against CompiledProject |
| `crates/oxvba-project/src/model.rs` | Add type-info fields to NativeExportDescriptor, add `ComClassExportDescriptor` |
| `crates/oxvba-project/src/lib.rs` | Add `pub mod validate;` |
| `crates/oxvba-compiler/src/emit.rs` | Make `bound_type_to_declare_param_type` pub |
| `crates/oxvba-compiler/src/lib.rs` | Re-export `bound_type_to_declare_param_type` |
| `crates/oxvba-project/tests/validate_tests.rs` (new) | Export validation tests |

---

## Execution Steps

1. Make `bound_type_to_declare_param_type` public in emit.rs and re-export from oxvba-compiler
2. Add `kind`, `param_types`, `return_type` fields (all `Option`) to `NativeExportDescriptor`
3. Create `validate.rs` with `validate_native_exports(exports, compiled, manifest) → Result<Vec<NativeExportDescriptor>, BasProjError>`
4. Implement validation: existence check via host_exports, module kind check, type derivation
5. Implement `validate_com_class_exports` for COM server class enumeration and DispatchMemberInfo collection
6. Write tests: valid export resolves with correct types, missing procedure errors, wrong module kind errors, COM class validation

---

## Closure Conditions

1. `validate_native_exports` enriches descriptors with `ExportKind`, param types, return type from compiled procedures
2. Missing/invalid exports produce clear diagnostics
3. Type derivation matches existing `bound_type_to_declare_param_type` behavior
4. `validate_com_class_exports` produces `ComClassExportDescriptor` entries with member inventories
5. COM server with no creatable classes errors with `ComServerNoCreatableClasses`
6. No regressions
