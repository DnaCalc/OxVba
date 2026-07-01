//! Standalone tests — hand-built manifests / typelib blobs → resolution. No
//! binder required (the whole point of a symbol-free-input model).

use std::collections::BTreeMap;

use oxvba_bundle::coreir::CoreConst;
use oxvba_bundle::{DeclareParamType, ProjectMemberKind};
use oxvba_com::{
    SourceTypeKind, TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind,
    TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibParamType, TypeLibResolvedIdentity,
    TypeLibWireType,
};

use crate::binding::{DispatchRoute, SpecialForm};
use crate::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
};
use crate::model::{
    ScopeKind, SymbolImpl, SymbolKind, SymbolModelError, SymbolNamespace, SymbolTable,
};
use crate::predeclared::predeclared_object;
use crate::provider::{Provider, ResolutionContext, TypeLibResolver, build_resolution_environment};
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
    let module = table
        .add_scope(ScopeKind::Module, table.global_scope(), Some("Mod1"))
        .unwrap();
    let proc = table
        .add_scope(ScopeKind::Procedure, module, Some("Foo"))
        .unwrap();
    table
        .declare_symbol(
            module,
            SymbolNamespace::Local,
            SymbolKind::Field,
            "x",
            Default::default(),
            SymbolImpl::None,
        )
        .unwrap();
    let local = table
        .declare_symbol(
            proc,
            SymbolNamespace::Local,
            SymbolKind::Local,
            "x",
            Default::default(),
            SymbolImpl::None,
        )
        .unwrap();
    // From the procedure scope, the local shadows the module-level field.
    assert_eq!(
        table
            .resolve_in_scope_chain(proc, SymbolNamespace::Local, "x")
            .unwrap(),
        Some(local)
    );
}

#[test]
fn table_keeps_namespaces_distinct() {
    let mut table = SymbolTable::new();
    let module = table
        .add_scope(ScopeKind::Module, table.global_scope(), Some("Mod1"))
        .unwrap();
    let as_module = table
        .declare_symbol(
            module,
            SymbolNamespace::Module,
            SymbolKind::Module,
            "Same",
            Default::default(),
            SymbolImpl::None,
        )
        .unwrap();
    let as_proc = table
        .declare_symbol(
            module,
            SymbolNamespace::Procedure,
            SymbolKind::Procedure,
            "Same",
            Default::default(),
            SymbolImpl::None,
        )
        .unwrap();
    assert_ne!(as_module, as_proc);
    assert_eq!(
        table
            .find_in_scope(module, SymbolNamespace::Module, "Same")
            .unwrap(),
        Some(as_module)
    );
    assert_eq!(
        table
            .find_in_scope(module, SymbolNamespace::Procedure, "Same")
            .unwrap(),
        Some(as_proc)
    );
}

// ── VBA library provider ─────────────────────────────────────────────────────

