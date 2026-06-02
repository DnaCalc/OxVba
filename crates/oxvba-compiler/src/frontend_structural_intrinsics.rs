#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralIntrinsic {
    NothingLiteral,
    NullLiteral,
    OmittedArgument,
    ProjectInstance,
    WithEventsGet,
    WithEventsSet,
    WithEventsClearOwner,
    WithEventsFirstOwner,
    WithEventsNextOwner,
    DynamicDispatchInvoke,
    DynamicDispatchEarlyInvoke,
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
            Self::WithEventsGet => "__oxvba_withevents_get",
            Self::WithEventsSet => "__oxvba_withevents_set",
            Self::WithEventsClearOwner => "__oxvba_withevents_clear_owner",
            Self::WithEventsFirstOwner => "__oxvba_withevents_first_owner",
            Self::WithEventsNextOwner => "__oxvba_withevents_next_owner",
            Self::DynamicDispatchInvoke => "dispatchinvoke",
            Self::DynamicDispatchEarlyInvoke => "__oxvbaearlyinvoke",
            Self::PtrOf => "ptrof",
            Self::ObjPtr => "objptr",
            Self::VarPtr => "varptr",
            Self::StrPtr => "strptr",
        }
    }

    pub fn from_legacy_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("__OxVbaEarlyPropertyGetInvoke")
            || name.eq_ignore_ascii_case("__OxVbaEarlyPropertyLetInvoke")
            || name.eq_ignore_ascii_case("__OxVbaEarlyPropertySetInvoke")
        {
            return Some(Self::DynamicDispatchEarlyInvoke);
        }
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
    StructuralIntrinsic::WithEventsGet,
    StructuralIntrinsic::WithEventsSet,
    StructuralIntrinsic::WithEventsClearOwner,
    StructuralIntrinsic::WithEventsFirstOwner,
    StructuralIntrinsic::WithEventsNextOwner,
    StructuralIntrinsic::DynamicDispatchInvoke,
    StructuralIntrinsic::DynamicDispatchEarlyInvoke,
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
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::WithEventsSet));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::DynamicDispatchInvoke));
        assert!(ALL_STRUCTURAL_INTRINSICS.contains(&StructuralIntrinsic::ObjPtr));
    }

    #[test]
    fn structural_intrinsics_accept_early_bound_property_aliases() {
        for alias in [
            "__OxVbaEarlyPropertyGetInvoke",
            "__OxVbaEarlyPropertyLetInvoke",
            "__OxVbaEarlyPropertySetInvoke",
        ] {
            assert_eq!(
                StructuralIntrinsic::from_legacy_name(alias),
                Some(StructuralIntrinsic::DynamicDispatchEarlyInvoke)
            );
        }
    }

    #[test]
    fn structural_intrinsics_do_not_accept_unknown_magic_strings() {
        assert_eq!(
            StructuralIntrinsic::from_legacy_name("__oxvba_not_real"),
            None
        );
    }
}
