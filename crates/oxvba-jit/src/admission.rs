//! Whole-image and procedure admission, plus type-support predicates.

use super::*;

pub(crate) fn program_lowers_native_or_com(program: &OxProgram) -> bool {
    program.funcs.iter().any(|func| {
        func.blocks.iter().any(|block| {
            block
                .instrs
                .iter()
                .any(|inst| matches!(inst, OxInst::ComCallEarly { .. }))
        })
    })
}

pub(crate) fn validate_program_shape(program: &OxProgram) -> Result<(), JitError> {
    if program_lowers_native_or_com(program) {
        return Err(JitError::unsupported("native/COM calls start in M4-9"));
    }
    for global in &program.globals {
        if !is_m4_4_slot_ty(&global.ty) {
            return Err(JitError::unsupported(format!(
                "JIT carrier support accepts scalar, Variant/String/FixedStr/Decimal/ProcRef/Object/Record, and legal SAFEARRAY globals; got {:?}",
                global.ty
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_func_shape(func: &OxFunc) -> Result<(), JitError> {
    if let Some(ret) = func.return_local
        && !func
            .locals
            .get(ret.0)
            .map(|local| is_jit_static_call_ty(&local.ty))
            .unwrap_or(false)
    {
        return Err(JitError::unsupported(format!(
            "JIT static calls accept every supported scalar/carrier return local, got {:?} in {}",
            func.locals.get(ret.0).map(|local| &local.ty),
            func.name
        )));
    }
    for local in &func.locals {
        if !is_m4_4_slot_ty(&local.ty) {
            return Err(JitError::unsupported(format!(
                "JIT carrier support accepts scalar, Variant/String/FixedStr/Decimal/ProcRef/Object/Record, and legal SAFEARRAY locals, got {:?} in {}",
                local.ty, func.name
            )));
        }
    }
    for (index, param) in func.locals.iter().take(func.param_count).enumerate() {
        if let Some(info) = param.param
            && info.variadic
        {
            if index + 1 != func.param_count {
                return Err(JitError::unsupported(format!(
                    "M4-4 ParamArray support requires the variadic parameter to be last, got {}.{}",
                    func.name, param.name
                )));
            }
            if !is_m4_4_supported_paramarray_param(&param.ty, info) {
                return Err(JitError::unsupported(format!(
                    "M4-4 ParamArray support is limited to ByVal Variant or dynamic Variant-array parameters, got {:?} in {}.{}",
                    param.ty, func.name, param.name
                )));
            }
            continue;
        }
        if !is_jit_static_call_ty(&param.ty) {
            return Err(JitError::unsupported(format!(
                "JIT static calls accept every supported scalar/carrier parameter, got {:?} in {}",
                param.ty, func.name
            )));
        }
    }
    for ty in &func.temps {
        if !is_m4_4_slot_ty(ty) {
            return Err(JitError::unsupported(format!(
                "JIT carrier support accepts scalar, Variant/String/FixedStr/Decimal/ProcRef/Object/Record, and legal SAFEARRAY temps, got {ty:?} in {}",
                func.name
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_static_scalar_call_arg(
    callee: &OxFunc,
    index: usize,
    arg: &OxArg,
) -> Result<(), JitError> {
    let Some(param) = callee.locals.get(index) else {
        return Err(JitError::Compile(format!(
            "callee {} has param_count without local {index}",
            callee.name
        )));
    };
    validate_scalar_call_arg_against_param(&callee.name, param, arg)
}

pub(crate) fn validate_scalar_call_arg_against_param(
    callee_name: &str,
    param: &OxLocal,
    arg: &OxArg,
) -> Result<(), JitError> {
    let Some(param_info) = param.param.as_ref() else {
        return Err(JitError::unsupported(format!(
            "M4-4 call subset requires parameter metadata for {}.{}",
            callee_name, param.name
        )));
    };
    if param_info.variadic {
        if !is_m4_4_supported_paramarray_param(&param.ty, *param_info) {
            return Err(JitError::unsupported(format!(
                "M4-4 ParamArray support is limited to ByVal Variant or dynamic Variant-array parameters, got {:?} in {}.{}",
                param.ty, callee_name, param.name
            )));
        }
        return match arg {
            OxArg::ByVal(_) => Ok(()),
            other => Err(JitError::unsupported(format!(
                "M4-4 ParamArray support requires a ByVal packed Variant-array argument for {}.{}, got {other:?}",
                callee_name, param.name
            ))),
        };
    }
    if !is_jit_static_call_ty(&param.ty) {
        return Err(JitError::unsupported(format!(
            "JIT static calls accept every supported non-ParamArray scalar/carrier parameter, got {:?} in {}.{}",
            param.ty, callee_name, param.name
        )));
    }
    match (arg, param_info.by_ref) {
        (OxArg::ByVal(_), false) | (OxArg::ByVal(_), true) | (OxArg::ByRef(_), true) => Ok(()),
        (OxArg::ByRef(_), false) => Err(JitError::unsupported(format!(
            "M4-4 call subset cannot alias ByVal scalar parameter {}.{}",
            callee_name, param.name
        ))),
        (OxArg::Omitted, false) if matches!(param.ty, OxTy::Variant) => Ok(()),
        (OxArg::Omitted, false) => Err(JitError::unsupported(format!(
            "M4-4 omitted arguments are currently limited to Optional ByVal Variant parameters, got {}.{}",
            callee_name, param.name
        ))),
        (OxArg::Omitted, true) => Err(JitError::unsupported(format!(
            "M4-4 omitted arguments cannot satisfy ByRef parameters, got {}.{}",
            callee_name, param.name
        ))),
    }
}

pub(crate) fn unsupported_project_object_inst_message(inst: &OxInst) -> Option<&'static str> {
    match inst {
        OxInst::ComCallEarly { .. } => Some(
            "JIT COM object dispatch instruction ComCallEarly is unsupported: typed COM invocation remains VM3-only",
        ),
        _ => None,
    }
}

pub(crate) fn place_ty<'a>(
    program: &'a OxProgram,
    func: &'a OxFunc,
    place: OxPlace,
) -> Result<&'a OxTy, JitError> {
    match place {
        OxPlace::Local(id) => func
            .locals
            .get(id.0)
            .map(|local| &local.ty)
            .ok_or_else(|| JitError::Compile(format!("local {} out of range", id.0))),
        OxPlace::Global(id) => program
            .globals
            .get(id.0)
            .map(|global| &global.ty)
            .ok_or_else(|| JitError::Compile(format!("global {} out of range", id.0))),
        OxPlace::Temp(id) => func
            .temps
            .get(id.0)
            .ok_or_else(|| JitError::Compile(format!("temp {} out of range", id.0))),
    }
}

pub(crate) fn operand_static_ty<'a>(
    program: &'a OxProgram,
    func: &'a OxFunc,
    operand: &'a OxOperand,
) -> Result<OxTy, JitError> {
    match operand {
        OxOperand::Use(place) => place_ty(program, func, *place).cloned(),
        OxOperand::Const(OxConst::Nothing) => Ok(OxTy::Object(ObjClass::Untyped)),
        OxOperand::Const(OxConst::Bool(_)) => Ok(OxTy::Bool),
        OxOperand::Const(OxConst::I16(_)) => Ok(OxTy::Integer),
        OxOperand::Const(OxConst::I32(_)) => Ok(OxTy::Long),
        OxOperand::Const(OxConst::I64(_)) => Ok(OxTy::LongLong),
        OxOperand::Const(OxConst::F32(_)) => Ok(OxTy::Single),
        OxOperand::Const(OxConst::F64(_)) => Ok(OxTy::Double),
        OxOperand::Const(OxConst::Currency(_)) => Ok(OxTy::Currency),
        OxOperand::Const(OxConst::Date(_)) => Ok(OxTy::Date),
        OxOperand::Const(OxConst::Str(_)) => Ok(OxTy::Str),
        OxOperand::Const(OxConst::Empty | OxConst::Null) => Ok(OxTy::Variant),
    }
}

pub(crate) fn place_addr(place: OxPlace) -> (u32, u32) {
    match place {
        OxPlace::Global(id) => (AREA_GLOBAL, id.0 as u32),
        OxPlace::Local(id) => (AREA_LOCAL, id.0 as u32),
        OxPlace::Temp(id) => (AREA_TEMP, id.0 as u32),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JitTypeSupport {
    FastScalar,
    VariantScalar,
    ArrayCarrier,
    UnsupportedIrOnly,
}

pub(crate) fn classify_jit_ty(ty: &OxTy) -> JitTypeSupport {
    match ty {
        OxTy::Long
        | OxTy::LongLong
        | OxTy::Currency
        | OxTy::Single
        | OxTy::Double
        | OxTy::Date
        | OxTy::Byte
        | OxTy::Integer
        | OxTy::Bool => JitTypeSupport::FastScalar,
        OxTy::Variant
        | OxTy::Str
        | OxTy::FixedStr(_)
        | OxTy::Decimal
        | OxTy::ProcRef
        | OxTy::Object(_)
        | OxTy::Record(_) => JitTypeSupport::VariantScalar,
        OxTy::Array(element, _) if is_jit_array_element_ty(element) => JitTypeSupport::ArrayCarrier,
        OxTy::Array(_, _) => JitTypeSupport::UnsupportedIrOnly,
    }
}

pub(crate) fn is_jit_array_element_ty(ty: &OxTy) -> bool {
    match ty {
        OxTy::Array(_, _) | OxTy::ProcRef => false,
        _ => matches!(
            classify_jit_ty(ty),
            JitTypeSupport::FastScalar | JitTypeSupport::VariantScalar
        ),
    }
}

pub(crate) fn is_jit_supported_slot_ty(ty: &OxTy) -> bool {
    !matches!(classify_jit_ty(ty), JitTypeSupport::UnsupportedIrOnly)
}

pub(crate) fn is_jit_variant_carrier_ty(ty: &OxTy) -> bool {
    matches!(
        classify_jit_ty(ty),
        JitTypeSupport::VariantScalar | JitTypeSupport::ArrayCarrier
    )
}

pub(crate) fn is_project_object_static_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Object(ObjClass::Class(_)) | OxTy::Object(ObjClass::Iface(_))
    )
}

pub(crate) fn is_jit_static_call_ty(ty: &OxTy) -> bool {
    is_jit_supported_slot_ty(ty)
}

pub(crate) fn is_m4_4_slot_ty(ty: &OxTy) -> bool {
    is_jit_supported_slot_ty(ty)
}

pub(crate) fn is_m4_4_call_scalar_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Long
            | OxTy::LongLong
            | OxTy::Currency
            | OxTy::Single
            | OxTy::Double
            | OxTy::Date
            | OxTy::Byte
            | OxTy::Integer
            | OxTy::Bool
            | OxTy::Str
            | OxTy::Variant
    )
}

pub(crate) fn is_m4_4_dynamic_variant_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Variant)
    )
}

pub(crate) fn is_m4_4_fixed_variant_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Variant)
    )
}

