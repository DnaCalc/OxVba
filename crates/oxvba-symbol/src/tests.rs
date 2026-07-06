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

use crate::binding::{Binding, DispatchRoute, SpecialForm};
use crate::cond_comp::ConditionalCompilationTarget;
use crate::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
};
use crate::model::{
    ScopeKind, SymbolImpl, SymbolKind, SymbolModelError, SymbolNamespace, SymbolTable,
};
use crate::predeclared::predeclared_object;
use crate::provider::{Provider, ResolutionContext, TypeLibResolver, build_resolution_environment};
use crate::providers::com::ComTypeLibProvider;
use crate::providers::host::HostProvider;
use crate::providers::vba_library::VbaLibraryProvider;
use crate::signature::{BuiltinType, DefaultValue, PassingMode, VarTypeRef};

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

fn class_module(name: &str, source: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.into(),
        module_kind: ModuleKind::Class,
        attributes: ModuleAttributes::named(name),
        source: source.into(),
    }
}

fn manifest(name: &str, modules: Vec<ModuleUnit>) -> SymbolProjectManifest {
    manifest_with_target(name, modules, ConditionalCompilationTarget::default())
}

fn manifest_with_target(
    name: &str,
    modules: Vec<ModuleUnit>,
    target: ConditionalCompilationTarget,
) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: name.into(),
        project_kind: ProjectKind::Source,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: target,
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
        conditional_compilation_target: Default::default(),
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
fn module_qualified_const_values_honor_private_module_scope() {
    let m = manifest(
        "Proj",
        vec![
            module(
                "ModA",
                "Const Secret As Long = 7\n\
                 Private Const ExplicitSecret As Long = 9\n\
                 Public Const SameModule As Long = ModA.Secret + 1\n\
                 Public Const SameExplicit As Long = ModA.ExplicitSecret + 1\n",
            ),
            module(
                "ModB",
                "Public Const FromPrivate As Long = ModA.Secret + 1\n\
                 Public Const FromProjectPrivate As Long = Proj.ModA.Secret + 2\n\
                 Public Const FromExplicitPrivate As Long = ModA.ExplicitSecret + 3\n",
            ),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let val = |module: &str, name: &str| -> Option<CoreConst> {
        let b = env.resolve_qualified(&[module, name])?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("ModA", "SameModule"), Some(CoreConst::I32(8)));
    assert_eq!(val("ModA", "SameExplicit"), Some(CoreConst::I32(10)));
    assert_eq!(val("ModB", "FromPrivate"), None);
    assert_eq!(val("ModB", "FromProjectPrivate"), None);
    assert_eq!(val("ModB", "FromExplicitPrivate"), None);
    assert!(
        env.resolve_qualified(&["ModA", "Secret"]).is_none(),
        "Private Const should not publish through module-qualified lookup"
    );
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
fn const_values_fold_enum_member_references() {
    let m = manifest(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const FromQualified As Long = WebFormat.Json + 1\n\
             Public Const FromBare As Long = Json + 2\n\
             Public Const EnumSeed As Long = 4\n\
             Public Enum WebFormat\n  PlainText = EnumSeed\n  Json\nEnd Enum\n",
        )],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Mod1").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("PlainText"), Some(CoreConst::I32(4)));
    assert_eq!(val("Json"), Some(CoreConst::I32(5)));
    assert_eq!(val("FromQualified"), Some(CoreConst::I32(6)));
    assert_eq!(val("FromBare"), Some(CoreConst::I32(7)));
}

#[test]
fn const_values_fold_cross_module_enum_member_references() {
    let m = manifest(
        "Proj",
        vec![
            module(
                "Types",
                "Public Enum WebFormat\n  PlainText = 1\n  Json\nEnd Enum\n",
            ),
            module(
                "Mod1",
                "Public Const FromBare As Long = Json + 1\n\
                 Public Const FromQualified As Long = WebFormat.Json + 2\n\
                 Public Const FromModuleMember As Long = Types.Json + 3\n\
                 Public Const FromProjectQualified As Long = Proj.WebFormat.Json + 4\n",
            ),
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Mod1").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("FromBare"), Some(CoreConst::I32(3)));
    assert_eq!(val("FromQualified"), Some(CoreConst::I32(4)));
    assert_eq!(val("FromModuleMember"), Some(CoreConst::I32(5)));
    assert_eq!(val("FromProjectQualified"), Some(CoreConst::I32(6)));
}

#[test]
fn referenced_project_enum_member_consts_fold_through_export_surface() {
    use crate::manifest::{ProjectReference, ReferencedProjectManifest};

    let lib_mod = module(
        "LibMod",
        "Public Const KBase As Long = 10\n\
         Public Enum Color\n  Red = 1\n  Green\nEnd Enum\n",
    );
    let lib = ReferencedProjectManifest {
        project_name: "Lib".into(),
        project_kind: ProjectKind::Library,
        modules: vec![lib_mod],
    };
    let m = SymbolProjectManifest {
        project_name: "App".into(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module(
                "Main",
                "Public Const FromPlainConst As Long = KBase + 1\n\
                 Public Const FromBare As Long = Green + 2\n\
                 Public Const FromQualified As Long = Color.Green + 3\n\
                 Public Const FromProjectQualified As Long = Lib.Color.Green + 4\n",
            ),
            module(
                "Shadow",
                "Public Const Color As Long = 99\n\
                 Public Const FromShadowedQualified As Long = Color.Green + 5\n",
            ),
        ],
        references: vec![ProjectReference::Project {
            referenced_project_name: "Lib".into(),
        }],
        reference_projects: vec![lib],
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };

    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let main_scope = env.module_scope("Main").unwrap();
    let shadow_scope = env.module_scope("Shadow").unwrap();
    let val_at = |scope, name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(
        val_at(main_scope, "FromPlainConst"),
        Some(CoreConst::I32(11))
    );
    assert_eq!(val_at(main_scope, "FromBare"), Some(CoreConst::I32(4)));
    assert_eq!(val_at(main_scope, "FromQualified"), Some(CoreConst::I32(5)));
    assert_eq!(
        val_at(main_scope, "FromProjectQualified"),
        Some(CoreConst::I32(6))
    );
    assert_eq!(val_at(shadow_scope, "Color"), Some(CoreConst::I32(99)));
    assert_eq!(val_at(shadow_scope, "FromShadowedQualified"), None);
}

#[test]
fn referenced_option_private_enum_member_consts_do_not_leak() {
    use crate::manifest::{ProjectReference, ReferencedProjectManifest};

    let lib = ReferencedProjectManifest {
        project_name: "Lib".into(),
        project_kind: ProjectKind::Library,
        modules: vec![
            module(
                "Visible",
                "Public Enum PublicColor\n  VisibleGreen = 2\nEnd Enum\n",
            ),
            module(
                "Hidden",
                "Option Private Module\n\
                 Public Enum HiddenColor\n  HiddenRed = 7\nEnd Enum\n",
            ),
        ],
    };
    let m = SymbolProjectManifest {
        project_name: "App".into(),
        project_kind: ProjectKind::Source,
        modules: vec![module(
            "Main",
            "Public Const FromVisible As Long = VisibleGreen + 1\n\
             Public Const FromHiddenBare As Long = HiddenRed + 1\n\
             Public Const FromHiddenQualified As Long = HiddenColor.HiddenRed + 1\n\
             Public Const FromHiddenProjectQualified As Long = Lib.HiddenColor.HiddenRed + 1\n",
        )],
        references: vec![ProjectReference::Project {
            referenced_project_name: "Lib".into(),
        }],
        reference_projects: vec![lib],
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };

    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Main").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("FromVisible"), Some(CoreConst::I32(3)));
    assert_eq!(val("FromHiddenBare"), None);
    assert_eq!(val("FromHiddenQualified"), None);
    assert_eq!(val("FromHiddenProjectQualified"), None);
    assert!(
        env.resolve(&ResolutionContext::at(scope), "HiddenRed")
            .is_none(),
        "Option Private Module enum member should not publish through the referenced surface"
    );
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

#[test]
fn longlong_const_comparisons_preserve_integer_precision() {
    let m = manifest(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const BigA As LongLong = 9007199254740993^\n\
             Public Const BigB As LongLong = 9007199254740992^\n\
             Public Const Different As Boolean = BigA <> BigB\n\
             Public Const Ordered As Boolean = BigA > BigB And BigB < BigA\n\
             Public Const Inclusive As Boolean = BigA >= BigB And BigB <= BigA\n\
             Public Const Same As Boolean = BigA = BigB\n",
        )],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).unwrap();
    let scope = env.module_scope("Mod1").unwrap();
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ResolutionContext::at(scope), name)?;
        env.const_value(b.symbol?).cloned()
    };
    assert_eq!(val("Different"), Some(CoreConst::Bool(true)));
    assert_eq!(val("Ordered"), Some(CoreConst::Bool(true)));
    assert_eq!(val("Inclusive"), Some(CoreConst::Bool(true)));
    assert_eq!(val("Same"), Some(CoreConst::Bool(false)));
}

#[test]
fn longptr_const_values_follow_target_width() {
    let src = "Public Const CMax As LongPtr = 2147483647\n\
               Public Const CTextMax As LongPtr = \"2147483647\"\n";
    let win32 = manifest_with_target(
        "Proj",
        vec![module("Mod1", src)],
        ConditionalCompilationTarget::windows_32_vba7(),
    );
    let env32 = build_resolution_environment(&win32, &NullTypeLibs).unwrap();
    let scope32 = env32.module_scope("Mod1").unwrap();
    let val32 = |name: &str| -> Option<CoreConst> {
        let b = env32.resolve(&ResolutionContext::at(scope32), name)?;
        env32.const_value(b.symbol?).cloned()
    };
    assert_eq!(val32("CMax"), Some(CoreConst::I32(2_147_483_647)));
    assert_eq!(val32("CTextMax"), Some(CoreConst::I32(2_147_483_647)));

    let win64 = manifest_with_target(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const CMax As LongPtr = 2147483647\n\
             Public Const CTextMax As LongPtr = \"2147483647\"\n\
             Public Const CAbove As LongPtr = 2147483648\n",
        )],
        ConditionalCompilationTarget::windows_64_vba7(),
    );
    let env64 = build_resolution_environment(&win64, &NullTypeLibs).unwrap();
    let scope64 = env64.module_scope("Mod1").unwrap();
    let val64 = |name: &str| -> Option<CoreConst> {
        let b = env64.resolve(&ResolutionContext::at(scope64), name)?;
        env64.const_value(b.symbol?).cloned()
    };
    assert_eq!(val64("CMax"), Some(CoreConst::I64(2_147_483_647)));
    assert_eq!(val64("CTextMax"), Some(CoreConst::I64(2_147_483_647)));
    assert_eq!(val64("CAbove"), Some(CoreConst::I64(2_147_483_648)));
}

