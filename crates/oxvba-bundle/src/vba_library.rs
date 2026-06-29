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
//! Today this exposes the `Collection` class and the migrated library members —
//! the whole `Strings`/`Math`/`DateTime`/`Conversion`/`Random`/`Financial` modules,
//! the `Information` predicates, the `Interaction` host functions, the `FileIo`
//! by-name members (functions + the by-name statements `Kill`/`MkDir`/… — exported
//! under the `FileSystem` module), and the name-less file STATEMENTS (`Open`/`Print`/
//! `Put`/`Get`/… — also under `FileSystem`, via fixed internal member names) — every
//! id for which [`NativeImplId::library_member`] or
//! [`NativeImplId::library_statement_member`] is `Some`. The bundle's `ops` are a lone
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
            is_default_member: spec.name.eq_ignore_ascii_case("Item"),
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

    // Each migrated id contributes one native-bodied module proc (a
    // `NativeBody::Library` body run through `oxvba-lib`) + a `ModuleFunc` export at
    // its `(module, member)` location, so the binder's `ExternMember { has_receiver:
    // false }` / `ExternProc` resolution links to it cross-bundle (an import token
    // matches this export token exactly). The by-NAME members come from
    // `library_member` (member = catalog primary; shared with the binder's
    // `name_to_intrinsic` reroute — the whole `Strings`/`Math`/`DateTime`/`Conversion`/
    // `Random`/`Financial` modules, the `Information` predicates, the `Interaction`
    // host functions, and the `FileSystem` by-name functions + by-name statements). The
    // name-LESS file STATEMENTS come from `library_statement_member` (fixed internal
    // names; shared with the parser-bound lowering in `oxvba-bind/stmt.rs`). Both yield
    // an identical `ExternProc`-callable proc, so a cross-bundle `CallExtern` reaches
    // the `oxvba-lib` body either way. Iterating `NativeImplId::ALL` exports a new
    // migrated id automatically (drift-guard tests assert coverage).
    for &id in NativeImplId::ALL {
        let Some((module, member, param_count)) = id
            .library_member()
            .map(|(m, mem)| (m, mem, id.library_param_count()))
            .or_else(|| {
                id.library_statement_member()
                    .map(|(m, mem)| (m, mem, id.library_statement_param_count()))
            })
        else {
            continue;
        };
        // `param_count` is informational for native bodies (the `oxvba-lib` body
        // reads its arguments positionally), sized at the maximum-arity form.
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
        global_initializer: None,
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

    /// The migrated-id gate: `library_member()` is `Some` for exactly the by-NAME
    /// migrated set and `None` for everything else. Migrated =
    /// - the whole `Strings`/`Math`/`DateTime`/`Conversion`/`Random`/`Financial`
    ///   modules, minus the name-less `MidStmt`/`Like`;
    /// - the `Information` **predicates** (but NOT the `IIf`/`Choose`/`Switch` special
    ///   forms);
    /// - the `Interaction` **host functions** (but NOT `CreateObject` or the `Com*`
    ///   event machinery);
    /// - the `FileIo` by-**name** members: the `Ordinary` functions (`FreeFile`/
    ///   `CurDir`/`FileLen`/`GetAttr`/`FileDateTime`/`EOF`/`LOF`/`Seek`/`Loc`) AND the
    ///   by-name `FileStatement` forms that resolve through `name_to_intrinsic` because
    ///   they are not lexer keywords (`Kill`/`MkDir`/`RmDir`/`ChDir`/`ChDrive`/
    ///   `SetAttr`/`FileCopy`). The name-LESS `FileStatement` forms (`FileOpen`/
    ///   `Print #`/`Put`/`Get`/… — parser-bound) are NOT here: they are
    ///   `library_statement_member`s, guarded separately below. The name-less
    ///   `FileRead` stays `None`.
    ///
    /// Everything else — those exceptions, the name-less file statements, `FileRead`,
    /// `Diagnostics`, and the `Collection` members — keeps the bespoke `Native` route
    /// (special forms) or is a statement member. The exclusions are asserted by id
    /// (not just by module) so a careless future change is caught.
    #[test]
    fn library_member_covers_exactly_the_migrated_ids() {
        use crate::native::LibraryModule as M;
        use NativeImplId::*;

        // Information predicates, Interaction host functions, and the FileIo by-name
        // members are migrated members of their (otherwise partially excluded)
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
        // The FileIo ids resolved by NAME — both the `Ordinary` function forms and the
        // `FileStatement` forms that are not lexer keywords (`Kill`/`MkDir`/…). The
        // name-LESS `FileStatement` forms (FileOpen/FilePrint/FilePut/FileGetInto/…)
        // are `library_statement_member`s, not `library_member`s; the name-less
        // `FileRead` stays on the Native route.
        let fileio_by_name = |id| {
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
                    | FileKill
                    | FileMkDir
                    | FileRmDir
                    | FileChDir
                    | FileChDrive
                    | FileSetAttr
                    | FileCopy
            )
        };

        for &id in NativeImplId::ALL {
            let migrated = match id.module() {
                M::Strings | M::Math | M::DateTime | M::Conversion | M::Random | M::Financial => {
                    !matches!(id, MidStmt | Like)
                }
                M::Information => information_predicate(id),
                M::Interaction => interaction_host_fn(id),
                M::FileIo => fileio_by_name(id),
                M::Diagnostics => false,
            };
            assert_eq!(
                id.library_member().is_some(),
                migrated,
                "library_member({id:?}) disagrees with the migrated-id set",
            );
            // A by-name member and a name-less statement member are disjoint: never both.
            assert!(
                !(id.library_member().is_some() && id.library_statement_member().is_some()),
                "{id:?} is both a library_member and a library_statement_member",
            );
        }

        // Spot-check the deliberate exclusions stay `None` (special forms / name-less
        // `FileRead`); the name-less file STATEMENT forms are excluded from
        // `library_member` but ARE `library_statement_member`s (asserted separately).
        for excluded in [
            IIf,
            Choose,
            Switch,
            CreateObject,
            ComSubscribeEvent,
            DebugPrint,
            FileRead,
        ] {
            assert!(
                excluded.library_member().is_none(),
                "{excluded:?} must stay on the Native route",
            );
            assert!(
                excluded.library_statement_member().is_none(),
                "{excluded:?} is not a file statement member",
            );
        }
        // And spot-check the by-name inclusions are present — including the by-name
        // file STATEMENTS (Kill/MkDir/…) that migrated this round.
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
            FileKill,
            FileMkDir,
            FileRmDir,
            FileChDir,
            FileChDrive,
            FileSetAttr,
            FileCopy,
        ] {
            assert!(
                included.library_member().is_some(),
                "{included:?} must route through the VBA bundle",
            );
        }

        // The FileSystem members export under the canonical VBA typelib module name,
        // not the `LibraryModule::FileIo` enum name; member == catalog primary.
        assert_eq!(FreeFile.library_member(), Some(("FileSystem", "FreeFile")));
        assert_eq!(FileCurDir.library_member(), Some(("FileSystem", "CurDir")));
        assert_eq!(
            FileGetAttr.library_member(),
            Some(("FileSystem", "GetAttr"))
        );
        assert_eq!(FileSeek.library_member(), Some(("FileSystem", "Seek")));
        assert_eq!(FileKill.library_member(), Some(("FileSystem", "Kill")));
        assert_eq!(FileCopy.library_member(), Some(("FileSystem", "FileCopy")));
        assert_eq!(
            FileSetAttr.library_member(),
            Some(("FileSystem", "SetAttr"))
        );
    }

    /// The name-less file STATEMENT gate: `library_statement_member()` is `Some` for
    /// exactly the parser-bound, empty-catalog-name `FileStatement` ids (lowered in
    /// `oxvba-bind/stmt.rs`) and `None` for everything else — including the by-name
    /// file members (which are `library_member`s) and `FileSeek` (whose statement form
    /// reuses the by-name `FileSystem.Seek`).
    #[test]
    fn library_statement_member_covers_exactly_the_parser_bound_statements() {
        use NativeImplId::*;

        let parser_bound_statement = |id| {
            matches!(
                id,
                FileOpen
                    | FileClose
                    | FilePrint
                    | FileWrite
                    | FileInput
                    | FileLineInput
                    | FileWidth
                    | FileRename
                    | FileLock
                    | FileUnlock
                    | FilePut
                    | FileGetInto
            )
        };
        for &id in NativeImplId::ALL {
            assert_eq!(
                id.library_statement_member().is_some(),
                parser_bound_statement(id),
                "library_statement_member({id:?}) disagrees with the parser-bound set",
            );
            if let Some((module, _member)) = id.library_statement_member() {
                assert_eq!(
                    module, "FileSystem",
                    "statement {id:?} must own to FileSystem"
                );
            }
        }
        // The fixed internal member names the binder targets.
        assert_eq!(
            FileOpen.library_statement_member(),
            Some(("FileSystem", "Open"))
        );
        assert_eq!(
            FilePrint.library_statement_member(),
            Some(("FileSystem", "Print"))
        );
        assert_eq!(
            FilePut.library_statement_member(),
            Some(("FileSystem", "Put"))
        );
        assert_eq!(
            FileGetInto.library_statement_member(),
            Some(("FileSystem", "Get"))
        );
        assert_eq!(
            FileRename.library_statement_member(),
            Some(("FileSystem", "Name"))
        );
        // `FileSeek` is a by-name member, not a statement member (its `Seek #n, pos`
        // statement reuses the by-name `FileSystem.Seek`).
        assert_eq!(FileSeek.library_statement_member(), None);
        // No two statement members collide on a member name within FileSystem.
        let mut names: Vec<&str> = NativeImplId::ALL
            .iter()
            .filter_map(|id| id.library_statement_member().map(|(_, m)| m))
            .collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "statement member names must be unique");
    }

    /// Drift-guard: every migrated id — both by-name members (`library_member()`) and
    /// name-less file statements (`library_statement_member()`) — is exported by the
    /// bundle as a `ModuleFunc` at its `(module, member)` location, targeting a proc
    /// with a `NativeBody::Library` body — so a cross-bundle `CallExtern` reaches the
    /// `oxvba-lib` body (the route the binder now lowers both kinds of call to).
    #[test]
    fn migrated_members_are_native_library_module_procs() {
        let b = vba_library_bundle();
        for &id in NativeImplId::ALL {
            // The location is whichever of the two tables yields it (they are
            // disjoint, asserted in `library_member_covers_exactly_the_migrated_ids`).
            let Some((module, member)) = id
                .library_member()
                .or_else(|| id.library_statement_member())
            else {
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
