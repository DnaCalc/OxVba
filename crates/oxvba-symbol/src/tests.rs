//! Standalone tests — hand-built manifests / typelib blobs → resolution. No
//! binder required (the whole point of a symbol-free-input model).

use std::collections::BTreeMap;

use oxvba_bundle::ProjectMemberKind;
use oxvba_com::{
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibMetadataBlob, TypeLibParamType, TypeLibResolvedIdentity,
};

use crate::binding::DispatchRoute;
use crate::manifest::{
    ModuleAttributes, ModuleKind, ProjectKind, SymbolProjectManifest, ModuleUnit,
};
use crate::model::{ScopeKind, SymbolImpl, SymbolKind, SymbolNamespace, SymbolTable};
use crate::predeclared::predeclared_object;
use crate::provider::{build_resolution_environment, Provider, ResolutionContext, TypeLibResolver};
use crate::providers::com::ComTypeLibProvider;
use crate::providers::vba_library::VbaLibraryProvider;
use crate::signature::{BuiltinType, DefaultValue, VarTypeRef};

struct NullTypeLibs;
impl TypeLibResolver for NullTypeLibs {
    fn resolve(&self, _request: &oxvba_com::TypeLibResolveRequest) -> Option<TypeLibMetadataBlob> {
        None
    }
}

fn module(name: &str, source: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.into(),
        module_kind: ModuleKind::Procedural,
        attributes: ModuleAttributes::named(name),
        source: source.into(),
    }
}

fn manifest(name: &str, modules: Vec<ModuleUnit>) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: name.into(),
        project_kind: ProjectKind::Source,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

