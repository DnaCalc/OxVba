//! Variant arithmetic / comparison / boolean / coercion for the primitive
//! operator opcodes. The contract (§6.2) keeps these as opcodes rather than
//! library calls because `Variant` boxing dominates the hot path.
//!
//! VBA numeric semantics with `Null` propagation. FIDELITY: result-type
//! promotion is Double-biased for `-`/`*`/`/`/`^` (observationally fine, but
//! `TypeName` of such a result may read `Double` where VBA keeps a narrower
//! type); `And`/`Or` with `Null` is simplified to `Null` propagation rather
//! than full three-valued logic.

use oxvba_bundle::{NumericCoerceTarget, StringCompareMode};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, coerce::coerce_to, variant_to_vba_string};

type R = Result<Variant, String>;

#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

pub fn is_null(v: &Variant) -> bool {
    matches!(v.vtype(), VarType::Null)
}

pub fn as_string(v: &Variant) -> String {
    variant_to_vba_string(v).map(|b| b.as_str()).unwrap_or_default()
}

/// Read any numeric Variant as `f64` (the robust path: read `Double`/`Single`/
/// `LongLong`/`Currency` directly, route the rest through `Double`).
fn read_f64(v: &Variant) -> Result<f64, String> {
    if let Some(x) = v.as_f64() {
        return Ok(x);
    }
    if let Some(x) = v.as_f32() {
        return Ok(x as f64);
    }
    if let Some(x) = v.as_i64() {
        return Ok(x as f64);
    }
    if let Some(x) = v.as_currency_scaled_i64() {
        return Ok(x as f64 / 10_000.0);
    }
    coerce_to(v, VarType::Double)?
        .as_f64()
        .ok_or_else(|| "Type mismatch".to_string())
}

/// Numeric value for arithmetic: `Empty`→0, `Boolean`→0/-1, numeric strings
/// parsed. `Null` is the caller's responsibility.
pub fn num(v: &Variant) -> Result<f64, String> {
    match v.vtype() {
        VarType::Empty => Ok(0.0),
        VarType::Null => Err("Invalid use of Null".to_string()),
        VarType::Boolean => Ok(if v.as_bool().unwrap_or(false) { -1.0 } else { 0.0 }),
        VarType::String => as_string(v)
            .trim()
            .parse::<f64>()
            .map_err(|_| "Type mismatch".to_string()),
        _ => read_f64(v),
    }
}

/// Integer value with VBA banker's rounding.
pub fn int(v: &Variant) -> Result<i64, String> {
    if let Some(x) = v.as_i64() {
        return Ok(x);
    }
    if let Some(x) = v.as_i32() {
        return Ok(i64::from(x));
    }
    let d = num(v)?;
    if !d.is_finite() || d.abs() >= 9.223_372_036_854_775e18 {
        return Err("Overflow".to_string());
    }
    Ok(d.round_ties_even() as i64)
}

/// VBA condition truthiness: `Null`/`Empty`/0/`False` are false.
pub fn is_truthy(v: &Variant) -> Result<bool, String> {
    match v.vtype() {
        VarType::Null | VarType::Empty => Ok(false),
        VarType::Boolean => Ok(v.as_bool().unwrap_or(false)),
        _ => Ok(num(v)? != 0.0),
    }
}

fn long_or_double(value: i64) -> Variant {
    if value >= i64::from(i32::MIN) && value <= i64::from(i32::MAX) {
        Variant::from_i32(value as i32)
    } else {
        Variant::from_f64(value as f64)
    }
}

fn numeric(l: &Variant, r: &Variant, f: impl Fn(f64, f64) -> Result<f64, String>) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    Ok(Variant::from_f64(f(num(l)?, num(r)?)?))
}

// ── Arithmetic operators ──────────────────────────────────────────────────────

pub fn add(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    if l.vtype() == VarType::String && r.vtype() == VarType::String {
        return Ok(Variant::from_string(format!("{}{}", as_string(l), as_string(r))));
    }
    if l.vtype() == VarType::String || r.vtype() == VarType::String {
        return Ok(Variant::from_f64(num(l)? + num(r)?));
    }
    // Integer/Long promotion handled by the runtime's reference `add`.
    oxvba_runtime::arithmetic::add(l, r)
}

pub fn sub(l: &Variant, r: &Variant) -> R {
    numeric(l, r, |a, b| Ok(a - b))
}
pub fn mul(l: &Variant, r: &Variant) -> R {
    numeric(l, r, |a, b| Ok(a * b))
}
pub fn div(l: &Variant, r: &Variant) -> R {
    numeric(l, r, |a, b| {
        if b == 0.0 {
            Err("Division by zero".to_string())
        } else {
            Ok(a / b)
        }
    })
}
pub fn pow(l: &Variant, r: &Variant) -> R {
    numeric(l, r, |a, b| Ok(a.powf(b)))
}

