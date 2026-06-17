//! Project **export-surface synthesis** — the COM contract a project presents to
//! a referencing project (and, later, emits as a `.tlb`).
//!
//! A VBA project's public surface is a *set* of typelib types: each `Public`
//! creatable class is a **coclass**, each standard module is a hidden
//! `TKIND_MODULE` whose members resolve **unqualified** (the way the VBA library
//! exposes `Strings`/`Interaction`/… and the way VB6 `GlobalMultiUse` works), and
//! `Public Enum` members + `Public Const`s are global-namespace named constants.
//!
//! This one artifact is consumed two ways (the binding model is identical for
//! both, so swapping a source reference for a compiled COM component never changes
//! observable behaviour):
//!   * a *referencing* project binds against it (presented as a provider), and
//!   * the COM-server build emits it as a real type library.
//!
//! Synthesis is implemented to the COM/typelib + VBA 7.1 spec — it does **not**
//! depend on the legacy compiler's COM-export path. Each member carries its
//! originating [`SymbolId`] so the binder can devirtualise a cross-boundary call
//! to an in-image `ProcId`/`ClassId`, and a **dispid assigned once here** that the
//! runtime class descriptor reuses (so source- and compiled-form dispatch agree).

use std::collections::HashMap;

use oxvba_bundle::ProjectMemberKind;
use oxvba_bundle::coreir::CoreConst;
use oxvba_com::{TypeLibMemberInvokeKind, TypeLibParamType};

use crate::manifest::{Instancing, ModuleKind, ModuleUnit};
use crate::model::{SymbolId, SymbolImpl, SymbolKind, SymbolTable, Visibility};
use crate::scanner::{ModuleScan, ScannedMember};
use crate::signature::{BuiltinType, PassingMode, Signature, SignatureTable, VarTypeRef};

/// A project's complete public export surface.
#[derive(Debug, Clone)]
pub struct ProjectExportSurface {
    pub project_name: String,
    /// Coclasses + hidden modules, each with members (and a coclass's events).
    pub types: Vec<SurfaceType>,
    /// `Public Enum` members + `Public Const`s — global-namespace named constants
    /// (resolvable unqualified, like VBA enum members).
    pub consts: Vec<SurfaceConst>,
    /// Bare names of this project's interface types — every class that appears as
    /// an `Implements` target of some class. A referrer mangles dispatch on a
    /// receiver typed as one of these (`Interface_Member`).
    pub interfaces: Vec<String>,
}