#[test]
fn environment_retains_active_module_csts_parsed_once() {
    let m = manifest(
        "Proj",
        vec![
            module("Mod1", "Sub Main()\nEnd Sub\n"),
            module("Mod2", "Function F() As Long\nF = 1\nEnd Function\n"),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let mods: Vec<_> = env.modules().collect();
    assert_eq!(
        mods.iter().map(|m| m.module_name).collect::<Vec<_>>(),
        vec!["Mod1", "Mod2"]
    );
    for cst in &mods {
        // The retained tree is real (has child nodes) and its scope matches what
        // `module_scope` reports — proving a single shared parse, not a re-parse.
        assert!(!cst.syntax.child_nodes().is_empty());
        assert_eq!(env.module_scope(cst.module_name), Some(cst.module_scope));
    }
}

// ── Symbol table (source scope chain) ────────────────────────────────────────

#[test]
fn table_resolves_nearest_scope_first() {
    let mut table = SymbolTable::new();
    let module = table.add_scope(ScopeKind::Module, table.global_scope(), Some("Mod1")).unwrap();
    let proc = table.add_scope(ScopeKind::Procedure, module, Some("Foo")).unwrap();
    table
        .declare_symbol(module, SymbolNamespace::Local, SymbolKind::Field, "x", Default::default(), SymbolImpl::None)
        .unwrap();
    let local = table
        .declare_symbol(proc, SymbolNamespace::Local, SymbolKind::Local, "x", Default::default(), SymbolImpl::None)
        .unwrap();
    // From the procedure scope, the local shadows the module-level field.
    assert_eq!(
        table.resolve_in_scope_chain(proc, SymbolNamespace::Local, "x").unwrap(),
        Some(local)
    );
}

#[test]
fn table_keeps_namespaces_distinct() {
    let mut table = SymbolTable::new();
    let module = table.add_scope(ScopeKind::Module, table.global_scope(), Some("Mod1")).unwrap();
    let as_module = table
        .declare_symbol(module, SymbolNamespace::Module, SymbolKind::Module, "Same", Default::default(), SymbolImpl::None)
        .unwrap();
    let as_proc = table
        .declare_symbol(module, SymbolNamespace::Procedure, SymbolKind::Procedure, "Same", Default::default(), SymbolImpl::None)
        .unwrap();
    assert_ne!(as_module, as_proc);
    assert_eq!(table.find_in_scope(module, SymbolNamespace::Module, "Same").unwrap(), Some(as_module));
    assert_eq!(table.find_in_scope(module, SymbolNamespace::Procedure, "Same").unwrap(), Some(as_proc));
}

// ── VBA library provider ─────────────────────────────────────────────────────

#[test]
fn library_resolves_constants_intrinsics_structural_and_special_forms() {
    let p = VbaLibraryProvider;
    assert!(matches!(p.resolve("vbCrLf"), Some(b) if matches!(b.route, DispatchRoute::LibraryConst(_))));
    assert!(matches!(p.resolve("Len"), Some(b) if matches!(b.route, DispatchRoute::Native(_))));
    assert!(matches!(p.resolve("VarPtr"), Some(b) if matches!(b.route, DispatchRoute::Structural(_))));
    assert!(matches!(p.resolve("Array"), Some(b) if matches!(b.route, DispatchRoute::SpecialForm(_))));
    assert!(matches!(p.resolve("Debug"), Some(b) if matches!(b.route, DispatchRoute::PredeclaredObject(_))));
}

#[test]
fn library_resolves_predeclared_members() {
    let p = VbaLibraryProvider;
    let debug = VarTypeRef::Object("debug".into());
    assert!(matches!(
        p.resolve_member(&debug, "Print", None),
        Some(b) if matches!(b.route, DispatchRoute::Native(oxvba_bundle::NativeImplId::DebugPrint))
    ));
    let err = VarTypeRef::Object("err".into());
    assert!(matches!(
        p.resolve_member(&err, "Number", None),
        Some(b) if matches!(b.route, DispatchRoute::ErrMember(_))
    ));
}

// ── Cross-project: referenced project resolved via its export surface ─────────

#[test]
fn referenced_project_resolves_through_its_export_surface() {
    use crate::manifest::{ProjectReference, ReferencedProjectManifest};

    let lib_mod = ModuleUnit {
        module_name: "LibMod".into(),
        module_kind: ModuleKind::Procedural,
        attributes: ModuleAttributes::named("LibMod"),
        source: "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function\n\
                 Private Sub Secret()\nEnd Sub\n\
                 Public Enum Color\n  Red = 1\nEnd Enum\n"
            .into(),
    };
    let mut widget_attrs = ModuleAttributes::named("Widget");
    widget_attrs.vb_exposed = true;
    widget_attrs.vb_creatable = true;
    let widget = ModuleUnit {
        module_name: "Widget".into(),
        module_kind: ModuleKind::Class,
        attributes: widget_attrs,
        source: "Public Function GetValue() As Long\nGetValue = 1\nEnd Function\n".into(),
    };
    let lib = ReferencedProjectManifest {
        project_name: "Lib".into(),
        project_kind: ProjectKind::Library,
        modules: vec![lib_mod, widget],
    };
    let m = SymbolProjectManifest {
        project_name: "App".into(),
        project_kind: ProjectKind::Source,
        modules: vec![module("Main", "Sub Main()\nEnd Sub\n")],
        references: vec![ProjectReference::Project { referenced_project_name: "Lib".into() }],
        reference_projects: vec![lib],
        conditional_constants: BTreeMap::new(),
    };

    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let ctx = ResolutionContext::at(env.module_scope("Main").unwrap());

    // A referenced standard-module Public function resolves UNQUALIFIED, as an
    // in-image project call (devirtualized hidden-module member).
    let add = env.resolve(&ctx, "Add").expect("referenced Add resolves unqualified");
    assert!(matches!(add.route, DispatchRoute::ProjectMember { .. }));

    // It also resolves qualified `Lib.LibMod.Add`.
    assert!(env.resolve_qualified(&["Lib", "LibMod", "Add"]).is_some());

    // A referenced Public Enum member is a global-namespace constant.
    let red = env.resolve(&ctx, "Red").expect("referenced enum member resolves");
    assert!(matches!(red.route, DispatchRoute::Value));

    // A referenced Private member does NOT cross the boundary.
    assert!(env.resolve(&ctx, "Secret").is_none());

    // A referenced exposed class member resolves on a typed receiver as an
    // early-bound COM member (the VM dispatches it in-image, WS-H).
    let recv = VarTypeRef::Object("Widget".into());
    let gv = env.resolve_member(&recv, "GetValue", None).expect("Widget.GetValue resolves");
    assert!(matches!(gv.route, DispatchRoute::ComMember { .. }));

    // The active project keeps its own surface available to the binder.
    assert_eq!(env.export_surfaces().len(), 2, "active + one referenced surface");
}

// ── COM provider: early/late + events ────────────────────────────────────────

fn widget_blob() -> TypeLibMetadataBlob {
    TypeLibMetadataBlob {
        identity: TypeLibResolvedIdentity {
            reference_name: "Widget".into(),
            requested_coclass: None,
            importlib: "widget".into(),
            libid: None,
            major_version: 1,
            minor_version: 0,
            lcid: None,
            cache_key: "widget".into(),
        },
        activation_prog_id: Some("Widget.Thing".into()),
        member_name_to_token: vec![("DoThing".into(), 5), ("Item".into(), 0)],
        members: vec![
            TypeLibMemberMetadata {
                name: "DoThing".into(),
                token: 5,
                vtable_slot: Some(7),
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["n".into()],
                parameter_optional: vec![false],
                is_default_member: false,
                parameter_types: vec![TypeLibParamType::ByRefLong],
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Item".into(),
                token: 0,
                vtable_slot: Some(8),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["Index".into()],
                parameter_optional: vec![false],
                is_default_member: true,
                parameter_types: vec![TypeLibParamType::Long],
                return_type: Some(TypeLibParamType::Variant),
            },
        ],
        events: vec![TypeLibEventMetadata {
            name: "Changed".into(),
            token: 9,
            callback_arity: 1,
            dispatch_path: TypeLibEventDispatchPath::SourceInterface,
            connection_point_iid: Some("{iid}".into()),
            dispatch_member_id: Some(9),
        }],
    }
}

#[test]
fn com_member_resolves_for_typed_receiver_with_both_dispid_and_name() {
    let provider = ComTypeLibProvider::new(widget_blob());
    let typed = VarTypeRef::Object("Widget".into());
    let binding = provider.resolve_member(&typed, "DoThing", None).expect("typed member");
    match binding.route {
        DispatchRoute::ComMember { member_name, dispid, vtable_slot, member_kind, param_by_ref, .. } => {
            assert_eq!(member_name, "DoThing"); // late path uses the name
            assert_eq!(dispid, 5); // early path uses the dispid
            assert_eq!(vtable_slot, Some(7));
            assert_eq!(member_kind, ProjectMemberKind::Method);
            // The typelib's `ByRefLong` param surfaces as a by-ref direction.
            assert_eq!(param_by_ref, vec![true]);
        }
        other => panic!("expected ComMember, got {other:?}"),
    }
}

#[test]
fn coclass_resolves_to_activation_prog_id() {
    // `New <coclass>` consults this hook to obtain the ProgID for activation.
    let provider = ComTypeLibProvider::new(widget_blob());
    assert_eq!(provider.resolve_coclass("Widget").as_deref(), Some("Widget.Thing"));
    assert_eq!(provider.resolve_coclass("Widget.Thing").as_deref(), Some("Widget.Thing"));
    assert_eq!(provider.resolve_coclass("Nope"), None);
}

#[test]
fn com_member_does_not_resolve_for_untyped_receiver() {
    // An `Object`/`Variant` receiver has no typelib to consult — the binder emits
    // a late dispatch by name; the provider correctly declines.
    let provider = ComTypeLibProvider::new(widget_blob());
    assert!(provider.resolve_member(&VarTypeRef::Variant, "DoThing", None).is_none());
}

#[test]
fn com_event_resolves_for_with_events_source_type() {
    let provider = ComTypeLibProvider::new(widget_blob());
    let typed = VarTypeRef::Object("Widget".into());
    match provider.resolve_member(&typed, "Changed", None).expect("event").route {
        DispatchRoute::ComEvent { token, callback_arity, dispatch_path, .. } => {
            assert_eq!(token, 9);
            assert_eq!(callback_arity, 1);
            assert_eq!(dispatch_path, TypeLibEventDispatchPath::SourceInterface);
        }
        other => panic!("expected ComEvent, got {other:?}"),
    }
}

// ── Full environment: cross-module + declare extraction ──────────────────────

#[test]
fn environment_resolves_unqualified_and_qualified_cross_module() {
    let m = manifest(
        "Proj",
        vec![
            module("Main", "Sub Run()\r\nEnd Sub\r\n"),
            module("Module1", "Public Function Value() As Long\r\nEnd Function\r\n"),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");

    // Unqualified, cross-module, via the project provider.
    let from_global = ResolutionContext::at(env.symbols.global_scope());
    assert!(matches!(
        env.resolve(&from_global, "Value"),
        Some(b) if matches!(b.route, DispatchRoute::ProjectMember { .. })
    ));

    // Qualified `Module1.Value`.
    assert!(matches!(
        env.resolve_qualified(&["Module1", "Value"]),
        Some(b) if matches!(b.route, DispatchRoute::ProjectMember { .. })
    ));
    // Project-qualified `Proj.Module1.Value`.
    assert!(env.resolve_qualified(&["Proj", "Module1", "Value"]).is_some());

    // Library + intrinsic still resolve through the same source-agnostic path.
    assert!(matches!(
        env.resolve(&from_global, "vbCrLf"),
        Some(b) if matches!(b.route, DispatchRoute::LibraryConst(_))
    ));
    assert!(matches!(
        env.resolve(&from_global, "Len"),
        Some(b) if matches!(b.route, DispatchRoute::Native(_))
    ));
}

#[test]
fn environment_extracts_declare_statements() {
    let src = "Declare PtrSafe Function GetTickCount Lib \"kernel32\" () As Long\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);
    let binding = env.resolve(&ctx, "GetTickCount").expect("declare resolves");
    assert!(matches!(binding.route, DispatchRoute::Declare { .. }));
    let symbol = env.symbols.symbol(binding.symbol.expect("symbol id")).expect("symbol");
    match &symbol.imp {
        SymbolImpl::Declare(declare) => {
            assert_eq!(declare.declared_name, "GetTickCount");
            assert_eq!(declare.library, "kernel32"); // read from the structured LibClause
            assert!(declare.is_function);
        }
        other => panic!("expected Declare impl, got {other:?}"),
    }
}

#[test]
fn com_resolves_default_member() {
    let provider = ComTypeLibProvider::new(widget_blob());
    let typed = VarTypeRef::Object("Widget".into());
    let binding = provider.resolve_default_member(&typed).expect("default member");
    assert!(binding.is_default);
    match binding.route {
        DispatchRoute::ComMember { member_name, is_default_member, .. } => {
            assert_eq!(member_name, "Item");
            assert!(is_default_member);
        }
        other => panic!("expected ComMember, got {other:?}"),
    }
}

// ── Reworks: property groups, parsed defaults, project default members ────────

#[test]
fn property_get_and_let_merge_into_one_group() {
    let src = "Property Get Foo() As Long\r\nEnd Property\r\n\
               Property Let Foo(ByVal v As Long)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env.resolve(&ResolutionContext::at(scope), "Foo").expect("property resolves");
    let symbol = env.symbols.symbol(binding.symbol.expect("symbol id")).expect("symbol");
    assert_eq!(symbol.kind, SymbolKind::Property);
    match &symbol.imp {
        SymbolImpl::Property(group) => {
            assert!(group.get.is_some(), "Get accessor");
            assert!(group.let_.is_some(), "Let accessor");
            assert!(group.set.is_none(), "no Set accessor");
        }
        other => panic!("expected Property group, got {other:?}"),
    }
}

#[test]
fn optional_parameter_default_is_parsed() {
    let src = "Sub S(Optional ByVal n As Long = 5)\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env.resolve(&ResolutionContext::at(scope), "S").expect("sub resolves");
    let symbol = env.symbols.symbol(binding.symbol.expect("symbol id")).expect("symbol");
    let SymbolImpl::Signature(sig_id) = symbol.imp else {
        panic!("expected a signature");
    };
    let signature = env.signatures.get(sig_id).expect("signature");
    let param = &signature.params[0];
    assert!(param.optional);
    assert_eq!(param.default, Some(DefaultValue::I32(5)));
}

#[test]
fn scanner_reads_per_declarator_types_from_structured_cst() {
    // Each declarator carries its own type (the old flat-token walker couldn't).
    let m = manifest("Proj", vec![module("Mod1", "Public a As Long, b As String\r\n")]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);

    let type_of = |name: &str| {
        let binding = env.resolve(&ctx, name).expect("resolves");
        match &env.symbols.symbol(binding.symbol.expect("symbol")).expect("symbol").imp {
            SymbolImpl::DeclaredType(ty) => ty.clone(),
            other => panic!("expected declared type, got {other:?}"),
        }
    };
    assert_eq!(type_of("a"), VarTypeRef::Builtin(BuiltinType::Long));
    assert_eq!(type_of("b"), VarTypeRef::Builtin(BuiltinType::String));
}

#[test]
fn scanner_declares_enum_members() {
    let src = "Public Enum Color\r\n    Red\r\n    Green = 5\r\n    Blue\r\nEnd Enum\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);
    for member in ["Red", "Green", "Blue"] {
        assert!(env.resolve(&ctx, member).is_some(), "enum member {member}");
    }
}

#[test]
fn predeclared_objects_are_recognized() {
    assert!(predeclared_object("Err").is_some());
    assert!(predeclared_object("Collection").is_some());
    assert!(predeclared_object("NotAnObject").is_none());
}