pub(crate) fn is_m4_4_dynamic_long_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Long)
    )
}

pub(crate) fn is_m4_4_fixed_long_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Long)
    )
}

pub(crate) fn is_m4_4_long_array_ty(ty: &OxTy) -> bool {
    is_m4_4_dynamic_long_array_ty(ty) || is_m4_4_fixed_long_array_ty(ty)
}

pub(crate) fn is_m4_4_dynamic_longlong_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::LongLong)
    )
}

pub(crate) fn is_m4_4_fixed_longlong_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::LongLong)
    )
}

pub(crate) fn is_m4_4_dynamic_single_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Single)
    )
}

pub(crate) fn is_m4_4_fixed_single_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Single)
    )
}

pub(crate) fn is_m4_4_dynamic_double_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Double)
    )
}

pub(crate) fn is_m4_4_fixed_double_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Double)
    )
}

pub(crate) fn is_m4_4_dynamic_currency_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Currency)
    )
}

pub(crate) fn is_m4_4_fixed_currency_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Currency)
    )
}

pub(crate) fn is_m4_4_dynamic_date_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Date)
    )
}

pub(crate) fn is_m4_4_fixed_date_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Date)
    )
}

pub(crate) fn is_m4_4_dynamic_string_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic)
            if is_m4_4_string_like_array_element_ty(element.as_ref())
    )
}

