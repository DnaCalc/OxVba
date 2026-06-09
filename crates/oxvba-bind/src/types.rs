//! VBA type lattice + coercion (binder side). Pure functions mapping the symbol
//! model's `VarTypeRef` into the coreir coercion / array / target-kind vocabulary,
//! plus the operator result lattice and the `coerce`-node insertion the
//! expression binder uses.
//!
//! Type inference here is deliberately a *safe over-approximation*: when in doubt
//! the result is `Variant`. The VM computes the real value; the binder only uses
//! the inferred type to decide which coercion node (if any) to insert. Inferring
//! `Variant` therefore never produces a wrong result — at worst a redundant
//! `Coerce` on assignment to a typed variable.

use oxvba_bundle::coreir::{CoerceTarget, CoreBinOp, CoreValue};
use oxvba_bundle::{ArrayElementType, AssignmentTargetKind, NumericCoerceTarget, NumericMode};
use oxvba_symbol::signature::{BuiltinType, VarTypeRef};

// ── Type → coreir mappings (frame builder + ReDim) ──────────────────────────

/// The coreir array element type for a declared element type.
pub fn array_element_of(elem: &VarTypeRef) -> ArrayElementType {
    match elem {
        VarTypeRef::Builtin(b) => builtin_element(*b),
        VarTypeRef::FixedString(_) => ArrayElementType::String,
        VarTypeRef::Object(_) | VarTypeRef::Udt(_) | VarTypeRef::Variant | VarTypeRef::Array(_) => {
            ArrayElementType::Variant
        }
    }
}

fn builtin_element(b: BuiltinType) -> ArrayElementType {
    match b {
        BuiltinType::Boolean => ArrayElementType::Boolean,
        BuiltinType::Byte => ArrayElementType::Byte,
        BuiltinType::Integer => ArrayElementType::Integer,
        BuiltinType::Long => ArrayElementType::Long,
        BuiltinType::LongLong => ArrayElementType::LongLong,
        BuiltinType::LongPtr => ArrayElementType::LongPtr,
        BuiltinType::Single => ArrayElementType::Single,
        BuiltinType::Double => ArrayElementType::Double,
        BuiltinType::Currency => ArrayElementType::Currency,
        BuiltinType::Date => ArrayElementType::Date,
        BuiltinType::String => ArrayElementType::String,
    }
}

/// `Some(element)` when the declared type is an array, for `CoreLocal`/`CoreGlobal`.
pub fn array_element(ty: &VarTypeRef) -> Option<ArrayElementType> {
    match ty {
        VarTypeRef::Array(inner) => Some(array_element_of(inner)),
        _ => None,
    }
}

/// The static assignment-target kind (drives Let/Set legality + `ValidateAssignment`).
pub fn assignment_target_kind(ty: &VarTypeRef) -> AssignmentTargetKind {
    match ty {
        VarTypeRef::Object(_) => AssignmentTargetKind::Object,
        VarTypeRef::Variant => AssignmentTargetKind::Variant,
        // A UDT is a value aggregate: `q = p` is a `Let` (value copy of the record),
        // never `Set`.
        VarTypeRef::Builtin(_)
        | VarTypeRef::Udt(_)
        | VarTypeRef::Array(_)
        | VarTypeRef::FixedString(_) => AssignmentTargetKind::Scalar,
    }
}

/// A display name for a declared type (for `ValidateAssignment` diagnostics).
pub fn type_name(ty: &VarTypeRef) -> String {
    match ty {
        VarTypeRef::Builtin(b) => format!("{b:?}"),
        VarTypeRef::Object(name) | VarTypeRef::Udt(name) => name.clone(),
        VarTypeRef::Variant => "Variant".into(),
        VarTypeRef::Array(inner) => format!("{}()", type_name(inner)),
        VarTypeRef::FixedString(_) => "String".into(),
    }
}

// ── Coercion insertion ──────────────────────────────────────────────────────

/// The narrowing target when storing into a fixed scalar type, or `None` when the
/// VM's store coercion suffices (`String`/`Boolean`/`Variant`/`Object`/array).
pub fn coerce_target(to: &VarTypeRef) -> Option<CoerceTarget> {
    match to {
        VarTypeRef::Builtin(b) => numeric_target(*b).map(CoerceTarget::Numeric),
        // Assigning to a fixed-length string pads/truncates to its length.
        VarTypeRef::FixedString(len) => Some(CoerceTarget::FixedString(*len as usize)),
        _ => None,
    }
}