#[test]
fn win32_longptr_const_above_long_rejects() {
    let m = manifest_with_target(
        "Proj",
        vec![module(
            "Mod1",
            "Public Const CAbove As LongPtr = 2147483648\n",
        )],
        ConditionalCompilationTarget::windows_32_vba7(),
    );
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Win32 LongPtr Const above Long max should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::InvalidConstValue { name } if name == "CAbove"
        ),
        "unexpected error: {err:?}"
    );
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
            parameter_types: vec![TypeLibParamType::Variant],
            dispatch_path: TypeLibEventDispatchPath::SourceInterface,
            connection_point_iid: Some("{iid}".into()),
            dispatch_member_id: Some(9),
            coclass: None,
        }],
        coclass_names: Vec::new(),
    }
}

fn widget_put_before_get_default_blob() -> TypeLibMetadataBlob {
    let mut blob = widget_blob();
    let getter = blob
        .members
        .iter()
        .find(|member| {
            member.name == "Item" && member.invoke_kind == TypeLibMemberInvokeKind::PropertyGet
        })
        .expect("Item property get")
        .clone();
    let mut putter = getter.clone();
    putter.invoke_kind = TypeLibMemberInvokeKind::PropertyPut;
    putter.requires_argument = true;
    putter.parameter_names = vec!["Index".into(), "Value".into()];
    putter.parameter_optional = vec![false, false];
    putter.parameter_optional_defaults = Vec::new();
    putter.parameter_types = vec![TypeLibParamType::Long, TypeLibParamType::Variant];
    putter.parameter_wire_types = vec![
        TypeLibWireType::Automation(TypeLibParamType::Long),
        TypeLibWireType::Automation(TypeLibParamType::Variant),
    ];
    putter.parameter_iids = vec![None, None];
    putter.return_type = None;
    putter.return_wire_type = None;
    blob.members = vec![putter, getter];
    blob
}

