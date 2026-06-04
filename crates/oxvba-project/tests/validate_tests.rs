//! Integration tests for `oxvba_project::validate` module.

use std::collections::BTreeMap;

use oxvba_compiler::{
    Bytecode, CompiledProject, DeclareParamType, ExportKind, HostProcedureExport,
    ProcedureKindDescriptor, ProcedureRuntimeMetadata, ProcedureRuntimeSlotMetadata,
    ProcedureSignatureDescriptor, ProjectCompileRoute, ProjectDynamicMemberKind,
    ProjectDynamicMemberRoute, ProjectDynamicObjectRoute, ProjectDynamicParamRoute, VbaTypeId,
};
use oxvba_project::validate::{validate_com_class_exports, validate_native_exports};
use oxvba_project::{
    BasProjModule, BasProjModuleKind, CallingConvention, ClassModuleMetadata, Instancing,
    NativeExportDescriptor,
};
/// Helper: build a minimal `CompiledProject` with the given host_exports,
/// procedure_runtime_metadata, and project_dynamic_objects.
fn make_compiled_project(
    host_exports: Vec<HostProcedureExport>,
    metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    dynamic_objects: Vec<ProjectDynamicObjectRoute>,
) -> CompiledProject {
    CompiledProject {
        bytecode: Bytecode {
            instructions: Vec::new(),
            external_call_descriptors: Vec::new(),
            slot_count: 0,
            user_slot_count: 0,
        },
        procedure_runtime_metadata: metadata,
        source_maps: oxvba_compiler::CompilerSourceMap::default(),
        rewritten_source: String::new(),
        compile_route: ProjectCompileRoute::LegacyFallback,
        compile_route_detail: None,
        host_exports,
        reference_visible_exports: Vec::new(),
        event_dispatch_bindings: Vec::new(),
        project_com_withevents_routes: Vec::new(),
        project_dynamic_objects: dynamic_objects,
        project_reflection: oxvba_compiler::ProjectReflection {
            identity: oxvba_compiler::ProjectIdentity {
                project_name: String::new(),
                project_id: String::new(),
                source_fingerprint: String::new(),
            },
            modules: Vec::new(),
            procedures: Vec::new(),
            capabilities: Vec::new(),
        },
    }
}

/// Helper: build a `NativeExportDescriptor` with only the fields required for
/// lookup; the `kind`, `param_types`, and `return_type` fields start as `None`.
fn make_native_export(
    exported_name: &str,
    module_name: &str,
    procedure_name: &str,
) -> NativeExportDescriptor {
    NativeExportDescriptor {
        exported_name: exported_name.to_string(),
        module_name: module_name.to_string(),
        procedure_name: procedure_name.to_string(),
        calling_convention: CallingConvention::Stdcall,
        ordinal: None,
        kind: None,
        param_types: None,
        return_type: None,
        category: None,
        description: None,
        argument_descriptions: None,
    }
}

fn make_runtime_metadata(
    module_name: &str,
    procedure_name: &str,
    entry_pc: usize,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
) -> ProcedureRuntimeMetadata {
    let param_count = param_slots.len();
    let has_return = return_slot.is_some();
    ProcedureRuntimeMetadata {
        module_name: module_name.to_string(),
        procedure_name: procedure_name.to_string(),
        entry_pc,
        source_line_start: 1,
        source_line_end: 1,
        statement_line_numbers: vec![1],
        statement_entry_pcs: vec![entry_pc],
        slots: Vec::<ProcedureRuntimeSlotMetadata>::new(),
        param_slots,
        return_slot,
        param_types: vec![DeclareParamType::Variant; param_count],
        return_type: has_return.then_some(DeclareParamType::Variant),
        signature: ProcedureSignatureDescriptor {
            procedure_name: procedure_name.to_string(),
            kind: if has_return {
                ProcedureKindDescriptor::Function
            } else {
                ProcedureKindDescriptor::Sub
            },
            parameters: Vec::new(),
            return_type: has_return.then_some(VbaTypeId::Variant),
            return_slot,
            property_group: None,
            implicit_current_object: None,
        },
        call_sites: Vec::new(),
        array_shapes: Vec::new(),
        udt_types: Vec::new(),
        object_types: Vec::new(),
        carrier_layouts: Vec::new(),
        value_states: Vec::new(),
        expression_semantics: Vec::new(),
        operator_semantics: Vec::new(),
        coercions: Vec::new(),
        name_bindings: Vec::new(),
        object_member_bindings: Vec::new(),
    }
}