/// One typelib type in the surface.
#[derive(Debug, Clone)]
pub struct SurfaceType {
    /// Display name (the class/module name).
    pub name: String,
    pub description: Option<String>,
    pub kind: SurfaceTypeKind,
    /// True if this type's members resolve **unqualified** in a referencing
    /// project: hidden `TKIND_MODULE`s and `GlobalMultiUse`/`GlobalSingleUse`
    /// classes inject their members into the referencer's global namespace.
    pub global_namespace: bool,
    /// True for a `VB_PredeclaredId = True` class (coclasses only): a referencing
    /// project reaches its global singleton by the class name (the `ThisWorkbook`
    /// document-module shape). Always `false` for a hidden module.
    pub predeclared: bool,
    pub members: Vec<SurfaceMember>,
    /// Source-interface events (coclasses only).
    pub events: Vec<SurfaceEvent>,
    /// Bare interface names this coclass `Implements` (coclasses only).
    pub implements: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SurfaceTypeKind {
    /// A `Public`/exposed class → a coclass that a referencing project can `New`.
    Coclass {
        prog_id: Option<String>,
        creatable: bool,
        /// The class module's type symbol — `New <coclass>` devirtualises through
        /// the binder's `class_of_sym` to this class's `ClassId`.
        class_symbol: SymbolId,
    },
    /// A standard module → a hidden `TKIND_MODULE`; members are global-namespace.
    Module,
}

/// One member (method / property accessor / field accessor) of a surface type.
#[derive(Debug, Clone)]
pub struct SurfaceMember {
    pub name: String,
    /// Dispatch id — assigned once here; the default member is `0`
    /// (`Attribute VB_UserMemId = 0`), the rest sequentially. The runtime class
    /// descriptor reuses these, so dispid dispatch agrees across forms.
    pub dispid: i32,
    /// Dual-interface vtable slot (after the 7 `IUnknown`+`IDispatch` slots).
    /// `None` for hidden-module members (modules are not vtable interfaces).
    pub vtable_slot: Option<u16>,
    pub invoke_kind: TypeLibMemberInvokeKind,
    /// The binder-facing dispatch kind (Method / PropertyGet / Let / Set).
    pub member_kind: ProjectMemberKind,
    pub is_default: bool,
    pub parameter_names: Vec<String>,
    pub parameter_types: Vec<TypeLibParamType>,
    pub parameter_optional: Vec<bool>,
    pub return_type: Option<TypeLibParamType>,
    /// The originating project symbol — the binder maps it to a `ProcId`
    /// (proc/property accessor) or a global place (field) for devirtualisation.
    pub symbol: SymbolId,
    pub origin: MemberOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberOrigin {
    /// A `Sub`/`Function` → its `ProcId` (module proc or class method).
    Proc,
    /// A `Property Get/Let/Set` accessor → its accessor `ProcId`.
    PropertyAccessor,
    /// A `Public` module/instance variable surfaced as a read/write property.
    Field,
}

#[derive(Debug, Clone)]
pub struct SurfaceEvent {
    pub name: String,
    pub callback_arity: u8,
    pub symbol: SymbolId,
    /// Per-coclass stable event id (source/scan order). The runtime routes a
    /// `RaiseEvent` by this id, and a sink's `WithEvents` routes are built from it,
    /// so it **must equal** the binder's `ids.event_index_of` for the source class.
    pub event_id: i32,
}

#[derive(Debug, Clone)]
pub struct SurfaceConst {
    pub name: String,
    /// `Public Const` vs an `Enum` member (both are global-namespace consts).
    pub symbol: SymbolId,
    /// The folded compile-time value — the published literal a referrer binds to.
    pub value: CoreConst,
    /// For an `Enum` member, the display name of its containing enum (so a referrer
    /// resolves `EnumName.Member`); `None` for a plain `Public Const`.
    pub enum_name: Option<String>,
}

/// Synthesize a project's export surface from its modules + scans.
///
/// `modules[i]` and `scans[i]` must describe the same module (parallel order, as
/// `build_resolution_environment` produces them).
pub fn synthesize_export_surface(
    project_name: &str,
    modules: &[ModuleUnit],
    scans: &[ModuleScan],
    symbols: &SymbolTable,
    signatures: &SignatureTable,
    const_values: &HashMap<SymbolId, CoreConst>,
) -> ProjectExportSurface {
    let mut types = Vec::new();
    let mut consts = Vec::new();
    // Project-wide interface set: every class named by some class's `Implements`.
    let mut interfaces: Vec<String> = Vec::new();
    for scan in scans {
        for iface in &scan.implements {
            if !interfaces.iter().any(|i| i.eq_ignore_ascii_case(iface)) {
                interfaces.push(iface.clone());
            }
        }
    }

    for (module, scan) in modules.iter().zip(scans.iter()) {
        let attrs = &module.attributes;
        // `Option Private Module` makes every Public member project-private — the
        // whole module is absent from the cross-project surface.
        if attrs.option_private_module || scan.option_private_module {
            continue;
        }
        let is_class = module.module_kind == ModuleKind::Class;
        // A class is only part of the COM surface if it is exposed.
        if is_class && !attrs.vb_exposed {
            collect_consts(scan, symbols, const_values, &mut consts);
            continue;
        }

        let kind = if is_class {
            SurfaceTypeKind::Coclass {
                prog_id: attrs.prog_id.clone(),
                creatable: attrs.vb_creatable || creatable_instancing(attrs.instancing),
                class_symbol: scan.module_symbol,
            }
        } else {
            SurfaceTypeKind::Module
        };
        let global_namespace = match kind {
            // Only a *standard* module's public procedures are callable unqualified
            // (injected into the referencer's global namespace). Document/Form/
            // Extension modules are class-like — their members are qualified-only.
            SurfaceTypeKind::Module => module.module_kind == ModuleKind::Procedural,
            // `GlobalMultiUse`/`GlobalSingleUse` classes inject members globally.
            SurfaceTypeKind::Coclass { .. } => global_instancing(attrs.instancing),
        };

        let mut members = Vec::new();
        let mut events = Vec::new();
        let mut next_dispid = 1i32;
        let mut next_vtable = 7u16;
        let mut next_event_id = 0i32;
        for member in &scan.members {
            // Events are assigned an id over the FULL event set in scan order — Public
            // AND Private — because the runtime/binder index (`ids.event_index_of`)
            // counts every `Event` regardless of visibility. The id must match for a
            // cross-bundle `WithEvents` sink to route correctly; only Public events are
            // then exposed in the surface.
            if member.kind == SymbolKind::Event {
                let event_id = next_event_id;
                next_event_id += 1;
                if member.visibility == Visibility::Public {
                    events.push(SurfaceEvent {
                        name: member_name(symbols, member),
                        callback_arity: event_arity(symbols, signatures, member.symbol),
                        symbol: member.symbol,
                        event_id,
                    });
                }
                continue;
            }
            if member.visibility != Visibility::Public {
                continue;
            }
            match member.kind {
                SymbolKind::Procedure | SymbolKind::Function => {
                    let dispid = take_dispid(member.is_default, &mut next_dispid);
                    members.push(method_member(
                        member,
                        dispid,
                        is_class,
                        &mut next_vtable,
                        symbols,
                        signatures,
                    ));
                }
                SymbolKind::Property => {
                    let dispid = take_dispid(member.is_default, &mut next_dispid);
                    property_members(
                        member,
                        dispid,
                        is_class,
                        &mut next_vtable,
                        symbols,
                        signatures,
                        &mut members,
                    );
                }
                // A public field surfaces as a read/write property pair. A
                // `WithEventsField` is Private-only in VBA (no `Public WithEvents`)
                // and is not an exportable accessor, so it never reaches the surface.
                SymbolKind::Field => {
                    let dispid = take_dispid(member.is_default, &mut next_dispid);
                    field_members(
                        member,
                        dispid,
                        is_class,
                        &mut next_vtable,
                        symbols,
                        &mut members,
                    );
                }
                _ => {}
            }
        }

        // A coclass publishes the interfaces it implements; a hidden module has none.
        let implements = if is_class {
            scan.implements.clone()
        } else {
            Vec::new()
        };
        types.push(SurfaceType {
            name: scan.module_name.clone(),
            description: attrs.description.clone(),
            kind,
            global_namespace,
            // Only an exposed class can be a predeclared coclass in the surface; a
            // hidden module is never predeclared.
            predeclared: is_class && attrs.vb_predeclared_id,
            members,
            events,
            implements,
        });
        collect_consts(scan, symbols, const_values, &mut consts);
    }

    ProjectExportSurface {
        project_name: project_name.to_string(),
        types,
        consts,
        interfaces,
    }
}

/// `Public Enum` members + `Public Const`s of a (non-private) module, each with its
/// folded value (the published literal) and, for an enum member, its enum's name.
fn collect_consts(
    scan: &ModuleScan,
    symbols: &SymbolTable,
    const_values: &HashMap<SymbolId, CoreConst>,
    out: &mut Vec<SurfaceConst>,
) {
    for member in &scan.members {
        if member.visibility != Visibility::Public {
            continue;
        }
        if matches!(member.kind, SymbolKind::Const | SymbolKind::EnumMember) {
            // A const that fails to fold publishes `Empty` (a referrer that reads a
            // missing value sees Empty rather than a binding failure).
            let value = const_values
                .get(&member.symbol)
                .cloned()
                .unwrap_or(CoreConst::Empty);
            out.push(SurfaceConst {
                name: member_name(symbols, member),
                symbol: member.symbol,
                value,
                enum_name: member.enum_name.clone(),
            });
        }
    }
}

fn take_dispid(is_default: bool, next: &mut i32) -> i32 {
    if is_default {
        0
    } else {
        let d = *next;
        *next += 1;
        d
    }
}

fn next_vtable_slot(is_class: bool, next: &mut u16) -> Option<u16> {
    if is_class {
        let s = *next;
        *next += 1;
        Some(s)
    } else {
        None
    }
}

fn method_member(
    member: &ScannedMember,
    dispid: i32,
    is_class: bool,
    next_vtable: &mut u16,
    symbols: &SymbolTable,
    signatures: &SignatureTable,
) -> SurfaceMember {
    let sig = member_signature(symbols, signatures, member.symbol);
    let (names, types, optional) = sig.map(param_lists).unwrap_or_default();
    let return_type = sig
        .and_then(|s| s.return_type.as_ref())
        .map(|t| param_type(t, false));
    SurfaceMember {
        name: member_name(symbols, member),
        dispid,
        vtable_slot: next_vtable_slot(is_class, next_vtable),
        invoke_kind: TypeLibMemberInvokeKind::Method,
        member_kind: ProjectMemberKind::Method,
        is_default: member.is_default,
        parameter_names: names,
        parameter_types: types,
        parameter_optional: optional,
        return_type,
        symbol: member.symbol,
        origin: MemberOrigin::Proc,
    }
}

fn property_members(
    member: &ScannedMember,
    dispid: i32,
    is_class: bool,
    next_vtable: &mut u16,
    symbols: &SymbolTable,
    signatures: &SignatureTable,
    out: &mut Vec<SurfaceMember>,
) {
    let name = member_name(symbols, member);
    let Some(SymbolImpl::Property(group)) = symbols.symbol(member.symbol).map(|s| &s.imp) else {
        return;
    };
    // Each present accessor is a member sharing the property's dispid.
    let accessors = [
        (
            group.get,
            TypeLibMemberInvokeKind::PropertyGet,
            ProjectMemberKind::PropertyGet,
        ),
        (
            group.let_,
            TypeLibMemberInvokeKind::PropertyPut,
            ProjectMemberKind::PropertyLet,
        ),
        (
            group.set,
            TypeLibMemberInvokeKind::PropertyPutRef,
            ProjectMemberKind::PropertySet,
        ),
    ];
    for (sig_id, invoke_kind, member_kind) in accessors {
        let Some(sig_id) = sig_id else { continue };
        let sig = signatures.get(sig_id);
        let (names, types, optional) = sig.map(param_lists).unwrap_or_default();
        let return_type = sig
            .and_then(|s| s.return_type.as_ref())
            .map(|t| param_type(t, false));
        out.push(SurfaceMember {
            name: name.clone(),
            dispid,
            vtable_slot: next_vtable_slot(is_class, next_vtable),
            invoke_kind,
            member_kind,
            is_default: member.is_default,
            parameter_names: names,
            parameter_types: types,
            parameter_optional: optional,
            return_type,
            symbol: member.symbol,
            origin: MemberOrigin::PropertyAccessor,
        });
    }
}

/// A `Public` variable surfaces as a read/write property pair.
fn field_members(
    member: &ScannedMember,
    dispid: i32,
    is_class: bool,
    next_vtable: &mut u16,
    symbols: &SymbolTable,
    out: &mut Vec<SurfaceMember>,
) {
    let name = member_name(symbols, member);
    let ty = match symbols.symbol(member.symbol).map(|s| &s.imp) {
        Some(SymbolImpl::DeclaredType(t)) => t.clone(),
        _ => VarTypeRef::Variant,
    };
    let is_object = matches!(ty, VarTypeRef::Object(_));
    // Read accessor.
    out.push(SurfaceMember {
        name: name.clone(),
        dispid,
        vtable_slot: next_vtable_slot(is_class, next_vtable),
        invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
        member_kind: ProjectMemberKind::PropertyGet,
        is_default: member.is_default,
        parameter_names: Vec::new(),
        parameter_types: Vec::new(),
        parameter_optional: Vec::new(),
        return_type: Some(param_type(&ty, false)),
        symbol: member.symbol,
        origin: MemberOrigin::Field,
    });
    // Write accessor — objects use Set (PutRef), scalars use Let (Put).
    let (invoke_kind, member_kind) = if is_object {
        (
            TypeLibMemberInvokeKind::PropertyPutRef,
            ProjectMemberKind::PropertySet,
        )
    } else {
        (
            TypeLibMemberInvokeKind::PropertyPut,
            ProjectMemberKind::PropertyLet,
        )
    };
    out.push(SurfaceMember {
        name,
        dispid,
        vtable_slot: next_vtable_slot(is_class, next_vtable),
        invoke_kind,
        member_kind,
        is_default: member.is_default,
        parameter_names: vec!["Value".to_string()],
        parameter_types: vec![param_type(&ty, false)],
        parameter_optional: vec![false],
        return_type: None,
        symbol: member.symbol,
        origin: MemberOrigin::Field,
    });
}

fn member_signature<'a>(
    symbols: &SymbolTable,
    signatures: &'a SignatureTable,
    sym: SymbolId,
) -> Option<&'a Signature> {
    match symbols.symbol(sym).map(|s| &s.imp) {
        Some(SymbolImpl::Signature(id)) => signatures.get(*id),
        _ => None,
    }
}