fn assert_com_accessor(
    binding: Binding,
    expected_kind: ProjectMemberKind,
    expected_invoke: TypeLibMemberInvokeKind,
) {
    match binding.route {
        DispatchRoute::ComMember {
            member_kind,
            invoke_kind,
            member,
            ..
        } => {
            assert_eq!(member_kind, expected_kind);
            assert_eq!(invoke_kind, expected_invoke);
            assert_eq!(member.invoke_kind, expected_invoke);
        }
        other => panic!("expected ComMember, got {other:?}"),
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
fn com_default_member_accessor_selection_ignores_typelib_order() {
    let typed = VarTypeRef::Object("Widget".into());
    let provider = ComTypeLibProvider::new(widget_put_before_get_default_blob());

    assert_com_accessor(
        provider
            .resolve_member(&typed, "Item", None)
            .expect("read lookup"),
        ProjectMemberKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyGet,
    );
    assert_com_accessor(
        provider
            .resolve_member(&typed, "Item", Some(ProjectMemberKind::PropertyLet))
            .expect("write lookup"),
        ProjectMemberKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPut,
    );
    assert_com_accessor(
        provider
            .resolve_default_member(&typed)
            .expect("default read lookup"),
        ProjectMemberKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyGet,
    );
    assert_com_accessor(
        provider
            .resolve_default_member_kind(&typed, Some(ProjectMemberKind::PropertyLet))
            .expect("default write lookup"),
        ProjectMemberKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPut,
    );

    let host = HostProvider::new(vec![widget_put_before_get_default_blob()]);
    assert_com_accessor(
        host.resolve_default_member_kind(&typed, Some(ProjectMemberKind::PropertyLet))
            .expect("host default write lookup"),
        ProjectMemberKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPut,
    );
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
    // A flat library-level member list is not scoped enough to answer members for
    // a specific coclass. The full environment adds scoped providers for used COM
    // types before member binding.
    assert!(
        provider
            .resolve_member(
                &VarTypeRef::Object("Dictionary".into()),
                "DoThing",
                Some(ProjectMemberKind::Method),
            )
            .is_none()
    );
    assert!(
        provider
            .resolve_member(
                &VarTypeRef::Object("Scripting.Dictionary".into()),
                "DoThing",
                Some(ProjectMemberKind::Method),
            )
            .is_none()
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

#[test]
fn com_events_scope_to_receiver_coclass_without_library_fallback() {
    let mut blob = widget_blob();
    blob.identity.reference_name = "Excel".into();
    blob.identity.requested_coclass = None;
    blob.activation_prog_id = None;
    blob.coclass_names = vec!["Application".into(), "Workbook".into()];
    blob.events = vec![TypeLibEventMetadata {
        name: "NewWorkbook".into(),
        token: 1565,
        callback_arity: 1,
        parameter_types: vec![TypeLibParamType::Object],
        dispatch_path: TypeLibEventDispatchPath::Dispatch,
        connection_point_iid: Some("{app-events}".into()),
        dispatch_member_id: Some(1565),
        coclass: Some("Application".into()),
    }];
    let provider = ComTypeLibProvider::new(blob);
    let app = VarTypeRef::Object("Excel.Application".into());
    let workbook = VarTypeRef::Object("Excel.Workbook".into());

    assert_eq!(
        provider.source_events(&app),
        Some(vec![("newworkbook".into(), 1565)])
    );
    assert_eq!(provider.source_events(&workbook), Some(Vec::new()));
    assert!(
        provider
            .resolve_member(&workbook, "NewWorkbook", None)
            .is_none(),
        "Workbook must not inherit Application events through the library-wide event list"
    );
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
fn context_member_resolution_honors_private_class_visibility() {
    let m = manifest(
        "Proj",
        vec![
            module("Main", "Sub Run()\r\nEnd Sub\r\n"),
            ModuleUnit {
                module_name: "Widget".into(),
                module_kind: ModuleKind::Class,
                attributes: ModuleAttributes::named("Widget"),
                source: "Private Function Secret() As Long\r\nEnd Function\r\n\
                         Friend Function FriendValue() As Long\r\nEnd Function\r\n\
                         Public Function Pub() As Long\r\nEnd Function\r\n"
                    .into(),
            },
        ],
    );
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let main_scope = env.module_scope("Main").expect("main scope");
    let widget_scope = env.module_scope("Widget").expect("widget scope");
    let recv = VarTypeRef::Object("Widget".to_string());

    assert!(
        env.resolve_member_from_scope(main_scope, &recv, "Secret", None)
            .is_none(),
        "Private class member must not bind from another module"
    );
    assert!(
        env.resolve_member_from_scope(widget_scope, &recv, "Secret", None)
            .is_some(),
        "Private class member remains visible to its declaring class"
    );
    assert!(
        env.resolve_member_from_scope(main_scope, &recv, "FriendValue", None)
            .is_some(),
        "Friend class members remain project-visible"
    );
    assert!(
        env.resolve_member_from_scope(main_scope, &recv, "Pub", None)
            .is_some(),
        "Public class members remain project-visible"
    );
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
fn duplicate_property_accessors_reject() {
    let src = "Property Get Foo() As Long\r\nEnd Property\r\n\
               Property Get Foo() As Long\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("duplicate Property Get accessor should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::DuplicatePropertyAccessor {
                property,
                accessor,
            } if property == "Foo" && *accessor == "Get"
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-DUPLICATE-PROPERTY-ACCESSOR"
    );
}

#[test]
fn signature_declarations_reject_as_new_types() {
    for (src, name, context) in [
        (
            "Sub Use(ByVal value As New Widget)\r\nEnd Sub\r\n",
            "value",
            "parameter",
        ),
        (
            "Function Make() As New Widget\r\nEnd Function\r\n",
            "Make",
            "return type",
        ),
        (
            "Property Get Item() As New Widget\r\nEnd Property\r\n",
            "Item",
            "return type",
        ),
        (
            "Declare PtrSafe Function Fetch Lib \"h\" () As New Widget\r\n",
            "Fetch",
            "return type",
        ),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("{src} should reject signature As New"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidAsNewDeclaration {
                    name: actual_name,
                    context: actual_context,
                } if actual_name == name && *actual_context == context
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-AS-NEW-DECLARATION"
        );
    }
}

#[test]
fn type_block_fields_reject_as_new_types() {
    let src = "Private Type Payload\r\n    Item As New Widget\r\nEnd Type\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Type field As New should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::InvalidAsNewDeclaration { name, context }
                if name == "Item" && *context == "Type field"
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-INVALID-AS-NEW-DECLARATION"
    );
}

#[test]
fn type_block_fields_reject_duplicate_names() {
    let src = "Private Type Payload\r\n    Item As Long\r\n    item As String\r\nEnd Type\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("duplicate Type field should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::DuplicateTypeField { type_name, field }
                if type_name == "Payload" && field == "item"
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-DUPLICATE-TYPE-FIELD"
    );
}

#[test]
fn object_module_type_blocks_must_be_private() {
    for src in [
        "Public Type Payload\r\n    Item As Long\r\nEnd Type\r\n",
        "Type Payload\r\n    Item As Long\r\nEnd Type\r\n",
    ] {
        let m = manifest("Proj", vec![class_module("Widget", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("public/default-public Type in class module should reject"),
            Err(err) => err,
        };
        assert!(matches!(
            &err,
            SymbolModelError::PublicTypeNotValidInObjectModule { name } if name == "Payload"
        ));
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-PUBLIC-TYPE-NOT-VALID-IN-OBJECT-MODULE"
        );
    }

    let private_src = "Private Type Payload\r\n    Item As Long\r\nEnd Type\r\n";
    let m = manifest("Proj", vec![class_module("Widget", private_src)]);
    build_resolution_environment(&m, &NullTypeLibs)
        .expect("private Type block should remain accepted in class modules");
}

#[test]
fn public_declare_is_not_valid_in_object_modules() {
    for src in [
        "Public Declare PtrSafe Sub Host Lib \"kernel32\" ()\r\n",
        "Declare PtrSafe Sub Host Lib \"kernel32\" ()\r\n",
    ] {
        let m = manifest("Proj", vec![class_module("Widget", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("public/implicit-public Declare in class module should reject"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            SymbolModelError::PublicDeclareNotValidInObjectModule {
                name: "Host".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-PUBLIC-DECLARE-NOT-VALID-IN-OBJECT-MODULE"
        );
    }

    let private_src = "Private Declare PtrSafe Sub Host Lib \"kernel32\" ()\r\n";
    let m = manifest("Proj", vec![class_module("Widget", private_src)]);
    build_resolution_environment(&m, &NullTypeLibs)
        .expect("private Declare should remain accepted in class modules");

    let public_standard_src = "Public Declare PtrSafe Sub Host Lib \"kernel32\" ()\r\n";
    let m = manifest("Proj", vec![module("Mod1", public_standard_src)]);
    build_resolution_environment(&m, &NullTypeLibs)
        .expect("public Declare should remain accepted in standard modules");

    let implicit_public_standard_src = "Declare PtrSafe Sub Host Lib \"kernel32\" ()\r\n";
    let m = manifest("Proj", vec![module("Mod1", implicit_public_standard_src)]);
    build_resolution_environment(&m, &NullTypeLibs)
        .expect("implicit-public Declare should remain accepted in standard modules");
}

#[test]
fn property_get_let_pairing_accepts_matching_accessors_in_any_order() {
    for src in [
        "Property Get Foo(ByVal index As Long) As String\r\nEnd Property\r\n\
         Property Let Foo(ByRef index As Long, ByRef value As String)\r\nEnd Property\r\n",
        "Property Let Foo(ByRef index As Long, ByRef value As String)\r\nEnd Property\r\n\
         Property Get Foo(ByVal index As Long) As String\r\nEnd Property\r\n",
    ] {
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
        let SymbolImpl::Property(group) = &symbol.imp else {
            panic!("expected Property group");
        };
        assert!(group.get.is_some(), "Get accessor should publish");
        assert!(group.let_.is_some(), "Let accessor should publish");
    }
}

#[test]
fn property_get_let_pairing_rejects_mismatches() {
    for (src, reason) in [
        (
            "Property Get Foo(ByVal index As Long) As String\r\nEnd Property\r\n\
             Property Let Foo(ByRef value As String)\r\nEnd Property\r\n",
            "Property Let must have the Property Get index parameters plus one final value parameter",
        ),
        (
            "Property Get Foo(ByVal index As Long) As String\r\nEnd Property\r\n\
             Property Let Foo(ByRef key As Long, ByRef value As String)\r\nEnd Property\r\n",
            "Property Let index parameter names must match Property Get",
        ),
        (
            "Property Get Foo(ByVal index As Long) As String\r\nEnd Property\r\n\
             Property Let Foo(ByRef index As String, ByRef value As String)\r\nEnd Property\r\n",
            "Property Let index parameter types must match Property Get",
        ),
        (
            "Property Get Foo(ByVal index As Long) As Long\r\nEnd Property\r\n\
             Property Let Foo(ByRef index As Long, ByRef value As String)\r\nEnd Property\r\n",
            "Property Let value parameter type must match Property Get return type",
        ),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("mismatched Property Get/Let pair should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::IncompatiblePropertyAccessor {
                    property,
                    accessor,
                    reason: actual_reason,
                } if property == "Foo" && *accessor == "Let" && *actual_reason == reason
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INCOMPATIBLE-PROPERTY-ACCESSOR"
        );
    }
}

#[test]
fn property_get_set_pairing_accepts_matching_index_accessors_in_any_order() {
    for src in [
        "Property Get Foo(ByVal index As Long) As Widget\r\nEnd Property\r\n\
         Property Set Foo(ByRef target As Long, ByRef value As Object)\r\nEnd Property\r\n",
        "Property Set Foo(ByRef target As Long, ByRef value As Object)\r\nEnd Property\r\n\
         Property Get Foo(ByVal index As Long) As Widget\r\nEnd Property\r\n",
    ] {
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
        let SymbolImpl::Property(group) = &symbol.imp else {
            panic!("expected Property group");
        };
        assert!(group.get.is_some(), "Get accessor should publish");
        assert!(group.set.is_some(), "Set accessor should publish");
    }
}

#[test]
fn property_get_set_pairing_rejects_index_mismatches() {
    for (src, reason) in [
        (
            "Property Get Foo(ByVal index As Long) As Widget\r\nEnd Property\r\n\
             Property Set Foo(ByRef value As Object)\r\nEnd Property\r\n",
            "Property Set must have the Property Get index parameters plus one final reference parameter",
        ),
        (
            "Property Get Foo(ByVal index As Long) As Widget\r\nEnd Property\r\n\
             Property Set Foo(ByRef index As String, ByRef value As Object)\r\nEnd Property\r\n",
            "Property Set index parameter types must match Property Get",
        ),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("mismatched Property Get/Set pair should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::IncompatiblePropertyAccessor {
                    property,
                    accessor,
                    reason: actual_reason,
                } if property == "Foo" && *accessor == "Set" && *actual_reason == reason
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INCOMPATIBLE-PROPERTY-ACCESSOR"
        );
    }
}

#[test]
fn property_let_set_pairing_rejects_index_mismatch_without_get() {
    let src = "Property Let Foo(ByRef index As Long, ByRef value As Long)\r\nEnd Property\r\n\
               Property Set Foo(ByRef index As String, ByRef value As Object)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("mismatched Property Let/Set pair should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::IncompatiblePropertyAccessor {
                property,
                accessor,
                reason,
            } if property == "Foo"
                && *accessor == "Set"
                && *reason == "Property Let and Property Set index parameter types must match"
        ),
        "unexpected error: {err:?}"
    );
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
fn event_declaration_rejects_standard_module() {
    let src = "Public Event Changed()\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("standard module Event declaration should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::EventNotValidInStandardModule { name } if name == "Changed"
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-EVENT-ONLY-VALID-IN-OBJECT-MODULE"
    );
}

#[test]
fn event_declaration_rejects_invalid_parameters() {
    for (src, parameter, reason, diagnostic) in [
        (
            "Public Event Changed(Optional ByVal value As Long)\r\n",
            "value",
            "Event arguments cannot be Optional",
            "SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION",
        ),
        (
            "Public Event Changed(ByVal value As Long = 1)\r\n",
            "value",
            "Event arguments cannot have default values",
            "SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION",
        ),
        (
            "Public Event Changed(ParamArray values() As Variant)\r\n",
            "values",
            "Event arguments cannot be ParamArray",
            "SYM-E-INVALID-PARAMARRAY-DECLARATION",
        ),
    ] {
        let m = manifest("Proj", vec![class_module("Source", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("invalid Event parameter declaration should reject"),
            Err(err) => err,
        };
        let matches_error = match &err {
            SymbolModelError::InvalidOptionalParameterDeclaration {
                procedure,
                parameter: actual_parameter,
                reason: actual_reason,
            }
            | SymbolModelError::InvalidParamArrayDeclaration {
                procedure,
                parameter: actual_parameter,
                reason: actual_reason,
            } => {
                procedure == "Changed" && actual_parameter == parameter && *actual_reason == reason
            }
            _ => false,
        };
        assert!(matches_error, "unexpected error: {err:?}");
        assert_eq!(err.to_diagnostic().code.as_str(), diagnostic);
    }
}

#[test]
fn event_declaration_rejects_as_new_parameters() {
    let src = "Public Event Changed(ByVal value As New Source)\r\n";
    let m = manifest("Proj", vec![class_module("Source", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Event As New parameter declaration should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::InvalidAsNewDeclaration { name, context }
                if name == "value" && *context == "Event argument"
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        err.to_diagnostic().code.as_str(),
        "SYM-E-INVALID-AS-NEW-DECLARATION"
    );
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
fn optional_longptr_defaults_follow_target_width() {
    let src = "Const CMax As LongPtr = 2147483647\r\n\
               Sub S(Optional ByVal literal As LongPtr = 2147483647, Optional ByVal fromConst As LongPtr = CMax)\r\n\
               End Sub\r\n";

    let win32 = manifest_with_target(
        "Proj",
        vec![module("Mod1", src)],
        ConditionalCompilationTarget::windows_32_vba7(),
    );
    let env32 = build_resolution_environment(&win32, &NullTypeLibs).expect("env");
    let scope32 = env32.module_scope("Mod1").expect("module scope");
    let binding32 = env32
        .resolve(&ResolutionContext::at(scope32), "S")
        .expect("proc resolves");
    let proc32 = binding32.symbol.expect("proc symbol");
    let symbol32 = env32.symbols.symbol(proc32).expect("symbol");
    let SymbolImpl::Signature(sig32) = &symbol32.imp else {
        panic!("expected signature");
    };
    let signature32 = env32.signatures.get(*sig32).expect("signature");
    assert_eq!(
        signature32.params[0].default,
        Some(DefaultValue::I32(2_147_483_647))
    );
    assert_eq!(
        env32.optional_default(proc32, 0),
        Some(&CoreConst::I32(2_147_483_647))
    );
    assert_eq!(
        env32.optional_default(proc32, 1),
        Some(&CoreConst::I32(2_147_483_647))
    );

    let win64 = manifest_with_target(
        "Proj",
        vec![module("Mod1", src)],
        ConditionalCompilationTarget::windows_64_vba7(),
    );
    let env64 = build_resolution_environment(&win64, &NullTypeLibs).expect("env");
    let scope64 = env64.module_scope("Mod1").expect("module scope");
    let binding64 = env64
        .resolve(&ResolutionContext::at(scope64), "S")
        .expect("proc resolves");
    let proc64 = binding64.symbol.expect("proc symbol");
    let symbol64 = env64.symbols.symbol(proc64).expect("symbol");
    let SymbolImpl::Signature(sig64) = &symbol64.imp else {
        panic!("expected signature");
    };
    let signature64 = env64.signatures.get(*sig64).expect("signature");
    assert_eq!(
        signature64.params[0].default,
        Some(DefaultValue::I64(2_147_483_647))
    );
    assert_eq!(
        env64.optional_default(proc64, 0),
        Some(&CoreConst::I64(2_147_483_647))
    );
    assert_eq!(
        env64.optional_default(proc64, 1),
        Some(&CoreConst::I64(2_147_483_647))
    );
}

#[test]
fn win32_longptr_optional_default_above_long_rejects() {
    let src = "Sub S(Optional ByVal p As LongPtr = 2147483648)\r\nEnd Sub\r\n";
    let m = manifest_with_target(
        "Proj",
        vec![module("Mod1", src)],
        ConditionalCompilationTarget::windows_32_vba7(),
    );
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Win32 LongPtr optional default above Long max should reject"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::InvalidOptionalDefault {
                procedure,
                parameter,
            } if procedure == "S" && parameter == "p"
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn invalid_optional_defaults_reject_instead_of_falling_back() {
    for (src, parameter) in [
        (
            "Sub S(Optional ByVal n As Long = \"abc\")\r\nEnd Sub\r\n",
            "n",
        ),
        (
            "Sub S(Optional ByVal wide As Long = 5000000000^)\r\nEnd Sub\r\n",
            "wide",
        ),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("invalid Optional default should not compile"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidOptionalDefault {
                    procedure,
                    parameter: actual,
                } if procedure == "S" && actual == parameter
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-OPTIONAL-DEFAULT"
        );
    }
}

#[test]
fn optional_object_defaults_accept_nothing_and_zero() {
    let src = "Sub S(Optional ByVal obj As Object = Nothing, Optional ByVal zeroObj As Object = 0)\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("proc resolves");
    let proc = binding.symbol.expect("proc symbol");
    assert_eq!(env.optional_default(proc, 0), Some(&CoreConst::Nothing));
    assert_eq!(env.optional_default(proc, 1), Some(&CoreConst::Nothing));
}

#[test]
fn optional_boolean_like_default_folds_charlists_and_ranges() {
    let src = "Sub S(Optional ByVal flag As Boolean = (\"f\" Like \"[a-z]\") And (\"9\" Like \"[0-9a-f]\") And (\"]\" Like \"[]x]\") And (\"F\" Like \"[!a-z]\"))\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("proc resolves");
    let proc = binding.symbol.expect("proc symbol");
    assert_eq!(env.optional_default(proc, 0), Some(&CoreConst::Bool(true)));
}

#[test]
fn scanner_rejects_invalid_paramarray_modifiers() {
    for (src, procedure, parameter, reason) in [
        (
            "Sub S(Optional ParamArray xs() As Variant)\r\nEnd Sub\r\n",
            "S",
            "xs",
            "ParamArray cannot be combined with Optional",
        ),
        (
            "Sub S(ByVal ParamArray xs() As Variant)\r\nEnd Sub\r\n",
            "S",
            "xs",
            "ParamArray cannot be combined with ByVal",
        ),
        (
            "Sub S(ByRef ParamArray xs() As Variant)\r\nEnd Sub\r\n",
            "S",
            "xs",
            "ParamArray cannot be combined with ByRef",
        ),
        (
            "Sub S(ParamArray xs() As Variant, ByVal tail As Long)\r\nEnd Sub\r\n",
            "S",
            "xs",
            "ParamArray must be the final parameter",
        ),
        (
            "Declare PtrSafe Sub Host Lib \"h\" (ByVal n As Long, Optional ParamArray xs() As Variant)\r\n",
            "Host",
            "xs",
            "ParamArray cannot be combined with Optional",
        ),
        (
            "Sub S(ParamArray xs() As Long)\r\nEnd Sub\r\n",
            "S",
            "xs",
            "ParamArray must be an array of Variant elements",
        ),
        (
            "Sub S(ParamArray xs As Variant)\r\nEnd Sub\r\n",
            "S",
            "xs",
            "ParamArray must be an array of Variant elements",
        ),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("{src} should reject invalid ParamArray declaration"),
            Err(err) => err,
        };
        assert!(matches!(
            err,
            SymbolModelError::InvalidParamArrayDeclaration {
                procedure: ref actual_procedure,
                parameter: ref actual_parameter,
                reason: actual_reason,
            } if actual_procedure == procedure
                && actual_parameter == parameter
                && actual_reason == reason
        ));
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-PARAMARRAY-DECLARATION"
        );
    }
}

#[test]
fn scanner_rejects_required_parameters_after_optional() {
    for (module_kind, src, procedure, parameter) in [
        (
            ModuleKind::Procedural,
            "Sub S(Optional ByVal first As Long, ByVal second As Long)\r\nEnd Sub\r\n",
            "S",
            "second",
        ),
        (
            ModuleKind::Procedural,
            "Property Get Item(Optional ByVal index As Long, ByVal tail As Long) As Long\r\nEnd Property\r\n",
            "Item",
            "tail",
        ),
        (
            ModuleKind::Procedural,
            "Declare PtrSafe Sub Host Lib \"h\" (Optional ByVal first As Long, ByVal second As Long)\r\n",
            "Host",
            "second",
        ),
        (
            ModuleKind::Procedural,
            "Sub S(Optional ByVal first As Long, ParamArray rest() As Variant)\r\nEnd Sub\r\n",
            "S",
            "rest",
        ),
    ] {
        let module = ModuleUnit {
            module_name: "Mod1".into(),
            module_kind,
            attributes: ModuleAttributes::named("Mod1"),
            source: src.into(),
        };
        let m = manifest("Proj", vec![module]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("{src} should reject required parameter after Optional"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidOptionalParameterDeclaration {
                    procedure: actual_procedure,
                    parameter: actual_parameter,
                    reason,
                } if actual_procedure == procedure
                    && actual_parameter == parameter
                    && *reason == "required parameters cannot follow Optional parameters"
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION"
        );
    }
}

#[test]
fn property_let_allows_required_value_after_optional_index_args() {
    let src =
        "Property Let Item(Optional ByVal index As Long, ByVal value As Long)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "Item")
        .expect("property resolves");
    let symbol = env
        .symbols
        .symbol(binding.symbol.expect("symbol id"))
        .expect("symbol");
    let SymbolImpl::Property(group) = &symbol.imp else {
        panic!("expected Property group");
    };
    assert!(group.let_.is_some(), "Property Let accessor should publish");
}

#[test]
fn property_writers_require_final_value_or_reference_parameter() {
    for (src, accessor) in [
        ("Property Let Item()\r\nEnd Property\r\n", "Let"),
        ("Property Set Item()\r\nEnd Property\r\n", "Set"),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("Property {accessor} without writer parameter should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::MissingPropertyWriterParameter {
                    procedure,
                    accessor: actual_accessor,
                } if procedure == "Item" && *actual_accessor == accessor
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-MISSING-PROPERTY-WRITER-PARAMETER"
        );
    }
}

#[test]
fn property_writer_final_parameter_is_published_byval() {
    let src = "Property Let Item(ByRef index As Long, ByRef value As Long)\r\nEnd Property\r\n\
               Property Set RefItem(ByRef index As Long, ByRef value As Object)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);

    let item_binding = env.resolve(&ctx, "Item").expect("Item property resolves");
    let item_symbol = env
        .symbols
        .symbol(item_binding.symbol.expect("Item symbol"))
        .expect("Item symbol");
    let SymbolImpl::Property(item_group) = &item_symbol.imp else {
        panic!("expected Item property group");
    };
    let item_sig = env
        .signatures
        .get(item_group.let_.expect("Item Let signature"))
        .expect("Item Let signature");
    assert_eq!(item_sig.params[0].mode, PassingMode::ByRef);
    assert_eq!(item_sig.params[1].mode, PassingMode::ByVal);

    let ref_binding = env
        .resolve(&ctx, "RefItem")
        .expect("RefItem property resolves");
    let ref_symbol = env
        .symbols
        .symbol(ref_binding.symbol.expect("RefItem symbol"))
        .expect("RefItem symbol");
    let SymbolImpl::Property(ref_group) = &ref_symbol.imp else {
        panic!("expected RefItem property group");
    };
    let ref_sig = env
        .signatures
        .get(ref_group.set.expect("RefItem Set signature"))
        .expect("RefItem Set signature");
    assert_eq!(ref_sig.params[0].mode, PassingMode::ByRef);
    assert_eq!(ref_sig.params[1].mode, PassingMode::ByVal);
}

#[test]
fn property_let_value_parameter_cannot_be_optional() {
    let src = "Property Let Item(Optional ByVal value As Long)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Property Let value parameter should reject Optional"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::InvalidOptionalParameterDeclaration {
                procedure,
                parameter,
                reason,
            } if procedure == "Item"
                && parameter == "value"
                && *reason == "Property Let value parameter cannot be Optional"
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn property_set_reference_parameter_cannot_be_optional() {
    let src = "Property Set Item(Optional value As Object)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let err = match build_resolution_environment(&m, &NullTypeLibs) {
        Ok(_) => panic!("Property Set reference parameter should reject Optional"),
        Err(err) => err,
    };
    assert!(
        matches!(
            &err,
            SymbolModelError::InvalidOptionalParameterDeclaration {
                procedure,
                parameter,
                reason,
            } if procedure == "Item"
                && parameter == "value"
                && *reason == "Property Set reference parameter cannot be Optional"
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn property_set_reference_parameter_must_be_object_compatible() {
    for src in [
        "Property Set Item(ByVal value As Long)\r\nEnd Property\r\n",
        "Property Set Item(ByVal value As String)\r\nEnd Property\r\n",
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("Property Set scalar reference parameter should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidPropertySetReferenceParameter {
                    procedure,
                    parameter,
                } if procedure == "Item" && parameter == "value"
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-PROPERTY-SET-REFERENCE"
        );
    }
}

#[test]
fn property_set_reference_parameter_rejects_udt_types() {
    let cases = [
        manifest(
            "Proj",
            vec![module(
                "Mod1",
                "Public Type Payload\r\n    Value As Long\r\nEnd Type\r\n\
                 Property Set Item(ByVal value As Payload)\r\nEnd Property\r\n",
            )],
        ),
        manifest(
            "Proj",
            vec![
                module(
                    "Types",
                    "Public Type Payload\r\n    Value As Long\r\nEnd Type\r\n",
                ),
                module(
                    "Mod1",
                    "Property Set Item(ByVal value As Types.Payload)\r\nEnd Property\r\n",
                ),
            ],
        ),
        manifest(
            "Proj",
            vec![
                module(
                    "Types",
                    "Public Type Payload\r\n    Value As Long\r\nEnd Type\r\n",
                ),
                module(
                    "Mod1",
                    "Property Set Item(ByVal value As Payload)\r\nEnd Property\r\n",
                ),
            ],
        ),
    ];

    for m in cases {
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("Property Set UDT reference parameter should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidPropertySetReferenceParameter {
                    procedure,
                    parameter,
                } if procedure == "Item" && parameter == "value"
            ),
            "unexpected error: {err:?}"
        );
    }
}

#[test]
fn optional_parameter_rejects_udt_types() {
    let cases = [
        manifest(
            "Proj",
            vec![module(
                "Mod1",
                "Public Type Payload\r\n    Value As Long\r\nEnd Type\r\n\
                 Sub Use(Optional ByVal value As Payload)\r\nEnd Sub\r\n",
            )],
        ),
        manifest(
            "Proj",
            vec![
                module(
                    "Types",
                    "Public Type Payload\r\n    Value As Long\r\nEnd Type\r\n",
                ),
                module(
                    "Mod1",
                    "Sub Use(Optional ByVal value As Types.Payload)\r\nEnd Sub\r\n",
                ),
            ],
        ),
        manifest(
            "Proj",
            vec![
                module(
                    "Types",
                    "Public Type Payload\r\n    Value As Long\r\nEnd Type\r\n",
                ),
                module(
                    "Mod1",
                    "Sub Use(Optional ByVal value As Payload)\r\nEnd Sub\r\n",
                ),
            ],
        ),
    ];

    for m in cases {
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("Optional UDT parameter should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidOptionalParameterDeclaration {
                    procedure,
                    parameter,
                    reason,
                } if procedure == "Use"
                    && parameter == "value"
                    && *reason == "Optional parameters cannot be user-defined types"
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION"
        );
    }
}

#[test]
fn property_set_accepts_variant_object_and_class_reference_parameters() {
    let src = "Property Set DefaultItem(value)\r\nEnd Property\r\n\
               Property Set VariantItem(value As Variant)\r\nEnd Property\r\n\
               Property Set ObjectItem(value As Object)\r\nEnd Property\r\n\
               Property Set WidgetItem(value As Widget)\r\nEnd Property\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);

    for name in ["DefaultItem", "VariantItem", "ObjectItem", "WidgetItem"] {
        let binding = env.resolve(&ctx, name).expect("property resolves");
        let symbol = env
            .symbols
            .symbol(binding.symbol.expect("symbol"))
            .expect("symbol");
        let SymbolImpl::Property(group) = &symbol.imp else {
            panic!("expected Property group for {name}");
        };
        assert!(group.set.is_some(), "Property Set accessor should publish");
    }
}

#[test]
fn property_writer_final_parameter_cannot_be_paramarray() {
    for (src, reason) in [
        (
            "Property Let Item(ParamArray value() As Variant)\r\nEnd Property\r\n",
            "Property Let value parameter cannot be ParamArray",
        ),
        (
            "Property Set Item(ParamArray value() As Variant)\r\nEnd Property\r\n",
            "Property Set reference parameter cannot be ParamArray",
        ),
    ] {
        let m = manifest("Proj", vec![module("Mod1", src)]);
        let err = match build_resolution_environment(&m, &NullTypeLibs) {
            Ok(_) => panic!("property writer final ParamArray should reject"),
            Err(err) => err,
        };
        assert!(
            matches!(
                &err,
                SymbolModelError::InvalidParamArrayDeclaration {
                    procedure,
                    parameter,
                    reason: actual_reason,
                } if procedure == "Item"
                    && parameter == "value"
                    && *actual_reason == reason
            ),
            "unexpected error: {err:?}"
        );
    }
}

#[test]
fn scanner_accepts_implicit_variant_paramarray_array() {
    let src = "DefStr X-Z\r\nSub S(ParamArray xs())\r\nEnd Sub\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let binding = env
        .resolve(&ResolutionContext::at(scope), "S")
        .expect("proc resolves");
    let proc = binding.symbol.expect("proc symbol");
    let symbol = env.symbols.symbol(proc).expect("S symbol");
    let SymbolImpl::Signature(sig_id) = symbol.imp else {
        panic!("expected signature");
    };
    let signature = env.signatures.get(sig_id).expect("signature");
    assert!(signature.params[0].param_array);
    assert_eq!(
        signature.params[0].ty,
        VarTypeRef::Array(Box::new(VarTypeRef::Variant))
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
fn enum_initializers_do_not_auto_counter_invalid_explicit_values() {
    let src = "Public Enum LongBits\r\n\
                   AllBits = &HFFFFFFFF\r\n\
                   AfterBits\r\n\
               End Enum\r\n\
               Public Enum Fractional\r\n\
                   Bad = 1.5\r\n\
                   AfterBad\r\n\
               End Enum\r\n\
               Public Enum Wide\r\n\
                   TooWide = 5000000000^\r\n\
               End Enum\r\n";
    let m = manifest("Proj", vec![module("Mod1", src)]);
    let env = build_resolution_environment(&m, &NullTypeLibs).expect("env");
    let scope = env.module_scope("Mod1").expect("module scope");
    let ctx = ResolutionContext::at(scope);
    let val = |name: &str| -> Option<CoreConst> {
        let b = env.resolve(&ctx, name)?;
        env.const_value(b.symbol?).cloned()
    };

    assert_eq!(val("AllBits"), Some(CoreConst::I32(-1)));
    assert_eq!(val("AfterBits"), Some(CoreConst::I32(0)));
    assert_eq!(val("Bad"), None);
    assert_eq!(val("AfterBad"), None);
    assert_eq!(val("TooWide"), None);
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