pub(crate) fn is_m4_4_fixed_string_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. })
            if is_m4_4_string_like_array_element_ty(element.as_ref())
    )
}

pub(crate) fn is_m4_4_string_like_array_element_ty(ty: &OxTy) -> bool {
    matches!(ty, OxTy::Str | OxTy::FixedStr(_))
}

pub(crate) fn is_m4_4_dynamic_integer_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Integer)
    )
}

pub(crate) fn is_m4_4_fixed_integer_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Integer)
    )
}

pub(crate) fn is_m4_4_dynamic_byte_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Byte)
    )
}

pub(crate) fn is_m4_4_fixed_byte_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Byte)
    )
}

pub(crate) fn is_m4_4_dynamic_boolean_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Dynamic) if matches!(element.as_ref(), OxTy::Bool)
    )
}

pub(crate) fn is_m4_4_fixed_boolean_array_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Array(element, ArrayShape::Fixed { .. }) if matches!(element.as_ref(), OxTy::Bool)
    )
}

pub(crate) fn is_m4_4_dynamic_array_ty_for_element(ty: &OxTy, element: &ArrayElementType) -> bool {
    match element {
        ArrayElementType::Record(_) => matches!(
            ty,
            OxTy::Array(slot, ArrayShape::Dynamic)
                if matches!(slot.as_ref(), OxTy::Record(_) | OxTy::Object(_))
        ),
        ArrayElementType::FixedString(_) => is_m4_4_dynamic_string_array_ty(ty),
        ArrayElementType::FixedArray { .. } => is_m4_4_dynamic_variant_array_ty(ty),
        ArrayElementType::Variant => {
            matches!(ty, OxTy::Variant)
                || matches!(
                    ty,
                    OxTy::Array(slot, ArrayShape::Dynamic) if is_jit_array_element_ty(slot)
                )
        }
        ArrayElementType::Long => is_m4_4_dynamic_long_array_ty(ty),
        ArrayElementType::LongLong => is_m4_4_dynamic_longlong_array_ty(ty),
        ArrayElementType::Single => is_m4_4_dynamic_single_array_ty(ty),
        ArrayElementType::Double => is_m4_4_dynamic_double_array_ty(ty),
        ArrayElementType::Currency => is_m4_4_dynamic_currency_array_ty(ty),
        ArrayElementType::Date => is_m4_4_dynamic_date_array_ty(ty),
        ArrayElementType::String => is_m4_4_dynamic_string_array_ty(ty),
        ArrayElementType::Integer => is_m4_4_dynamic_integer_array_ty(ty),
        ArrayElementType::Byte => is_m4_4_dynamic_byte_array_ty(ty),
        ArrayElementType::Boolean => is_m4_4_dynamic_boolean_array_ty(ty),
    }
}

