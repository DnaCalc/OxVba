#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralIntrinsic {
    NothingLiteral,
    NullLiteral,
    OmittedArgument,
    ProjectInstance,
    WithEventsAttach,
    WithEventsDetach,
    DynamicDispatchInvoke,
    DynamicDispatchGet,
    DynamicDispatchLet,
    DynamicDispatchSet,
    PtrOf,
    ObjPtr,
    VarPtr,
    StrPtr,
}

impl StructuralIntrinsic {
    pub fn legacy_name(self) -> &'static str {
        match self {
            Self::NothingLiteral => "__oxvba_nothing",
            Self::NullLiteral => "__oxvba_null",
            Self::OmittedArgument => "__oxvba_omitted_arg",
            Self::ProjectInstance => "__oxvba_project_instance",
            Self::WithEventsAttach => "__oxvba_withevents_attach",
            Self::WithEventsDetach => "__oxvba_withevents_detach",
            Self::DynamicDispatchInvoke => "__oxvba_dispatch_invoke",
            Self::DynamicDispatchGet => "__oxvba_dispatch_get",
            Self::DynamicDispatchLet => "__oxvba_dispatch_let",
            Self::DynamicDispatchSet => "__oxvba_dispatch_set",
            Self::PtrOf => "ptrof",
            Self::ObjPtr => "objptr",
            Self::VarPtr => "varptr",
            Self::StrPtr => "strptr",
        }
    }

    pub fn from_legacy_name(name: &str) -> Option<Self> {
        ALL_STRUCTURAL_INTRINSICS
            .iter()
            .copied()
            .find(|intrinsic| intrinsic.legacy_name().eq_ignore_ascii_case(name))
    }
}

pub const ALL_STRUCTURAL_INTRINSICS: &[StructuralIntrinsic] = &[
    StructuralIntrinsic::NothingLiteral,
    StructuralIntrinsic::NullLiteral,
    StructuralIntrinsic::OmittedArgument,
    StructuralIntrinsic::ProjectInstance,
    StructuralIntrinsic::WithEventsAttach,
    StructuralIntrinsic::WithEventsDetach,
    StructuralIntrinsic::DynamicDispatchInvoke,
    StructuralIntrinsic::DynamicDispatchGet,
    StructuralIntrinsic::DynamicDispatchLet,
    StructuralIntrinsic::DynamicDispatchSet,
    StructuralIntrinsic::PtrOf,
    StructuralIntrinsic::ObjPtr,
    StructuralIntrinsic::VarPtr,
    StructuralIntrinsic::StrPtr,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_intrinsics_round_trip_legacy_names() {
        for intrinsic in ALL_STRUCTURAL_INTRINSICS {
            assert_eq!(
                StructuralIntrinsic::from_legacy_name(intrinsic.legacy_name()),
                Some(*intrinsic)
            );
        }
    }

    #[test]
    fn structural_intrinsics_cover_required_families() {
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::NothingLiteral));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::NullLiteral));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::OmittedArgument));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::ProjectInstance));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::WithEventsAttach));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::DynamicDispatchInvoke));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::ObjPtr));
    }

    #[test]
    fn structural_intrinsics_do_not_accept_unknown_magic_strings() {
        assert_eq!(
            StructuralIntrinsic::from_legacy_name("__oxvba_not_real"),
            None
        );
    }
}