fn event_arity(symbols: &SymbolTable, signatures: &SignatureTable, sym: SymbolId) -> u8 {
    member_signature(symbols, signatures, sym)
        .map(|s| s.params.len() as u8)
        .unwrap_or(0)
}

#[allow(clippy::type_complexity)]
fn param_lists(sig: &Signature) -> (Vec<String>, Vec<TypeLibParamType>, Vec<bool>) {
    let mut names = Vec::with_capacity(sig.params.len());
    let mut types = Vec::with_capacity(sig.params.len());
    let mut optional = Vec::with_capacity(sig.params.len());
    for p in &sig.params {
        names.push(p.name.clone());
        types.push(param_type(&p.ty, p.mode == PassingMode::ByRef));
        optional.push(p.optional);
    }
    (names, types, optional)
}

fn member_name(symbols: &SymbolTable, member: &ScannedMember) -> String {
    symbols
        .symbol(member.symbol)
        .and_then(|s| symbols.name(s.name))
        .map(|n| n.first_spelling.clone())
        .unwrap_or_else(|| member.name_folded.clone())
}

fn creatable_instancing(instancing: Option<Instancing>) -> bool {
    matches!(
        instancing,
        Some(
            Instancing::MultiUse
                | Instancing::SingleUse
                | Instancing::GlobalMultiUse
                | Instancing::GlobalSingleUse
        )
    )
}