pub(crate) fn is_m4_4_fixed_array_ty_for_element(ty: &OxTy, element: &ArrayElementType) -> bool {
    match element {
        ArrayElementType::Record(_) => matches!(
            ty,
            OxTy::Array(slot, ArrayShape::Fixed { .. })
                if matches!(slot.as_ref(), OxTy::Record(_) | OxTy::Object(_))
        ),
        ArrayElementType::FixedString(_) => is_m4_4_fixed_string_array_ty(ty),
        ArrayElementType::FixedArray { .. } => is_m4_4_fixed_variant_array_ty(ty),
        ArrayElementType::Variant => matches!(
            ty,
            OxTy::Array(slot, ArrayShape::Fixed { .. }) if is_jit_array_element_ty(slot)
        ),
        ArrayElementType::Long => is_m4_4_fixed_long_array_ty(ty),
        ArrayElementType::LongLong => is_m4_4_fixed_longlong_array_ty(ty),
        ArrayElementType::Single => is_m4_4_fixed_single_array_ty(ty),
        ArrayElementType::Double => is_m4_4_fixed_double_array_ty(ty),
        ArrayElementType::Currency => is_m4_4_fixed_currency_array_ty(ty),
        ArrayElementType::Date => is_m4_4_fixed_date_array_ty(ty),
        ArrayElementType::String => is_m4_4_fixed_string_array_ty(ty),
        ArrayElementType::Integer => is_m4_4_fixed_integer_array_ty(ty),
        ArrayElementType::Byte => is_m4_4_fixed_byte_array_ty(ty),
        ArrayElementType::Boolean => is_m4_4_fixed_boolean_array_ty(ty),
    }
}

pub(crate) fn is_m4_4_array_ty_for_element(ty: &OxTy, element: &ArrayElementType) -> bool {
    is_m4_4_dynamic_array_ty_for_element(ty, element)
        || is_m4_4_fixed_array_ty_for_element(ty, element)
}

pub(crate) fn is_m4_4_variant_array_carrier_ty(ty: &OxTy) -> bool {
    matches!(ty, OxTy::Variant) || is_m4_4_dynamic_variant_array_ty(ty)
}

pub(crate) fn is_m4_4_array_index_carrier_ty(ty: &OxTy) -> bool {
    matches!(ty, OxTy::Variant) || matches!(classify_jit_ty(ty), JitTypeSupport::ArrayCarrier)
}

pub(crate) fn is_m4_4_array_get_dst_ty_for_array(dst_ty: &OxTy, array_ty: &OxTy) -> bool {
    if matches!(dst_ty, OxTy::Variant) {
        return true;
    }
    let OxTy::Array(element, _) = array_ty else {
        return false;
    };
    element.as_ref() == dst_ty
        || matches!(
            (element.as_ref(), dst_ty),
            (OxTy::Str | OxTy::FixedStr(_), OxTy::Str | OxTy::FixedStr(_))
        )
}

pub(crate) fn is_m4_4_for_each_source_ty(ty: &OxTy) -> bool {
    is_m4_4_variant_descriptor_operand_ty(ty) || matches!(ty, OxTy::Str)
}