/// Helper: build a `BasProjModule` for a class module with the given
/// `vb_exposed` and `vb_creatable` flags.
fn make_class_module(include: &str, vb_exposed: bool, vb_creatable: bool) -> BasProjModule {
    BasProjModule {
        kind: BasProjModuleKind::ClassModule,
        include: include.to_string(),
        vb_predeclared_id: false,
        vb_exposed,
        vb_global_namespace: false,
        vb_creatable,
        host_document_type: None,
        instancing: None,
        prog_id: None,
        description: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Valid native export resolves correctly
// ---------------------------------------------------------------------------

#[test]
fn valid_native_export_resolves_kind_and_params() {
    let host_exports = vec![HostProcedureExport {
        project_name: "TestProject".to_string(),
        module_name: "MathUtils".to_string(),
        procedure_name: "AddTwo".to_string(),
        kind: ExportKind::Function,
    }];

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "mathutils.addtwo".to_string(),
        make_runtime_metadata("MathUtils", "AddTwo", 0, vec![1, 2], Some(3)),
    );

    let compiled = make_compiled_project(host_exports, metadata, Vec::new());
    let mut exports = vec![make_native_export("AddTwo", "MathUtils", "AddTwo")];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(result.is_ok(), "validate_native_exports should succeed");

    let export = &exports[0];
    assert_eq!(export.kind, Some(ExportKind::Function));
    assert_eq!(
        export.param_types,
        Some(vec![DeclareParamType::Variant, DeclareParamType::Variant]),
        "each param_slot should map to DeclareParamType::Variant"
    );
    assert_eq!(
        export.return_type,
        Some(Some(DeclareParamType::Variant)),
        "return_slot.is_some() should produce Some(Variant)"
    );
}

#[test]
fn valid_native_export_sub_with_no_return_slot() {
    let host_exports = vec![HostProcedureExport {
        project_name: "TestProject".to_string(),
        module_name: "Helpers".to_string(),
        procedure_name: "DoWork".to_string(),
        kind: ExportKind::Sub,
    }];

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "helpers.dowork".to_string(),
        make_runtime_metadata("Helpers", "DoWork", 0, vec![1], None),
    );

    let compiled = make_compiled_project(host_exports, metadata, Vec::new());
    let mut exports = vec![make_native_export("DoWork", "Helpers", "DoWork")];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(result.is_ok());

    let export = &exports[0];
    assert_eq!(export.kind, Some(ExportKind::Sub));
    assert_eq!(
        export.param_types,
        Some(vec![DeclareParamType::Variant]),
        "single param_slot should produce a single Variant param type"
    );
    assert_eq!(
        export.return_type,
        Some(None),
        "return_slot == None should produce Some(None)"
    );
}

#[test]
fn native_export_uses_decorated_runtime_metadata_types() {
    let host_exports = vec![HostProcedureExport {
        project_name: "ScalarAddin".to_string(),
        module_name: "ScalarExports".to_string(),
        procedure_name: "AddDouble".to_string(),
        kind: ExportKind::Function,
    }];

    let mut typed_metadata =
        make_runtime_metadata("ScalarExports", "AddDouble", 0, vec![1, 2], Some(3));
    typed_metadata.param_types = vec![DeclareParamType::Double, DeclareParamType::Double];
    typed_metadata.return_type = Some(DeclareParamType::Double);

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "pmr_scalaraddin_scalarexports_adddouble".to_string(),
        typed_metadata,
    );

    let compiled = make_compiled_project(host_exports, metadata, Vec::new());
    let mut exports = vec![make_native_export(
        "AddDouble",
        "ScalarExports",
        "AddDouble",
    )];

    validate_native_exports(&mut exports, &compiled).expect("native export validation");

    assert_eq!(
        exports[0].param_types,
        Some(vec![DeclareParamType::Double, DeclareParamType::Double])
    );
    assert_eq!(exports[0].return_type, Some(Some(DeclareParamType::Double)));
}

