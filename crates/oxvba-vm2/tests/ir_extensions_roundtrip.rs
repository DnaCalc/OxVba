//! Round-trip coverage for the IR extensions added for the binder:
//! the `Xor`/`Eqv`/`Imp` operators and the `Single`/`Double`/`Currency`/`Date`
//! narrowing-coercion targets. Built as symbol-free Core IR (like
//! `linearize_roundtrip.rs`) so a failure localizes to the IR, not the binder.

use oxvba_bundle::coreir::*;
use oxvba_bundle::linearize::linearize;
use oxvba_bundle::{AssignmentIntent, AssignmentTargetKind, NumericCoerceTarget, ProcedureKind, StringCompareMode};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;

// ── Builders ───────────────────────────────────────────────────────────────

fn single(local_count: usize, body: Vec<CoreStmt>) -> CoreProgram {
    let main = CoreProc {
        name: "Main".into(),
        kind: ProcedureKind::Sub,
        params: Vec::new(),
        locals: (0..local_count).map(|i| CoreLocal { name: format!("v{i}"), array_element: None }).collect(),
        return_local: None,
        body,
    };
    CoreProgram {
        globals: Vec::new(),
        procs: vec![main],
        classes: Vec::new(),
        event_routes: Vec::new(),
        external_calls: Vec::new(),
        com_class_exports: Vec::new(),
        entry: None,
        ..Default::default()
    }
}

fn ci(value: i32) -> CoreValue {
    CoreValue::Const(CoreConst::I32(value))
}
fn cf(value: f64) -> CoreValue {
    CoreValue::Const(CoreConst::F64(value.to_bits()))
}
fn cb(value: bool) -> CoreValue {
    CoreValue::Const(CoreConst::Bool(value))
}
fn bin(op: CoreBinOp, lhs: CoreValue, rhs: CoreValue) -> CoreValue {
    CoreValue::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), mode: StringCompareMode::Binary }
}
fn coerce(value: CoreValue, target: NumericCoerceTarget) -> CoreValue {
    CoreValue::Coerce { value: Box::new(value), to: CoerceTarget::Numeric(target) }
}
fn set(slot: usize, value: CoreValue) -> CoreStmt {
    CoreStmt::Assign {
        place: CorePlace::Local(LocalId(slot)),
        value,
        intent: AssignmentIntent::Let,
        target_kind: AssignmentTargetKind::Variant,
        target_name: format!("v{slot}"),
        target_type_name: "Variant".into(),
    }
}

/// Read the first local as a number across every scalar Variant subtype the new
/// ops can produce (Long/Double/Single/Currency/Date/Boolean).
fn first_local_num(program: &CoreProgram) -> Option<f64> {
    let bundle = linearize(program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    let value = vm.slot(bundle.global_count)?;
    value
        .as_f64()
        .or_else(|| value.as_f32().map(f64::from))
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
        .or_else(|| value.as_currency_scaled_i64().map(|v| v as f64 / 10_000.0))
        .or_else(|| value.as_date_f64())
        .or_else(|| value.as_bool().map(|b| if b { -1.0 } else { 0.0 }))
}

// ── Xor / Eqv / Imp ──────────────────────────────────────────────────────────

#[test]
fn xor_integers() {
    // 6 Xor 3 = 0b110 ^ 0b011 = 0b101 = 5
    assert_eq!(first_local_num(&single(1, vec![set(0, bin(CoreBinOp::Xor, ci(6), ci(3)))])), Some(5.0));
}

#[test]
fn xor_booleans() {
    // True Xor False = True (-1)
    assert_eq!(first_local_num(&single(1, vec![set(0, bin(CoreBinOp::Xor, cb(true), cb(false)))])), Some(-1.0));
}

#[test]
fn eqv_equal_operands_is_all_ones() {
    // 5 Eqv 5 = Not(5 Xor 5) = Not(0) = -1
    assert_eq!(first_local_num(&single(1, vec![set(0, bin(CoreBinOp::Eqv, ci(5), ci(5)))])), Some(-1.0));
}

#[test]
fn eqv_booleans() {
    // True Eqv False = False (0)
    assert_eq!(first_local_num(&single(1, vec![set(0, bin(CoreBinOp::Eqv, cb(true), cb(false)))])), Some(0.0));
}

#[test]
fn imp_true_implies_false_is_false() {
    // True Imp False = (Not True) Or False = False (0)
    assert_eq!(first_local_num(&single(1, vec![set(0, bin(CoreBinOp::Imp, cb(true), cb(false)))])), Some(0.0));
}

#[test]
fn imp_false_antecedent_is_true() {
    // False Imp False = (Not False) Or False = True (-1)
    assert_eq!(first_local_num(&single(1, vec![set(0, bin(CoreBinOp::Imp, cb(false), cb(false)))])), Some(-1.0));
}

// ── Narrowing coercions: Single / Currency / Date ────────────────────────────

#[test]
fn coerce_single() {
    // CSng(2.5) → Single 2.5
    assert_eq!(first_local_num(&single(1, vec![set(0, coerce(cf(2.5), NumericCoerceTarget::Single))])), Some(2.5));
}

#[test]
fn coerce_currency() {
    // CCur(1.5) → Currency (scaled 15000) → 1.5
    assert_eq!(first_local_num(&single(1, vec![set(0, coerce(cf(1.5), NumericCoerceTarget::Currency))])), Some(1.5));
}

#[test]
fn coerce_date() {
    // CDate(40000.0) → Date serial 40000.0
    assert_eq!(first_local_num(&single(1, vec![set(0, coerce(cf(40000.0), NumericCoerceTarget::Date))])), Some(40000.0));
}
