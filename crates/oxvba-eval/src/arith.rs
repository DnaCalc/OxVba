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
use oxvba_runtime::{
    Variant, arithmetic as rt,
    coerce::{coerce_to, parse_vba_numeric_string},
    variant_to_vba_string,
};

const CURRENCY_SCALE: i128 = 10_000;
const CURRENCY_MIN: i128 = i64::MIN as i128;
const CURRENCY_MAX: i128 = i64::MAX as i128;

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
    pub fn invalid_call() -> Self {
        Self {
            code: 5,
            message: "Invalid procedure call or argument".into(),
        }
    }
    pub fn null_use() -> Self {
        Self {
            code: 94,
            message: "Invalid use of Null".into(),
        }
    }
    pub fn object_not_set() -> Self {
        Self {
            code: 91,
            message: "Object variable or With block variable not set".into(),
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

fn is_object_nothing(v: &Variant) -> bool {
    if v.vtype() != VarType::Object {
        return false;
    }
    v.as_object_ref().map(|object| object.raw()).unwrap_or(0) == 0
}

fn reject_object_nothing(l: &Variant, r: &Variant) -> Result<(), ArithError> {
    if is_object_nothing(l) || is_object_nothing(r) {
        return Err(ArithError::object_not_set());
    }
    Ok(())
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
    if let Some(x) = v.as_proc_ref() {
        return Ok(x as f64);
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
        VarType::Object if is_object_nothing(v) => Err(ArithError::object_not_set()),
        VarType::Boolean => Ok(if v.as_bool().unwrap_or(false) {
            -1.0
        } else {
            0.0
        }),
        VarType::String => {
            parse_vba_numeric_string(&as_string(v)).ok_or_else(ArithError::type_mismatch)
        }
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
    if let Some(x) = v.as_proc_ref() {
        return i64::try_from(x).map_err(|_| ArithError::overflow());
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
        NumericMode::Checked(NumericCoerceTarget::Currency) => currency_add(l, r),
        NumericMode::Checked(ty) => checked_binop(l, r, ty, |a, b| a.checked_add(b), |a, b| a + b),
        NumericMode::Widening => widening_add(l, r),
    }
}

pub fn sub(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    match mode {
        NumericMode::Checked(NumericCoerceTarget::Currency) => currency_sub(l, r),
        NumericMode::Checked(ty) => checked_binop(l, r, ty, |a, b| a.checked_sub(b), |a, b| a - b),
        NumericMode::Widening => widening_sub(l, r),
    }
}

pub fn mul(l: &Variant, r: &Variant, mode: NumericMode) -> R {
    match mode {
        NumericMode::Checked(NumericCoerceTarget::Currency) => currency_mul(l, r),
        NumericMode::Checked(ty) => checked_binop(l, r, ty, |a, b| a.checked_mul(b), |a, b| a * b),
        NumericMode::Widening => widening_mul(l, r),
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
        NumericMode::Widening if is_object_nothing(v) => Err(ArithError::object_not_set()),
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
    reject_object_nothing(l, r)?;
    f(l, r).map_err(ArithError::from)
}

fn checked_currency(scaled: i128) -> R {
    if !(CURRENCY_MIN..=CURRENCY_MAX).contains(&scaled) {
        return Err(ArithError::overflow());
    }
    Ok(Variant::from_currency_scaled_i64(scaled as i64))
}

fn scale_integer_to_currency(value: i128) -> Result<i128, ArithError> {
    value
        .checked_mul(CURRENCY_SCALE)
        .filter(|scaled| (CURRENCY_MIN..=CURRENCY_MAX).contains(scaled))
        .ok_or_else(ArithError::overflow)
}

fn exact_currency_operand(v: &Variant) -> Option<Result<i128, ArithError>> {
    let scaled = match v.vtype() {
        VarType::Empty => 0,
        VarType::Boolean => {
            if v.as_bool().unwrap_or(false) {
                -CURRENCY_SCALE
            } else {
                0
            }
        }
        VarType::SignedByte => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_i8().unwrap_or(0),
            )));
        }
        VarType::Byte => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_u8().unwrap_or(0),
            )));
        }
        VarType::Integer => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_i16().unwrap_or(0),
            )));
        }
        VarType::UnsignedInteger => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_u16().unwrap_or(0),
            )));
        }
        VarType::Long => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_i32().unwrap_or(0),
            )));
        }
        VarType::UnsignedLong | VarType::UnsignedInt => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_u32().unwrap_or(0),
            )));
        }
        VarType::LongLong => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_i64().unwrap_or(0),
            )));
        }
        VarType::UnsignedLongLong => {
            return Some(scale_integer_to_currency(i128::from(
                v.as_u64().unwrap_or(0),
            )));
        }
        VarType::Currency => i128::from(v.as_currency_scaled_i64().unwrap_or(0)),
        _ => return None,
    };
    Some(Ok(scaled))
}