// ---------------------------------------------------------------------------
// 2. Missing procedure produces ExportProcedureNotFound
// ---------------------------------------------------------------------------

#[test]
fn missing_procedure_returns_export_procedure_not_found() {
    // Compiled project has no host_exports at all.
    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());
    let mut exports = vec![make_native_export(
        "MissingExport",
        "SomeModule",
        "SomeProc",
    )];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(result.is_err(), "should fail when procedure is missing");

    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("MissingExport"),
        "error should mention the exported_name"
    );
    assert!(
        msg.contains("SomeModule"),
        "error should mention the module_name"
    );
    assert!(
        msg.contains("SomeProc"),
        "error should mention the procedure_name"
    );
}

#[test]
fn wrong_module_name_returns_export_procedure_not_found() {
    // Host export exists but with a different module name.
    let host_exports = vec![HostProcedureExport {
        project_name: "TestProject".to_string(),
        module_name: "ModuleA".to_string(),
        procedure_name: "Run".to_string(),
        kind: ExportKind::Sub,
    }];

    let compiled = make_compiled_project(host_exports, BTreeMap::new(), Vec::new());
    let mut exports = vec![make_native_export("Run", "ModuleB", "Run")];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(
        result.is_err(),
        "should fail when module name does not match"
    );
}

#[test]
fn wrong_procedure_name_returns_export_procedure_not_found() {
    // Host export exists but with a different procedure name.
    let host_exports = vec![HostProcedureExport {
        project_name: "TestProject".to_string(),
        module_name: "ModuleA".to_string(),
        procedure_name: "Foo".to_string(),
        kind: ExportKind::Function,
    }];

    let compiled = make_compiled_project(host_exports, BTreeMap::new(), Vec::new());
    let mut exports = vec![make_native_export("Bar", "ModuleA", "Bar")];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(
        result.is_err(),
        "should fail when procedure name does not match"
    );
}

// ---------------------------------------------------------------------------
// 3. COM class with vb_exposed + vb_creatable is extracted
// ---------------------------------------------------------------------------

#[test]
fn com_class_exported_when_exposed_and_creatable() {
    let modules = vec![make_class_module("src/MyClass.cls", true, true)];

    let dynamic_objects = vec![ProjectDynamicObjectRoute {
        object_handle: 42,
        project_name: "TestProject".to_string(),
        module_name: "MyClass".to_string(),
        members: vec![ProjectDynamicMemberRoute {
            member_name: "Calculate".to_string(),
            lowered_name: "calculate".to_string(),
            known_dispatch_token: None,
            dispatch_id: None,
            member_flags: None,
            is_default_member: false,
            kind: ProjectDynamicMemberKind::Function,
            visible_param_count: 2,
            params: vec![
                ProjectDynamicParamRoute {
                    name: "lhs".to_string(),
                    optional: false,
                    param_array: false,
                    default_value: None,
                },
                ProjectDynamicParamRoute {
                    name: "rhs".to_string(),
                    optional: false,
                    param_array: false,
                    default_value: None,
                },
            ],
            param_types: vec![DeclareParamType::Double, DeclareParamType::Double],
            return_type: Some(DeclareParamType::Double),
            entry_pc: 0,
            param_slots: vec![1, 2],
            return_slot: Some(3),
        }],
        field_tokens: Vec::new(),
        field_arrays: Vec::new(),
        implements_interfaces: Vec::new(),
        class_terminate: None,
    }];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), dynamic_objects);
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert_eq!(descriptors.len(), 1, "should produce exactly one COM class");
    assert_eq!(descriptors[0].class_name, "MyClass");
    assert_eq!(descriptors[0].members.len(), 1);
    assert_eq!(descriptors[0].members[0].member_name, "Calculate");
    assert_eq!(
        descriptors[0].members[0].kind,
        ProjectDynamicMemberKind::Function
    );
    assert_eq!(descriptors[0].members[0].param_count, 2);
    assert_eq!(
        descriptors[0].members[0].param_types,
        vec![DeclareParamType::Double, DeclareParamType::Double]
    );
    assert_eq!(
        descriptors[0].members[0].return_type,
        Some(DeclareParamType::Double)
    );
}