pub(crate) fn is_m4_4_supported_paramarray_param(
    ty: &OxTy,
    param_info: oxvba_oxir::OxParamInfo,
) -> bool {
    param_info.variadic
        && !param_info.by_ref
        && (matches!(ty, OxTy::Variant) || is_m4_4_dynamic_variant_array_ty(ty))
}

pub(crate) fn is_m4_4_call_return_destination_ty(dst_ty: &OxTy, ret_ty: &OxTy) -> bool {
    dst_ty == ret_ty || matches!(dst_ty, OxTy::Variant)
}

pub(crate) fn is_m4_4_unknown_proc_ref_variant_return_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Long
            | OxTy::LongLong
            | OxTy::Currency
            | OxTy::Single
            | OxTy::Double
            | OxTy::Date
            | OxTy::Byte
            | OxTy::Integer
            | OxTy::Bool
            | OxTy::Str
            | OxTy::Variant
    )
}

pub(crate) fn unknown_proc_ref_exact_return_kind(ty: &OxTy) -> Option<i32> {
    match ty {
        OxTy::LongLong => Some(JIT_PROC_REF_RET_EXACT_LONGLONG),
        OxTy::Currency => Some(JIT_PROC_REF_RET_EXACT_CURRENCY),
        OxTy::Single => Some(JIT_PROC_REF_RET_EXACT_SINGLE),
        OxTy::Double => Some(JIT_PROC_REF_RET_EXACT_DOUBLE),
        OxTy::Date => Some(JIT_PROC_REF_RET_EXACT_DATE),
        OxTy::Byte => Some(JIT_PROC_REF_RET_EXACT_BYTE),
        OxTy::Integer => Some(JIT_PROC_REF_RET_EXACT_INTEGER),
        OxTy::Bool => Some(JIT_PROC_REF_RET_EXACT_BOOL),
        _ => None,
    }
}

pub(crate) fn unknown_proc_ref_exact_return_matches(kind: i32, ty: &OxTy) -> bool {
    matches!(
        (kind, ty),
        (JIT_PROC_REF_RET_EXACT_LONGLONG, OxTy::LongLong)
            | (JIT_PROC_REF_RET_EXACT_CURRENCY, OxTy::Currency)
            | (JIT_PROC_REF_RET_EXACT_SINGLE, OxTy::Single)
            | (JIT_PROC_REF_RET_EXACT_DOUBLE, OxTy::Double)
            | (JIT_PROC_REF_RET_EXACT_DATE, OxTy::Date)
            | (JIT_PROC_REF_RET_EXACT_BYTE, OxTy::Byte)
            | (JIT_PROC_REF_RET_EXACT_INTEGER, OxTy::Integer)
            | (JIT_PROC_REF_RET_EXACT_BOOL, OxTy::Bool)
    )
}

pub(crate) fn is_m4_4_call_extern_destination_ty(ty: &OxTy) -> bool {
    matches!(
        ty,
        OxTy::Long
            | OxTy::LongLong
            | OxTy::Currency
            | OxTy::Single
            | OxTy::Double
            | OxTy::Date
            | OxTy::Byte
            | OxTy::Integer
            | OxTy::Bool
            | OxTy::Str
            | OxTy::FixedStr(_)
            | OxTy::Variant
    )
}

pub(crate) fn is_m4_4_variant_operand_ty(ty: &OxTy) -> bool {
    is_m4_4_dynamic_variant_array_ty(ty)
        || matches!(
            ty,
            OxTy::Long
                | OxTy::LongLong
                | OxTy::Currency
                | OxTy::Single
                | OxTy::Double
                | OxTy::Date
                | OxTy::Byte
                | OxTy::Integer
                | OxTy::Bool
                | OxTy::Variant
                | OxTy::FixedStr(_)
        )
}

pub(crate) fn is_m4_4_variant_descriptor_operand_ty(ty: &OxTy) -> bool {
    is_jit_supported_slot_ty(ty)
}