fn numeric_target(b: BuiltinType) -> Option<NumericCoerceTarget> {
    Some(match b {
        // Storing into a `Boolean` is `CBool`: non-zero → `True`, zero → `False`.
        BuiltinType::Boolean => NumericCoerceTarget::Boolean,
        BuiltinType::Byte => NumericCoerceTarget::Byte,
        BuiltinType::Integer => NumericCoerceTarget::Integer,
        // LongPtr is LongLong on 64-bit Office (the modern default).
        BuiltinType::Long => NumericCoerceTarget::Long,
        BuiltinType::LongPtr | BuiltinType::LongLong => NumericCoerceTarget::LongLong,
        BuiltinType::Single => NumericCoerceTarget::Single,
        BuiltinType::Double => NumericCoerceTarget::Double,
        BuiltinType::Currency => NumericCoerceTarget::Currency,
        BuiltinType::Date => NumericCoerceTarget::Date,
        BuiltinType::String => return None,
    })
}

/// The [`NumericMode`] for an arithmetic op (`+`/`-`/`*`/`\`/`Mod`/unary `-`) whose
/// static result type is `ty`: `Checked(target)` when the operands promote to a fixed
/// numeric type (so the VM computes in it and raises Overflow on out-of-range), else
/// `Widening` (a `Variant` operand, or the `Double` ceiling — the VBA widening regime).
/// VBA's overflow-vs-widen choice is *static* (identical run-time tags can mean either),
/// so the binder picks it here and stamps it on the op; the VM and JIT read it back.
/// `Boolean`/`String`/`Date`/`Variant` results map to `Widening` (non-arithmetic or
/// already-widest), as does `Double`.
pub fn numeric_mode(ty: &VarTypeRef) -> NumericMode {
    let VarTypeRef::Builtin(b) = ty else { return NumericMode::Widening };
    match b {
        BuiltinType::Double | BuiltinType::Boolean | BuiltinType::Date | BuiltinType::String => {
            NumericMode::Widening
        }
        other => numeric_target(*other).map_or(NumericMode::Widening, NumericMode::Checked),
    }
}

/// Coerce a value being **stored** into a declared scalar target to that target's
/// type — unconditionally (unlike [`coerce`], which trusts the source's static type
/// and skips identity conversions). A declared variable must hold its declared type
/// at run time regardless of the value's static-vs-runtime tag (the VM ops are
/// Double-biased and integer literals carry a `Long` payload). No-op for
/// `String`/`Object`/`Variant`/array targets (their store needs no narrowing). Skips
/// re-wrapping a value already coerced to the same target.
pub fn coerce_store(value: CoreValue, to: &VarTypeRef) -> CoreValue {
    let Some(target) = coerce_target(to) else { return value };
    if let CoreValue::Coerce { to: existing, .. } = &value
        && *existing == target
    {
        return value;
    }
    CoreValue::Coerce { value: Box::new(value), to: target }
}

/// Coerce `value` (of type `from`) to type `to`, wrapping in a `Coerce` node only
/// when a numeric conversion is actually needed (skips identity conversions).
pub fn coerce(value: CoreValue, from: &VarTypeRef, to: &VarTypeRef) -> CoreValue {
    if from == to {
        return value;
    }
    // An array value is never numerically coercible — passing an array argument to a
    // parameter (`f(arr)`) copies the array, it does not coerce its elements. Without
    // this guard a `Dim a() As Byte` passed by value (or to a scalar-typed slot) would
    // emit a `CoerceNumeric` on the whole array and fault at run time.
    if matches!(from, VarTypeRef::Array(_)) {
        return value;
    }
    match coerce_target(to) {
        Some(target) => CoreValue::Coerce { value: Box::new(value), to: target },
        None => value,
    }
}

pub fn is_longlong(ty: &VarTypeRef) -> bool {
    matches!(ty, VarTypeRef::Builtin(BuiltinType::LongLong | BuiltinType::LongPtr))
}

// ── Operator result lattice ─────────────────────────────────────────────────