pub fn int_div(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    let b = int(r)?;
    if b == 0 {
        return Err("Division by zero".to_string());
    }
    Ok(long_or_double(int(l)? / b))
}

pub fn modulo(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    let b = int(r)?;
    if b == 0 {
        return Err("Division by zero".to_string());
    }
    Ok(long_or_double(int(l)? % b))
}

pub fn neg(v: &Variant) -> R {
    if is_null(v) {
        return Ok(Variant::null());
    }
    Ok(Variant::from_f64(-num(v)?))
}

/// VBA `&`: `Null & Null` is `Null`; otherwise `Null` acts as `""`.
pub fn concat(l: &Variant, r: &Variant) -> R {
    if is_null(l) && is_null(r) {
        return Ok(Variant::null());
    }
    let ls = if is_null(l) { String::new() } else { as_string(l) };
    let rs = if is_null(r) { String::new() } else { as_string(r) };
    Ok(Variant::from_string(format!("{ls}{rs}")))
}

// ── Comparison ────────────────────────────────────────────────────────────────

fn norm(s: String, mode: StringCompareMode) -> String {
    match mode {
        StringCompareMode::Text => s.to_ascii_lowercase(),
        StringCompareMode::Binary => s,
    }
}

fn cmp_order(
    l: &Variant,
    r: &Variant,
    mode: StringCompareMode,
) -> Result<std::cmp::Ordering, String> {
    let both_string = l.vtype() == VarType::String && r.vtype() == VarType::String;
    if both_string {
        return Ok(norm(as_string(l), mode).cmp(&norm(as_string(r), mode)));
    }
    match (num(l), num(r)) {
        (Ok(a), Ok(b)) => Ok(a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)),
        // A non-numeric operand falls back to a string comparison.
        _ => Ok(norm(as_string(l), mode).cmp(&norm(as_string(r), mode))),
    }
}

pub fn compare(l: &Variant, r: &Variant, mode: StringCompareMode, op: CmpOp) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    use std::cmp::Ordering::*;
    let ord = cmp_order(l, r, mode)?;
    let result = match op {
        CmpOp::Eq => ord == Equal,
        CmpOp::Ne => ord != Equal,
        CmpOp::Lt => ord == Less,
        CmpOp::Le => ord != Greater,
        CmpOp::Gt => ord == Greater,
        CmpOp::Ge => ord != Less,
    };
    Ok(Variant::from_bool(result))
}

// ── Boolean / bitwise ─────────────────────────────────────────────────────────

pub fn not(v: &Variant) -> R {
    if is_null(v) {
        return Ok(Variant::null());
    }
    if v.vtype() == VarType::Boolean {
        return Ok(Variant::from_bool(!v.as_bool().unwrap_or(false)));
    }
    Ok(long_or_double(!int(v)?))
}

fn bitlogic(
    l: &Variant,
    r: &Variant,
    bit: impl Fn(i64, i64) -> i64,
    logic: impl Fn(bool, bool) -> bool,
) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    if l.vtype() == VarType::Boolean && r.vtype() == VarType::Boolean {
        return Ok(Variant::from_bool(logic(
            l.as_bool().unwrap_or(false),
            r.as_bool().unwrap_or(false),
        )));
    }
    Ok(long_or_double(bit(int(l)?, int(r)?)))
}

pub fn and(l: &Variant, r: &Variant) -> R {
    bitlogic(l, r, |a, b| a & b, |a, b| a && b)
}
pub fn or(l: &Variant, r: &Variant) -> R {
    bitlogic(l, r, |a, b| a | b, |a, b| a || b)
}

// ── Coercion ──────────────────────────────────────────────────────────────────

pub fn coerce_numeric(v: &Variant, target: NumericCoerceTarget) -> R {
    if is_null(v) {
        return Ok(Variant::null());
    }
    let n = int(v)?;
    let overflow = || "Overflow".to_string();
    match target {
        NumericCoerceTarget::Byte => {
            if (0..=255).contains(&n) {
                Ok(Variant::from_u8(n as u8))
            } else {
                Err(overflow())
            }
        }
        NumericCoerceTarget::Integer => {
            if n >= i64::from(i16::MIN) && n <= i64::from(i16::MAX) {
                Ok(Variant::from_i16(n as i16))
            } else {
                Err(overflow())
            }
        }
        NumericCoerceTarget::Long => {
            if n >= i64::from(i32::MIN) && n <= i64::from(i32::MAX) {
                Ok(Variant::from_i32(n as i32))
            } else {
                Err(overflow())
            }
        }
        NumericCoerceTarget::LongLong => Ok(Variant::from_i64(n)),
    }
}

/// Pad with trailing spaces or truncate to a fixed character length.
pub fn coerce_fixed_string(v: &Variant, len: usize) -> Variant {
    let mut s: Vec<char> = as_string(v).chars().collect();
    if s.len() > len {
        s.truncate(len);
    } else {
        while s.len() < len {
            s.push(' ');
        }
    }
    Variant::from_string(s.into_iter().collect::<String>())
}