pub(crate) fn scalar_unary_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Abs => {
            Some("Abs(Null/Empty/Boolean/Integer/Long/LongLong/Single/Double/Currency)")
        }
        NativeImplId::Int | NativeImplId::Fix => Some(
            "Int/Fix(Null/Empty/Boolean/Integer/Long/LongLong/Single/Double/Currency/Date/Variant)",
        ),
        NativeImplId::Sgn => {
            Some("Sgn(Null/Empty/Boolean/Integer/Long/LongLong/Single/Double/Currency)")
        }
        NativeImplId::CVErr => {
            Some("CVErr(Null/Empty/Boolean/Integer/Long/LongLong/Single/Double/Currency)")
        }
        NativeImplId::Hex | NativeImplId::Oct | NativeImplId::Str => {
            Some("Hex/Oct/Str(Integer/Long/LongLong/Single/Double/Currency)")
        }
        NativeImplId::Round => Some("Round(Integer/Long/LongLong/Single/Double/Currency)"),
        NativeImplId::Sqr
        | NativeImplId::Sin
        | NativeImplId::Cos
        | NativeImplId::Log
        | NativeImplId::Exp
        | NativeImplId::Atn
        | NativeImplId::Tan => {
            Some("Sqr/Sin/Cos/Log/Exp/Atn/Tan(Integer/Long/LongLong/Single/Double/Currency)")
        }
        NativeImplId::Year
        | NativeImplId::Month
        | NativeImplId::Day
        | NativeImplId::Weekday
        | NativeImplId::Hour
        | NativeImplId::Minute
        | NativeImplId::Second => Some("Year/Month/Day/Weekday/Hour/Minute/Second(Date/Variant)"),
        NativeImplId::DateValue | NativeImplId::TimeValue => {
            Some("DateValue/TimeValue(Date/StringLiteral)")
        }
        NativeImplId::MonthName | NativeImplId::WeekdayName => {
            Some("MonthName/WeekdayName(Integer/Long/LongLong)")
        }
        NativeImplId::QbColor => Some("QBColor(Integer/Long/LongLong)"),
        NativeImplId::ErrorText => Some("Error(Integer/Long/LongLong)"),
        NativeImplId::Chr | NativeImplId::ChrW => Some("Chr/ChrW(Integer/Long/LongLong)"),
        NativeImplId::Space => Some("Space(Integer/Long/LongLong)"),
        NativeImplId::Asc | NativeImplId::AscW => Some("Asc/AscW(Variant/scalar const)"),
        NativeImplId::LCase | NativeImplId::UCase => Some("LCase/UCase(Variant)"),
        NativeImplId::Trim | NativeImplId::LTrim | NativeImplId::RTrim => {
            Some("Trim/LTrim/RTrim(Variant)")
        }
        NativeImplId::StrReverse => Some("StrReverse(Variant)"),
        NativeImplId::Val => Some("Val(Variant)"),
        NativeImplId::Len | NativeImplId::LenB => Some("Len/LenB(Variant/scalar const)"),
        NativeImplId::Format => Some("Format(Variant/scalar const)"),
        NativeImplId::IsArray
        | NativeImplId::VarType
        | NativeImplId::TypeName
        | NativeImplId::IsNumeric
        | NativeImplId::IsDate
        | NativeImplId::IsObject
        | NativeImplId::IsNull
        | NativeImplId::IsEmpty
        | NativeImplId::IsError
        | NativeImplId::IsMissing => {
            Some("VarType/TypeName/Is*(scalar/Variant/Variant()/StringLiteral)")
        }
        NativeImplId::CStr => Some("CStr(scalar)"),
        NativeImplId::CDate => Some("CDate(scalar/String literal/place)"),
        NativeImplId::CBool => Some("CBool(scalar)"),
        NativeImplId::CByte => Some("CByte(scalar)"),
        NativeImplId::CInt => Some("CInt(scalar)"),
        NativeImplId::CLng => Some("CLng(scalar)"),
        NativeImplId::CLngLng => Some("CLngLng(scalar)"),
        NativeImplId::CLngPtr => Some("CLngPtr(scalar)"),
        NativeImplId::CSng => Some("CSng(scalar)"),
        NativeImplId::CDbl => Some("CDbl(scalar)"),
        NativeImplId::CCur => Some("CCur(scalar)"),
        NativeImplId::CDec => Some("CDec(scalar)"),
        NativeImplId::CVar => Some("CVar(scalar)"),
        _ => None,
    }
}

pub(crate) fn scalar_optional_fixed_call_extern_shape(
    native_impl: NativeImplId,
) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Round => Some("Round(number, digits)"),
        _ => None,
    }
}

pub(crate) fn date_part_optional_fixed_call_extern_shape(
    native_impl: NativeImplId,
) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Weekday => Some("Weekday(date, firstday)"),
        _ => None,
    }
}

pub(crate) fn date_name_optional_call_extern_shape(
    native_impl: NativeImplId,
) -> Option<&'static str> {
    match native_impl {
        NativeImplId::MonthName => Some("MonthName(month, abbreviate)"),
        NativeImplId::WeekdayName => Some("WeekdayName(weekday, abbreviate, firstday)"),
        _ => None,
    }
}