fn global_instancing(instancing: Option<Instancing>) -> bool {
    matches!(
        instancing,
        Some(Instancing::GlobalMultiUse | Instancing::GlobalSingleUse)
    )
}

/// Map a resolved VBA type to its typelib parameter type, wrapping in the `ByRef*`
/// variant when the parameter is passed by reference.
fn param_type(ty: &VarTypeRef, by_ref: bool) -> TypeLibParamType {
    use TypeLibParamType as T;
    let base = match ty {
        VarTypeRef::Builtin(b) => match b {
            BuiltinType::Boolean => T::Boolean,
            BuiltinType::Byte => T::Byte,
            BuiltinType::Integer => T::Integer,
            BuiltinType::Long => T::Long,
            BuiltinType::LongLong => T::LongLong,
            BuiltinType::LongPtr => T::LongPtr,
            BuiltinType::Single => T::Single,
            BuiltinType::Double => T::Double,
            BuiltinType::Currency => T::Currency,
            BuiltinType::Date => T::Date,
            BuiltinType::String => T::String,
        },
        VarTypeRef::FixedString(_) => T::String,
        VarTypeRef::Object(_) => T::Object,
        // A SAFEARRAY parameter marshals as a Variant at the COM boundary.
        VarTypeRef::Array(_) => T::Variant,
        // A UDT crossing the COM boundary marshals as a Variant (record export is out
        // of scope; in-project UDTs never reach here).
        VarTypeRef::Udt(_) => T::Variant,
        VarTypeRef::Variant => T::Variant,
    };
    if by_ref { to_by_ref(base) } else { base }
}