#[test]
fn com_class_carries_prog_id_from_module() {
    let mut module = make_class_module("src/Widget.cls", true, true);
    module.prog_id = Some("TestLib.Widget".to_string());
    module.instancing = Some(Instancing::MultiUse);
    module.description = Some("A widget class".to_string());

    let modules = vec![module];
    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].prog_id, Some("TestLib.Widget".to_string()));
    assert_eq!(descriptors[0].instancing, Some(Instancing::MultiUse));
    assert_eq!(
        descriptors[0].description,
        Some("A widget class".to_string())
    );
}

#[test]
fn com_class_metadata_overrides_module_fields() {
    let mut module = make_class_module("src/Gadget.cls", true, true);
    module.prog_id = Some("Old.ProgId".to_string());
    module.instancing = Some(Instancing::Private);
    module.description = Some("old description".to_string());

    let modules = vec![module];
    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());

    let mut class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();
    class_metadata.insert(
        "Gadget".to_string(),
        ClassModuleMetadata {
            instancing: Some(Instancing::MultiUse),
            prog_id: Some("New.ProgId".to_string()),
            description: Some("new description".to_string()),
        },
    );

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(
        descriptors[0].prog_id,
        Some("New.ProgId".to_string()),
        "class_metadata prog_id should take precedence over module prog_id"
    );
    assert_eq!(
        descriptors[0].instancing,
        Some(Instancing::MultiUse),
        "class_metadata instancing should take precedence"
    );
    assert_eq!(
        descriptors[0].description,
        Some("new description".to_string()),
        "class_metadata description should take precedence"
    );
}

#[test]
fn com_class_with_multiple_members() {
    let modules = vec![make_class_module("src/MultiMember.cls", true, true)];

    let dynamic_objects = vec![ProjectDynamicObjectRoute {
        object_handle: 10,
        project_name: "TestProject".to_string(),
        module_name: "MultiMember".to_string(),
        members: vec![
            ProjectDynamicMemberRoute {
                member_name: "Name".to_string(),
                lowered_name: "name".to_string(),
                known_dispatch_token: None,
                dispatch_id: None,
                member_flags: None,
                is_default_member: false,
                kind: ProjectDynamicMemberKind::PropertyGet,
                visible_param_count: 0,
                params: Vec::new(),
                param_types: Vec::new(),
                return_type: None,
                entry_pc: 0,
                param_slots: Vec::new(),
                return_slot: Some(0),
            },
            ProjectDynamicMemberRoute {
                member_name: "Name".to_string(),
                lowered_name: "name".to_string(),
                known_dispatch_token: None,
                dispatch_id: None,
                member_flags: None,
                is_default_member: false,
                kind: ProjectDynamicMemberKind::PropertyLet,
                visible_param_count: 1,
                params: vec![ProjectDynamicParamRoute {
                    name: "value".to_string(),
                    optional: false,
                    param_array: false,
                    default_value: None,
                }],
                param_types: vec![DeclareParamType::Variant],
                return_type: None,
                entry_pc: 10,
                param_slots: vec![1],
                return_slot: None,
            },
            ProjectDynamicMemberRoute {
                member_name: "DoSomething".to_string(),
                lowered_name: "dosomething".to_string(),
                known_dispatch_token: None,
                dispatch_id: None,
                member_flags: None,
                is_default_member: false,
                kind: ProjectDynamicMemberKind::Method,
                visible_param_count: 0,
                params: Vec::new(),
                param_types: Vec::new(),
                return_type: None,
                entry_pc: 20,
                param_slots: Vec::new(),
                return_slot: None,
            },
        ],
        field_tokens: Vec::new(),
        field_arrays: Vec::new(),
        implements_interfaces: Vec::new(),
        class_terminate: None,
    }];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), dynamic_objects);
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(
        descriptors[0].members.len(),
        3,
        "all three member routes should appear as dispatch members"
    );
}

// ---------------------------------------------------------------------------
// 4. Class without vb_exposed is skipped
// ---------------------------------------------------------------------------

