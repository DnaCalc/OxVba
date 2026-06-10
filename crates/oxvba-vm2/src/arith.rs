//! Variant arithmetic / comparison / boolean / coercion for the primitive
//! operator opcodes. The contract (§6.2) keeps these as opcodes rather than
//! library calls because `Variant` boxing dominates the hot path.
//!
//! **Typed arithmetic.** `Add`/`Sub`/`Mul`/`Neg`/`IntDiv`/`Mod` carry a
//! [`NumericMode`] from the binder: `Checked(ty)` computes in the operands' promoted
//! fixed type and raises Overflow (error 6) when the result leaves its range;
//! `Widening` is the VBA Variant regime (Integer→Long→Double promotion, never errors).
//! Integer arithmetic is exact (computed in `i64`); only the genuinely floating result
//! types go through `f64`. Errors carry their VBA code structurally ([`ArithError`]),
//! so `Err.Number` / uncaught codes are right without string matching.

use oxvba_bundle::{NumericCoerceTarget, NumericMode, StringCompareMode};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, arithmetic as rt, coerce::coerce_to, variant_to_vba_string};

/// An arithmetic/coercion error carrying its VBA run-time error code.
#[derive(Debug, Clone)]
pub struct ArithError {
    pub code: i32,
    pub message: String,
}

impl ArithError {
    pub fn overflow() -> Self {
        Self {
            code: 6,
            message: "Overflow".into(),
        }
    }
    pub fn div_by_zero() -> Self {
        Self {
            code: 11,
            message: "Division by zero".into(),
        }
    }
    pub fn type_mismatch() -> Self {
        Self {
            code: 13,
            message: "Type mismatch".into(),
        }
    }
    pub fn null_use() -> Self {
        Self {
            code: 94,
            message: "Invalid use of Null".into(),
        }
    }
}

/// A bare message (e.g. from the runtime coercion layer) is Type mismatch (13) unless
/// re-classified explicitly by the caller.
impl From<String> for ArithError {
    fn from(message: String) -> Self {
        Self { code: 13, message }
    }
}
impl From<&str> for ArithError {
    fn from(message: &str) -> Self {
        Self {
            code: 13,
            message: message.to_string(),
        }
    }
}

type R = Result<Variant, ArithError>;

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
    variant_to_vba_string(v)
        .map(|b| b.as_str())
        .unwrap_or_default()
}

/// Read any numeric Variant as `f64` (the robust path: read `Double`/`Single`/
/// `LongLong`/`Currency` directly, route the rest through `Double`).
fn read_f64(v: &Variant) -> Result<f64, ArithError> {
    if let Some(x) = v.as_f64() {
        return Ok(x);
    }
    if let Some(x) = v.as_f32() {
        return Ok(f64::from(x));
    }
    if let Some(x) = v.as_i64() {
        return Ok(x as f64);
    }
    if let Some(x) = v.as_currency_scaled_i64() {
        return Ok(x as f64 / 10_000.0);
    }
    coerce_to(v, VarType::Double)?
        .as_f64()
        .ok_or_else(ArithError::type_mismatch)
}

/// Numeric value for arithmetic: `Empty`→0, `Boolean`→0/-1, numeric strings
/// parsed. `Null` is the caller's responsibility.
pub fn num(v: &Variant) -> Result<f64, ArithError> {
    match v.vtype() {
        VarType::Empty => Ok(0.0),
        VarType::Null => Err(ArithError::null_use()),
        VarType::Boolean => Ok(if v.as_bool().unwrap_or(false) {
            -1.0
        } else {
            0.0
        }),
        VarType::String => as_string(v)
            .trim()
            .parse::<f64>()
            .map_err(|_| ArithError::type_mismatch()),
        _ => read_f64(v),
    }
}

/// Integer value with VBA banker's rounding.
pub fn int(v: &Variant) -> Result<i64, ArithError> {
    if let Some(x) = v.as_i64() {
        return Ok(x);
    }
    if let Some(x) = v.as_i32() {
        return Ok(i64::from(x));
    }
    let d = num(v)?;
    if !d.is_finite() || d.abs() >= 9.223_372_036_854_775e18 {
        return Err(ArithError::overflow());
    }
    Ok(d.round_ties_even() as i64)
}