#[test]
fn library_resolves_constants_intrinsics_structural_and_special_forms() {
    let p = VbaLibraryProvider;
    let constant = |name: &str| {
        p.resolve(name).and_then(|binding| match binding.route {
            DispatchRoute::ConstValue(value) => Some(value),
            _ => None,
        })
    };
    assert!(
        matches!(p.resolve("vbCrLf"), Some(b) if matches!(b.route, DispatchRoute::ConstValue(_)))
    );
    assert_eq!(constant("vbModeless"), Some(CoreConst::I32(0)));
    assert_eq!(constant("vbModal"), Some(CoreConst::I32(1)));
    // The by-name `FileStatement` forms (`Kill`/`MkDir`/… are not lexer keywords, so
    // they resolve by name) now route cross-bundle to the `VBA` unit's `FileSystem`
    // module, exactly like the by-name file functions — P4 migrated them off the
    // bespoke `Native` route. (The name-LESS file statements `Open`/`Print #`/… are
    // parser-bound, never resolved by name; the only intrinsic still on the `Native`
    // route is the predeclared `Debug.Print`, exercised in
    // `library_resolves_predeclared_members`.)
    assert!(matches!(
        p.resolve("Kill"),
        Some(b) if matches!(
            &b.route,
            DispatchRoute::ExternMember { unit, owner, member, has_receiver: false, .. }
                if unit == "VBA" && owner == "FileSystem" && member == "Kill"
        )
    ));
    // A `FileIo` by-name FUNCTION (`FreeFile`) now routes cross-bundle to the `VBA`
    // unit's `FileSystem` module (migrated this round).
    assert!(matches!(
        p.resolve("FreeFile"),
        Some(b) if matches!(
            &b.route,
            DispatchRoute::ExternMember { unit, owner, member, has_receiver: false, .. }
                if unit == "VBA" && owner == "FileSystem" && member == "FreeFile"
        )
    ));
    // An `Information` predicate now routes cross-bundle to the `VBA` unit's
    // `Information` module (migrated this round).
    assert!(matches!(
        p.resolve("IsNumeric"),
        Some(b) if matches!(
            &b.route,
            DispatchRoute::ExternMember { unit, owner, member, has_receiver: false, .. }
                if unit == "VBA" && owner == "Information" && member == "IsNumeric"
        )
    ));
    // An `Interaction` host function routes cross-bundle to the `VBA` unit's
    // `Interaction` module.
    assert!(matches!(
        p.resolve("Environ"),
        Some(b) if matches!(
            &b.route,
            DispatchRoute::ExternMember { unit, owner, member, has_receiver: false, .. }
                if unit == "VBA" && owner == "Interaction" && member == "Environ"
        )
    ));
    // The `IIf` special form stays a `SpecialForm` route (NOT migrated).
    assert!(
        matches!(p.resolve("IIf"), Some(b) if matches!(b.route, DispatchRoute::SpecialForm(_)))
    );
    // A `Strings`-module library function now resolves as a cross-bundle member of
    // the synthetic `VBA` unit (no receiver), like a referenced free function.
    assert!(matches!(
        p.resolve("Len"),
        Some(b) if matches!(
            &b.route,
            DispatchRoute::ExternMember { unit, owner, member, has_receiver: false, .. }
                if unit == "VBA" && owner == "Strings" && member == "Len"
        )
    ));
    // A `Math` function now also routes cross-bundle to the `VBA` unit's `Math`
    // module, proving the migration generalized beyond `Strings`.
    assert!(matches!(
        p.resolve("Abs"),
        Some(b) if matches!(
            &b.route,
            DispatchRoute::ExternMember { unit, owner, member, has_receiver: false, .. }
                if unit == "VBA" && owner == "Math" && member == "Abs"
        )
    ));
    assert!(
        matches!(p.resolve("VarPtr"), Some(b) if matches!(b.route, DispatchRoute::Structural(_)))
    );
    assert!(
        matches!(p.resolve("Array"), Some(b) if matches!(b.route, DispatchRoute::SpecialForm(_)))
    );
    assert!(
        matches!(p.resolve("Erl"), Some(b) if matches!(b.route, DispatchRoute::SpecialForm(SpecialForm::Erl)))
    );
    assert!(
        matches!(p.resolve("Debug"), Some(b) if matches!(b.route, DispatchRoute::PredeclaredObject(_)))
    );
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
        references: vec![ProjectReference::Project {
            referenced_project_name: "Lib".into(),
        }],
        reference_projects: vec![lib],
        conditional_constants: BTreeMap::new(),
    };

    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let ctx = ResolutionContext::at(env.module_scope("Main").unwrap());

    // A referenced standard-module Public function resolves UNQUALIFIED, as a
    // cross-bundle extern with no receiver (an import-backed `ExternProc` call).
    let add = env
        .resolve(&ctx, "Add")
        .expect("referenced Add resolves unqualified");
    assert!(matches!(
        add.route,
        DispatchRoute::ExternMember { has_receiver: false, ref unit, .. } if unit == "Lib"
    ));

    // It also resolves qualified `Lib.LibMod.Add`.
    assert!(env.resolve_qualified(&["Lib", "LibMod", "Add"]).is_some());

    // A referenced Public Enum member resolves to its published literal value.
    let red = env
        .resolve(&ctx, "Red")
        .expect("referenced enum member resolves");
    assert!(matches!(
        red.route,
        DispatchRoute::ConstValue(CoreConst::I32(1))
    ));

    // A referenced Private member does NOT cross the boundary.
    assert!(env.resolve(&ctx, "Secret").is_none());

    // A referenced exposed class member resolves on a typed receiver as a
    // cross-bundle extern WITH a receiver (dispatched by name in the object's bundle).
    let recv = VarTypeRef::Object("Widget".into());
    let gv = env
        .resolve_member(&recv, "GetValue", None)
        .expect("Widget.GetValue resolves");
    assert!(matches!(
        gv.route,
        DispatchRoute::ExternMember { has_receiver: true, ref member, .. } if member == "GetValue"
    ));

    // `New Lib.Widget` (and bare `New Widget`) resolve to a creatable extern coclass.
    assert_eq!(
        env.resolve_extern_coclass("Lib.Widget"),
        Some(("Lib".to_string(), "Widget".to_string()))
    );
    assert_eq!(
        env.resolve_extern_coclass("Widget"),
        Some(("Lib".to_string(), "Widget".to_string()))
    );

    // The active project keeps its own surface available to the binder.
    assert_eq!(
        env.export_surfaces().len(),
        2,
        "active + one referenced surface"
    );
}

