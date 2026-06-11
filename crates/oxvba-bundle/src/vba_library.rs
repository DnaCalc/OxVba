//! The synthetic built-in **`VBA`** library bundle.
//!
//! Part of modelling the VBA built-in surface as an internal referenced library:
//! the built-in surface lives in a real, always-linked bundle named `"VBA"`,
//! resolved and dispatched through the ordinary surface/reference machinery
//! rather than via bespoke routes/opcodes:
//!
//! - Built-in **objects** are classes (`Collection`, …), instantiated and
//!   dispatched cross-bundle (`NewExtern` + late member dispatch). Their methods
//!   carry [`NativeBody::Method`] bodies (the VM runs them with `&mut self`).
//! - Built-in **library functions** are module procedures (`Strings.Left`, …),
//!   resolved by the binder as `ExternMember { has_receiver: false }` →
//!   `ExportToken::ModuleFunc` and called via `Op::CallExtern`. Each carries a
//!   [`NativeBody::Library`] body, which the VM runs through `oxvba-lib` exactly
//!   like an `Op::CallNative { NativeCallee::Builtin(..) }` — no frame, no bespoke
//!   `CoreCallee::Native` route.
//!
//! Today this exposes the `Collection` class and the `Strings` library module.
//! The bundle's `ops` are a lone `Return` placeholder that is never executed
//! (native bodies bypass the frame machinery, and the class has no
//! `Class_Initialize`).

use std::sync::OnceLock;

use crate::native::{NativeBody, NativeImplId, NativeMethodId};
use crate::{
    Bundle, BundleExport, ClassDescriptor, ClassMethod, ExportTarget, ExportToken, Op,
    ProcedureDescriptor, ProcedureKind, ProjectMemberKind,
};

/// The process-wide built-in `VBA` library bundle. Linked into every VM image
/// (see `Vm::link`) so `New Collection` and friends resolve against a real unit.
/// `'static`, so it satisfies any VM lifetime without per-run allocation.
pub fn vba_library_bundle() -> &'static Bundle {
    static BUNDLE: OnceLock<Bundle> = OnceLock::new();
    BUNDLE.get_or_init(build)
}

/// One native-bodied class method: its VBA name, accessor kind, the `ProcedureKind`
/// recorded on the descriptor (informational for native bodies), the native body
/// id, and the argument count (informational — the native body reads args directly).
struct MethodSpec {
    name: &'static str,
    member_kind: ProjectMemberKind,
    proc_kind: ProcedureKind,
    native: NativeMethodId,
    param_count: usize,
}

/// The `VBA` typelib module that owns the string library functions.
const STRINGS_MODULE: &str = "Strings";

/// The `Strings`-module library functions exported by the `VBA` bundle, each with
/// the parameter count recorded on its descriptor. The id's `module()` is
/// `LibraryModule::Strings` and its `strings_member_name()` is the export name;
/// `param_count` is informational for native bodies (the `oxvba-lib` body reads
/// its arguments positionally), so it is sized at the maximum-arity form. The
/// `String`/`String$` repeat function appears as `StringRepeat` (canonical name
/// `"String"`); `MidStmt` (assignment form) and `Like` (operator) are excluded —
/// neither is an ordinary by-name library function (see
/// [`NativeImplId::strings_member_name`]). A test (`strings_exports_cover_module`)
/// asserts this list covers every named `Strings` id, so it cannot silently drift.
const STRINGS_FUNCS: &[(NativeImplId, usize)] = &[
    (NativeImplId::Len, 1),
    (NativeImplId::LenB, 1),
    (NativeImplId::Left, 2),
    (NativeImplId::Right, 2),
    (NativeImplId::Mid, 3),
    (NativeImplId::InStr, 4),
    (NativeImplId::InStrRev, 4),
    (NativeImplId::LCase, 1),
    (NativeImplId::UCase, 1),
    (NativeImplId::Split, 4),
    (NativeImplId::Join, 2),
    (NativeImplId::Replace, 6),
    (NativeImplId::Trim, 1),
    (NativeImplId::LTrim, 1),
    (NativeImplId::RTrim, 1),
    (NativeImplId::StrComp, 3),
    (NativeImplId::Chr, 1),
    (NativeImplId::Asc, 1),
    (NativeImplId::Space, 1),
    (NativeImplId::StringRepeat, 2),
    (NativeImplId::StrReverse, 1),
    (NativeImplId::StrConv, 3),
    (NativeImplId::Format, 4),
    (NativeImplId::Filter, 4),
];

