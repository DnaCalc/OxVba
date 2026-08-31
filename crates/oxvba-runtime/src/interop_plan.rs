//! Backend-neutral verified interop call plan (`WIN-PLAN-001`).
//!
//! First slice: late IDispatch and x64 `Declare`. VM3 and JIT consume the same
//! verified plan identity and differ only in execution-adapter mechanics.

use crate::object_ref::RuntimeMemberInvokeKind;

/// Failure from constructing or verifying an interop plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropPlanError {
    pub message: String,
}

impl InteropPlanError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for InteropPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InteropPlanError {}

/// Accepted Windows target architecture for this profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropTargetArch {
    X64,
}

/// First-slice call kinds owned by the shared plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropCallKind {
    LateDispatch,
    DeclareX64,
}

/// Wire transport. Late calls stay IDispatch; Declare stays Win64 ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropTransport {
    IDispatch,
    Win64Abi,
}

/// ByRef copy-back happens after the native call and before the VBA caller resumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropWritebackOrder {
    AfterNativeBeforeReturn,
}

/// Error mapping for the two first-slice transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropErrorMapping {
    HresultExcepinfoToVbaErr,
    LastDllErrorImmediate,
}

/// Pins and marshalling temporaries are released on success and every fault edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropCleanupPolicy {
    ReleasePinsAndTemporariesOnEveryEdge,
}

/// First-slice plans do not register callbacks or reenter VBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropReentryPolicy {
    NoneThisSlice,
}

/// First-slice client/Declare calls run on the caller's STA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropApartmentPolicy {
    CallerSta,
}

/// Late-bound invoke kind, independent of COM crate enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropInvokeKind {
    Method,
    PropertyGet,
    PropertyPut,
    PropertyPutRef,
}

impl InteropInvokeKind {
    pub fn from_runtime(kind: RuntimeMemberInvokeKind) -> Self {
        match kind {
            RuntimeMemberInvokeKind::Method => Self::Method,
            RuntimeMemberInvokeKind::PropertyGet => Self::PropertyGet,
            RuntimeMemberInvokeKind::PropertyLet => Self::PropertyPut,
            RuntimeMemberInvokeKind::PropertySet => Self::PropertyPutRef,
        }
    }

    pub fn as_canonical(self) -> &'static str {
        match self {
            Self::Method => "method",
            Self::PropertyGet => "property-get",
            Self::PropertyPut => "property-put",
            Self::PropertyPutRef => "property-putref",
        }
    }
}

/// Call-specific signature and identity payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteropCallSpec {
    LateDispatch {
        member_name: String,
        default_member: bool,
        invoke_kind: InteropInvokeKind,
        named_arg_count: usize,
        byref_slots: Vec<u32>,
    },
    DeclareX64 {
        descriptor_id: u32,
        library: String,
        entry: String,
        calling_convention: String,
        param_count: usize,
        param_by_ref: Vec<bool>,
        return_type: Option<String>,
        capture_last_dll_error: bool,
    },
}

/// Stable identity shared by VM3 and JIT for the same call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropPlanIdentity {
    pub canonical: String,
}

/// Verifier-checked backend-neutral interop plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedInteropPlan {
    pub identity: InteropPlanIdentity,
    pub arch: InteropTargetArch,
    pub kind: InteropCallKind,
    pub transport: InteropTransport,
    pub call: InteropCallSpec,
    pub writeback: InteropWritebackOrder,
    pub error_mapping: InteropErrorMapping,
    pub cleanup: InteropCleanupPolicy,
    pub reentry: InteropReentryPolicy,
    pub apartment: InteropApartmentPolicy,
}

impl VerifiedInteropPlan {
    /// Build and verify a late IDispatch plan.
    pub fn late_dispatch(
        member_name: impl Into<String>,
        default_member: bool,
        invoke_kind: InteropInvokeKind,
        named_arg_count: usize,
        byref_slots: Vec<u32>,
    ) -> Result<Self, InteropPlanError> {
        let member_name = member_name.into();
        let mut plan = Self {
            identity: InteropPlanIdentity {
                canonical: String::new(),
            },
            arch: InteropTargetArch::X64,
            kind: InteropCallKind::LateDispatch,
            transport: InteropTransport::IDispatch,
            call: InteropCallSpec::LateDispatch {
                member_name,
                default_member,
                invoke_kind,
                named_arg_count,
                byref_slots,
            },
            writeback: InteropWritebackOrder::AfterNativeBeforeReturn,
            error_mapping: InteropErrorMapping::HresultExcepinfoToVbaErr,
            cleanup: InteropCleanupPolicy::ReleasePinsAndTemporariesOnEveryEdge,
            reentry: InteropReentryPolicy::NoneThisSlice,
            apartment: InteropApartmentPolicy::CallerSta,
        };
        plan.identity = InteropPlanIdentity {
            canonical: plan.canonical_identity(),
        };
        verify_interop_plan(&plan)?;
        Ok(plan)
    }