fn to_by_ref(t: TypeLibParamType) -> TypeLibParamType {
    use TypeLibParamType as T;
    match t {
        T::Variant => T::ByRefVariant,
        T::Long => T::ByRefLong,
        T::Integer => T::ByRefInteger,
        T::String => T::ByRefString,
        T::Boolean => T::ByRefBoolean,
        T::Double => T::ByRefDouble,
        T::Single => T::ByRefSingle,
        T::Currency => T::ByRefCurrency,
        T::Date => T::ByRefDate,
        T::Decimal => T::ByRefDecimal,
        T::Object => T::ByRefObject,
        T::Byte => T::ByRefByte,
        T::LongLong => T::ByRefLongLong,
        T::LongPtr => T::ByRefLongPtr,
        already_by_ref => already_by_ref,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ModuleAttributes, ModuleKind, ModuleUnit};
    use crate::model::ScopeKind;

    fn proc_mod(name: &str, src: &str, private_module: bool) -> ModuleUnit {
        let mut attrs = ModuleAttributes::named(name);
        attrs.option_private_module = private_module;
        ModuleUnit {
            module_name: name.into(),
            module_kind: ModuleKind::Procedural,
            attributes: attrs,
            source: src.into(),
        }
    }

    fn class_mod(name: &str, src: &str, exposed: bool, creatable: bool) -> ModuleUnit {
        let mut attrs = ModuleAttributes::named(name);
        attrs.vb_exposed = exposed;
        attrs.vb_creatable = creatable;
        ModuleUnit {
            module_name: name.into(),
            module_kind: ModuleKind::Class,
            attributes: attrs,
            source: src.into(),
        }
    }

    fn synth(modules: Vec<ModuleUnit>) -> ProjectExportSurface {
        let mut symbols = SymbolTable::new();
        let mut signatures = SignatureTable::new();
        let mut next = 0u32;
        let project = symbols
            .add_scope(ScopeKind::Project, symbols.global_scope(), Some("P"))
            .unwrap();
        let mut scans = Vec::new();
        let mut parses = Vec::new();
        for m in &modules {
            let parse = oxvba_syntax::parse(&m.source);
            assert!(
                parse.errors().is_empty(),
                "parse errors in {}: {:?}",
                m.module_name,
                parse.errors()
            );
            let scan = crate::scanner::scan_module(
                &mut symbols,
                &mut signatures,
                &mut next,
                m,
                parse.syntax(),
                project,
            )
            .unwrap();
            scans.push(scan);
            parses.push(parse);
        }
        let roots: Vec<_> = scans
            .iter()
            .zip(parses.iter())
            .map(|(s, p)| (s.module_scope, p.syntax()))
            .collect();
        let const_values = crate::const_eval::fold_const_values(&symbols, &roots);
        synthesize_export_surface("P", &modules, &scans, &symbols, &signatures, &const_values)
    }

    fn find_type<'a>(s: &'a ProjectExportSurface, name: &str) -> Option<&'a SurfaceType> {
        s.types.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    fn member<'a>(t: &'a SurfaceType, name: &str) -> Option<&'a SurfaceMember> {
        t.members.iter().find(|m| m.name.eq_ignore_ascii_case(name))
    }

    #[test]
    fn procedural_module_is_a_global_namespace_hidden_module() {
        let s = synth(vec![proc_mod(
            "Lib",
            "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function\n\
             Public Sub Log()\nEnd Sub\n\
             Private Sub Secret()\nEnd Sub\n\
             Public Const K As Long = 7\n",
            false,
        )]);

        let lib = find_type(&s, "Lib").expect("Lib module in surface");
        assert!(matches!(lib.kind, SurfaceTypeKind::Module));
        assert!(
            lib.global_namespace,
            "standard module members resolve unqualified"
        );

        // Public procs are members; the Private one is excluded.
        let add = member(lib, "Add").expect("Add exposed");
        let log = member(lib, "Log").expect("Log exposed");
        assert!(
            member(lib, "Secret").is_none(),
            "Private members do not cross the boundary"
        );

        // Dispids are assigned sequentially (no default member → starts at 1).
        assert_eq!(add.dispid, 1);
        assert_eq!(log.dispid, 2);
        assert_eq!(add.member_kind, ProjectMemberKind::Method);
        // VBA params default to ByRef → ByRef typelib param types.
        assert_eq!(
            add.parameter_types,
            vec![TypeLibParamType::ByRefLong, TypeLibParamType::ByRefLong]
        );
        assert_eq!(add.return_type, Some(TypeLibParamType::Long));
        // A hidden module is not a vtable interface.
        assert_eq!(add.vtable_slot, None);

        // The Public Const is a global-namespace constant; nothing private leaks.
        assert!(s.consts.iter().any(|c| c.name.eq_ignore_ascii_case("K")));
    }

    #[test]
    fn exposed_class_is_a_coclass_with_vtable_members() {
        let s = synth(vec![class_mod(
            "Widget",
            "Public Function GetValue() As Long\nGetValue = 1\nEnd Function\n\
             Friend Sub Internal()\nEnd Sub\n",
            true,
            true,
        )]);
        let widget = find_type(&s, "Widget").expect("Widget coclass in surface");
        match &widget.kind {
            SurfaceTypeKind::Coclass { creatable, .. } => assert!(*creatable),
            _ => panic!("Widget should be a coclass"),
        }
        assert!(
            !widget.global_namespace,
            "a plain class is not global-namespace"
        );
        let get = member(widget, "GetValue").expect("GetValue exposed");
        assert_eq!(
            get.vtable_slot,
            Some(7),
            "class members get a dual-interface vtable slot"
        );
        // Friend members do not cross the project boundary.
        assert!(
            member(widget, "Internal").is_none(),
            "Friend is excluded cross-project"
        );
    }

    #[test]
    fn surface_preserves_bare_object_boundary_types() {
        let s = synth(vec![class_mod(
            "Returner",
            "Public Function ReturnSelf() As Object\nEnd Function\n\
             Public Function UseObject(ByVal target As Object) As Long\nEnd Function\n",
            true,
            true,
        )]);
        let returner = find_type(&s, "Returner").expect("Returner coclass in surface");
        let return_self = member(returner, "ReturnSelf").expect("ReturnSelf exposed");
        assert_eq!(return_self.return_type, Some(TypeLibParamType::Object));

        let use_object = member(returner, "UseObject").expect("UseObject exposed");
        assert_eq!(use_object.parameter_types, vec![TypeLibParamType::Object]);
        assert_eq!(use_object.return_type, Some(TypeLibParamType::Long));
    }

    #[test]
    fn non_exposed_class_and_private_module_are_absent() {
        let s = synth(vec![
            class_mod("Hidden", "Public Sub H()\nEnd Sub\n", false, false),
            proc_mod("Internals", "Public Sub I()\nEnd Sub\n", true),
        ]);
        assert!(
            find_type(&s, "Hidden").is_none(),
            "a non-exposed class is project-private"
        );
        assert!(
            find_type(&s, "Internals").is_none(),
            "Option Private Module is project-private"
        );
    }

    #[test]
    fn source_option_private_module_is_project_private() {
        let s = synth(vec![proc_mod(
            "Internals",
            "Option Private Module\nPublic Const K As Long = 7\nPublic Sub I()\nEnd Sub\n",
            false,
        )]);
        assert!(
            find_type(&s, "Internals").is_none(),
            "source Option Private Module should hide the module"
        );
        assert!(
            s.consts.is_empty(),
            "source Option Private Module should hide public constants"
        );
    }

    #[test]
    fn public_enum_members_are_global_namespace_consts() {
        let s = synth(vec![proc_mod(
            "Lib",
            "Public Enum Color\n  Red = 1\n  Green = 2\nEnd Enum\n\
             Private Enum Secret\n  Hidden = 9\nEnd Enum\n",
            false,
        )]);
        assert!(s.consts.iter().any(|c| c.name.eq_ignore_ascii_case("Red")));
        assert!(
            s.consts
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case("Green"))
        );
        // Members of a Private enum do not cross the boundary.
        assert!(
            !s.consts
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case("Hidden"))
        );
    }

    #[test]
    fn surface_publishes_const_values_enum_ids_events_and_interfaces() {
        let s = synth(vec![
            proc_mod(
                "Lib",
                "Public Const KMax As Long = 10\n\
                 Public Enum Color\n  Red = 1\n  Green\n  Blue = 10\n  Indigo\nEnd Enum\n",
                false,
            ),
            class_mod(
                "Clock",
                "Public Event Tick(ByVal n As Long)\nPublic Event Done()\n",
                true,
                true,
            ),
            class_mod("IShape", "Public Sub Draw()\nEnd Sub\n", true, false),
            class_mod(
                "Circle",
                "Implements IShape\nPrivate Sub IShape_Draw()\nEnd Sub\n",
                true,
                true,
            ),
        ]);

        // Const + enum-member values are published (auto-increment + reset).
        let val = |name: &str| {
            s.consts
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(name))
                .map(|c| c.value.clone())
        };
        assert_eq!(val("KMax"), Some(CoreConst::I32(10)));
        assert_eq!(val("Red"), Some(CoreConst::I32(1)));
        assert_eq!(val("Green"), Some(CoreConst::I32(2)), "auto-increment");
        assert_eq!(
            val("Blue"),
            Some(CoreConst::I32(10)),
            "explicit value resets"
        );
        assert_eq!(
            val("Indigo"),
            Some(CoreConst::I32(11)),
            "resumes from reset"
        );

        // An enum member carries its enum's name; a plain Const does not.
        let red = s
            .consts
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case("Red"))
            .unwrap();
        assert_eq!(red.enum_name.as_deref(), Some("Color"));
        let kmax = s
            .consts
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case("KMax"))
            .unwrap();
        assert_eq!(kmax.enum_name, None);

        // Coclass events get sequential per-class ids in source order.
        let clock = find_type(&s, "Clock").unwrap();
        let tick = clock
            .events
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("Tick"))
            .unwrap();
        let done = clock
            .events
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("Done"))
            .unwrap();
        assert_eq!(tick.event_id, 0);
        assert_eq!(done.event_id, 1);

        // The implementing coclass publishes its interface; the project interface
        // set names the interface type.
        let circle = find_type(&s, "Circle").unwrap();
        assert!(
            circle
                .implements
                .iter()
                .any(|i| i.eq_ignore_ascii_case("IShape"))
        );
        assert!(
            s.interfaces
                .iter()
                .any(|i| i.eq_ignore_ascii_case("IShape"))
        );
        // A non-implementing class carries no implements.
        assert!(clock.implements.is_empty());
    }

    #[test]
    fn property_get_let_become_members_sharing_a_dispid() {
        let s = synth(vec![class_mod(
            "Widget",
            "Public Property Get Name() As String\nName = \"x\"\nEnd Property\n\
             Public Property Let Name(ByVal v As String)\nEnd Property\n",
            true,
            true,
        )]);
        let widget = find_type(&s, "Widget").unwrap();
        let accessors: Vec<&SurfaceMember> = widget
            .members
            .iter()
            .filter(|m| m.name.eq_ignore_ascii_case("Name"))
            .collect();
        assert_eq!(accessors.len(), 2, "Get + Let are two members");
        let dispid = accessors[0].dispid;
        assert!(
            accessors.iter().all(|m| m.dispid == dispid),
            "accessors share one dispid"
        );
        assert!(
            accessors
                .iter()
                .any(|m| m.member_kind == ProjectMemberKind::PropertyGet)
        );
        assert!(
            accessors
                .iter()
                .any(|m| m.member_kind == ProjectMemberKind::PropertyLet)
        );
    }
}