#[test]
fn class_without_vb_exposed_is_skipped() {
    let modules = vec![make_class_module("src/Internal.cls", false, true)];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert!(
        descriptors.is_empty(),
        "class with vb_exposed=false should not appear in results"
    );
}

#[test]
fn class_without_vb_creatable_is_skipped() {
    let modules = vec![make_class_module("src/NonCreatable.cls", true, false)];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert!(
        descriptors.is_empty(),
        "class with vb_creatable=false should not appear in results"
    );
}

#[test]
fn procedural_module_is_skipped_for_com_class_export() {
    let modules = vec![BasProjModule {
        kind: BasProjModuleKind::Module,
        include: "src/Helpers.bas".to_string(),
        vb_predeclared_id: false,
        vb_exposed: true,
        vb_global_namespace: false,
        vb_creatable: true,
        host_document_type: None,
        instancing: None,
        prog_id: None,
        description: None,
    }];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert!(
        descriptors.is_empty(),
        "procedural modules should never appear as COM class exports"
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn case_insensitive_module_and_procedure_lookup() {
    // Host export uses mixed case; the native export uses different casing.
    let host_exports = vec![HostProcedureExport {
        project_name: "TestProject".to_string(),
        module_name: "MathUtils".to_string(),
        procedure_name: "AddTwo".to_string(),
        kind: ExportKind::Function,
    }];

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "mathutils.addtwo".to_string(),
        make_runtime_metadata("MathUtils", "AddTwo", 0, vec![1, 2, 3], Some(4)),
    );

    let compiled = make_compiled_project(host_exports, metadata, Vec::new());
    let mut exports = vec![make_native_export("AddTwo", "MATHUTILS", "ADDTWO")];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(
        result.is_ok(),
        "case-insensitive lookup should find the host export"
    );
    assert_eq!(exports[0].kind, Some(ExportKind::Function));
    assert_eq!(
        exports[0].param_types,
        Some(vec![
            DeclareParamType::Variant,
            DeclareParamType::Variant,
            DeclareParamType::Variant,
        ])
    );
}

#[test]
fn native_export_without_metadata_still_sets_kind() {
    // Host export exists but no corresponding procedure_runtime_metadata entry.
    let host_exports = vec![HostProcedureExport {
        project_name: "TestProject".to_string(),
        module_name: "Mod1".to_string(),
        procedure_name: "Init".to_string(),
        kind: ExportKind::Sub,
    }];

    let compiled = make_compiled_project(host_exports, BTreeMap::new(), Vec::new());
    let mut exports = vec![make_native_export("Init", "Mod1", "Init")];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(
        result.is_ok(),
        "should succeed even without runtime metadata"
    );

    let export = &exports[0];
    assert_eq!(
        export.kind,
        Some(ExportKind::Sub),
        "kind should still be populated from host export"
    );
    assert_eq!(
        export.param_types, None,
        "param_types should remain None without metadata"
    );
    assert_eq!(
        export.return_type, None,
        "return_type should remain None without metadata"
    );
}

#[test]
fn multiple_native_exports_all_resolve() {
    let host_exports = vec![
        HostProcedureExport {
            project_name: "P".to_string(),
            module_name: "M".to_string(),
            procedure_name: "A".to_string(),
            kind: ExportKind::Function,
        },
        HostProcedureExport {
            project_name: "P".to_string(),
            module_name: "M".to_string(),
            procedure_name: "B".to_string(),
            kind: ExportKind::Sub,
        },
    ];

    let mut metadata = BTreeMap::new();
    metadata.insert(
        "m.a".to_string(),
        make_runtime_metadata("M", "A", 0, vec![1], Some(2)),
    );
    metadata.insert(
        "m.b".to_string(),
        make_runtime_metadata("M", "B", 10, Vec::new(), None),
    );

    let compiled = make_compiled_project(host_exports, metadata, Vec::new());
    let mut exports = vec![
        make_native_export("A", "M", "A"),
        make_native_export("B", "M", "B"),
    ];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(result.is_ok());

    assert_eq!(exports[0].kind, Some(ExportKind::Function));
    assert_eq!(
        exports[0].param_types,
        Some(vec![DeclareParamType::Variant])
    );
    assert_eq!(
        exports[0].return_type,
        Some(Some(DeclareParamType::Variant))
    );

    assert_eq!(exports[1].kind, Some(ExportKind::Sub));
    assert_eq!(exports[1].param_types, Some(Vec::new()));
    assert_eq!(exports[1].return_type, Some(None));
}

#[test]
fn first_missing_export_fails_entire_batch() {
    // Second export is valid, but first is missing — the whole call should fail.
    let host_exports = vec![HostProcedureExport {
        project_name: "P".to_string(),
        module_name: "M".to_string(),
        procedure_name: "Exists".to_string(),
        kind: ExportKind::Sub,
    }];

    let compiled = make_compiled_project(host_exports, BTreeMap::new(), Vec::new());
    let mut exports = vec![
        make_native_export("Missing", "M", "Missing"),
        make_native_export("Exists", "M", "Exists"),
    ];

    let result = validate_native_exports(&mut exports, &compiled);
    assert!(
        result.is_err(),
        "should fail when any export procedure is not found"
    );
}

#[test]
fn com_class_with_no_dynamic_route_has_empty_members() {
    // Class passes the vb_exposed + vb_creatable gate but has no matching
    // ProjectDynamicObjectRoute — members list should be empty, not an error.
    let modules = vec![make_class_module("src/EmptyClass.cls", true, true)];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), Vec::new());
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let result = validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject");
    assert!(result.is_ok());

    let descriptors = result.unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].class_name, "EmptyClass");
    assert!(
        descriptors[0].members.is_empty(),
        "class with no dynamic route should have empty members list"
    );
}