/// VBA condition truthiness: `Null`/`Empty`/0/`False` are false.
pub fn is_truthy(v: &Variant) -> Result<bool, ArithError> {
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

// ── Typed arithmetic (mode-driven) ────────────────────────────────────────────

/// Is `ty` an integer type computed exactly in `i64`? (vs. a floating result type
/// that goes through `f64`).
fn is_integer_target(ty: NumericCoerceTarget) -> bool {
    matches!(
        ty,
        NumericCoerceTarget::Byte
            | NumericCoerceTarget::Integer
            | NumericCoerceTarget::Long
            | NumericCoerceTarget::LongLong
    )
}

/// A `Checked(ty)` binary op: integer types compute exactly in `i64` (a `None` from the
/// checked op = `i64` overflow → Overflow); floating types compute in `f64`. The raw
/// result is then narrowed to `ty` (range-checked + tagged) via [`coerce_numeric`].
fn checked_binop(
    l: &Variant,
    r: &Variant,
    ty: NumericCoerceTarget,
    int_op: impl Fn(i64, i64) -> Option<i64>,
    float_op: impl Fn(f64, f64) -> f64,
) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    let raw = if is_integer_target(ty) {
        Variant::from_i64(int_op(int(l)?, int(r)?).ok_or_else(ArithError::overflow)?)
    } else {
        Variant::from_f64(float_op(num(l)?, num(r)?))
    };
    coerce_numeric(&raw, ty)
}

pub fn add(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    match mode {
        NumericMode::Checked(ty) => checked_binop(l, r, ty, |a, b| a.checked_add(b), |a, b| a + b),
        NumericMode::Widening => widening_add(l, r),
    }
}

pub fn sub(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    match mode {
        NumericMode::Checked(ty) => checked_binop(l, r, ty, |a, b| a.checked_sub(b), |a, b| a - b),
        NumericMode::Widening => widen(l, r, rt::sub),
    }
}

pub fn mul(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    match mode {
        NumericMode::Checked(ty) => checked_binop(l, r, ty, |a, b| a.checked_mul(b), |a, b| a * b),
        NumericMode::Widening => widen(l, r, rt::mul),
    }
}

pub fn neg(v: &Variant, mode: NumericMode) -> R {
    if is_null(v) {
        return Ok(Variant::null());
    }
    match mode {
        NumericMode::Checked(ty) if is_integer_target(ty) => coerce_numeric(
            &Variant::from_i64(int(v)?.checked_neg().ok_or_else(ArithError::overflow)?),
            ty,
        ),
        NumericMode::Checked(ty) => coerce_numeric(&Variant::from_f64(-num(v)?), ty),
        NumericMode::Widening => rt::neg(v).map_err(ArithError::from),
    }
}

/// The result tag for `\` / `Mod`: the binder always emits `Checked(Long|LongLong)`.
fn int_result_type(mode: NumericMode) -> NumericCoerceTarget {
    match mode {
        NumericMode::Checked(ty) => ty,
        NumericMode::Widening => NumericCoerceTarget::Long,
    }
}

pub fn int_div(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    let b = int(r)?;
    if b == 0 {
        return Err(ArithError::div_by_zero());
    }
    let q = int(l)?.checked_div(b).ok_or_else(ArithError::overflow)?;
    coerce_numeric(&Variant::from_i64(q), int_result_type(mode))
}

pub fn modulo(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    let b = int(r)?;
    if b == 0 {
        return Err(ArithError::div_by_zero());
    }
    let m = int(l)?.checked_rem(b).ok_or_else(ArithError::overflow)?;
    coerce_numeric(&Variant::from_i64(m), int_result_type(mode))
}

// ── Widening (Variant regime) arithmetic ──────────────────────────────────────

/// Null-aware wrapper over a runtime integer-preserving operator.
fn widen(l: &Variant, r: &Variant, f: impl Fn(&Variant, &Variant) -> Result<Variant, String>) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    f(l, r).map_err(ArithError::from)
}

/// VBA `+`: string operands concatenate; otherwise numeric (integer-preserving).
fn widening_add(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    if l.vtype() == VarType::String && r.vtype() == VarType::String {
        return Ok(Variant::from_string(format!(
            "{}{}",
            as_string(l),
            as_string(r)
        )));
    }
    if l.vtype() == VarType::String || r.vtype() == VarType::String {
        return Ok(Variant::from_f64(num(l)? + num(r)?));
    }
    rt::add(l, r).map_err(ArithError::from)
}

