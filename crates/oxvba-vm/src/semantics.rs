//! Shared pure semantic functions for VBA runtime operations.
//!
//! These functions are extracted from the interpreter so they can be reused
//! by the JIT runtime helpers without duplication.

use oxvba_com::{ComCallbackToken, ComMemberToken, ComSubscriptionToken, DynamicMemberSelector};
use oxvba_compiler::bytecode::{
    RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
};
use oxvba_runtime::{BindingHandle, CurrencyValue, F64Value, ObjectHandle, RuntimeValue, bstr::BStr};

// ── Coercion & Type Checks ────────────────────────────────────────────

pub fn either_null(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
    matches!(lhs, RuntimeValue::Null) || matches!(rhs, RuntimeValue::Null)
}

pub fn either_error(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
    matches!(lhs, RuntimeValue::ErrorCode(_)) || matches!(rhs, RuntimeValue::ErrorCode(_))
}

pub fn either_is_f64(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
    matches!(
        lhs,
        RuntimeValue::F64(_) | RuntimeValue::Currency(_) | RuntimeValue::Decimal(_)
    ) || matches!(
        rhs,
        RuntimeValue::F64(_) | RuntimeValue::Currency(_) | RuntimeValue::Decimal(_)
    )
}

pub fn runtime_value_legacy_token(value: &RuntimeValue, field: &str) -> Result<i32, String> {
    value
        .to_legacy_i32()
        .map_err(|detail| format!("{field} requires legacy-compatible token: {detail}"))
}

pub fn runtime_value_as_f64(value: &RuntimeValue) -> Result<f64, String> {
    match value {
        RuntimeValue::Empty => Ok(0.0),
        RuntimeValue::I32(v) => Ok(*v as f64),
        RuntimeValue::I64(v) => Ok(*v as f64),
        RuntimeValue::F64(v) => Ok(v.as_f64()),
        RuntimeValue::Bool(v) => Ok(if *v { -1.0 } else { 0.0 }),
        RuntimeValue::Currency(c) => Ok(c.scaled_i64() as f64 / CurrencyValue::SCALE as f64),
        RuntimeValue::Decimal(d) => {
            let mag = d.magnitude_u128() as f64;
            let scale = 10f64.powi(d.scale() as i32);
            Ok(if d.is_negative() {
                -(mag / scale)
            } else {
                mag / scale
            })
        }
        other => Err(format!("cannot coerce {:?} to f64", other)),
    }
}

pub fn runtime_value_to_usize(value: &RuntimeValue) -> Result<usize, String> {
    match value {
        RuntimeValue::Empty => Ok(0),
        RuntimeValue::I32(v) => Ok(*v as usize),
        RuntimeValue::I64(v) => Ok(*v as usize),
        RuntimeValue::F64(v) => Ok(v.as_f64() as usize),
        other => {
            let v = runtime_value_legacy_token(other, "usize operand")?;
            Ok(v as usize)
        }
    }
}

pub fn legacy_truthy_value(value: &RuntimeValue) -> Result<bool, String> {
    if matches!(value, RuntimeValue::Null) {
        return Ok(false);
    }
    if let RuntimeValue::F64(v) = value {
        return Ok(v.as_f64() != 0.0);
    }
    Ok(runtime_value_legacy_token(value, "boolean operand")? != 0)
}

pub fn runtime_value_is_object(value: &RuntimeValue) -> bool {
    matches!(
        value,
        RuntimeValue::ObjectHandle(_) | RuntimeValue::BindingHandle(_)
    )
}

// ── Arithmetic ────────────────────────────────────────────────────────

pub fn legacy_add_const_value(
    value: &RuntimeValue,
    delta: i32,
    field: &str,
) -> Result<RuntimeValue, String> {
    if matches!(value, RuntimeValue::Null) {
        return Ok(RuntimeValue::Null);
    }
    if matches!(value, RuntimeValue::ErrorCode(_)) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if let RuntimeValue::F64(v) = value {
        return Ok(RuntimeValue::F64(F64Value::from_f64(
            v.as_f64() + delta as f64,
        )));
    }
    let value = runtime_value_legacy_token(value, field)?;
    Ok(RuntimeValue::I32(value + delta))
}

pub fn legacy_add_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if either_is_f64(lhs, rhs) {
        let l = runtime_value_as_f64(lhs)?;
        let r = runtime_value_as_f64(rhs)?;
        return Ok(RuntimeValue::F64(F64Value::from_f64(l + r)));
    }
    let lhs = runtime_value_legacy_token(lhs, "add lhs")?;
    let rhs = runtime_value_legacy_token(rhs, "add rhs")?;
    Ok(RuntimeValue::I32(lhs + rhs))
}