pub(crate) fn random_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Rnd => Some("Rnd([number])"),
        NativeImplId::Randomize => Some("Randomize([number])"),
        _ => None,
    }
}

pub(crate) fn scalar_double_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::StringRepeat => Some("String(Integer/Long/LongLong x2)"),
        _ => None,
    }
}

pub(crate) fn variant_double_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::InStr
        | NativeImplId::InStrRev
        | NativeImplId::StrComp
        | NativeImplId::Split
        | NativeImplId::Join => Some("InStr/InStrRev/StrComp/Split/Join(Variant/scalar const x2)"),
        _ => None,
    }
}

pub(crate) fn variant_triple_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Replace => Some("Replace(Variant x3)"),
        NativeImplId::Rate | NativeImplId::NPer => Some("Rate/NPer(Variant/scalar const x3)"),
        _ => None,
    }
}

pub(crate) fn variant_quad_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Fv | NativeImplId::Pv | NativeImplId::Pmt => {
            Some("FV/PV/PMT(Variant/scalar const x4)")
        }
        _ => None,
    }
}

pub(crate) fn variant_string_optional_call_extern_shape(
    native_impl: NativeImplId,
) -> Option<&'static str> {
    match native_impl {
        NativeImplId::InStr => Some("InStr(start, Variant, Variant[, compare])"),
        NativeImplId::InStrRev => Some("InStrRev(Variant, Variant[, start[, compare]])"),
        NativeImplId::StrComp => Some("StrComp(Variant, Variant, compare)"),
        NativeImplId::Replace => Some("Replace(Variant x3[, start[, count[, compare]]])"),
        _ => None,
    }
}

pub(crate) fn variant_fixed_double_call_extern_shape(
    native_impl: NativeImplId,
) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Left | NativeImplId::LeftB | NativeImplId::Right | NativeImplId::RightB => {
            Some("Left/Right/LeftB/RightB(Variant/scalar const, Integer/Long/LongLong)")
        }
        NativeImplId::Mid => Some("Mid(Variant/scalar const, Integer/Long/LongLong)"),
        NativeImplId::StrConv => Some("StrConv(Variant/scalar const, Integer/Long/LongLong)"),
        _ => None,
    }
}

pub(crate) fn variant_fixed_triple_call_extern_shape(
    native_impl: NativeImplId,
) -> Option<&'static str> {
    match native_impl {
        NativeImplId::Mid => Some("Mid(Variant/scalar const, Integer/Long/LongLong x2)"),
        _ => None,
    }
}

pub(crate) fn date_interval_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::DateAdd => {
            Some("DateAdd(Variant/scalar const, Variant/scalar const, Date/Variant)")
        }
        NativeImplId::DateDiff => Some("DateDiff(Variant/scalar const, Date/Variant x2)"),
        _ => None,
    }
}

pub(crate) fn scalar_triple_call_extern_shape(native_impl: NativeImplId) -> Option<&'static str> {
    match native_impl {
        NativeImplId::DateSerial | NativeImplId::TimeSerial => {
            Some("DateSerial/TimeSerial(Integer/Long/LongLong x3)")
        }
        NativeImplId::Rgb => Some("RGB(Integer/Long/LongLong x3)"),
        _ => None,
    }
}

pub(crate) fn raw_unbox_target(ty: &OxTy) -> Option<i32> {
    match ty {
        OxTy::Variant => Some(-1),
        OxTy::Bool => Some(VarType::Boolean as i32),
        OxTy::Byte => Some(VarType::Byte as i32),
        OxTy::Integer => Some(VarType::Integer as i32),
        OxTy::Long => Some(VarType::Long as i32),
        OxTy::LongLong => Some(VarType::LongLong as i32),
        OxTy::Single => Some(VarType::Single as i32),
        OxTy::Double => Some(VarType::Double as i32),
        OxTy::Currency => Some(VarType::Currency as i32),
        OxTy::Date => Some(VarType::Date as i32),
        OxTy::Decimal => Some(VarType::Decimal as i32),
        OxTy::Str | OxTy::FixedStr(_) => Some(VarType::String as i32),
        OxTy::Object(_) => Some(VarType::Object as i32),
        OxTy::Record(_) => Some(VarType::Record as i32),
        OxTy::Array(_, _) => Some(VarType::ArrayVariant as i32),
        OxTy::ProcRef => Some(VarType::ProcRef as i32),
    }
}

pub(crate) fn raw_assignment_intent(intent: AssignmentIntent) -> i32 {
    match intent {
        AssignmentIntent::Implicit => JIT_ASSIGN_INTENT_IMPLICIT,
        AssignmentIntent::Let => JIT_ASSIGN_INTENT_LET,
        AssignmentIntent::Set => JIT_ASSIGN_INTENT_SET,
    }
}