fn build() -> Bundle {
    let specs = [
        MethodSpec {
            name: "Add",
            member_kind: ProjectMemberKind::Method,
            proc_kind: ProcedureKind::Sub,
            native: NativeMethodId::CollectionAdd,
            param_count: 4, // item, [key], [before], [after]
        },
        MethodSpec {
            name: "Item",
            member_kind: ProjectMemberKind::Method,
            proc_kind: ProcedureKind::Function,
            native: NativeMethodId::CollectionItem,
            param_count: 1,
        },
        MethodSpec {
            name: "Count",
            member_kind: ProjectMemberKind::PropertyGet,
            proc_kind: ProcedureKind::PropertyGet,
            native: NativeMethodId::CollectionCount,
            param_count: 0,
        },
        MethodSpec {
            name: "Remove",
            member_kind: ProjectMemberKind::Method,
            proc_kind: ProcedureKind::Sub,
            native: NativeMethodId::CollectionRemove,
            param_count: 1,
        },
    ];

    let mut procedures = Vec::with_capacity(specs.len() + STRINGS_FUNCS.len());
    let mut methods = Vec::with_capacity(specs.len());
    for spec in &specs {
        let proc = procedures.len();
        procedures.push(ProcedureDescriptor {
            name: spec.name.to_string(),
            // Placeholder body (`ops[0]` = Return); never executed — the native
            // body returns before any bytecode runs.
            entry_pc: 0,
            kind: spec.proc_kind,
            param_count: spec.param_count,
            // Me + params; unused by native bodies, kept valid for completeness.
            frame_slots: spec.param_count + 1,
            return_slot: None,
            native: Some(NativeBody::Method(spec.native)),
        });
        methods.push(ClassMethod {
            name: spec.name.to_string(),
            kind: spec.member_kind,
            proc,
        });
    }

    let classes = vec![ClassDescriptor {
        name: "Collection".to_string(),
        initialize: None,
        terminate: None,
        methods,
        implements: Vec::new(),
    }];

    let mut exports = vec![BundleExport {
        token: ExportToken::Class {
            name: "Collection".to_string(),
        },
        target: ExportTarget::Class(0),
    }];

    // The `Strings` library module: each function is a native-bodied module proc
    // (a `NativeBody::Library` body run through `oxvba-lib`), exported as a
    // `ModuleFunc` so the binder's `ExternMember { has_receiver: false }`
    // resolution links to it cross-bundle. The exported member name is the id's
    // canonical name (`NativeImplId::strings_member_name`), shared with the binder
    // so an import token matches this export token exactly.
    for &(id, param_count) in STRINGS_FUNCS {
        let member = id
            .strings_member_name()
            .expect("STRINGS_FUNCS lists only named Strings ids");
        let proc = procedures.len();
        procedures.push(ProcedureDescriptor {
            name: member.to_string(),
            // Placeholder body (`ops[0]` = Return); never executed — the native
            // library body returns its result before any bytecode runs.
            entry_pc: 0,
            kind: ProcedureKind::Function,
            param_count,
            frame_slots: param_count + 1,
            return_slot: None,
            native: Some(NativeBody::Library(id)),
        });
        exports.push(BundleExport {
            token: ExportToken::ModuleFunc {
                module: STRINGS_MODULE.to_string(),
                member: member.to_string(),
                kind: ProjectMemberKind::Method,
            },
            target: ExportTarget::Proc(proc),
        });
    }

    Bundle {
        ops: vec![Op::Return],
        procedures,
        entry_pc: 0,
        global_count: 0,
        entry_frame_slots: 0,
        statement_starts: Vec::new(),
        external_calls: Vec::new(),
        source_map: Vec::new(),
        com_class_exports: Vec::new(),
        classes,
        event_routes: Vec::new(),
        unit_name: "VBA".to_string(),
        exports,
        imports: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vba_bundle_exports_collection_with_native_methods() {
        let b = vba_library_bundle();
        assert_eq!(b.unit_name, "VBA");
        // The Collection class is exported by name.
        assert!(b.exports.iter().any(|e| matches!(
            &e.token,
            ExportToken::Class { name } if name.eq_ignore_ascii_case("Collection")
        )));
        let collection = &b.classes[0];
        assert_eq!(collection.methods.len(), 4);
        // Every method resolves to a native-bodied procedure.
        for m in &collection.methods {
            assert!(
                matches!(b.procedures[m.proc].native, Some(NativeBody::Method(_))),
                "method {} must have a native object-method body",
                m.name
            );
        }
    }

    /// `STRINGS_FUNCS` must list every named `Strings` library id (so the bundle
    /// exports the whole module and the binder's `ExternMember` route always
    /// resolves). The only `Strings` ids without a bundle export are `MidStmt` (the
    /// assignment-statement form) and `Like` (the operator) — both name-less.
    #[test]
    fn strings_exports_cover_module() {
        use crate::native::LibraryModule;
        let listed: std::collections::HashSet<NativeImplId> =
            STRINGS_FUNCS.iter().map(|&(id, _)| id).collect();
        for &id in NativeImplId::ALL {
            if id.module() != LibraryModule::Strings {
                continue;
            }
            match id.strings_member_name() {
                // A named Strings function must be exported by the bundle.
                Some(_) => assert!(
                    listed.contains(&id),
                    "named Strings id {id:?} is missing from STRINGS_FUNCS"
                ),
                // MidStmt / Like are intentionally not bundle members.
                None => assert!(
                    matches!(id, NativeImplId::MidStmt | NativeImplId::Like),
                    "unexpected name-less Strings id {id:?}"
                ),
            }
        }
    }

    /// Every `Strings` function is exported as a `ModuleFunc` whose target proc has
    /// a `NativeBody::Library` body, so a cross-bundle `CallExtern` reaches the
    /// `oxvba-lib` body (the route the binder now lowers these calls to).
    #[test]
    fn strings_funcs_are_native_library_module_procs() {
        let b = vba_library_bundle();
        for &(id, _) in STRINGS_FUNCS {
            let member = id.strings_member_name().unwrap();
            let export = b
                .exports
                .iter()
                .find(|e| {
                    matches!(
                        &e.token,
                        ExportToken::ModuleFunc { module, member: m, kind }
                            if module == "Strings"
                                && m.eq_ignore_ascii_case(member)
                                && *kind == ProjectMemberKind::Method
                    )
                })
                .unwrap_or_else(|| panic!("missing Strings export for {id:?}"));
            let ExportTarget::Proc(proc) = export.target else {
                panic!("Strings export {id:?} must target a proc");
            };
            assert_eq!(
                b.procedures[proc].native,
                Some(NativeBody::Library(id)),
                "Strings export {id:?} must have a NativeBody::Library body",
            );
        }
    }
}
