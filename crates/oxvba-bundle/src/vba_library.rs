//! The synthetic built-in **`VBA`** library bundle.
//!
//! Part of modelling the VBA built-in surface as an internal referenced library:
//! built-in objects are classes in a real, always-linked bundle named `"VBA"`,
//! instantiated and dispatched through the ordinary cross-bundle machinery
//! (`NewExtern` + late member dispatch), not via bespoke opcodes. The class
//! methods carry [`NativeMethodId`] bodies (see [`ProcedureDescriptor::native`]),
//! so the VM runs them as native code instead of pushing a bytecode frame.
//!
//! Today this exposes the `Collection` class. The bundle's `ops` are a lone
//! `Return` placeholder that is never executed (native bodies bypass the frame
//! machinery, and the class has no `Class_Initialize`).

use std::sync::OnceLock;

use crate::native::NativeMethodId;
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

    let mut procedures = Vec::with_capacity(specs.len());
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
            native: Some(spec.native),
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

    let exports = vec![BundleExport {
        token: ExportToken::Class {
            name: "Collection".to_string(),
        },
        target: ExportTarget::Class(0),
    }];

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
                b.procedures[m.proc].native.is_some(),
                "method {} must have a native body",
                m.name
            );
        }
    }
}