#[test]
fn com_class_export_preserves_dispatch_metadata() {
    let modules = vec![make_class_module("src/Widget.cls", true, true)];

    let dynamic_objects = vec![ProjectDynamicObjectRoute {
        object_handle: 11,
        project_name: "TestProject".to_string(),
        module_name: "Widget".to_string(),
        members: vec![
            ProjectDynamicMemberRoute {
                member_name: "Value".to_string(),
                lowered_name: "value".to_string(),
                known_dispatch_token: None,
                dispatch_id: Some(0),
                member_flags: None,
                is_default_member: true,
                kind: ProjectDynamicMemberKind::PropertyGet,
                visible_param_count: 0,
                params: Vec::new(),
                param_types: Vec::new(),
                return_type: None,
                entry_pc: 0,
                param_slots: Vec::new(),
                return_slot: Some(0),
            },
            ProjectDynamicMemberRoute {
                member_name: "NewEnum".to_string(),
                lowered_name: "newenum".to_string(),
                known_dispatch_token: None,
                dispatch_id: Some(-4),
                member_flags: Some(0x40),
                is_default_member: false,
                kind: ProjectDynamicMemberKind::PropertyGet,
                visible_param_count: 0,
                params: Vec::new(),
                param_types: Vec::new(),
                return_type: None,
                entry_pc: 1,
                param_slots: Vec::new(),
                return_slot: Some(0),
            },
        ],
        field_tokens: Vec::new(),
        field_arrays: Vec::new(),
        implements_interfaces: Vec::new(),
        class_terminate: None,
    }];

    let compiled = make_compiled_project(Vec::new(), BTreeMap::new(), dynamic_objects);
    let class_metadata: BTreeMap<String, ClassModuleMetadata> = BTreeMap::new();

    let descriptors =
        validate_com_class_exports(&modules, &compiled, &class_metadata, "TestProject")
            .expect("validation should succeed");
    let members = &descriptors[0].members;

    let value = members
        .iter()
        .find(|member| member.member_name == "Value")
        .expect("Value should be exported");
    assert_eq!(value.dispatch_id, Some(0));
    assert!(value.is_default_member);
    assert!(value.is_default_bind());

    let new_enum = members
        .iter()
        .find(|member| member.member_name == "NewEnum")
        .expect("NewEnum should be exported");
    assert_eq!(new_enum.dispatch_id, Some(-4));
    assert_eq!(new_enum.member_flags, Some(0x40));
    assert!(new_enum.is_restricted());
    assert!(new_enum.is_hidden());
}