pub fn legacy_sub_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if either_is_f64(lhs, rhs) {
        let l = runtime_value_as_f64(lhs)?;
        let r = runtime_value_as_f64(rhs)?;
        return Ok(RuntimeValue::F64(F64Value::from_f64(l - r)));
    }
    let lhs = runtime_value_legacy_token(lhs, "sub lhs")?;
    let rhs = runtime_value_legacy_token(rhs, "sub rhs")?;
    Ok(RuntimeValue::I32(lhs - rhs))
}

pub fn legacy_mul_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if either_is_f64(lhs, rhs) {
        let l = runtime_value_as_f64(lhs)?;
        let r = runtime_value_as_f64(rhs)?;
        return Ok(RuntimeValue::F64(F64Value::from_f64(l * r)));
    }
    let lhs = runtime_value_legacy_token(lhs, "mul lhs")?;
    let rhs = runtime_value_legacy_token(rhs, "mul rhs")?;
    let result = (lhs as i64) * (rhs as i64);
    let truncated = result as i32;
    Ok(RuntimeValue::I32(truncated))
}

pub fn legacy_pow_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    let base = runtime_value_as_f64(lhs)
        .or_else(|_| runtime_value_legacy_token(lhs, "pow base").map(|v| v as f64))?;
    let exp = runtime_value_as_f64(rhs)
        .or_else(|_| runtime_value_legacy_token(rhs, "pow exponent").map(|v| v as f64))?;
    let result = base.powf(exp);
    Ok(RuntimeValue::F64(F64Value::from_f64(result)))
}

pub fn legacy_concat_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> RuntimeValue {
    let lhs_str = match lhs {
        RuntimeValue::Null | RuntimeValue::Empty => String::new(),
        RuntimeValue::String(s) => s.0.clone(),
        RuntimeValue::F64(v) => v.as_f64().to_string(),
        other => {
            if let Ok(token) = runtime_value_legacy_token(other, "concat lhs") {
                token.to_string()
            } else {
                String::new()
            }
        }
    };
    let rhs_str = match rhs {
        RuntimeValue::Null | RuntimeValue::Empty => String::new(),
        RuntimeValue::String(s) => s.0.clone(),
        RuntimeValue::F64(v) => v.as_f64().to_string(),
        other => {
            if let Ok(token) = runtime_value_legacy_token(other, "concat rhs") {
                token.to_string()
            } else {
                String::new()
            }
        }
    };
    RuntimeValue::String(BStr(format!("{lhs_str}{rhs_str}")))
}

pub fn legacy_neg_value(val: &RuntimeValue) -> Result<RuntimeValue, String> {
    if matches!(val, RuntimeValue::Null) {
        return Ok(RuntimeValue::Null);
    }
    if let RuntimeValue::F64(v) = val {
        return Ok(RuntimeValue::F64(F64Value::from_f64(-v.as_f64())));
    }
    let v = runtime_value_legacy_token(val, "neg operand")?;
    Ok(RuntimeValue::I32(-v))
}

pub fn legacy_increment_value(value: &RuntimeValue) -> Result<RuntimeValue, String> {
    if let RuntimeValue::F64(v) = value {
        return Ok(RuntimeValue::F64(F64Value::from_f64(v.as_f64() + 1.0)));
    }
    let value = runtime_value_legacy_token(value, "increment operand")?;
    Ok(RuntimeValue::I32(value + 1))
}

// ── Division (with error codes) ───────────────────────────────────────

/// Returns Ok(value) or Err(error_code) for division by zero (code 11).
pub fn legacy_div_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<Result<RuntimeValue, i32>, String> {
    if either_null(lhs, rhs) {
        return Ok(Ok(RuntimeValue::Null));
    }
    let r = runtime_value_as_f64(rhs)
        .or_else(|_| runtime_value_legacy_token(rhs, "div rhs").map(|v| v as f64))?;
    if r == 0.0 {
        return Ok(Err(11));
    }
    let l = runtime_value_as_f64(lhs)
        .or_else(|_| runtime_value_legacy_token(lhs, "div lhs").map(|v| v as f64))?;
    Ok(Ok(RuntimeValue::F64(F64Value::from_f64(l / r))))
}

pub fn legacy_intdiv_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<Result<RuntimeValue, i32>, String> {
    if either_null(lhs, rhs) {
        return Ok(Ok(RuntimeValue::Null));
    }
    let r = runtime_value_as_f64(rhs)
        .or_else(|_| runtime_value_legacy_token(rhs, "intdiv rhs").map(|v| v as f64))?;
    let r_trunc = r as i32;
    if r_trunc == 0 {
        return Ok(Err(11));
    }
    let l = runtime_value_as_f64(lhs)
        .or_else(|_| runtime_value_legacy_token(lhs, "intdiv lhs").map(|v| v as f64))?;
    Ok(Ok(RuntimeValue::I32((l / r).trunc() as i32)))
}