    /// Build and verify an x64 Declare plan.
    pub fn declare_x64(
        descriptor_id: u32,
        library: impl Into<String>,
        entry: impl Into<String>,
        calling_convention: impl Into<String>,
        param_by_ref: Vec<bool>,
        return_type: Option<String>,
    ) -> Result<Self, InteropPlanError> {
        let param_count = param_by_ref.len();
        let mut plan = Self {
            identity: InteropPlanIdentity {
                canonical: String::new(),
            },
            arch: InteropTargetArch::X64,
            kind: InteropCallKind::DeclareX64,
            transport: InteropTransport::Win64Abi,
            call: InteropCallSpec::DeclareX64 {
                descriptor_id,
                library: library.into(),
                entry: entry.into(),
                calling_convention: calling_convention.into(),
                param_count,
                param_by_ref,
                return_type,
                capture_last_dll_error: true,
            },
            writeback: InteropWritebackOrder::AfterNativeBeforeReturn,
            error_mapping: InteropErrorMapping::LastDllErrorImmediate,
            cleanup: InteropCleanupPolicy::ReleasePinsAndTemporariesOnEveryEdge,
            reentry: InteropReentryPolicy::NoneThisSlice,
            apartment: InteropApartmentPolicy::CallerSta,
        };
        plan.identity = InteropPlanIdentity {
            canonical: plan.canonical_identity(),
        };
        verify_interop_plan(&plan)?;
        Ok(plan)
    }

    fn canonical_identity(&self) -> String {
        match &self.call {
            InteropCallSpec::LateDispatch {
                member_name,
                default_member,
                invoke_kind,
                named_arg_count,
                byref_slots,
            } => {
                let byref = byref_slots
                    .iter()
                    .map(|slot| slot.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "win-plan-v1|x64|late-dispatch|idispatch|member={member_name}|default={}|kind={}|named={named_arg_count}|byref={byref}|writeback=after-native-before-return|error=hresult-excepinfo|cleanup=pins-temps-every-edge|reentry=none|apartment=caller-sta",
                    if *default_member { 1 } else { 0 },
                    invoke_kind.as_canonical()
                )
            }
            InteropCallSpec::DeclareX64 {
                descriptor_id,
                library,
                entry,
                calling_convention,
                param_count,
                param_by_ref,
                return_type,
                capture_last_dll_error,
            } => {
                let byref = param_by_ref
                    .iter()
                    .map(|flag| if *flag { "1" } else { "0" })
                    .collect::<Vec<_>>()
                    .join(",");
                let ret = return_type.as_deref().unwrap_or("");
                format!(
                    "win-plan-v1|x64|declare-x64|win64-abi|id={descriptor_id}|lib={library}|entry={entry}|conv={calling_convention}|params={param_count}|byref={byref}|ret={ret}|lastdll={}|writeback=after-native-before-return|error=last-dll-error-immediate|cleanup=pins-temps-every-edge|reentry=none|apartment=caller-sta",
                    if *capture_last_dll_error { 1 } else { 0 }
                )
            }
        }
    }
}

