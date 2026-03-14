use crate::{
    ComBinding, ComDirectDispatchSpec, ComInvokeArg, ComInvokeRequest, ComMemberSpec,
    ComMemberToken, DISPATCH_INVOKE_MISSING_ARG_TOKEN,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundRuntimeInvokePlan {
    pub effective_member: ComMemberToken,
    pub effective_cached_dispid: Option<i32>,
    pub named_default_member_spec: Option<(ComMemberToken, ComMemberSpec)>,
    pub direct_dispatch_spec: Option<ComDirectDispatchSpec>,
    pub legacy_vtable_candidate_args: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnboundRuntimeInvokePlan {
    MemberSpec(ComMemberSpec),
    DirectPropertyGet { dispid: ComMemberToken },
}

pub fn legacy_runtime_arg_values(args: &[ComInvokeArg]) -> Option<Vec<i32>> {
    args.iter()
        .map(|arg| match arg.value.as_ref() {
            Some(value) => value.to_runtime_token().ok(),
            None => Some(DISPATCH_INVOKE_MISSING_ARG_TOKEN),
        })
        .collect()
}

pub fn validate_named_arg_order(args: &[ComInvokeArg]) -> Result<usize, String> {
    let mut named_started = false;
    let mut named_count = 0usize;
    for arg in args {
        if arg.name.is_some() {
            named_started = true;
            named_count += 1;
        } else if named_started {
            return Err("named COM invoke arguments must trail positional arguments".to_string());
        }
    }
    Ok(named_count)
}

pub fn canonicalize_member_known_args(
    spec: &ComMemberSpec,
    args: &[ComInvokeArg],
) -> Result<Vec<ComInvokeArg>, String> {
    if spec.parameter_names.is_empty() || args.is_empty() {
        return Ok(args.to_vec());
    }
    if args.len() > spec.parameter_names.len() {
        return Err(format!(
            "member `{}` received {} arguments but only {} are defined in current metadata",
            spec.name,
            args.len(),
            spec.parameter_names.len()
        ));
    }
    let mut ordered: Vec<Option<ComInvokeArg>> = vec![None; spec.parameter_names.len()];
    let mut next_positional = 0usize;
    for arg in args {
        if let Some(name) = &arg.name {
            let Some(index) = spec
                .parameter_names
                .iter()
                .position(|candidate| candidate.eq_ignore_ascii_case(name))
            else {
                return Err(format!(
                    "named COM argument `{name}` is not defined for member `{}` in current metadata",
                    spec.name
                ));
            };
            if ordered[index].is_some() {
                return Err(format!(
                    "named COM argument `{name}` was provided more than once for member `{}`",
                    spec.name
                ));
            }
            ordered[index] = Some(arg.clone());
            continue;
        }
        while next_positional < ordered.len() && ordered[next_positional].is_some() {
            next_positional += 1;
        }
        if next_positional >= ordered.len() {
            return Err(format!(
                "member `{}` received too many positional arguments for current metadata",
                spec.name
            ));
        }
        ordered[next_positional] = Some(arg.clone());
        next_positional += 1;
    }
    Ok(ordered
        .into_iter()
        .zip(spec.parameter_names.iter())
        .map(|(arg, name)| arg.unwrap_or_else(|| ComInvokeArg::omitted_named(name.clone())))
        .collect())
}

pub fn plan_unbound_runtime_invoke(
    member: ComMemberToken,
    args: &[ComInvokeArg],
    known_spec: Option<ComMemberSpec>,
) -> Result<UnboundRuntimeInvokePlan, String> {
    if let Some(spec) = known_spec {
        return Ok(UnboundRuntimeInvokePlan::MemberSpec(spec));
    }
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(
            "named arguments require a resolved COM member name and remain unsupported for default-member/direct-DISPID dispatch".to_string(),
        );
    }
    Ok(UnboundRuntimeInvokePlan::DirectPropertyGet { dispid: member })
}

pub fn plan_bound_runtime_invoke(
    binding: &ComBinding,
    request: &ComInvokeRequest,
    cached_dispid: Option<i32>,
) -> Result<BoundRuntimeInvokePlan, String> {
    let args = request.args.as_slice();
    let _ = validate_named_arg_order(args)?;
    let has_named_args = args.iter().any(|arg| arg.name.is_some());
    let named_default_member_spec = if request.member.raw() == 0 && has_named_args {
        binding.default_member_token.and_then(|token| {
            binding
                .member_specs
                .get(&token)
                .cloned()
                .map(|spec| (token, spec))
        })
    } else {
        None
    };
    if request.member.raw() == 0 && has_named_args && named_default_member_spec.is_none() {
        return Err(
            "default member identity unavailable for named late-bound dispatch".to_string(),
        );
    }
    let effective_member = named_default_member_spec
        .as_ref()
        .map(|(token, _)| *token)
        .unwrap_or(request.member);
    let effective_cached_dispid = if effective_member == request.member {
        cached_dispid
    } else {
        binding.member_dispids.get(&effective_member).copied()
    };
    let direct_dispatch_spec = binding
        .direct_dispatch_specs
        .get(&effective_member)
        .copied();
    let legacy_vtable_candidate_args = if args
        .iter()
        .all(|arg| arg.name.is_none() && arg.value.is_some())
    {
        legacy_runtime_arg_values(args)
    } else {
        None
    };
    Ok(BoundRuntimeInvokePlan {
        effective_member,
        effective_cached_dispid,
        named_default_member_spec,
        direct_dispatch_spec,
        legacy_vtable_candidate_args,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        ComBinding, ComDirectDispatchSpec, ComInvokeArg, ComInvokeRequest, ComMemberSpec,
        ComMemberToken, ComValue, TypeLibMemberInvokeKind,
    };
    use oxvba_runtime::ObjectHandle;

    use super::{
        canonicalize_member_known_args, legacy_runtime_arg_values, plan_bound_runtime_invoke,
        plan_unbound_runtime_invoke, validate_named_arg_order,
    };

    #[test]
    fn validate_named_arg_order_requires_named_args_to_trail() {
        let err = validate_named_arg_order(&[
            ComInvokeArg::named_value(ComValue::I32(1), "lhs"),
            ComInvokeArg::positional_value(ComValue::I32(2)),
        ])
        .expect_err("mixed ordering should be rejected");
        assert!(err.contains("must trail positional"));
    }

    #[test]
    fn canonicalize_member_known_args_orders_named_and_omitted_parameters() {
        let spec = ComMemberSpec {
            name: "Pair".to_string(),
            requires_argument: true,
            invoke_kind: TypeLibMemberInvokeKind::Method,
            parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
            is_default_member: false,
        };
        let args = canonicalize_member_known_args(
            &spec,
            &[ComInvokeArg::named_value(ComValue::I32(7), "rhs")],
        )
        .expect("canonicalization should succeed");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], ComInvokeArg::omitted_named("lhs"));
        assert_eq!(args[1], ComInvokeArg::named_value(ComValue::I32(7), "rhs"));
    }

    #[test]
    fn plan_unbound_runtime_invoke_rejects_named_args_without_known_member() {
        let err = plan_unbound_runtime_invoke(
            ComMemberToken::new(9),
            &[ComInvokeArg::named_value(ComValue::I32(5), "value")],
            None,
        )
        .expect_err("named unbound dispatch without metadata should fail");
        assert!(err.contains("resolved COM member name"));
    }

    #[test]
    fn plan_bound_runtime_invoke_uses_named_default_member_when_available() {
        let mut binding = ComBinding::new("Test.Dispatch".to_string(), 1);
        binding.default_member_token = Some(ComMemberToken::new(17));
        binding.member_specs.insert(
            ComMemberToken::new(17),
            ComMemberSpec {
                name: "Value".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["value".to_string()],
                is_default_member: true,
            },
        );
        let request = ComInvokeRequest {
            object: ObjectHandle::new(20_001),
            member: ComMemberToken::new(0),
            args: vec![ComInvokeArg::named_value(ComValue::I32(19), "value")],
            invoke_kind_hint: None,
        };
        let plan = plan_bound_runtime_invoke(&binding, &request, None)
            .expect("default-member plan should succeed");
        assert_eq!(plan.effective_member.raw(), 17);
        assert!(plan.named_default_member_spec.is_some());
    }

    #[test]
    fn plan_bound_runtime_invoke_rejects_named_default_member_without_identity() {
        let binding = ComBinding::new("Test.Dispatch".to_string(), 1);
        let request = ComInvokeRequest {
            object: ObjectHandle::new(20_001),
            member: ComMemberToken::new(0),
            args: vec![ComInvokeArg::named_value(ComValue::I32(19), "value")],
            invoke_kind_hint: None,
        };
        let err = plan_bound_runtime_invoke(&binding, &request, None)
            .expect_err("missing default identity should fail");
        assert!(err.contains("default member identity unavailable"));
    }

    #[test]
    fn plan_bound_runtime_invoke_preserves_direct_dispatch_spec() {
        let mut binding = ComBinding::new("Test.Dispatch".to_string(), 1);
        binding.direct_dispatch_specs.insert(
            ComMemberToken::new(42),
            ComDirectDispatchSpec {
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            },
        );
        let request = ComInvokeRequest {
            object: ObjectHandle::new(20_001),
            member: ComMemberToken::new(42),
            args: Vec::new(),
            invoke_kind_hint: None,
        };
        let plan =
            plan_bound_runtime_invoke(&binding, &request, Some(91)).expect("plan should succeed");
        assert_eq!(plan.effective_member.raw(), 42);
        assert_eq!(plan.effective_cached_dispid, Some(91));
        assert_eq!(
            plan.direct_dispatch_spec,
            Some(ComDirectDispatchSpec {
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet
            })
        );
    }

    #[test]
    fn legacy_runtime_arg_values_preserve_omitted_token() {
        let values = legacy_runtime_arg_values(&[
            ComInvokeArg::positional_value(ComValue::I32(3)),
            ComInvokeArg::omitted(),
        ])
        .expect("legacy values should be recoverable");
        assert_eq!(values, vec![3, crate::DISPATCH_INVOKE_MISSING_ARG_TOKEN]);
    }
}