pub fn legacy_mod_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<Result<RuntimeValue, i32>, String> {
    if either_null(lhs, rhs) {
        return Ok(Ok(RuntimeValue::Null));
    }
    let r = runtime_value_as_f64(rhs)
        .or_else(|_| runtime_value_legacy_token(rhs, "mod rhs").map(|v| v as f64))?;
    let r_int = r as i32;
    if r_int == 0 {
        return Ok(Err(11));
    }
    let l = runtime_value_as_f64(lhs)
        .or_else(|_| runtime_value_legacy_token(lhs, "mod lhs").map(|v| v as f64))?;
    Ok(Ok(RuntimeValue::I32((l as i32) % r_int)))
}

// ── Comparison ────────────────────────────────────────────────────────

pub fn normalize_for_compare(text: String, mode: StringCompareMode) -> String {
    match mode {
        StringCompareMode::Binary => text,
        StringCompareMode::Text => text.to_ascii_lowercase(),
    }
}

pub fn typed_compare_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
    mode: StringCompareMode,
    pred: fn(std::cmp::Ordering) -> bool,
) -> Result<bool, String> {
    if either_null(lhs, rhs) {
        return Ok(false);
    }
    match (lhs, rhs) {
        (RuntimeValue::String(a), RuntimeValue::String(b)) => {
            let a = normalize_for_compare(a.0.clone(), mode);
            let b = normalize_for_compare(b.0.clone(), mode);
            Ok(pred(a.cmp(&b)))
        }
        (RuntimeValue::F64(a), RuntimeValue::F64(b)) => {
            let ord = a
                .as_f64()
                .partial_cmp(&b.as_f64())
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        (RuntimeValue::I32(a), RuntimeValue::F64(b)) => {
            let ord = (*a as f64)
                .partial_cmp(&b.as_f64())
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        (RuntimeValue::F64(a), RuntimeValue::I32(b)) => {
            let ord = a
                .as_f64()
                .partial_cmp(&(*b as f64))
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        _ => {
            let l = runtime_value_legacy_token(lhs, "comparison lhs")?;
            let r = runtime_value_legacy_token(rhs, "comparison rhs")?;
            Ok(pred(l.cmp(&r)))
        }
    }
}

// ── Assignment Validation ─────────────────────────────────────────────

pub fn runtime_assignment_value_label(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Empty => "Empty",
        RuntimeValue::Null => "Null",
        RuntimeValue::ErrorCode(_) => "Error",
        RuntimeValue::I32(_) => "Long",
        RuntimeValue::I64(_) => "LongLong",
        RuntimeValue::F64(value) => match value.subtype() {
            oxvba_runtime::F64Subtype::Single => "Single",
            oxvba_runtime::F64Subtype::Double => "Double",
            oxvba_runtime::F64Subtype::Date => "Date",
        },
        RuntimeValue::Decimal(_) => "Decimal",
        RuntimeValue::Currency(_) => "Currency",
        RuntimeValue::Bool(_) => "Boolean",
        RuntimeValue::String(_) => "String",
        RuntimeValue::ArrayIntent(_) => "Array",
        RuntimeValue::ObjectHandle(_) => "Object",
        RuntimeValue::BindingHandle(_) => "Binding",
    }
}

pub fn validate_runtime_assignment(
    value: &RuntimeValue,
    intent: RuntimeAssignmentIntent,
    target_kind: RuntimeAssignmentTargetKind,
    target_name: &str,
    target_type_name: &str,
) -> Result<(), String> {
    match (intent, target_kind) {
        (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Variant)
        | (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Object) => {
            if runtime_value_is_object(value) {
                Ok(())
            } else {
                Err(format!(
                    "Set requires object value for variable {target_name}"
                ))
            }
        }
        (RuntimeAssignmentIntent::Implicit, RuntimeAssignmentTargetKind::Object) => {
            if runtime_value_is_object(value) {
                Err(format!("Set required for Object variable {target_name}"))
            } else {
                Err(format!(
                    "cannot assign {} to Object variable {target_name}",
                    runtime_assignment_value_label(value)
                ))
            }
        }
        (RuntimeAssignmentIntent::Let, RuntimeAssignmentTargetKind::Object) => Err(format!(
            "Let cannot assign to Object variable {target_name}"
        )),
        (
            RuntimeAssignmentIntent::Implicit | RuntimeAssignmentIntent::Let,
            RuntimeAssignmentTargetKind::Scalar,
        ) => {
            if runtime_value_is_object(value) {
                Err(format!(
                    "cannot assign Object to {target_type_name} variable {target_name}"
                ))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

// ── Formatting ────────────────────────────────────────────────────────

pub fn format_number(n: f64, fmt: Option<&str>) -> String {
    match fmt {
        None => {
            if n == (n as i64) as f64 && n.abs() < i64::MAX as f64 {
                format!("{}", n as i64)
            } else {
                format!("{}", n)
            }
        }
        Some("0") => format!("{}", n.round() as i64),
        Some(pat) if pat.starts_with("0.") && pat[2..].chars().all(|c| c == '0') => {
            let decimals = pat.len() - 2;
            format!("{:.prec$}", n, prec = decimals)
        }
        Some("0%") => format!("{}%", (n * 100.0).round() as i64),
        Some("#,##0") => {
            let i = n.round() as i64;
            let negative = i < 0;
            let abs_str = (i.unsigned_abs()).to_string();
            let mut grouped = String::new();
            for (idx, ch) in abs_str.chars().rev().enumerate() {
                if idx > 0 && idx % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(ch);
            }
            let grouped: String = grouped.chars().rev().collect();
            if negative {
                format!("-{}", grouped)
            } else {
                grouped
            }
        }
        Some(_) => {
            if n == (n as i64) as f64 && n.abs() < i64::MAX as f64 {
                format!("{}", n as i64)
            } else {
                format!("{}", n)
            }
        }
    }
}

pub fn proper_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c.is_whitespace() {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            for lc in c.to_lowercase() {
                result.push(lc);
            }
        }
    }
    result
}

// ── COM Token Conversions ─────────────────────────────────────────────

pub fn runtime_value_to_com_object(
    value: &RuntimeValue,
    field: &str,
) -> Result<ObjectHandle, String> {
    match value {
        RuntimeValue::ObjectHandle(handle) => Ok(*handle),
        other => runtime_value_legacy_token(other, field).map(Into::into),
    }
}

pub fn runtime_value_to_com_member_token(
    value: &RuntimeValue,
    field: &str,
) -> Result<ComMemberToken, String> {
    runtime_value_legacy_token(value, field).map(ComMemberToken::new)
}

pub fn runtime_value_to_com_subscription_token(
    value: &RuntimeValue,
    field: &str,
) -> Result<ComSubscriptionToken, String> {
    runtime_value_legacy_token(value, field).map(ComSubscriptionToken::new)
}

pub fn runtime_value_to_com_callback_token(
    value: &RuntimeValue,
    field: &str,
) -> Result<ComCallbackToken, String> {
    runtime_value_legacy_token(value, field).map(ComCallbackToken::new)
}

pub fn runtime_value_to_dynamic_member_selector(
    value: &RuntimeValue,
    field: &str,
) -> Result<DynamicMemberSelector, String> {
    match value {
        RuntimeValue::String(text) => Ok(DynamicMemberSelector::Name(text.0.clone())),
        other => {
            let token = runtime_value_legacy_token(other, field)?;
            if token == 0 {
                Ok(DynamicMemberSelector::DefaultMember)
            } else {
                Ok(DynamicMemberSelector::Token(token))
            }
        }
    }
}

pub fn runtime_value_to_usize_index(value: &RuntimeValue, field: &str) -> Result<usize, String> {
    let index = runtime_value_legacy_token(value, field)?;
    if index < 0 {
        return Err(format!("{field} cannot be negative: {index}"));
    }
    usize::try_from(index).map_err(|_| format!("{field} exceeds usize range: {index}"))
}

// ── WithEvents Key Functions ──────────────────────────────────────────

pub fn withevents_binding_key(owner: ObjectHandle, binding: BindingHandle) -> i64 {
    ((owner.raw() as i64) << 32) | (binding.raw() as u32 as i64)
}

pub fn withevents_binding_from_key(key: i64) -> BindingHandle {
    BindingHandle::new((key as u32) as i32)
}

pub fn withevents_owner_from_key(key: i64) -> ObjectHandle {
    ObjectHandle::new((key >> 32) as i32)
}

pub fn withevents_binding_handle(
    value: &RuntimeValue,
    field: &str,
) -> Result<BindingHandle, String> {
    match value {
        RuntimeValue::BindingHandle(handle) => Ok(*handle),
        other => runtime_value_legacy_token(other, &format!("WithEvents {field}")).map(Into::into),
    }
}

pub fn withevents_owner_handle(
    value: &RuntimeValue,
    field: &str,
) -> Result<ObjectHandle, String> {
    match value {
        RuntimeValue::ObjectHandle(handle) => Ok(*handle),
        other => runtime_value_legacy_token(other, &format!("WithEvents {field}")).map(Into::into),
    }
}