/// The (over-approximated) result type of a binary operator.
pub fn result_type(op: CoreBinOp, lhs: &VarTypeRef, rhs: &VarTypeRef) -> VarTypeRef {
    use CoreBinOp::*;
    match op {
        Eq | Ne | Lt | Le | Gt | Ge | Is | Like => builtin(BuiltinType::Boolean),
        Concat => builtin(BuiltinType::String),
        And | Or | Xor | Eqv | Imp => {
            if is_boolean(lhs) && is_boolean(rhs) {
                builtin(BuiltinType::Boolean)
            } else if is_variant(lhs) || is_variant(rhs) {
                VarTypeRef::Variant
            } else {
                builtin(BuiltinType::Long)
            }
        }
        IntDiv | Mod => builtin(BuiltinType::Long),
        Div | Pow => {
            if is_variant(lhs) || is_variant(rhs) {
                VarTypeRef::Variant
            } else {
                builtin(BuiltinType::Double)
            }
        }
        Add | Sub | Mul => arith_result(lhs, rhs),
    }
}

/// Numeric-widening result for `+`/`-`/`*` (Variant when either side isn't a
/// plain numeric builtin — safe over-approximation).
fn arith_result(lhs: &VarTypeRef, rhs: &VarTypeRef) -> VarTypeRef {
    match (numeric_rank(lhs), numeric_rank(rhs)) {
        (Some(a), Some(b)) => builtin(rank_to_builtin(a.max(b))),
        _ => VarTypeRef::Variant,
    }
}

fn numeric_rank(ty: &VarTypeRef) -> Option<u8> {
    match ty {
        VarTypeRef::Builtin(b) => match b {
            BuiltinType::Byte => Some(0),
            BuiltinType::Integer => Some(1),
            BuiltinType::Long | BuiltinType::LongPtr => Some(2),
            BuiltinType::LongLong => Some(3),
            BuiltinType::Currency => Some(4),
            BuiltinType::Single => Some(5),
            BuiltinType::Double => Some(6),
            // Boolean/String/Date aren't ranked for `+`/`-`/`*` widening.
            _ => None,
        },
        _ => None,
    }
}

fn rank_to_builtin(rank: u8) -> BuiltinType {
    match rank {
        0 => BuiltinType::Byte,
        1 => BuiltinType::Integer,
        2 => BuiltinType::Long,
        3 => BuiltinType::LongLong,
        4 => BuiltinType::Currency,
        5 => BuiltinType::Single,
        _ => BuiltinType::Double,
    }
}

fn builtin(b: BuiltinType) -> VarTypeRef {
    VarTypeRef::Builtin(b)
}

pub fn is_variant(ty: &VarTypeRef) -> bool {
    matches!(ty, VarTypeRef::Variant)
}

pub fn is_boolean(ty: &VarTypeRef) -> bool {
    matches!(ty, VarTypeRef::Builtin(BuiltinType::Boolean))
}

/// Consumed by the objects/COM phase (early/late dispatch by receiver type).
#[allow(dead_code)]
pub fn is_object(ty: &VarTypeRef) -> bool {
    matches!(ty, VarTypeRef::Object(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long() -> VarTypeRef {
        builtin(BuiltinType::Long)
    }

    #[test]
    fn widening_picks_the_wider_type() {
        assert_eq!(
            result_type(CoreBinOp::Add, &builtin(BuiltinType::Integer), &builtin(BuiltinType::Double)),
            builtin(BuiltinType::Double)
        );
        assert_eq!(result_type(CoreBinOp::Mul, &long(), &long()), long());
    }

    #[test]
    fn comparisons_and_concat_have_fixed_result() {
        assert_eq!(result_type(CoreBinOp::Lt, &long(), &long()), builtin(BuiltinType::Boolean));
        assert_eq!(
            result_type(CoreBinOp::Concat, &long(), &builtin(BuiltinType::String)),
            builtin(BuiltinType::String)
        );
    }

    #[test]
    fn variant_operand_propagates() {
        assert_eq!(result_type(CoreBinOp::Add, &VarTypeRef::Variant, &long()), VarTypeRef::Variant);
    }

    #[test]
    fn coerce_skips_identity_and_wraps_narrowing() {
        let v = CoreValue::Const(oxvba_bundle::coreir::CoreConst::I32(5));
        // Long → Long: no node.
        assert!(matches!(coerce(v.clone(), &long(), &long()), CoreValue::Const(_)));
        // Long → Integer: a Coerce node.
        assert!(matches!(
            coerce(v, &long(), &builtin(BuiltinType::Integer)),
            CoreValue::Coerce { .. }
        ));
        // String target: no numeric coercion.
        assert!(coerce_target(&builtin(BuiltinType::String)).is_none());
        // Currency target: now expressible.
        assert!(coerce_target(&builtin(BuiltinType::Currency)).is_some());
    }
}