// ── Float-only operators ──────────────────────────────────────────────────────

fn numeric(l: &Variant, r: &Variant, f: impl Fn(f64, f64) -> R) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    f(num(l)?, num(r)?)
}

pub fn div(l: &Variant, r: &Variant) -> R {
    numeric(l, r, |a, b| {
        if b == 0.0 {
            Err(ArithError::div_by_zero())
        } else {
            Ok(Variant::from_f64(a / b))
        }
    })
}
pub fn pow(l: &Variant, r: &Variant) -> R {
    numeric(l, r, |a, b| Ok(Variant::from_f64(a.powf(b))))
}

/// VBA `&`: `Null & Null` is `Null`; otherwise `Null` acts as `""`.
pub fn concat(l: &Variant, r: &Variant) -> R {
    if is_null(l) && is_null(r) {
        return Ok(Variant::null());
    }
    let ls = if is_null(l) {
        String::new()
    } else {
        as_string(l)
    };
    let rs = if is_null(r) {
        String::new()
    } else {
        as_string(r)
    };
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
) -> Result<std::cmp::Ordering, ArithError> {
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
pub fn xor(l: &Variant, r: &Variant) -> R {
    bitlogic(l, r, |a, b| a ^ b, |a, b| a != b)
}
pub fn eqv(l: &Variant, r: &Variant) -> R {
    // `a Eqv b` = `Not (a Xor b)` (bitwise) / `a == b` (Boolean).
    bitlogic(l, r, |a, b| !(a ^ b), |a, b| a == b)
}
pub fn imp(l: &Variant, r: &Variant) -> R {
    // `a Imp b` = `(Not a) Or b` (bitwise) / `!a || b` (Boolean).
    bitlogic(l, r, |a, b| !a | b, |a, b| !a || b)
}

// ── Coercion ──────────────────────────────────────────────────────────────────

pub fn coerce_numeric(v: &Variant, target: NumericCoerceTarget) -> R {
    if is_null(v) {
        return Ok(Variant::null());
    }
    match target {
        // `CBool`: any non-zero numeric is `True`, zero is `False`. A `Boolean` source
        // passes through; a non-numeric source raises Type mismatch via `num`.
        NumericCoerceTarget::Boolean => {
            if v.vtype() == VarType::Boolean {
                return Ok(v.clone());
            }
            Ok(Variant::from_bool(num(v)? != 0.0))
        }
        NumericCoerceTarget::Byte => {
            let n = int(v)?;
            if (0..=255).contains(&n) {
                Ok(Variant::from_u8(n as u8))
            } else {
                Err(ArithError::overflow())
            }
        }
        NumericCoerceTarget::Integer => {
            let n = int(v)?;
            if n >= i64::from(i16::MIN) && n <= i64::from(i16::MAX) {
                Ok(Variant::from_i16(n as i16))
            } else {
                Err(ArithError::overflow())
            }
        }
        NumericCoerceTarget::Long => {
            let n = int(v)?;
            if n >= i64::from(i32::MIN) && n <= i64::from(i32::MAX) {
                Ok(Variant::from_i32(n as i32))
            } else {
                Err(ArithError::overflow())
            }
        }
        NumericCoerceTarget::LongLong => Ok(Variant::from_i64(int(v)?)),
        NumericCoerceTarget::Single => {
            let x = num(v)?;
            if x.is_finite() && x.abs() <= f64::from(f32::MAX) {
                Ok(Variant::from_f32(x as f32))
            } else {
                Err(ArithError::overflow())
            }
        }
        NumericCoerceTarget::Double => Ok(Variant::from_f64(num(v)?)),
        NumericCoerceTarget::Currency => {
            let scaled = (num(v)? * 10_000.0).round_ties_even();
            if scaled.is_finite() && scaled >= i64::MIN as f64 && scaled <= i64::MAX as f64 {
                Ok(Variant::from_currency_scaled_i64(scaled as i64))
            } else {
                Err(ArithError::overflow())
            }
        }
        NumericCoerceTarget::Date => Ok(Variant::from_date_f64(num(v)?)),
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
