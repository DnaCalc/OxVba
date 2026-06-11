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
//! Today this exposes the `Collection` class and the migrated library functions —
//! the whole `Strings`/`Math`/`DateTime`/`Conversion`/`Random`/`Financial` modules,
//! the `Information` predicates, the `Interaction` host functions, and the `FileIo`
//! by-name functions (exported under the `FileSystem` module) — every id for which
//! [`NativeImplId::library_member`] is `Some`. The bundle's `ops` are a lone
//! `Return` placeholder that is never executed (native bodies bypass the frame
//! machinery, and the class has no `Class_Initialize`).

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

    let mut procedures = Vec::with_capacity(specs.len() + NativeImplId::ALL.len());
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

    // The migrated library functions (the `Strings`/`Math`/`DateTime`/`Conversion`/
    // `Random`/`Financial` modules, the `Information` predicates, the `Interaction`
    // host functions, and the `FileIo` by-name functions): each is a native-bodied module proc (a
    // `NativeBody::Library` body run through `oxvba-lib`), exported as a `ModuleFunc`
    // so the binder's `ExternMember { has_receiver: false }` resolution links to it
    // cross-bundle. The `(module, member)` location and the export's member name
    // come from the single source of truth `NativeImplId::library_member`, shared
    // with the binder so an import token matches this export token exactly. We
    // iterate `NativeImplId::ALL` and keep every id for which `library_member` is
    // `Some`, so a new migrated id is exported automatically (a drift-guard test
    // asserts coverage).
    for &id in NativeImplId::ALL {
        let Some((module, member)) = id.library_member() else {
            continue;
        };
        // `param_count` is informational for native bodies (the `oxvba-lib` body
        // reads its arguments positionally), sized at the maximum-arity form.
        let param_count = id.library_param_count();
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
                module: module.to_string(),
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

    /// The migrated-id gate: `library_member()` is `Some` for exactly the migrated
    /// set and `None` for everything else. Migrated =
    /// - the whole `Strings`/`Math`/`DateTime`/`Conversion`/`Random`/`Financial`
    ///   modules, minus the name-less `MidStmt`/`Like`;
    /// - the `Information` **predicates** (but NOT the `IIf`/`Choose`/`Switch` special
    ///   forms);
    /// - the `Interaction` **host functions** (but NOT `CreateObject` or the `Com*`
    ///   event machinery);
    /// - the `FileIo` by-name **functions** (`FreeFile`/`CurDir`/`FileLen`/`GetAttr`/
    ///   `FileDateTime`/`EOF`/`LOF`/`Seek`/`Loc` — the catalog-`Ordinary`, non-empty-
    ///   name members), but NOT the `FileStatement` forms or the name-less `FileRead`.
    ///
    /// Everything else — those exceptions, the `FileIo` statement forms, `Diagnostics`,
    /// and the `Collection` members — keeps the bespoke `Native` route. The exclusions
    /// are asserted by id (not just by module) so a careless future change to one of
    /// the partially migrated modules is caught.
    #[test]
    fn library_member_covers_exactly_the_migrated_ids() {
        use crate::native::LibraryModule as M;
        use NativeImplId::*;

        // Information predicates, Interaction host functions, and the FileIo by-name
        // functions are migrated members of their (otherwise partially excluded)
        // modules; everything else in those modules is explicitly excluded below.
        let information_predicate = |id| {
            matches!(
                id,
                IsArray
                    | VarType
                    | TypeName
                    | IsNumeric
                    | IsError
                    | IsDate
                    | IsObject
                    | IsNull
                    | IsEmpty
                    | IsMissing
            )
        };
        let interaction_host_fn = |id| {
            matches!(
                id,
                MsgBox | InputBox | Beep | DoEvents | Shell | Environ | Dir
            )
        };
        // The FileIo ids whose catalog `CallShape` is `Ordinary` AND whose name set is
        // non-empty — the by-name FUNCTION forms. The `FileStatement` forms (FileOpen,
        // FileClose, Kill, MkDir, …, Print #, Put, Name, Lock, …) and the name-less
        // `FileRead` stay on the Native route (P4).
        let fileio_function = |id| {
            matches!(
                id,
                FreeFile
                    | FileCurDir
                    | FileLen
                    | FileGetAttr
                    | FileDateTime
                    | FileEof
                    | FileLof
                    | FileSeek
                    | FileLoc
            )
        };

        for &id in NativeImplId::ALL {
            let migrated = match id.module() {
                M::Strings | M::Math | M::DateTime | M::Conversion | M::Random | M::Financial => {
                    !matches!(id, MidStmt | Like)
                }
                M::Information => information_predicate(id),
                M::Interaction => interaction_host_fn(id),
                M::FileIo => fileio_function(id),
                M::Collection | M::Diagnostics => false,
            };
            assert_eq!(
                id.library_member().is_some(),
                migrated,
                "library_member({id:?}) disagrees with the migrated-id set",
            );
        }

        // Spot-check the deliberate exclusions stay `None` even though their modules
        // are partially migrated. The `FileIo` STATEMENT forms (and the name-less
        // `FileRead`) must NOT migrate — they are P4.
        for excluded in [
            IIf,
            Choose,
            Switch,
            CreateObject,
            ComSubscribeEvent,
            DebugPrint,
            FileOpen,
            FileClose,
            FileKill,
            FileChDir,
            FileSetAttr,
            FileCopy,
            FilePut,
            FileRead,
        ] {
            assert!(
                excluded.library_member().is_none(),
                "{excluded:?} must stay on the Native route",
            );
        }
        // And spot-check the new inclusions are present.
        for included in [
            VarType,
            TypeName,
            IsNumeric,
            IsMissing,
            MsgBox,
            Environ,
            Dir,
            FreeFile,
            FileCurDir,
            FileLen,
            FileGetAttr,
            FileDateTime,
            FileEof,
            FileLof,
            FileSeek,
            FileLoc,
        ] {
            assert!(
                included.library_member().is_some(),
                "{included:?} must route through the VBA bundle",
            );
        }

        // The FileSystem functions export under the canonical VBA typelib module name,
        // not the `LibraryModule::FileIo` enum name.
        assert_eq!(FreeFile.library_member(), Some(("FileSystem", "FreeFile")));
        assert_eq!(FileCurDir.library_member(), Some(("FileSystem", "CurDir")));
        assert_eq!(
            FileGetAttr.library_member(),
            Some(("FileSystem", "GetAttr"))
        );
        assert_eq!(FileSeek.library_member(), Some(("FileSystem", "Seek")));
    }

    /// Drift-guard: every migrated id (`library_member()` is `Some`) is exported by
    /// the bundle as a `ModuleFunc` at its `(module, member)` location, targeting a
    /// proc with a `NativeBody::Library` body — so a cross-bundle `CallExtern`
    /// reaches the `oxvba-lib` body (the route the binder now lowers these calls to).
    #[test]
    fn migrated_funcs_are_native_library_module_procs() {
        let b = vba_library_bundle();
        for &id in NativeImplId::ALL {
            let Some((module, member)) = id.library_member() else {
                continue;
            };
            let export = b
                .exports
                .iter()
                .find(|e| {
                    matches!(
                        &e.token,
                        ExportToken::ModuleFunc { module: owner, member: m, kind }
                            if owner == module
                                && m.eq_ignore_ascii_case(member)
                                && *kind == ProjectMemberKind::Method
                    )
                })
                .unwrap_or_else(|| panic!("missing {module} export for {id:?}"));
            let ExportTarget::Proc(proc) = export.target else {
                panic!("library export {id:?} must target a proc");
            };
            assert_eq!(
                b.procedures[proc].native,
                Some(NativeBody::Library(id)),
                "library export {id:?} must have a NativeBody::Library body",
            );
        }
    }
}