fn currency_operand(v: &Variant) -> Result<i128, ArithError> {
    match exact_currency_operand(v) {
        Some(result) => result,
        None => coerce_numeric(v, NumericCoerceTarget::Currency)?
            .as_currency_scaled_i64()
            .map(i128::from)
            .ok_or_else(ArithError::type_mismatch),
    }
}

fn has_currency_exact_lane(l: &Variant, r: &Variant) -> bool {
    exact_currency_operand(l).is_some() && exact_currency_operand(r).is_some()
}

fn currency_add(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    checked_currency(
        currency_operand(l)?
            .checked_add(currency_operand(r)?)
            .ok_or_else(ArithError::overflow)?,
    )
}

fn currency_sub(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    checked_currency(
        currency_operand(l)?
            .checked_sub(currency_operand(r)?)
            .ok_or_else(ArithError::overflow)?,
    )
}

fn div_round_ties_even(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    let negative = numerator < 0;
    let magnitude = if negative { -numerator } else { numerator };
    let quotient = magnitude / denominator;
    let remainder = magnitude % denominator;
    let twice_remainder = remainder * 2;
    let rounded =
        if twice_remainder > denominator || (twice_remainder == denominator && quotient % 2 != 0) {
            quotient + 1
        } else {
            quotient
        };
    if negative { -rounded } else { rounded }
}

fn currency_mul(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    let product = currency_operand(l)?
        .checked_mul(currency_operand(r)?)
        .ok_or_else(ArithError::overflow)?;
    checked_currency(div_round_ties_even(product, CURRENCY_SCALE))
}

/// Is this operand a `Date`? `Date` arithmetic re-tags its (otherwise `Double`) result
/// back to `Date` per live VBA (see date-arith-loses-date-type): `Date + <anything>`
/// — including `Date + Date` — is a `Date`; in a `-`, a *single* `Date` operand keeps
/// the result a `Date` (`Date - n`, `n - Date`), but `Date - Date` is the elapsed time
/// as a plain `Double`. `*`, `/`, `\`, `Mod` are never `Date`.
fn is_date(v: &Variant) -> bool {
    v.vtype() == VarType::Date
}

/// Re-tag a numeric result as `Date` (keeping the serial value; the carrier is always a
/// `Double` from `widen_double`/`from_f64`, so `as_f64` is `Some`).
fn as_date(v: Variant) -> Variant {
    match v.as_f64() {
        Some(serial) => Variant::from_date_f64(serial),
        None => v,
    }
}

/// VBA `+`: string operands concatenate; otherwise numeric (integer-preserving). A
/// `Date` operand makes the result a `Date`.
fn widening_add(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    reject_object_nothing(l, r)?;
    if l.vtype() == VarType::String && r.vtype() == VarType::String {
        // Code-unit concatenation (see `concat_units`) so a `+` over two strings
        // preserves surrogate halves exactly, matching `&`.
        let mut units = concat_units(l);
        units.extend(concat_units(r));
        return Ok(Variant::from_utf16_units(&units));
    }
    if l.vtype() == VarType::Empty || r.vtype() == VarType::Empty {
        if l.vtype() == VarType::String || r.vtype() == VarType::String {
            return Err(ArithError::type_mismatch());
        }
        let raw = Variant::from_f64(num(l)? + num(r)?);
        return Ok(if is_date(l) || is_date(r) {
            as_date(raw)
        } else {
            raw
        });
    }
    let raw = if l.vtype() == VarType::String || r.vtype() == VarType::String {
        Variant::from_f64(num(l)? + num(r)?)
    } else if has_currency_exact_lane(l, r)
        && (l.vtype() == VarType::Currency || r.vtype() == VarType::Currency)
    {
        currency_add(l, r)?
    } else {
        rt::add(l, r).map_err(ArithError::from)?
    };
    Ok(if is_date(l) || is_date(r) {
        as_date(raw)
    } else {
        raw
    })
}