#[test]
fn const_and_enum_values_fold_into_the_type_system() {
    let m = manifest(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const KMax As Long = 10\n\
             Public Const KTwice As Long = KMax * 2\n\
             Public Enum Color\n  Red = 1\n  Green\n  Blue = 10\n  Indigo\nEnd Enum\n",
        )],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Mod1").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("KMax"), Some(CoreConst::I32(10)));
    assert_eq!(val("KTwice"), Some(CoreConst::I32(20))); // cross-const reference
    assert_eq!(val("Red"), Some(CoreConst::I32(1)));
    assert_eq!(val("Green"), Some(CoreConst::I32(2))); // enum auto-increment
    assert_eq!(val("Blue"), Some(CoreConst::I32(10))); // explicit value resets
    assert_eq!(val("Indigo"), Some(CoreConst::I32(11))); // resumes from 10
}

#[test]
fn module_qualified_const_values_fold_across_modules() {
    let m = manifest(
        "Proj",
        vec![
            module("ModA", "Public Const Derived As Long = ModB.Base + 1\n"),
            module(
                "ModB",
                "Public Const Base As Long = 7\n\
                 Public Const X As Long = ModA.Derived + 1\n\
                 Public Const Y As Long = Proj.ModA.Derived + 2\n",
            ),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let val = |module: &str, name: &str| -> Option<CoreConst> {
        let b = env.resolve_qualified(&[module, name])?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("ModB", "Base"), Some(CoreConst::I32(7)));
    assert_eq!(val("ModA", "Derived"), Some(CoreConst::I32(8)));
    assert_eq!(val("ModB", "X"), Some(CoreConst::I32(9)));
    assert_eq!(val("ModB", "Y"), Some(CoreConst::I32(10)));
}

#[test]
fn typed_const_values_preserve_exact_type_system_carriers() {
    let m = manifest(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const CSingle As Single = 1.5!\n\
             Public Const CAmount As Currency = 1.25@\n\
             Public Const CStamp As Date = #2026-02-28#\n\
             Public Const CText As String = CSingle & \"|\" & CAmount\n",
        )],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Mod1").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("CSingle"), Some(CoreConst::F32(1.5f32.to_bits())));
    assert_eq!(val("CAmount"), Some(CoreConst::Currency(12_500)));
    assert_eq!(val("CStamp"), Some(CoreConst::Date(46_081.0f64.to_bits())));
    assert_eq!(val("CText"), Some(CoreConst::Str("1.5|1.25".to_string())));
}

#[test]
fn active_enum_member_resolves_qualified_by_enum_name() {
    let m = manifest(
        "Proj",
        vec![module(
            "Mod1",
            "Public Enum WebFormat\n  PlainText = 0\n  Json = 1\nEnd Enum\n",
        )],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let binding = env
        .resolve_qualified(&["WebFormat", "Json"])
        .expect("enum-qualified member");
    let value = binding.symbol.and_then(|sym| env.const_value(sym)).cloned();
    assert_eq!(value, Some(CoreConst::I32(1)));
}

#[test]
fn string_typed_const_values_coerce_to_declared_scalar_carriers() {
    let m = manifest(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const CLong As Long = \"7\"\n\
             Public Const CBool As Boolean = \"False\"\n\
             Public Const CSingle As Single = \"1.5\"\n\
             Public Const CDouble As Double = \"2.5\"\n\
             Public Const CAmount As Currency = \"1.25\"\n\
             Public Const CStamp As Date = \"2026-02-28\"\n",
        )],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Mod1").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("CLong"), Some(CoreConst::I32(7)));
    assert_eq!(val("CBool"), Some(CoreConst::Bool(false)));
    assert_eq!(val("CSingle"), Some(CoreConst::F32(1.5f32.to_bits())));
    assert_eq!(val("CDouble"), Some(CoreConst::F64(2.5f64.to_bits())));
    assert_eq!(val("CAmount"), Some(CoreConst::Currency(12_500)));
    assert_eq!(val("CStamp"), Some(CoreConst::Date(46_081.0f64.to_bits())));
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
                parameter_wire_types: vec![TypeLibWireType::Automation(
                    TypeLibParamType::ByRefLong,
                )],
                parameter_iids: vec![None],
                return_type: None,
                return_wire_type: None,
                callconv_is_stdcall: true,
                is_dual: true,
                interface_iid: None,
                parameter_optional_defaults: Vec::new(),
                source_typekind: Some(SourceTypeKind::Interface),
                vtable_slot_bound: Some(64),
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
                parameter_wire_types: vec![TypeLibWireType::Automation(TypeLibParamType::Long)],
                parameter_iids: vec![None],
                return_type: Some(TypeLibParamType::Variant),
                return_wire_type: Some(TypeLibWireType::Automation(TypeLibParamType::Variant)),
                callconv_is_stdcall: true,
                is_dual: true,
                interface_iid: None,
                parameter_optional_defaults: Vec::new(),
                source_typekind: Some(SourceTypeKind::Interface),
                vtable_slot_bound: Some(64),
            },
        ],
        events: vec![TypeLibEventMetadata {
            name: "Changed".into(),
            token: 9,
            callback_arity: 1,
            dispatch_path: TypeLibEventDispatchPath::SourceInterface,
            connection_point_iid: Some("{iid}".into()),
            dispatch_member_id: Some(9),
            coclass: None,
        }],
        coclass_names: Vec::new(),
    }
}