/// Verify a constructed plan. Mutations that break first-slice invariants fail closed.
pub fn verify_interop_plan(plan: &VerifiedInteropPlan) -> Result<(), InteropPlanError> {
    if plan.arch != InteropTargetArch::X64 {
        return Err(InteropPlanError::new(
            "verified interop plan accepts only x64",
        ));
    }
    if plan.writeback != InteropWritebackOrder::AfterNativeBeforeReturn {
        return Err(InteropPlanError::new(
            "verified interop plan requires ByRef writeback after the native call and before return",
        ));
    }
    if plan.cleanup != InteropCleanupPolicy::ReleasePinsAndTemporariesOnEveryEdge {
        return Err(InteropPlanError::new(
            "verified interop plan requires pin/temporary cleanup on every edge",
        ));
    }
    if plan.reentry != InteropReentryPolicy::NoneThisSlice {
        return Err(InteropPlanError::new(
            "first-slice verified interop plan does not admit callback reentry",
        ));
    }
    if plan.apartment != InteropApartmentPolicy::CallerSta {
        return Err(InteropPlanError::new(
            "first-slice verified interop plan requires caller STA",
        ));
    }
    match (&plan.kind, &plan.transport, &plan.error_mapping, &plan.call) {
        (
            InteropCallKind::LateDispatch,
            InteropTransport::IDispatch,
            InteropErrorMapping::HresultExcepinfoToVbaErr,
            InteropCallSpec::LateDispatch {
                member_name,
                default_member,
                ..
            },
        ) => {
            if member_name.trim().is_empty() && !*default_member {
                return Err(InteropPlanError::new(
                    "late IDispatch plan requires a member name unless it is a default member",
                ));
            }
        }
        (
            InteropCallKind::DeclareX64,
            InteropTransport::Win64Abi,
            InteropErrorMapping::LastDllErrorImmediate,
            InteropCallSpec::DeclareX64 {
                library,
                entry,
                param_count,
                param_by_ref,
                capture_last_dll_error,
                ..
            },
        ) => {
            if library.trim().is_empty() {
                return Err(InteropPlanError::new("x64 Declare plan requires a library"));
            }
            if entry.trim().is_empty() {
                return Err(InteropPlanError::new(
                    "x64 Declare plan requires an entry/alias",
                ));
            }
            if *param_count != param_by_ref.len() {
                return Err(InteropPlanError::new(
                    "x64 Declare plan param_count must match param_by_ref length",
                ));
            }
            if !*capture_last_dll_error {
                return Err(InteropPlanError::new(
                    "x64 Declare plan must capture LastDllError immediately after the native call",
                ));
            }
        }
        (InteropCallKind::DeclareX64, InteropTransport::IDispatch, ..) => {
            return Err(InteropPlanError::new(
                "x64 Declare plan must not fall back to IDispatch",
            ));
        }
        _ => {
            return Err(InteropPlanError::new(
                "verified interop plan kind, transport, error mapping, and call spec are inconsistent",
            ));
        }
    }
    let expected = plan.canonical_identity();
    if plan.identity.canonical != expected {
        return Err(InteropPlanError::new(
            "verified interop plan identity does not match canonical fields",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn late_count() -> VerifiedInteropPlan {
        VerifiedInteropPlan::late_dispatch(
            "Count",
            false,
            InteropInvokeKind::PropertyGet,
            0,
            Vec::new(),
        )
        .expect("canonical late Count plan")
    }

    fn declare_sqrt() -> VerifiedInteropPlan {
        VerifiedInteropPlan::declare_x64(
            1,
            "msvcrt",
            "sqrt",
            "CDecl",
            vec![false],
            Some("Double".to_string()),
        )
        .expect("canonical declare sqrt plan")
    }

    #[test]
    fn late_dispatch_and_declare_plans_verify() {
        let late = late_count();
        let declare = declare_sqrt();
        assert_eq!(late.kind, InteropCallKind::LateDispatch);
        assert_eq!(late.transport, InteropTransport::IDispatch);
        assert_eq!(declare.kind, InteropCallKind::DeclareX64);
        assert_eq!(declare.transport, InteropTransport::Win64Abi);
        assert_ne!(late.identity, declare.identity);
    }

    #[test]
    fn plan_identity_is_backend_neutral() {
        let a = late_count();
        let b = late_count();
        assert_eq!(a.identity, b.identity);
        let c = declare_sqrt();
        let d = declare_sqrt();
        assert_eq!(c.identity, d.identity);
    }

    #[test]
    fn verifier_rejects_empty_late_member() {
        let err =
            VerifiedInteropPlan::late_dispatch("", false, InteropInvokeKind::Method, 0, Vec::new())
                .expect_err("empty member");
        assert!(err.message.contains("member name"));
    }

    #[test]
    fn verifier_rejects_empty_declare_library() {
        let err = VerifiedInteropPlan::declare_x64(
            1,
            "",
            "sqrt",
            "CDecl",
            vec![false],
            Some("Double".to_string()),
        )
        .expect_err("empty library");
        assert!(err.message.contains("library"));
    }

    #[test]
    fn verifier_rejects_missing_last_dll_error_capture() {
        let mut plan = declare_sqrt();
        if let InteropCallSpec::DeclareX64 {
            capture_last_dll_error,
            ..
        } = &mut plan.call
        {
            *capture_last_dll_error = false;
        }
        plan.identity.canonical = plan.canonical_identity();
        let err = verify_interop_plan(&plan).expect_err("missing LastDllError");
        assert!(err.message.contains("LastDllError"));
    }

    #[test]
    fn verifier_rejects_idispatch_fallback_on_declare() {
        let mut plan = declare_sqrt();
        plan.transport = InteropTransport::IDispatch;
        plan.identity.canonical = plan.canonical_identity();
        let err = verify_interop_plan(&plan).expect_err("IDispatch fallback");
        assert!(err.message.contains("IDispatch"));
    }

    #[test]
    fn verifier_rejects_tampered_identity() {
        let mut plan = late_count();
        plan.identity.canonical = "tampered".to_string();
        let err = verify_interop_plan(&plan).expect_err("tampered identity");
        assert!(err.message.contains("identity"));
    }

    #[test]
    fn default_member_may_omit_name() {
        let plan =
            VerifiedInteropPlan::late_dispatch("", true, InteropInvokeKind::Method, 0, Vec::new())
                .expect("default member");
        assert!(matches!(
            plan.call,
            InteropCallSpec::LateDispatch {
                default_member: true,
                ..
            }
        ));
    }
}