/// VBA `-` in the Variant regime. A single `Date` operand keeps the result a `Date`;
/// two `Date`s yield the elapsed time as a plain `Double` (so the `Date` tag is dropped).
fn widening_sub(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    reject_object_nothing(l, r)?;
    let raw = if has_currency_exact_lane(l, r)
        && (l.vtype() == VarType::Currency || r.vtype() == VarType::Currency)
    {
        currency_sub(l, r)?
    } else {
        rt::sub(l, r).map_err(ArithError::from)?
    };
    Ok(if is_date(l) != is_date(r) {
        as_date(raw)
    } else {
        raw
    })
}

fn widening_mul(l: &Variant, r: &Variant) -> R {
    if is_null(l) || is_null(r) {
        return Ok(Variant::null());
    }
    if has_currency_exact_lane(l, r)
        && (l.vtype() == VarType::Currency || r.vtype() == VarType::Currency)
    {
        return currency_mul(l, r);
    }
    widen(l, r, rt::mul)
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
    numeric(l, r, |a, b| {
        let value = a.powf(b);
        if value.is_nan() {
            Err(ArithError::invalid_call())
        } else {
            Ok(Variant::from_f64(value))
        }
    })
}

/// The operand's UTF-16 code units for concatenation: a String Variant's units are
/// read VERBATIM off its BSTR (preserving lone/paired surrogate halves), while a
/// non-string coerces through its VBA string form (which carries no surrogates).
/// Concatenating at the code-unit level is what lets `ChrW(&HD83D) & ChrW(&HDE00)`
/// form a real astral pair — a Rust-`String` round-trip would fold each half to U+FFFD.
fn concat_units(v: &Variant) -> Vec<u16> {
    v.string_units()
        .unwrap_or_else(|| as_string(v).encode_utf16().collect())
}

/// VBA `&`: `Null & Null` is `Null`; otherwise `Null` acts as `""`.
pub fn concat(l: &Variant, r: &Variant) -> R {
    if is_null(l) && is_null(r) {
        return Ok(Variant::null());
    }
    let mut units = Vec::new();
    if !is_null(l) {
        units.extend(concat_units(l));
    }
    if !is_null(r) {
        units.extend(concat_units(r));
    }
    Ok(Variant::from_utf16_units(&units))
}

// ── Comparison ────────────────────────────────────────────────────────────────

fn norm(s: String, mode: StringCompareMode) -> String {
    match mode {
        StringCompareMode::Text => s.to_ascii_lowercase(),
        StringCompareMode::Binary => s,
    }
}

/// Case-fold UTF-16 code units the same way [`norm`] folds a `String` — ASCII A–Z
/// only — but at the code-unit level so lone surrogate halves survive. Comparing the
/// lossy `as_string` would fold every unpaired surrogate to U+FFFD and make distinct
/// surrogate strings (e.g. `ChrW(&HD800)` vs `ChrW(&HDC00)`) compare EQUAL, which
/// contradicts VBA's binary UTF-16 code-unit string semantics (and FU#3's concat
/// fidelity). Binary mode passes the units through verbatim.
fn norm_units(units: &[u16], mode: StringCompareMode) -> Vec<u16> {
    match mode {
        StringCompareMode::Text => units
            .iter()
            .map(|&u| {
                if (0x41..=0x5A).contains(&u) {
                    u + 0x20
                } else {
                    u
                }
            })
            .collect(),
        StringCompareMode::Binary => units.to_vec(),
    }
}