#[test]
fn com_member_resolves_for_typed_receiver_with_both_dispid_and_name() {
    let provider = ComTypeLibProvider::new(widget_blob());
    let typed = VarTypeRef::Object("Widget".into());
    let binding = provider
        .resolve_member(&typed, "DoThing", None)
        .expect("typed member");
    match binding.route {
        DispatchRoute::ComMember {
            member_name,
            dispid,
            vtable_slot,
            member_kind,
            param_by_ref,
            ..
        } => {
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
    assert_eq!(
        provider.resolve_coclass("Widget").as_deref(),
        Some("Widget.Thing")
    );
    assert_eq!(
        provider.resolve_coclass("Widget.Thing").as_deref(),
        Some("Widget.Thing")
    );
    assert_eq!(provider.resolve_coclass("Nope"), None);
}

#[test]
fn library_level_coclass_resolves_bare_and_qualified_names() {
    // A library-wide COM reference such as `Scripting` has no requested coclass,
    // but its coclasses are still valid early-bound type names in VBA.
    let mut blob = widget_blob();
    blob.identity.reference_name = "Scripting".into();
    blob.identity.requested_coclass = None;
    blob.activation_prog_id = None;
    blob.coclass_names = vec!["Dictionary".into()];
    let provider = ComTypeLibProvider::new(blob);

    assert_eq!(
        provider.resolve_coclass("Dictionary").as_deref(),
        Some("Scripting.Dictionary")
    );
    assert_eq!(
        provider.resolve_coclass("Scripting.Dictionary").as_deref(),
        Some("Scripting.Dictionary")
    );
    assert!(
        provider
            .resolve_member(
                &VarTypeRef::Object("Dictionary".into()),
                "DoThing",
                Some(ProjectMemberKind::Method),
            )
            .is_some()
    );
    assert!(
        provider
            .resolve_member(
                &VarTypeRef::Object("Scripting.Dictionary".into()),
                "DoThing",
                Some(ProjectMemberKind::Method),
            )
            .is_some()
    );
    assert_eq!(provider.resolve_coclass("Scripting.Nope"), None);
}

struct ExcelHostTypeLibs;

impl TypeLibResolver for ExcelHostTypeLibs {
    fn resolve(&self, request: &oxvba_com::TypeLibResolveRequest) -> Option<TypeLibMetadataBlob> {
        assert_eq!(request.reference_name, "Excel");
        assert_eq!(request.requested_coclass.as_deref(), Some("Application"));
        let mut blob = widget_blob();
        blob.identity.reference_name = "Excel".into();
        blob.identity.requested_coclass = Some("Application".into());
        blob.activation_prog_id = Some("Excel.Application".into());
        blob.coclass_names = vec!["Application".into()];
        Some(blob)
    }
}

#[test]
fn host_injected_prog_id_reference_splits_library_and_coclass() {
    let mut manifest = manifest("Proj", vec![module("Main", "Sub Main()\nEnd Sub\n")]);
    manifest.references = vec![ProjectReference::HostInjected {
        referenced_project_name: "Excel.Application".into(),
    }];
    let env = build_resolution_environment(&manifest, &ExcelHostTypeLibs).unwrap();
    let scope = env.module_scope("Main").expect("main module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "Application")
        .expect("host Application root");
    match binding.route {
        DispatchRoute::ComObjectRoot { type_name, prog_id } => {
            assert_eq!(type_name, "Application");
            assert_eq!(prog_id.as_deref(), Some("Excel.Application"));
        }
        other => panic!("expected host COM object root, got {other:?}"),
    }
}

#[test]
fn com_member_does_not_resolve_for_untyped_receiver() {
    // An `Object`/`Variant` receiver has no typelib to consult — the binder emits
    // a late dispatch by name; the provider correctly declines.
    let provider = ComTypeLibProvider::new(widget_blob());
    assert!(
        provider
            .resolve_member(&VarTypeRef::Variant, "DoThing", None)
            .is_none()
    );
}

#[test]
fn com_event_resolves_for_with_events_source_type() {
    let provider = ComTypeLibProvider::new(widget_blob());
    let typed = VarTypeRef::Object("Widget".into());
    match provider
        .resolve_member(&typed, "Changed", None)
        .expect("event")
        .route
    {
        DispatchRoute::ComEvent {
            token,
            callback_arity,
            dispatch_path,
            ..
        } => {
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
            module(
                "Module1",
                "Public Function Value() As Long\r\nEnd Function\r\n",
            ),
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
    assert!(
        env.resolve_qualified(&["Proj", "Module1", "Value"])
            .is_some()
    );
    assert!(env.is_project_name("Proj"));
    assert!(
        env.resolve_qualified(&["WrongProj", "Module1", "Value"])
            .is_none()
    );

    // Library + intrinsic still resolve through the same source-agnostic path.
    assert!(matches!(
        env.resolve(&from_global, "vbCrLf"),
        Some(b) if matches!(b.route, DispatchRoute::ConstValue(_))
    ));
    // Migrated members resolve cross-bundle to the `VBA` unit, all via the same
    // source-agnostic provider path: `Len` (a `Strings` member), `IsNumeric` (an
    // `Information` predicate), `FreeFile` (a `FileSystem` by-name function), and `Kill`
    // (a by-name `FileSystem` STATEMENT — not a lexer keyword, so it resolves by name).
    assert!(matches!(
        env.resolve(&from_global, "Kill"),
        Some(b) if matches!(b.route, DispatchRoute::ExternMember { has_receiver: false, .. })
    ));
    assert!(matches!(
        env.resolve(&from_global, "Len"),
        Some(b) if matches!(b.route, DispatchRoute::ExternMember { has_receiver: false, .. })
    ));
    assert!(matches!(
        env.resolve(&from_global, "IsNumeric"),
        Some(b) if matches!(b.route, DispatchRoute::ExternMember { has_receiver: false, .. })
    ));
    assert!(matches!(
        env.resolve(&from_global, "FreeFile"),
        Some(b) if matches!(b.route, DispatchRoute::ExternMember { has_receiver: false, .. })
    ));
}

#[test]
fn duplicate_public_unqualified_members_are_ambiguous_before_library_fallback() {
    let m = manifest(
        "Proj",
        vec![
            module("Main", "Sub Run()\r\nEnd Sub\r\n"),
            module("Alpha", "Public Function Len() As Long\r\nEnd Function\r\n"),
            module("Beta", "Public Function Len() As Long\r\nEnd Function\r\n"),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let from_global = ResolutionContext::at(env.symbols.global_scope());

    assert!(env.has_ambiguous_unqualified_name("Len"));
    assert!(env.resolve(&from_global, "Len").is_none());
    assert!(matches!(
        env.resolve_qualified(&["Alpha", "Len"]),
        Some(b) if matches!(b.route, DispatchRoute::ProjectMember { .. })
    ));
    assert!(matches!(
        env.resolve_qualified(&["Beta", "Len"]),
        Some(b) if matches!(b.route, DispatchRoute::ProjectMember { .. })
    ));
}

#[test]
fn unrelated_class_property_does_not_shadow_vba_left_intrinsic() {
    let control = ModuleUnit {
        module_name: "ControlLike".into(),
        module_kind: ModuleKind::Class,
        attributes: ModuleAttributes::named("ControlLike"),
        source: "Public Property Get Left() As Single\r\nEnd Property\r\n".into(),
    };
    let m = manifest(
        "Proj",
        vec![module("Main", "Sub Run()\r\nEnd Sub\r\n"), control],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let ctx = ResolutionContext::at(env.module_scope("Main").expect("Main scope"));
    let binding = env.resolve(&ctx, "Left").expect("Left resolves");
    assert!(matches!(
        binding.route,
        DispatchRoute::ExternMember {
            ref unit,
            ref owner,
            ref member,
            has_receiver: false,
            ..
        } if unit == "VBA" && owner == "Strings" && member == "Left"
    ));
    let suffixed = env.resolve(&ctx, "Left$").expect("Left$ resolves");
    assert!(matches!(
        suffixed.route,
        DispatchRoute::ExternMember {
            ref unit,
            ref owner,
            ref member,
            has_receiver: false,
            ..
        } if unit == "VBA" && owner == "Strings" && member == "Left$"
    ));

    let receiver = VarTypeRef::Object("ControlLike".into());
    assert!(matches!(
        env.resolve_member(&receiver, "Left", None),
        Some(b) if matches!(b.route, DispatchRoute::ProjectMember { .. })
    ));
}

#[test]
fn public_standard_module_member_still_shadows_vba_intrinsic() {
    let m = manifest(
        "Proj",
        vec![
            module("Main", "Sub Run()\r\nEnd Sub\r\n"),
            module(
                "Helpers",
                "Public Function Left() As String\r\nEnd Function\r\n",
            ),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let ctx = ResolutionContext::at(env.module_scope("Main").expect("Main scope"));
    assert!(matches!(
        env.resolve(&ctx, "Left"),
        Some(b) if matches!(b.route, DispatchRoute::ProjectMember { .. })
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
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
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
fn declare_function_type_suffix_supplies_return_type() {
    let src = "DefDbl A-Z\r\nDeclare PtrSafe Function GetTickCount& Lib \"kernel32\" ()\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "GetTickCount")
        .expect("declare resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
    let SymbolImpl::Declare(declare) = &symbol.imp else {
        panic!("expected Declare impl");
    };
    assert_eq!(declare.return_type, Some(DeclareParamType::Long));
}

#[test]
fn com_resolves_default_member() {
    let provider = ComTypeLibProvider::new(widget_blob());
    let typed = VarTypeRef::Object("Widget".into());
    let binding = provider
        .resolve_default_member(&typed)
        .expect("default member");
    assert!(binding.is_default);
    match binding.route {
        DispatchRoute::ComMember {
            member_name,
            is_default_member,
            ..
        } => {
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
    let binding = env
        .resolve(&ResolutionContext::at(scope), "Foo")
        .expect("property resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
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
fn exported_member_attribute_marks_project_default_member() {
    let src = "Property Get Value(ByVal i As Long) As Long\r\n    Value = i\r\nEnd Property\r\n\
               Attribute Value.VB_UserMemId = 0\r\n";
    let m = manifest("Proj", vec![module("Widget", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let binding = env
        .resolve_default_member(&VarTypeRef::Object("Widget".to_string()))
        .expect("default member");
    assert!(binding.is_default);
    assert!(matches!(
        binding.route,
        DispatchRoute::ProjectMember {
            kind: ProjectMemberKind::PropertyGet
        }
    ));
}

#[test]
fn optional_parameter_default_is_parsed() {
    let src = "Sub S(Optional ByVal n As Long = 5)\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("sub resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
    let SymbolImpl::Signature(sig_id) = symbol.imp else {
        panic!("expected a signature");
    };
    let signature = env.signatures.get(sig_id).expect("signature");
    let param = &signature.params[0];
    assert!(param.optional);
    assert_eq!(param.default, Some(DefaultValue::I32(5)));
}

#[test]
fn optional_variant_integer_default_preserves_i16_metadata_carrier() {
    let src = "Sub S(Optional ByVal n As Variant = 5)\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("sub resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
    let SymbolImpl::Signature(sig_id) = symbol.imp else {
        panic!("expected a signature");
    };
    let signature = env.signatures.get(sig_id).expect("signature");
    assert_eq!(signature.params[0].default, Some(DefaultValue::I16(5)));
}

#[test]
fn optional_parameter_string_defaults_coerce_to_declared_metadata() {
    let src = "Sub S(Optional ByVal n As Long = \"7\", Optional ByVal b As Boolean = \"False\", Optional ByVal c As Currency = \"1.25\", Optional ByVal d As Date = \"2026-02-28\")\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("sub resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
    let SymbolImpl::Signature(sig_id) = symbol.imp else {
        panic!("expected a signature");
    };
    let signature = env.signatures.get(sig_id).expect("signature");
    assert_eq!(signature.params[0].default, Some(DefaultValue::I32(7)));
    assert_eq!(signature.params[1].default, Some(DefaultValue::Bool(false)));
    assert_eq!(
        signature.params[2].default,
        Some(DefaultValue::CurrencyScaledI64(12_500))
    );
    assert_eq!(
        signature.params[3].default,
        Some(DefaultValue::DateSerialF64(46_081.0f64.to_bits()))
    );
}

#[test]
fn optional_single_default_preserves_f32_metadata_carrier() {
    let src = "Sub S(Optional ByVal n As Single = 1.5!)\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("proc resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol"))
        .expect("symbol");
    let SymbolImpl::Signature(sig) = &symbol.imp else {
        panic!("expected signature");
    };
    let signature = env.signatures.get(*sig).expect("signature");
    assert_eq!(
        signature.params[0].default,
        Some(DefaultValue::F32(1.5f32.to_bits()))
    );
}

#[test]
fn scanner_reads_per_declarator_types_from_structured_cst() {
    // Each declarator carries its own type (the old flat-token walker couldn't).
    let m = manifest(
        "Proj",
        vec![module("Mod1", "Public a As Long, b As String\r\n")],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);

    let type_of = |name: &str| {
        let binding = env.resolve(&ctx, name).expect("resolves");
        match &env
            .symbols
            .symbol(binding.symbol.expect("symbol"))
            .expect("symbol")
            .imp
        {
            SymbolImpl::DeclaredType(ty) => ty.clone(),
            other => panic!("expected declared type, got {other:?}"),
        }
    };
    assert_eq!(type_of("a"), VarTypeRef::Builtin(BuiltinType::Long));
    assert_eq!(type_of("b"), VarTypeRef::Builtin(BuiltinType::String));
}

#[test]
fn scanner_applies_deftype_to_variables_params_and_returns() {
    let src = "DefLng A-Z\r\n\
               Public fieldValue\r\n\
               Function Fold(value)\r\n\
                   Dim localValue\r\n\
                   Fold = value\r\n\
               End Function\r\n\
               Property Get Count()\r\n\
                   Count = 1\r\n\
               End Property\r\n\
               Private Declare PtrSafe Function Fetch Lib \"kernel32\" ()\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);

    let field = env.resolve(&ctx, "fieldValue").expect("field resolves");
    let field_symbol = env
        .symbols
        .symbol(field.symbol.expect("field symbol"))
        .expect("field symbol");
    assert_eq!(
        &field_symbol.imp,
        &SymbolImpl::DeclaredType(VarTypeRef::Builtin(BuiltinType::Long))
    );

    let fold = env.resolve(&ctx, "Fold").expect("function resolves");
    let fold_symbol = env
        .symbols
        .symbol(fold.symbol.expect("function symbol"))
        .expect("function symbol");
    let SymbolImpl::Signature(fold_sig) = &fold_symbol.imp else {
        panic!("expected Fold signature");
    };
    let fold_sig = env.signatures.get(*fold_sig).expect("Fold signature");
    assert_eq!(
        fold_sig.return_type,
        Some(VarTypeRef::Builtin(BuiltinType::Long))
    );
    assert_eq!(
        fold_sig.params[0].ty,
        VarTypeRef::Builtin(BuiltinType::Long)
    );

    let count = env.resolve(&ctx, "Count").expect("property resolves");
    let count_symbol = env
        .symbols
        .symbol(count.symbol.expect("property symbol"))
        .expect("property symbol");
    let SymbolImpl::Property(group) = &count_symbol.imp else {
        panic!("expected Count property");
    };
    let get_sig = env
        .signatures
        .get(group.get.expect("property get signature"))
        .expect("property get signature");
    assert_eq!(
        get_sig.return_type,
        Some(VarTypeRef::Builtin(BuiltinType::Long))
    );

    let fetch = env.resolve(&ctx, "Fetch").expect("Declare resolves");
    let fetch_symbol = env
        .symbols
        .symbol(fetch.symbol.expect("Declare symbol"))
        .expect("Declare symbol");
    let SymbolImpl::Declare(fetch_decl) = &fetch_symbol.imp else {
        panic!("expected Declare symbol");
    };
    assert_eq!(fetch_decl.return_type, Some(DeclareParamType::Long));
}

#[test]
fn scanner_honors_type_precedence_over_deftype() {
    let src = "DefLng A-Z\r\n\
               Public implicitValue\r\n\
               Public suffixedValue$\r\n\
               Public explicitValue As Double\r\n\
               Function Suffixed$()\r\n\
               End Function\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);

    let type_of = |name: &str| {
        let binding = env.resolve(&ctx, name).expect("resolves");
        match &env
            .symbols
            .symbol(binding.symbol.expect("symbol"))
            .expect("symbol")
            .imp
        {
            SymbolImpl::DeclaredType(ty) => ty.clone(),
            other => panic!("expected declared type, got {other:?}"),
        }
    };
    assert_eq!(
        type_of("implicitValue"),
        VarTypeRef::Builtin(BuiltinType::Long)
    );
    assert_eq!(
        type_of("suffixedValue"),
        VarTypeRef::Builtin(BuiltinType::String)
    );
    assert_eq!(
        type_of("explicitValue"),
        VarTypeRef::Builtin(BuiltinType::Double)
    );

    let binding = env.resolve(&ctx, "Suffixed").expect("function resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("function symbol"))
        .expect("function symbol");
    let SymbolImpl::Signature(sig) = &symbol.imp else {
        panic!("expected signature");
    };
    assert_eq!(
        env.signatures.get(*sig).expect("signature").return_type,
        Some(VarTypeRef::Builtin(BuiltinType::String))
    );
}

#[test]
fn scanner_rejects_duplicate_deftype_letter_ranges() {
    let src = "DefLng A-C\r\nDefStr C-D\r\nSub Main()\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("overlapping DefType ranges should fail"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        SymbolModelError::DuplicateDefTypeLetter { letter: 'C' }
    ));
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-DUPLICATE-DEFTYPE-RANGE"
    );
}

#[test]
fn scanner_rejects_deftype_after_a_z_range() {
    let src = "DefLng A-Z\r\nDefStr S\r\nSub Main()\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("A-Z DefType should reject later subranges"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        SymbolModelError::DuplicateDefTypeLetter { letter: 'S' }
    ));
}

#[test]
fn scanner_rejects_defdec_declared_decimal_storage() {
    let src = "DefDec A-Z\r\nSub Main()\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("DefDec should not be silently ignored"),
        Err(err) => err,
    };
    assert!(matches!(err, SymbolModelError::UnsupportedDefDec));
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-UNSUPPORTED-DEFDEC"
    );
}

#[test]
fn scanner_rejects_explicit_declared_decimal_storage() {
    let src = "Sub Main()\r\n    Dim alpha As Decimal\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("As Decimal should not become an object type"),
        Err(err) => err,
    };
    assert!(matches!(err, SymbolModelError::UnsupportedDeclaredDecimal));
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-UNSUPPORTED-DECLARED-DECIMAL"
    );
}

#[test]
fn scanner_allows_qualified_decimal_type_reference() {
    let src = "Public alpha As SomeLib.Decimal\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);
    let alpha = env.resolve(&ctx, "alpha").expect("alpha resolves");
    let alpha_symbol = env
        .symbols
        .symbol(alpha.symbol.expect("alpha symbol"))
        .expect("alpha symbol");
    assert_eq!(
        &alpha_symbol.imp,
        &SymbolImpl::DeclaredType(VarTypeRef::Object("somelib.decimal".into()))
    );
}

#[test]
fn scanner_rejects_option_compare_database_collation() {
    let src = "Option Compare Database\r\nSub Main()\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Option Compare Database should not use Binary as an approximation"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        SymbolModelError::UnsupportedOptionCompareDatabase
    ));
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-UNSUPPORTED-OPTION-COMPARE-DATABASE"
    );
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
    assert!(predeclared_object("Debug").is_some());
    // `Collection` is a class of the built-in `VBA` library bundle, not a
    // predeclared object — it resolves as a creatable extern coclass instead.
    assert!(predeclared_object("Collection").is_none());
    assert!(predeclared_object("NotAnObject").is_none());
}

#[test]
fn collection_resolves_as_vba_extern_coclass_and_members() {
    use crate::binding::DispatchRoute;
    use crate::provider::Provider;
    use crate::providers::vba_library::VbaLibraryProvider;
    use crate::signature::VarTypeRef;

    let p = VbaLibraryProvider;
    assert_eq!(
        p.resolve_extern_coclass("Collection"),
        Some(("VBA".to_string(), "Collection".to_string()))
    );
    let recv = VarTypeRef::Object("collection".to_string());
    let count = p
        .resolve_member(&recv, "Count", None)
        .expect("Count member");
    match count.route {
        DispatchRoute::ExternMember {
            unit,
            member,
            kind,
            has_receiver,
            ..
        } => {
            assert_eq!(unit, "VBA");
            assert_eq!(member, "Count");
            assert_eq!(kind, oxvba_bundle::ProjectMemberKind::PropertyGet);
            assert!(has_receiver);
        }
        other => panic!("expected ExternMember, got {other:?}"),
    }
    assert!(p.resolve_member(&recv, "Add", None).is_some());
    assert!(p.resolve_member(&recv, "Bogus", None).is_none());
}