pub(crate) fn raw_assignment_target_kind(kind: AssignmentTargetKind) -> i32 {
    match kind {
        AssignmentTargetKind::Variant => JIT_ASSIGN_TARGET_VARIANT,
        AssignmentTargetKind::Object => JIT_ASSIGN_TARGET_OBJECT,
        AssignmentTargetKind::Scalar => JIT_ASSIGN_TARGET_SCALAR,
    }
}

pub(crate) fn raw_member_invoke_kind(kind: TypeLibMemberInvokeKind) -> i32 {
    match kind {
        TypeLibMemberInvokeKind::PropertyGet => 2,
        TypeLibMemberInvokeKind::Method => 1,
        TypeLibMemberInvokeKind::PropertyPut => 4,
        TypeLibMemberInvokeKind::PropertyPutRef => 8,
    }
}

pub(crate) fn raw_err_field(field: ErrField) -> u32 {
    match field {
        ErrField::Number => RT_ERR_FIELD_NUMBER,
        ErrField::Description => RT_ERR_FIELD_DESCRIPTION,
        ErrField::Source => RT_ERR_FIELD_SOURCE,
        ErrField::HelpFile => RT_ERR_FIELD_HELP_FILE,
        ErrField::HelpContext => RT_ERR_FIELD_HELP_CONTEXT,
        ErrField::LastDllError => RT_ERR_FIELD_LAST_DLL_ERROR,
    }
}

pub(crate) fn raw_arith_op(op: ArithOp) -> Option<u32> {
    match op {
        ArithOp::Add => Some(RT_ARITH_ADD),
        ArithOp::Sub => Some(RT_ARITH_SUB),
        ArithOp::Mul => Some(RT_ARITH_MUL),
        ArithOp::IntDiv => Some(RT_ARITH_INT_DIV),
        ArithOp::Mod => Some(RT_ARITH_MOD),
    }
}

pub(crate) fn raw_logical_op(op: LogicalOp) -> u32 {
    match op {
        LogicalOp::And => RT_LOGIC_AND,
        LogicalOp::Or => RT_LOGIC_OR,
        LogicalOp::Xor => RT_LOGIC_XOR,
        LogicalOp::Eqv => RT_LOGIC_EQV,
        LogicalOp::Imp => RT_LOGIC_IMP,
    }
}

pub(crate) fn raw_compare_op(op: CmpOp) -> u32 {
    match op {
        CmpOp::Eq => RT_COMPARE_EQ,
        CmpOp::Ne => RT_COMPARE_NE,
        CmpOp::Lt => RT_COMPARE_LT,
        CmpOp::Le => RT_COMPARE_LE,
        CmpOp::Gt => RT_COMPARE_GT,
        CmpOp::Ge => RT_COMPARE_GE,
    }
}

pub(crate) fn raw_string_compare_mode(mode: StringCompareMode) -> u32 {
    match mode {
        StringCompareMode::Binary => RT_STRING_COMPARE_BINARY,
        StringCompareMode::Text => RT_STRING_COMPARE_TEXT,
    }
}

pub(crate) fn raw_numeric_mode(mode: NumericMode) -> Result<u32, JitError> {
    match mode {
        NumericMode::Widening => Ok(RT_NUMERIC_WIDENING),
        NumericMode::Checked(target) => raw_numeric_target(target),
    }
}

pub(crate) fn raw_numeric_target(target: NumericCoerceTarget) -> Result<u32, JitError> {
    match target {
        NumericCoerceTarget::Byte => Ok(RT_NUMERIC_CHECKED_BYTE),
        NumericCoerceTarget::Integer => Ok(RT_NUMERIC_CHECKED_INTEGER),
        NumericCoerceTarget::Long => Ok(RT_NUMERIC_CHECKED_LONG),
        NumericCoerceTarget::LongLong => Ok(RT_NUMERIC_CHECKED_LONGLONG),
        NumericCoerceTarget::Single => Ok(RT_NUMERIC_CHECKED_SINGLE),
        NumericCoerceTarget::Double => Ok(RT_NUMERIC_CHECKED_DOUBLE),
        NumericCoerceTarget::Currency => Ok(RT_NUMERIC_CHECKED_CURRENCY),
        NumericCoerceTarget::Date => Ok(RT_NUMERIC_CHECKED_DATE),
        NumericCoerceTarget::Boolean => Ok(RT_NUMERIC_CHECKED_BOOLEAN),
    }
}