/// A `String`-subtype operand (the only kind compared lexically).
fn cmp_is_string(v: &Variant) -> bool {
    v.vtype() == VarType::String
}

/// A numeric-subtype operand (`Boolean`/`Date`/`Currency`/`Decimal` are numeric in VBA). An
/// `Empty` operand is intentionally NOT numeric here — it adapts to the other side (0 or "").
fn cmp_is_numeric(v: &Variant) -> bool {
    matches!(
        v.vtype(),
        VarType::Boolean
            | VarType::SignedByte
            | VarType::Byte
            | VarType::Integer
            | VarType::UnsignedInteger
            | VarType::Long
            | VarType::UnsignedLong
            | VarType::UnsignedInt
            | VarType::LongLong
            | VarType::UnsignedLongLong
            | VarType::Single
            | VarType::Double
            | VarType::Currency
            | VarType::Decimal
            | VarType::Date
    )
}

fn cmp_order(
    l: &Variant,
    r: &Variant,
    mode: StringCompareMode,
) -> Result<std::cmp::Ordering, ArithError> {
    // A `String` compared with a numeric is a Type mismatch in VBA — even when the string is
    // numeric-looking (`"5" = 5` raises 13). Only same-kind operands compare; `Empty`/`Null`
    // are exempt (`Null` is handled by the caller; `Empty` adapts to the other operand).
    if (cmp_is_string(l) && cmp_is_numeric(r)) || (cmp_is_numeric(l) && cmp_is_string(r)) {
        return Err(ArithError::type_mismatch());
    }
    let both_string = l.vtype() == VarType::String && r.vtype() == VarType::String;
    if both_string {
        // Compare at the UTF-16 code-unit level (faithful to lone surrogate halves)
        // rather than via the lossy `as_string`. Both are String Variants here, so
        // `string_units` is `Some`.
        let lu = l.string_units().unwrap_or_default();
        let ru = r.string_units().unwrap_or_default();
        return Ok(norm_units(&lu, mode).cmp(&norm_units(&ru, mode)));
    }
    let left_num = num(l);
    let right_num = num(r);
    if left_num.as_ref().is_err_and(|e| e.code == 91) {
        return left_num.map(|_| std::cmp::Ordering::Equal);
    }
    if right_num.as_ref().is_err_and(|e| e.code == 91) {
        return right_num.map(|_| std::cmp::Ordering::Equal);
    }
    match (left_num, right_num) {
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
    // `Not <array>` — the VBA `(Not Not arr)` allocation-probe idiom. Real VBA
    // applies `Not` to the array's underlying `SAFEARRAY*` pointer: an
    // unallocated dynamic array (`Dim a()`, never `ReDim`-ed) is a null
    // descriptor, so `Not Not a` is 0; an allocated one yields a stable non-zero.
    // Here an unallocated array is an `Empty` slot (handled by the numeric path
    // below: `!0 = -1`, then the outer `Not(-1) = 0`), so only an *allocated*
    // array Variant reaches this arm — model it as a non-zero descriptor handle
    // so the double-`Not` round-trips to that non-zero (≠ 0). The handle's exact
    // value is irrelevant; only its non-zeroness (which survives `!!handle`) is.
    if v.vtype() == VarType::ArrayVariant {
        let allocated = v.as_safearray().is_some_and(|a| a.bounds().is_some());
        let handle: i64 = if allocated { 1 } else { 0 };
        return Ok(long_or_double(!handle));
    }
    Ok(long_or_double(!int(v)?))
}

fn bitlogic(
    l: &Variant,
    r: &Variant,
    bit: impl Fn(i64, i64) -> i64,
    logic: impl Fn(bool, bool) -> bool,
) -> R {
    let l_null = is_null(l);
    let r_null = is_null(r);
    if l_null && r_null {
        return Ok(Variant::null());
    }
    if l_null || r_null {
        // VBA three-valued logic: one operand is `Null` (every bit unknown). A result bit is
        // known only where the *other* operand alone forces it; if any bit still depends on
        // the unknown operand the whole result is `Null`. We detect this by evaluating the
        // bit-op with the unknown operand as all-0 vs all-1: where the two agree, the known
        // operand decided every bit. (`x And Null` = Null unless x=0→0; `x Or Null` = Null
        // unless x=-1→-1; `Xor`/`Eqv` never agree → Null; `Imp` = `(Not a) Or b` follows.)
        let (known, known_ty) = if l_null {
            (int(r)?, r.vtype())
        } else {
            (int(l)?, l.vtype())
        };
        let (lo, hi) = if l_null {
            (bit(0, known), bit(-1, known))
        } else {
            (bit(known, 0), bit(known, -1))
        };
        if lo != hi {
            return Ok(Variant::null());
        }
        // Determined: a `Boolean` known operand yields a `Boolean` result (e.g.
        // `False And Null` = False, `True Or Null` = True); otherwise a `Long`.
        return Ok(if known_ty == VarType::Boolean {
            Variant::from_bool(lo != 0)
        } else {
            long_or_double(lo)
        });
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
        return Err(ArithError::null_use());
    }
    match target {
        // `CBool`: any non-zero numeric is `True`, zero is `False`. A `Boolean` source
        // passes through; the string literals `"True"`/`"False"` are recognized (so the
        // implicit Let-coercion `Dim b As Boolean: b = "True"` agrees with explicit
        // `CBool` via the shared recognizer); any other non-numeric raises Type mismatch
        // via `num`.
        NumericCoerceTarget::Boolean => {
            if v.vtype() == VarType::Boolean {
                return Ok(v.clone());
            }
            let parsed_bool = if v.vtype() == VarType::String {
                oxvba_runtime::coerce::parse_bool_text(&as_string(v))
            } else {
                None
            };
            if let Some(b) = parsed_bool {
                return Ok(Variant::from_bool(b));
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
            if let Some(scaled) = exact_currency_operand(v) {
                return checked_currency(scaled?);
            }
            let scaled = (num(v)? * 10_000.0).round_ties_even();
            if scaled.is_finite() && scaled >= i64::MIN as f64 && scaled <= i64::MAX as f64 {
                Ok(Variant::from_currency_scaled_i64(scaled as i64))
            } else {
                Err(ArithError::overflow())
            }
        }
        NumericCoerceTarget::Date => {
            if v.vtype() == VarType::String {
                let text = as_string(v);
                if let Some(serial) = oxvba_runtime::date::parse_date_text_serial(&text) {
                    return Ok(Variant::from_date_f64(serial));
                }
            }
            Ok(Variant::from_date_f64(num(v)?))
        }
    }
}

pub fn coerce_string(v: &Variant) -> R {
    if is_null(v) {
        return Err(ArithError::null_use());
    }
    coerce_to(v, VarType::String).map_err(ArithError::from)
}

/// Pad with trailing spaces or truncate to a fixed UTF-16 code-unit length.
pub fn coerce_fixed_string(v: &Variant, len: usize) -> R {
    let text = coerce_string(v)?;
    let mut units = text.string_units().ok_or_else(ArithError::type_mismatch)?;
    if units.len() > len {
        units.truncate(len);
    } else {
        while units.len() < len {
            units.push(0x20);
        }
    }
    Ok(Variant::from_utf16_units(&units))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_runtime::safe_array::SafeArray;

    /// Widening `+`/`-` re-tag a `Date` result per live VBA: any `Date` operand in a
    /// `+` (incl. `Date + Date`) gives a `Date`; a single `Date` operand in a `-` gives
    /// a `Date`, but `Date - Date` is a plain `Double`. `*` is never a `Date`.
    #[test]
    fn date_arithmetic_preserves_or_drops_the_date_subtype() {
        let d = Variant::from_date_f64(36526.0); // #1/1/2000#
        let one = Variant::from_i32(1);
        let w = NumericMode::Widening;
        let ty = |r: R| r.unwrap().vtype();
        assert_eq!(ty(add(&d, &one, w)), VarType::Date);
        assert_eq!(ty(add(&one, &d, w)), VarType::Date);
        assert_eq!(ty(add(&d, &d, w)), VarType::Date);
        assert_eq!(ty(sub(&d, &one, w)), VarType::Date);
        assert_eq!(ty(sub(&one, &d, w)), VarType::Date);
        assert_eq!(ty(sub(&d, &d, w)), VarType::Double);
        assert_eq!(ty(mul(&d, &Variant::from_i32(2), w)), VarType::Double);
    }

    #[test]
    fn currency_arithmetic_uses_exact_scaled_integer_math() {
        let c = |scaled| Variant::from_currency_scaled_i64(scaled);
        let checked = NumericMode::Checked(NumericCoerceTarget::Currency);
        let widening = NumericMode::Widening;

        let near = c(300_000_000_001);
        let product = mul(&near, &near, checked).expect("near-boundary product");
        assert_eq!(
            product.as_currency_scaled_i64(),
            Some(9_000_000_000_060_000_000)
        );

        let ordinary = mul(&c(12_345), &c(67_891), widening).expect("ordinary product");
        assert_eq!(ordinary.vtype(), VarType::Currency);
        assert_eq!(ordinary.as_currency_scaled_i64(), Some(83_811));

        let negative = mul(&c(-12_345), &c(67_891), widening).expect("negative product");
        assert_eq!(negative.as_currency_scaled_i64(), Some(-83_811));

        let sum = add(&c(9_223_372_036_854_770_000), &c(5_807), widening).expect("exact max add");
        assert_eq!(sum.as_currency_scaled_i64(), Some(i64::MAX));

        let diff = sub(&c(i64::MIN), &c(-1), checked).expect("exact min plus one");
        assert_eq!(diff.as_currency_scaled_i64(), Some(i64::MIN + 1));

        let recoerced =
            coerce_numeric(&c(9_000_000_000_060_000_000), NumericCoerceTarget::Currency)
                .expect("Currency-to-Currency coercion");
        assert_eq!(
            recoerced.as_currency_scaled_i64(),
            Some(9_000_000_000_060_000_000)
        );
    }

    #[test]
    fn currency_multiplication_rounds_ties_to_even_scaled_units() {
        let c = |scaled| Variant::from_currency_scaled_i64(scaled);
        let checked = NumericMode::Checked(NumericCoerceTarget::Currency);

        let down_to_even = mul(&c(1), &c(5_000), checked).expect("0.5 scaled-unit tie");
        assert_eq!(down_to_even.as_currency_scaled_i64(), Some(0));

        let up_to_even = mul(&c(3), &c(5_000), checked).expect("1.5 scaled-unit tie");
        assert_eq!(up_to_even.as_currency_scaled_i64(), Some(2));
    }

    #[test]
    fn currency_arithmetic_overflow_reports_overflow() {
        let c = |scaled| Variant::from_currency_scaled_i64(scaled);
        let checked = NumericMode::Checked(NumericCoerceTarget::Currency);

        assert_eq!(add(&c(i64::MAX), &c(1), checked).unwrap_err().code, 6);
        assert_eq!(sub(&c(i64::MIN), &c(1), checked).unwrap_err().code, 6);
        assert_eq!(mul(&c(i64::MAX), &c(20_000), checked).unwrap_err().code, 6);
    }

    /// Coercing a string to `Boolean` (the implicit `Dim b As Boolean: b = …` path)
    /// recognizes the `"True"`/`"False"` literals like `CBool`, converts numeric
    /// strings by non-zero-ness, and raises Type mismatch (13) on anything else.
    #[test]
    fn coerce_string_to_boolean_matches_cbool() {
        let b = |s: &str| coerce_numeric(&Variant::from_string(s), NumericCoerceTarget::Boolean);
        assert_eq!(b("True").unwrap().as_bool(), Some(true));
        assert_eq!(b("false").unwrap().as_bool(), Some(false));
        assert_eq!(b("tRuE").unwrap().as_bool(), Some(true));
        assert_eq!(b("5").unwrap().as_bool(), Some(true));
        assert_eq!(b("0").unwrap().as_bool(), Some(false));
        assert_eq!(b("-1").unwrap().as_bool(), Some(true));
        // Strict like live VBA: padded/non-numeric, non-literal strings are Type mismatch.
        assert_eq!(b("  False  ").unwrap_err().code, 13);
        assert_eq!(b("yes").unwrap_err().code, 13);
    }

    /// `(Not Not arr) = 0` for an unallocated dynamic array (an `Empty` slot),
    /// and `≠ 0` for an allocated one — the VBA allocation-probe idiom.
    #[test]
    fn not_not_array_probes_allocation() {
        // Unallocated `Dim a()` is an Empty slot: `!0 = -1`, then `!(-1) = 0`.
        let unallocated = Variant::empty();
        let inner = not(&unallocated).unwrap();
        let outer = not(&inner).unwrap();
        assert_eq!(int(&outer).unwrap(), 0, "unallocated array probes as 0");

        // An allocated array (a bounded SafeArray) double-Nots to a non-zero.
        let allocated = Variant::from_safearray(SafeArray::from_variants(vec![
            Variant::from_i32(1),
            Variant::from_i32(2),
        ]));
        let inner = not(&allocated).unwrap();
        let outer = not(&inner).unwrap();
        assert_ne!(
            int(&outer).unwrap(),
            0,
            "allocated array probes as non-zero"
        );
    }

    /// A single `Not <allocated array>` no longer faults type 13 — it returns a
    /// numeric value (the complement of the descriptor handle).
    #[test]
    fn not_of_allocated_array_is_numeric_not_a_fault() {
        let allocated =
            Variant::from_safearray(SafeArray::from_variants(vec![Variant::from_i32(7)]));
        let result = not(&allocated).expect("Not on an array must not fault");
        assert!(int(&result).is_ok());
    }

    /// FU#3 fidelity: string comparison distinguishes lone surrogate halves instead of
    /// folding them to U+FFFD. `ChrW(&HD800)` and `ChrW(&HDC00)` are distinct UTF-16 code
    /// units and must compare UNEQUAL — the pre-fix lossy `as_string` path folded both to
    /// U+FFFD and reported them equal. Also pins round-trip, ordering, and concat parity.
    #[test]
    fn lone_surrogate_strings_compare_by_code_unit() {
        let high = Variant::from_utf16_units(&[0xD800]);
        let low = Variant::from_utf16_units(&[0xDC00]);
        let replacement = Variant::from_utf16_units(&[0xFFFD]);
        let mode = StringCompareMode::Binary;

        // Each half survives verbatim (faithful round-trip).
        assert_eq!(high.string_units(), Some(vec![0xD800]));
        assert_eq!(low.string_units(), Some(vec![0xDC00]));

        // Distinct halves are NOT equal (the lossy path returned True here pre-fix), and a
        // lone high surrogate is not the replacement char either.
        let eq = compare(&high, &low, mode, CmpOp::Eq).unwrap();
        assert_eq!(
            int(&eq).unwrap(),
            0,
            "ChrW(&HD800) = ChrW(&HDC00) must be False"
        );
        let eq_repl = compare(&high, &replacement, mode, CmpOp::Eq).unwrap();
        assert_eq!(
            int(&eq_repl).unwrap(),
            0,
            "ChrW(&HD800) = ChrW(&HFFFD) must be False"
        );

        // A lone-surrogate string equals itself, and ordering is by code unit.
        let eq_self = compare(
            &high,
            &Variant::from_utf16_units(&[0xD800]),
            mode,
            CmpOp::Eq,
        )
        .unwrap();
        assert_ne!(
            int(&eq_self).unwrap(),
            0,
            "a lone-surrogate string equals itself"
        );
        let lt = compare(&high, &low, mode, CmpOp::Lt).unwrap();
        assert_ne!(int(&lt).unwrap(), 0, "0xD800 < 0xDC00 by code unit");

        // Concatenating the two halves yields the real surrogate PAIR verbatim.
        let pair = concat(&high, &low).unwrap();
        assert_eq!(pair.string_units(), Some(vec![0xD800, 0xDC00]));
    }
}
