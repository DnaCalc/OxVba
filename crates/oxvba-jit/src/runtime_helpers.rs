//! Runtime helper functions for JIT-compiled VBA code.
//!
//! Each helper is an `extern "C"` function registered with Cranelift's
//! `JITBuilder::symbol()`. They implement instruction semantics by calling
//! the same shared logic as the VM interpreter (via `oxvba_vm::semantics`).
//!
//! Return convention: 0 = success, nonzero = error code.

use std::cmp::Ordering;

use oxvba_com::{ComValue, DynamicCallArg, DynamicCallRequest, DynamicObjectBridge};
use oxvba_compiler::bytecode::{
    DispatchInvokeArg, RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
};
use oxvba_hal::HalComDynamicBridge;
use oxvba_hal::error::{HalError, HalErrorKind};
use oxvba_hal::model::CapabilityId;
use oxvba_runtime::safe_array::{array_len_from_tag, is_array_tag as runtime_is_array_tag};
use oxvba_runtime::value_tags::{
    EMPTY_TAG, NULL_TAG, error_tag_from_code, is_error_tag as runtime_is_error_tag,
};
use oxvba_runtime::{F64Value, RuntimeValue, bstr::BStr};
use oxvba_vm::semantics;

use crate::jit_context::JitContext;
// slot_abi types used indirectly via JitContext's slot array

// ── Error code constants ──────────────────────────────────────────────

const OK: i32 = 0;
const ERR_RUNTIME: i32 = -1; // Generic runtime error (fatal)

// ── Macro for common helper pattern ───────────────────────────────────

macro_rules! read_slot {
    ($ctx:expr, $slot:expr) => {{
        debug_assert!(!$ctx.is_null(), "read_slot: null JitContext pointer");
        unsafe { (*$ctx).read_slot($slot) }
    }};
}

macro_rules! write_slot {
    ($ctx:expr, $slot:expr, $value:expr) => {{
        debug_assert!(!$ctx.is_null(), "write_slot: null JitContext pointer");
        unsafe { (*$ctx).write_slot($slot, $value) }
    }};
}

// ── Arithmetic helpers ────────────────────────────────────────────────

/// AddSlots: dst = lhs + rhs
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_add_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_add_values(&lhs_val, &rhs_val) {
        Ok(result) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// SubSlots: dst = lhs - rhs
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sub_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_sub_values(&lhs_val, &rhs_val) {
        Ok(result) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// MulSlots: dst = lhs * rhs
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_mul_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_mul_values(&lhs_val, &rhs_val) {
        Ok(result) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// DivSlots: dst = lhs / rhs (floating-point division)
/// Returns error code 11 for division by zero.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_div_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_div_values(&lhs_val, &rhs_val) {
        Ok(Ok(result)) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Ok(Err(error_code)) => error_code,
        Err(_) => ERR_RUNTIME,
    }
}

/// IntDivSlots: dst = lhs \ rhs (integer division)
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_intdiv_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_intdiv_values(&lhs_val, &rhs_val) {
        Ok(Ok(result)) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Ok(Err(error_code)) => error_code,
        Err(_) => ERR_RUNTIME,
    }
}

/// ModSlots: dst = lhs Mod rhs
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_mod_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_mod_values(&lhs_val, &rhs_val) {
        Ok(Ok(result)) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Ok(Err(error_code)) => error_code,
        Err(_) => ERR_RUNTIME,
    }
}

/// PowSlots: dst = lhs ^ rhs
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_pow_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match semantics::legacy_pow_values(&lhs_val, &rhs_val) {
        Ok(result) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// NegSlot: dst = -src
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_neg_slot(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match semantics::legacy_neg_value(&val) {
        Ok(result) => {
            write_slot!(ctx, dst, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// ConcatSlots: dst = lhs & rhs
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_concat_slots(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    let result = semantics::legacy_concat_values(&lhs_val, &rhs_val);
    write_slot!(ctx, dst, result);
    OK
}

/// AddConstI32: slot += value
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_add_const(ctx: *mut JitContext, slot: u32, value: i32) -> i32 {
    let val = read_slot!(ctx, slot);
    match semantics::legacy_add_const_value(&val, value, "add-const operand") {
        Ok(result) => {
            write_slot!(ctx, slot, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// SubConstI32: slot -= value (implemented as add_const with negated value)
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sub_const(ctx: *mut JitContext, slot: u32, value: i32) -> i32 {
    let val = read_slot!(ctx, slot);
    match semantics::legacy_add_const_value(&val, -value, "sub-const operand") {
        Ok(result) => {
            write_slot!(ctx, slot, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// IncSlot: slot += 1
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_inc_slot(ctx: *mut JitContext, slot: u32) -> i32 {
    let val = read_slot!(ctx, slot);
    match semantics::legacy_increment_value(&val) {
        Ok(result) => {
            write_slot!(ctx, slot, result);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// ── Comparison helpers ────────────────────────────────────────────────

fn compare_slots(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
    pred: fn(Ordering) -> bool,
) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    let scm = if mode == 1 {
        StringCompareMode::Text
    } else {
        StringCompareMode::Binary
    };
    match semantics::typed_compare_values(&lhs_val, &rhs_val, scm, pred) {
        Ok(result) => {
            write_slot!(ctx, dst, RuntimeValue::I32(i32::from(result)));
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cmp_eq(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    compare_slots(ctx, dst, lhs, rhs, mode, |o| o == Ordering::Equal)
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cmp_ne(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    compare_slots(ctx, dst, lhs, rhs, mode, |o| o != Ordering::Equal)
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cmp_lt(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    compare_slots(ctx, dst, lhs, rhs, mode, |o| o == Ordering::Less)
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cmp_le(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    compare_slots(ctx, dst, lhs, rhs, mode, |o| {
        o == Ordering::Less || o == Ordering::Equal
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cmp_gt(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    compare_slots(ctx, dst, lhs, rhs, mode, |o| o == Ordering::Greater)
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cmp_ge(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    compare_slots(ctx, dst, lhs, rhs, mode, |o| {
        o == Ordering::Greater || o == Ordering::Equal
    })
}

// ── Boolean helpers ───────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_bool_not(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match semantics::legacy_truthy_value(&val) {
        Ok(truthy) => {
            write_slot!(ctx, dst, RuntimeValue::I32(i32::from(!truthy)));
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_bool_and(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match (
        semantics::legacy_truthy_value(&lhs_val),
        semantics::legacy_truthy_value(&rhs_val),
    ) {
        (Ok(l), Ok(r)) => {
            write_slot!(ctx, dst, RuntimeValue::I32(i32::from(l && r)));
            OK
        }
        _ => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_bool_or(ctx: *mut JitContext, dst: u32, lhs: u32, rhs: u32) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    match (
        semantics::legacy_truthy_value(&lhs_val),
        semantics::legacy_truthy_value(&rhs_val),
    ) {
        (Ok(l), Ok(r)) => {
            write_slot!(ctx, dst, RuntimeValue::I32(i32::from(l || r)));
            OK
        }
        _ => ERR_RUNTIME,
    }
}

// ── Intrinsic math helpers ────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_abs(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let result = match &val {
        RuntimeValue::Null => RuntimeValue::Null,
        RuntimeValue::F64(v) => RuntimeValue::F64(F64Value::from_f64(v.as_f64().abs())),
        RuntimeValue::I32(v) => {
            if *v == i32::MIN {
                RuntimeValue::I32(i32::MAX) // saturate
            } else {
                RuntimeValue::I32(v.abs())
            }
        }
        _ => {
            if let Ok(token) = semantics::runtime_value_legacy_token(&val, "abs operand") {
                if token == i32::MIN {
                    RuntimeValue::I32(i32::MAX)
                } else {
                    RuntimeValue::I32(token.abs())
                }
            } else {
                return ERR_RUNTIME;
            }
        }
    };
    write_slot!(ctx, dst, result);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sgn(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let result = match &val {
        RuntimeValue::Null => RuntimeValue::Null,
        RuntimeValue::F64(v) => {
            let f = v.as_f64();
            RuntimeValue::I32(if f > 0.0 {
                1
            } else if f < 0.0 {
                -1
            } else {
                0
            })
        }
        RuntimeValue::I32(v) => RuntimeValue::I32(v.signum()),
        _ => {
            if let Ok(token) = semantics::runtime_value_legacy_token(&val, "sgn operand") {
                RuntimeValue::I32(token.signum())
            } else {
                return ERR_RUNTIME;
            }
        }
    };
    write_slot!(ctx, dst, result);
    OK
}

/// Int/Fix for i32 values (pass-through).
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_int_fix(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let result = match &val {
        RuntimeValue::Null => RuntimeValue::Null,
        RuntimeValue::F64(v) => RuntimeValue::I32(v.as_f64().floor() as i32),
        _ => val,
    };
    write_slot!(ctx, dst, result);
    OK
}

// ── Slot copy helper ──────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_copy_slot(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    write_slot!(ctx, dst, val);
    OK
}

// ── LoadConstString helper ────────────────────────────────────────────

/// Load a string constant into a slot. The string data is passed as a
/// pointer + length pair (UTF-8 bytes from the instruction's value field).
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_string(
    ctx: *mut JitContext,
    dst: u32,
    ptr: *const u8,
    len: u32,
) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let s = String::from_utf8_lossy(bytes).into_owned();
    write_slot!(ctx, dst, RuntimeValue::String(BStr(s)));
    OK
}

// ── LoadConstF64 helper ───────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_f64(ctx: *mut JitContext, dst: u32, bits: u64) -> i32 {
    write_slot!(ctx, dst, RuntimeValue::F64(F64Value::from_bits(bits)));
    OK
}

// ── Phase 2: Intrinsic function helpers ───────────────────────────────

// ── String ops ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_len(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match &val {
        RuntimeValue::String(s) => {
            write_slot!(ctx, dst, RuntimeValue::I32(s.0.len() as i32));
        }
        _ => {
            if let Ok(token) = semantics::runtime_value_legacy_token(&val, "Len operand") {
                let len = len_digits(token);
                write_slot!(ctx, dst, RuntimeValue::I32(len));
            } else {
                return ERR_RUNTIME;
            }
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_left(ctx: *mut JitContext, dst: u32, src: u32, count: u32) -> i32 {
    let src_val = read_slot!(ctx, src);
    match &src_val {
        RuntimeValue::String(s) => {
            let count_val = read_slot!(ctx, count);
            let n = match semantics::runtime_value_to_usize(&count_val) {
                Ok(n) => n,
                Err(_) => return ERR_RUNTIME,
            };
            let result = if n >= s.0.len() {
                s.0.clone()
            } else {
                s.0[..n].to_string()
            };
            write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&src_val, "Left src") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let c_val = read_slot!(ctx, count);
            let c = match semantics::runtime_value_legacy_token(&c_val, "Left count") {
                Ok(c) => c,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(left_digits(v, c)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_right(ctx: *mut JitContext, dst: u32, src: u32, count: u32) -> i32 {
    let src_val = read_slot!(ctx, src);
    match &src_val {
        RuntimeValue::String(s) => {
            let count_val = read_slot!(ctx, count);
            let n = match semantics::runtime_value_to_usize(&count_val) {
                Ok(n) => n,
                Err(_) => return ERR_RUNTIME,
            };
            let len = s.0.len();
            let result = if n >= len {
                s.0.clone()
            } else {
                s.0[len - n..].to_string()
            };
            write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&src_val, "Right src") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let c_val = read_slot!(ctx, count);
            let c = match semantics::runtime_value_legacy_token(&c_val, "Right count") {
                Ok(c) => c,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(right_digits(v, c)));
        }
    }
    OK
}

/// Mid: dst = Mid$(src, start [, count])
/// count_slot == u32::MAX means no count.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_mid(
    ctx: *mut JitContext,
    dst: u32,
    src: u32,
    start: u32,
    count_slot: u32,
) -> i32 {
    let src_val = read_slot!(ctx, src);
    match &src_val {
        RuntimeValue::String(s) => {
            let start_val = read_slot!(ctx, start);
            let st = match semantics::runtime_value_to_usize(&start_val) {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let cnt = if count_slot == u32::MAX {
                None
            } else {
                let cv = read_slot!(ctx, count_slot);
                match semantics::runtime_value_to_usize(&cv) {
                    Ok(v) => Some(v),
                    Err(_) => return ERR_RUNTIME,
                }
            };
            let len = s.0.len();
            let begin = if st == 0 { 0 } else { (st - 1).min(len) };
            let end = match cnt {
                Some(c) => (begin + c).min(len),
                None => len,
            };
            let result = s.0[begin..end].to_string();
            write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&src_val, "Mid src") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let st_val = read_slot!(ctx, start);
            let st = match semantics::runtime_value_legacy_token(&st_val, "Mid start") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let cnt = if count_slot == u32::MAX {
                None
            } else {
                let cv = read_slot!(ctx, count_slot);
                match semantics::runtime_value_legacy_token(&cv, "Mid count") {
                    Ok(v) => Some(v),
                    Err(_) => return ERR_RUNTIME,
                }
            };
            write_slot!(ctx, dst, RuntimeValue::I32(mid_digits(v, st, cnt)));
        }
    }
    OK
}

/// Mid statement: Mid$(target, start [, count]) = value
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_mid_stmt(
    ctx: *mut JitContext,
    target: u32,
    start: u32,
    count_slot: u32,
    value: u32,
) -> i32 {
    let target_val = read_slot!(ctx, target);
    let tv = match semantics::runtime_value_legacy_token(&target_val, "MidStmt target") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let st_val = read_slot!(ctx, start);
    let st = match semantics::runtime_value_legacy_token(&st_val, "MidStmt start") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let cnt = if count_slot == u32::MAX {
        None
    } else {
        let cv = read_slot!(ctx, count_slot);
        match semantics::runtime_value_legacy_token(&cv, "MidStmt count") {
            Ok(v) => Some(v),
            Err(_) => return ERR_RUNTIME,
        }
    };
    let val = read_slot!(ctx, value);
    let vv = match semantics::runtime_value_legacy_token(&val, "MidStmt value") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        target,
        RuntimeValue::I32(mid_stmt_digits(tv, st, cnt, vv))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_instr(
    ctx: *mut JitContext,
    dst: u32,
    haystack: u32,
    needle: u32,
    mode: u32,
) -> i32 {
    let hay_val = read_slot!(ctx, haystack);
    let nee_val = read_slot!(ctx, needle);
    let scm = if mode == 1 {
        StringCompareMode::Text
    } else {
        StringCompareMode::Binary
    };
    match (&hay_val, &nee_val) {
        (RuntimeValue::String(h), RuntimeValue::String(n)) => {
            let h = semantics::normalize_for_compare(h.0.clone(), scm);
            let n = semantics::normalize_for_compare(n.0.clone(), scm);
            let pos = h.find(&n).map_or(0, |idx| (idx + 1) as i32);
            write_slot!(ctx, dst, RuntimeValue::I32(pos));
        }
        _ => {
            let h = match semantics::runtime_value_legacy_token(&hay_val, "InStr haystack") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let n = match semantics::runtime_value_legacy_token(&nee_val, "InStr needle") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(instr_digits(h, n, scm)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_instrrev(
    ctx: *mut JitContext,
    dst: u32,
    haystack: u32,
    needle: u32,
    mode: u32,
) -> i32 {
    let hay_val = read_slot!(ctx, haystack);
    let nee_val = read_slot!(ctx, needle);
    let scm = if mode == 1 {
        StringCompareMode::Text
    } else {
        StringCompareMode::Binary
    };
    match (&hay_val, &nee_val) {
        (RuntimeValue::String(h), RuntimeValue::String(n)) => {
            let h = semantics::normalize_for_compare(h.0.clone(), scm);
            let n = semantics::normalize_for_compare(n.0.clone(), scm);
            let pos = h.rfind(&n).map_or(0, |idx| (idx + 1) as i32);
            write_slot!(ctx, dst, RuntimeValue::I32(pos));
        }
        _ => {
            let h = match semantics::runtime_value_legacy_token(&hay_val, "InStrRev haystack") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let n = match semantics::runtime_value_legacy_token(&nee_val, "InStrRev needle") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(instrrev_digits(h, n, scm)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_lower(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match &val {
        RuntimeValue::String(s) => {
            write_slot!(
                ctx,
                dst,
                RuntimeValue::String(BStr(s.0.to_ascii_lowercase()))
            );
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&val, "LCase operand") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(to_lower_digits(v)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_upper(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match &val {
        RuntimeValue::String(s) => {
            write_slot!(
                ctx,
                dst,
                RuntimeValue::String(BStr(s.0.to_ascii_uppercase()))
            );
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&val, "UCase operand") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(to_upper_digits(v)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_split(ctx: *mut JitContext, dst: u32, src: u32, delimiter: u32) -> i32 {
    let v_val = read_slot!(ctx, src);
    let d_val = read_slot!(ctx, delimiter);
    let v = match semantics::runtime_value_legacy_token(&v_val, "Split src") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let d = match semantics::runtime_value_legacy_token(&d_val, "Split delimiter") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(split_count_digits(v, d)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_join(ctx: *mut JitContext, dst: u32, src: u32, delimiter: u32) -> i32 {
    let v_val = read_slot!(ctx, src);
    let d_val = read_slot!(ctx, delimiter);
    let v = match semantics::runtime_value_legacy_token(&v_val, "Join src") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let d = match semantics::runtime_value_legacy_token(&d_val, "Join delimiter") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(join_digits(v, d)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_replace(
    ctx: *mut JitContext,
    dst: u32,
    src: u32,
    find: u32,
    replace: u32,
) -> i32 {
    let src_val = read_slot!(ctx, src);
    let find_val = read_slot!(ctx, find);
    let replace_val = read_slot!(ctx, replace);
    match (&src_val, &find_val, &replace_val) {
        (RuntimeValue::String(s), RuntimeValue::String(f), RuntimeValue::String(r)) => {
            let result = s.0.replace(&f.0[..], &r.0[..]);
            write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&src_val, "Replace src") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let f = match semantics::runtime_value_legacy_token(&find_val, "Replace find") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let r = match semantics::runtime_value_legacy_token(&replace_val, "Replace replace") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(replace_digits(v, f, r)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_trim(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match &val {
        RuntimeValue::String(s) => {
            write_slot!(ctx, dst, RuntimeValue::String(BStr(s.0.trim().to_string())));
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&val, "Trim operand") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(trim_digits(v)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_ltrim(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match &val {
        RuntimeValue::String(s) => {
            write_slot!(
                ctx,
                dst,
                RuntimeValue::String(BStr(s.0.trim_start().to_string()))
            );
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&val, "LTrim operand") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(ltrim_digits(v)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_rtrim(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match &val {
        RuntimeValue::String(s) => {
            write_slot!(
                ctx,
                dst,
                RuntimeValue::String(BStr(s.0.trim_end().to_string()))
            );
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&val, "RTrim operand") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(rtrim_digits(v)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_strcomp(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    rhs: u32,
    mode: u32,
) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let rhs_val = read_slot!(ctx, rhs);
    let scm = if mode == 1 {
        StringCompareMode::Text
    } else {
        StringCompareMode::Binary
    };
    match (&lhs_val, &rhs_val) {
        (RuntimeValue::String(l), RuntimeValue::String(r)) => {
            let l = semantics::normalize_for_compare(l.0.clone(), scm);
            let r = semantics::normalize_for_compare(r.0.clone(), scm);
            let result = match l.cmp(&r) {
                Ordering::Less => -1,
                Ordering::Equal => 0,
                Ordering::Greater => 1,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(result));
        }
        _ => {
            let l = match semantics::runtime_value_legacy_token(&lhs_val, "StrComp lhs") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let r = match semantics::runtime_value_legacy_token(&rhs_val, "StrComp rhs") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            write_slot!(ctx, dst, RuntimeValue::I32(strcomp_digits(l, r, scm)));
        }
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_like(
    ctx: *mut JitContext,
    dst: u32,
    lhs: u32,
    pattern: u32,
    mode: u32,
) -> i32 {
    let lhs_val = read_slot!(ctx, lhs);
    let pat_val = read_slot!(ctx, pattern);
    let scm = if mode == 1 {
        StringCompareMode::Text
    } else {
        StringCompareMode::Binary
    };
    let l = match semantics::runtime_value_legacy_token(&lhs_val, "Like lhs") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let p = match semantics::runtime_value_legacy_token(&pat_val, "Like pattern") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(like_digits(l, p, scm)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_strconv(ctx: *mut JitContext, dst: u32, src: u32, conversion: u32) -> i32 {
    let src_val = read_slot!(ctx, src);
    let s = match &src_val {
        RuntimeValue::String(s) => s.0.clone(),
        _ => match semantics::runtime_value_legacy_token(&src_val, "StrConv source") {
            Ok(v) => v.to_string(),
            Err(_) => return ERR_RUNTIME,
        },
    };
    let conv_val = read_slot!(ctx, conversion);
    let conv = match semantics::runtime_value_legacy_token(&conv_val, "StrConv conversion") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let result = match conv {
        1 => s.to_uppercase(),
        2 => s.to_lowercase(),
        3 => semantics::proper_case(&s),
        _ => s,
    };
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

// ── Char/format ops ──────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_chr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Chr operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let ch = char::from_u32(v as u32).unwrap_or('\0');
    write_slot!(ctx, dst, RuntimeValue::String(BStr(ch.to_string())));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_asc(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let code = match &val {
        RuntimeValue::String(s) => {
            if s.0.is_empty() {
                0
            } else {
                s.0.as_bytes()[0] as i32
            }
        }
        _ => {
            let v = match semantics::runtime_value_legacy_token(&val, "Asc operand") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            let digits = format!("{v}");
            if digits.is_empty() {
                0
            } else {
                digits.as_bytes()[0] as i32
            }
        }
    };
    write_slot!(ctx, dst, RuntimeValue::I32(code));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_space(ctx: *mut JitContext, dst: u32, count: u32) -> i32 {
    let val = read_slot!(ctx, count);
    let n = match semantics::runtime_value_legacy_token(&val, "Space count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let result = " ".repeat(n.max(0) as usize);
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_string_repeat(ctx: *mut JitContext, dst: u32, count: u32, ch: u32) -> i32 {
    let n_val = read_slot!(ctx, count);
    let n = match semantics::runtime_value_legacy_token(&n_val, "String$ count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let ch_val = read_slot!(ctx, ch);
    let c = match &ch_val {
        RuntimeValue::String(s) => {
            if s.0.is_empty() {
                '\0'
            } else {
                s.0.chars().next().unwrap_or('\0')
            }
        }
        _ => {
            let code = match semantics::runtime_value_legacy_token(&ch_val, "String$ char") {
                Ok(v) => v,
                Err(_) => return ERR_RUNTIME,
            };
            char::from_u32(code as u32).unwrap_or('\0')
        }
    };
    let result = c.to_string().repeat(n.max(0) as usize);
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_hex(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Hex operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let result = format!("{:X}", v as u32);
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_oct(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Oct operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let result = format!("{:o}", v as u32);
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

/// Format: dst = Format$(value [, format_string])
/// format_slot == u32::MAX means no format string.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_format(ctx: *mut JitContext, dst: u32, value: u32, format_slot: u32) -> i32 {
    let val = read_slot!(ctx, value);
    let n = match semantics::runtime_value_as_f64(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fmt_str = if format_slot == u32::MAX {
        None
    } else {
        let fmt_val = read_slot!(ctx, format_slot);
        match &fmt_val {
            RuntimeValue::String(s) => Some(s.0.clone()),
            _ => None,
        }
    };
    let result = semantics::format_number(n, fmt_str.as_deref());
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_strreverse(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let s = match &val {
        RuntimeValue::String(s) => s.0.clone(),
        _ => match semantics::runtime_value_legacy_token(&val, "StrReverse source") {
            Ok(v) => v.to_string(),
            Err(_) => return ERR_RUNTIME,
        },
    };
    let result: String = s.chars().rev().collect();
    write_slot!(ctx, dst, RuntimeValue::String(BStr(result)));
    OK
}

// ── Date/time ops ────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_date_serial(
    ctx: *mut JitContext,
    dst: u32,
    year: u32,
    month: u32,
    day: u32,
) -> i32 {
    let y = match semantics::runtime_value_legacy_token(&read_slot!(ctx, year), "DateSerial year") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let m = match semantics::runtime_value_legacy_token(&read_slot!(ctx, month), "DateSerial month")
    {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let d = match semantics::runtime_value_legacy_token(&read_slot!(ctx, day), "DateSerial day") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(date_serial_digits(y, m, d)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_time_serial(
    ctx: *mut JitContext,
    dst: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> i32 {
    let h = match semantics::runtime_value_legacy_token(&read_slot!(ctx, hour), "TimeSerial hour") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let m = match semantics::runtime_value_legacy_token(
        &read_slot!(ctx, minute),
        "TimeSerial minute",
    ) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let s = match semantics::runtime_value_legacy_token(
        &read_slot!(ctx, second),
        "TimeSerial second",
    ) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(time_serial_digits(h, m, s)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_date_value(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "DateValue src") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_time_value(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "TimeValue src") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_date_add(
    ctx: *mut JitContext,
    dst: u32,
    interval: u32,
    number: u32,
    date: u32,
) -> i32 {
    let i =
        match semantics::runtime_value_legacy_token(&read_slot!(ctx, interval), "DateAdd interval")
        {
            Ok(v) => v,
            Err(_) => return ERR_RUNTIME,
        };
    let n = match semantics::runtime_value_legacy_token(&read_slot!(ctx, number), "DateAdd number")
    {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let d = match semantics::runtime_value_legacy_token(&read_slot!(ctx, date), "DateAdd date") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(date_add_digits(i, n, d)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_date_diff(
    ctx: *mut JitContext,
    dst: u32,
    interval: u32,
    date1: u32,
    date2: u32,
) -> i32 {
    let i = match semantics::runtime_value_legacy_token(
        &read_slot!(ctx, interval),
        "DateDiff interval",
    ) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let d1 = match semantics::runtime_value_legacy_token(&read_slot!(ctx, date1), "DateDiff date1")
    {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let d2 = match semantics::runtime_value_legacy_token(&read_slot!(ctx, date2), "DateDiff date2")
    {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(date_diff_digits(i, d1, d2)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_year(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Year operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v / 10_000));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_month(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Month operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32((v / 100) % 100));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_day(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Day operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v % 100));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_weekday(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let packed = match semantics::runtime_value_legacy_token(&val, "Weekday operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let year = packed / 10_000;
    let month = (packed / 100) % 100;
    let day = packed % 100;
    let dow = day_of_week(year, month, day);
    write_slot!(ctx, dst, RuntimeValue::I32(dow + 1));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_month_name(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let month = match semantics::runtime_value_legacy_token(&val, "MonthName operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let names = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let name = if month >= 1 && month <= 12 {
        names[(month - 1) as usize]
    } else {
        ""
    };
    write_slot!(ctx, dst, RuntimeValue::String(BStr(name.to_string())));
    OK
}

// ── Math ops ─────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_round(ctx: *mut JitContext, dst: u32, src: u32, digits_slot: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Round operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let digits = if digits_slot == u32::MAX {
        0
    } else {
        match semantics::runtime_value_legacy_token(&read_slot!(ctx, digits_slot), "Round digits") {
            Ok(v) => v,
            Err(_) => return ERR_RUNTIME,
        }
    };
    write_slot!(ctx, dst, RuntimeValue::I32(round_i32(v, digits)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sqr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Sqr operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::I32((v.saturating_abs() as f64).sqrt() as i32)
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sin(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Sin operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32((v as f64).sin().round() as i32));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cos(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Cos operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32((v as f64).cos().round() as i32));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_log(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Log operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let result = if v > 0 {
        (v as f64).ln().round() as i32
    } else {
        0
    };
    write_slot!(ctx, dst, RuntimeValue::I32(result));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_exp(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Exp operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32((v as f64).exp().round() as i32));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_atn(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Atn operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::I32((v as f64).atan().round() as i32)
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_tan(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "Tan operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32((v as f64).tan().round() as i32));
    OK
}

// ── Type checking ops ────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_vartype_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    // For typed values, use the typed VarType logic directly
    let code = match &val {
        RuntimeValue::Empty => 0,
        RuntimeValue::Null => 1,
        RuntimeValue::ErrorCode(_) => 10,
        RuntimeValue::ArrayIntent(_) => 8192 + 12,
        _ => {
            // Try legacy path for i32 tag values
            match semantics::runtime_value_legacy_token(&val, "VarType operand") {
                Ok(v) => {
                    if runtime_is_array_tag(v) {
                        8192 + 12
                    } else if v == EMPTY_TAG {
                        0
                    } else if v == NULL_TAG {
                        1
                    } else if runtime_is_error_tag(v) {
                        10
                    } else {
                        3
                    }
                }
                Err(_) => return ERR_RUNTIME,
            }
        }
    };
    write_slot!(ctx, dst, RuntimeValue::I32(code));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_vartype(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let code = match &val {
        RuntimeValue::Empty => 0,
        RuntimeValue::Null => 1,
        // Check for legacy error tags encoded as I32
        RuntimeValue::I32(v) if runtime_is_error_tag(*v) => 10,
        RuntimeValue::I32(v) if runtime_is_array_tag(*v) => 8192 + 12,
        RuntimeValue::I32(v) if *v >= -32768 && *v <= 32767 => 2,
        RuntimeValue::I32(_) => 3,
        RuntimeValue::I64(_) => 3,
        RuntimeValue::F64(v) => match v.subtype() {
            oxvba_runtime::F64Subtype::Single => 4,
            oxvba_runtime::F64Subtype::Double => 5,
            oxvba_runtime::F64Subtype::Date => 7,
        },
        RuntimeValue::Currency(_) => 6,
        RuntimeValue::Bool(_) => 11,
        RuntimeValue::String(_) => 8,
        RuntimeValue::ErrorCode(_) => 10,
        RuntimeValue::Decimal(_) => 14,
        RuntimeValue::ObjectHandle(_) => 9,
        RuntimeValue::BindingHandle(_) => 9,
        RuntimeValue::ArrayIntent(_) => 8192 + 12,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(code));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_typename_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "TypeName operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let out = if runtime_is_array_tag(v) { 1001 } else { 1002 };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_numeric_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    // Handle typed values directly
    let out = match &val {
        RuntimeValue::Empty
        | RuntimeValue::Null
        | RuntimeValue::ErrorCode(_)
        | RuntimeValue::ArrayIntent(_) => 0,
        _ => match semantics::runtime_value_legacy_token(&val, "IsNumeric operand") {
            Ok(v) => {
                if runtime_is_array_tag(v)
                    || v == EMPTY_TAG
                    || v == NULL_TAG
                    || runtime_is_error_tag(v)
                {
                    0
                } else {
                    1
                }
            }
            Err(_) => return ERR_RUNTIME,
        },
    };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_numeric(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let is_numeric = match &val {
        // Legacy error tags encoded as I32 are not numeric
        RuntimeValue::I32(v) if runtime_is_error_tag(*v) => false,
        RuntimeValue::I32(_)
        | RuntimeValue::I64(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::Currency(_)
        | RuntimeValue::Decimal(_)
        | RuntimeValue::Bool(_) => true,
        _ => false,
    };
    write_slot!(ctx, dst, RuntimeValue::Bool(is_numeric));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_error(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let is_error = match &val {
        RuntimeValue::ErrorCode(_) => true,
        // Also check legacy error tags encoded as I32
        RuntimeValue::I32(v) => runtime_is_error_tag(*v),
        _ => false,
    };
    write_slot!(ctx, dst, RuntimeValue::Bool(is_error));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_date_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "IsDate operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let out = if (1_000_000..=99_999_999).contains(&v) {
        1
    } else {
        0
    };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_object_tag(ctx: *mut JitContext, dst: u32, _src: u32) -> i32 {
    write_slot!(ctx, dst, RuntimeValue::I32(0));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_null(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::Bool(matches!(val, RuntimeValue::Null))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_empty(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::Bool(matches!(val, RuntimeValue::Empty))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_array_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "IsArray operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::I32(if runtime_is_array_tag(v) { 1 } else { 0 })
    );
    OK
}

// ── Financial ops ────────────────────────────────────────────────────

const FIN_MAX_ITERS: usize = 60;
const FIN_EPS: f64 = 1e-10;
const FIN_DERIVATIVE_STEP: f64 = 1e-7;
const FIN_RATE_ERROR_CODE: i32 = 2001;
const FIN_NPER_ERROR_CODE: i32 = 2002;

/// fv_slot: dst = FV(rate, nper, pmt [, pv] [, due])
/// pv_slot and due_slot use u32::MAX for None.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_fv(
    ctx: *mut JitContext,
    dst: u32,
    rate: u32,
    nper: u32,
    pmt: u32,
    pv_slot: u32,
    due_slot: u32,
) -> i32 {
    let r = legacy_slot(ctx, rate, "FV rate");
    let n = legacy_slot(ctx, nper, "FV nper");
    let p = legacy_slot(ctx, pmt, "FV pmt");
    let (r, n, p) = match (r, n, p) {
        (Ok(r), Ok(n), Ok(p)) => (r, n, p),
        _ => return ERR_RUNTIME,
    };
    let pv = opt_legacy_slot(ctx, pv_slot, 0);
    let due = opt_legacy_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_legacy_i32(fv_i32(r, n, p, pv, due))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_pv(
    ctx: *mut JitContext,
    dst: u32,
    rate: u32,
    nper: u32,
    pmt: u32,
    fv_slot: u32,
    due_slot: u32,
) -> i32 {
    let r = legacy_slot(ctx, rate, "PV rate");
    let n = legacy_slot(ctx, nper, "PV nper");
    let p = legacy_slot(ctx, pmt, "PV pmt");
    let (r, n, p) = match (r, n, p) {
        (Ok(r), Ok(n), Ok(p)) => (r, n, p),
        _ => return ERR_RUNTIME,
    };
    let fv = opt_legacy_slot(ctx, fv_slot, 0);
    let due = opt_legacy_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_legacy_i32(pv_i32(r, n, p, fv, due))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_pmt(
    ctx: *mut JitContext,
    dst: u32,
    rate: u32,
    nper: u32,
    pv: u32,
    fv_slot: u32,
    due_slot: u32,
) -> i32 {
    let r = legacy_slot(ctx, rate, "PMT rate");
    let n = legacy_slot(ctx, nper, "PMT nper");
    let p = legacy_slot(ctx, pv, "PMT pv");
    let (r, n, p) = match (r, n, p) {
        (Ok(r), Ok(n), Ok(p)) => (r, n, p),
        _ => return ERR_RUNTIME,
    };
    let fv = opt_legacy_slot(ctx, fv_slot, 0);
    let due = opt_legacy_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_legacy_i32(pmt_i32(r, n, p, fv, due))
    );
    OK
}

/// NPV helper: reads rate from rate_slot and values from slots_ptr/slots_len.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_npv(
    ctx: *mut JitContext,
    dst: u32,
    rate: u32,
    slots_ptr: *const u32,
    slots_len: u32,
) -> i32 {
    let r = match legacy_slot(ctx, rate, "NPV rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let slot_indices = unsafe { std::slice::from_raw_parts(slots_ptr, slots_len as usize) };
    let mut cash_flows = Vec::with_capacity(slots_len as usize);
    for &slot in slot_indices {
        match legacy_slot(ctx, slot, "NPV value") {
            Ok(v) => cash_flows.push(v),
            Err(_) => return ERR_RUNTIME,
        }
    }
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_legacy_i32(npv_i32(r, &cash_flows))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_irr(ctx: *mut JitContext, dst: u32, value: u32, guess_slot: u32) -> i32 {
    let v = match legacy_slot(ctx, value, "IRR value") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let guess = opt_legacy_slot(ctx, guess_slot, 10);
    write_slot!(ctx, dst, RuntimeValue::from_legacy_i32(irr_i32(v, guess)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_mirr(
    ctx: *mut JitContext,
    dst: u32,
    value: u32,
    finance_rate: u32,
    reinvest_rate: u32,
) -> i32 {
    let v = match legacy_slot(ctx, value, "MIRR value") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fr = match legacy_slot(ctx, finance_rate, "MIRR finance_rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let rr = match legacy_slot(ctx, reinvest_rate, "MIRR reinvest_rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::from_legacy_i32(mirr_i32(v, fr, rr)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_rate(
    ctx: *mut JitContext,
    dst: u32,
    nper: u32,
    pmt: u32,
    pv: u32,
    fv_slot: u32,
    due_slot: u32,
    guess_slot: u32,
) -> i32 {
    let n = match legacy_slot(ctx, nper, "Rate nper") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let p = match legacy_slot(ctx, pmt, "Rate pmt") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let pv = match legacy_slot(ctx, pv, "Rate pv") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fv = opt_legacy_slot(ctx, fv_slot, 0);
    let due = opt_legacy_slot(ctx, due_slot, 0);
    let guess = opt_legacy_slot(ctx, guess_slot, 10);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_legacy_i32(rate_i32(n, p, pv, fv, due, guess))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_nper(
    ctx: *mut JitContext,
    dst: u32,
    rate: u32,
    pmt: u32,
    pv: u32,
    fv_slot: u32,
    due_slot: u32,
) -> i32 {
    let r = match legacy_slot(ctx, rate, "NPer rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let p = match legacy_slot(ctx, pmt, "NPer pmt") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let pv = match legacy_slot(ctx, pv, "NPer pv") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fv = opt_legacy_slot(ctx, fv_slot, 0);
    let due = opt_legacy_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_legacy_i32(nper_i32(r, p, pv, fv, due))
    );
    OK
}

// ── Array ops ────────────────────────────────────────────────────────

/// ArrayLiteral: reads values from slot_indices, creates ArrayIntent.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_array_literal(
    ctx: *mut JitContext,
    dst: u32,
    slots_ptr: *const u32,
    slots_len: u32,
) -> i32 {
    let slot_indices = unsafe { std::slice::from_raw_parts(slots_ptr, slots_len as usize) };
    let mut elements = Vec::with_capacity(slots_len as usize);
    for &slot in slot_indices {
        elements.push(read_slot!(ctx, slot));
    }
    write_slot!(
        ctx,
        dst,
        RuntimeValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_values(elements),)
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_lbound(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "LBound operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let out = if runtime_is_array_tag(v) { 0 } else { -1 };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_ubound(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_legacy_token(&val, "UBound operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let out = if runtime_is_array_tag(v) {
        array_len_from_tag(v)
            .and_then(|count| i32::try_from(count).ok())
            .unwrap_or(0)
            - 1
    } else {
        -1
    };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

// ── Collection ops ───────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_collection_add(
    ctx: *mut JitContext,
    dst: u32,
    count: u32,
    item: u32,
) -> i32 {
    let c = match legacy_slot(ctx, count, "Collection.Add count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let _item = legacy_slot(ctx, item, "Collection.Add item");
    write_slot!(ctx, dst, RuntimeValue::I32((c + 1).max(0)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_collection_item(
    ctx: *mut JitContext,
    dst: u32,
    count: u32,
    index: u32,
) -> i32 {
    let c = match legacy_slot(ctx, count, "Collection.Item count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let i = match legacy_slot(ctx, index, "Collection.Item index") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let out = if i >= 1 && i <= c { i } else { 0 };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_collection_remove(
    ctx: *mut JitContext,
    dst: u32,
    count: u32,
    index: u32,
) -> i32 {
    let c = match legacy_slot(ctx, count, "Collection.Remove count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let _i = legacy_slot(ctx, index, "Collection.Remove index");
    write_slot!(ctx, dst, RuntimeValue::I32((c - 1).max(0)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_collection_count(ctx: *mut JitContext, dst: u32, count: u32) -> i32 {
    let c = match legacy_slot(ctx, count, "Collection.Count count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(c.max(0)));
    OK
}

// ── Random ops ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_rnd(ctx: *mut JitContext, dst: u32, seed_slot: u32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if seed_slot != u32::MAX {
        let seed_val = read_slot!(ctx, seed_slot);
        let seed = match semantics::runtime_value_legacy_token(&seed_val, "Rnd seed") {
            Ok(v) => v,
            Err(_) => return ERR_RUNTIME,
        };
        if seed < 0 {
            ctx_ref.rnd_state = (seed as u32) & 0x00FF_FFFF;
        } else if seed == 0 {
            let result = ctx_ref.rnd_state as f64 / 16_777_216.0;
            write_slot!(ctx, dst, RuntimeValue::F64(F64Value::from_f64(result)));
            return OK;
        }
    }
    ctx_ref.rnd_state = ctx_ref
        .rnd_state
        .wrapping_mul(0x43FD_43FD)
        .wrapping_add(0x0026_9EC3)
        & 0x00FF_FFFF;
    let result = ctx_ref.rnd_state as f64 / 16_777_216.0;
    write_slot!(ctx, dst, RuntimeValue::F64(F64Value::from_f64(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_randomize(ctx: *mut JitContext, dst: u32, seed_slot: u32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if seed_slot != u32::MAX {
        let seed_val = read_slot!(ctx, seed_slot);
        let seed = match semantics::runtime_value_legacy_token(&seed_val, "Randomize seed") {
            Ok(v) => v,
            Err(_) => return ERR_RUNTIME,
        };
        ctx_ref.rnd_state = (seed as u32) & 0x00FF_FFFF;
    } else {
        ctx_ref.rnd_state = 0x50000;
    }
    write_slot!(ctx, dst, RuntimeValue::I32(0));
    OK
}

// ── Assignment validation ────────────────────────────────────────────

/// ValidateRuntimeAssignment: checks Set/Let compatibility.
/// intent and target_kind are encoded as u32:
///   intent: 0=Implicit, 1=Let, 2=Set
///   target_kind: 0=Variant, 1=Object, 2=Scalar
/// target_name and target_type_name passed as ptr+len pairs.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_validate_assignment(
    ctx: *mut JitContext,
    src: u32,
    intent: u32,
    target_kind: u32,
    name_ptr: *const u8,
    name_len: u32,
    type_ptr: *const u8,
    type_len: u32,
) -> i32 {
    let val = read_slot!(ctx, src);
    let intent = match intent {
        0 => RuntimeAssignmentIntent::Implicit,
        1 => RuntimeAssignmentIntent::Let,
        2 => RuntimeAssignmentIntent::Set,
        _ => return ERR_RUNTIME,
    };
    let target_kind = match target_kind {
        0 => RuntimeAssignmentTargetKind::Variant,
        1 => RuntimeAssignmentTargetKind::Object,
        2 => RuntimeAssignmentTargetKind::Scalar,
        _ => return ERR_RUNTIME,
    };
    let name_bytes = unsafe { std::slice::from_raw_parts(name_ptr, name_len as usize) };
    let name = String::from_utf8_lossy(name_bytes);
    let type_bytes = unsafe { std::slice::from_raw_parts(type_ptr, type_len as usize) };
    let type_name = String::from_utf8_lossy(type_bytes);
    match semantics::validate_runtime_assignment(&val, intent, target_kind, &name, &type_name) {
        Ok(()) => OK,
        Err(_) => ERR_RUNTIME,
    }
}

// ── Phase 3: Error Handling helpers ──────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_set_on_error_resume_next(ctx: *mut JitContext) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.on_error_resume_next = 1;
    ctx_ref.on_error_goto_target = -1;
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_set_on_error_goto0(ctx: *mut JitContext) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.on_error_resume_next = 0;
    ctx_ref.on_error_goto_target = -1;
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_set_on_error_goto_label(ctx: *mut JitContext, target_pc: u32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.on_error_resume_next = 0;
    ctx_ref.on_error_goto_target = target_pc as i32;
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_err_number(ctx: *mut JitContext, slot: u32) -> i32 {
    let ctx_ref = unsafe { &*ctx };
    let err_num = ctx_ref.last_error;
    write_slot!(ctx, slot, RuntimeValue::I32(err_num));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_err_description(ctx: *mut JitContext, slot: u32) -> i32 {
    // JitContext doesn't store error description strings (simplified model).
    // Return empty string, matching the minimal JIT error model.
    write_slot!(ctx, slot, RuntimeValue::String(BStr(String::new())));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_err_source(ctx: *mut JitContext, slot: u32) -> i32 {
    write_slot!(ctx, slot, RuntimeValue::String(BStr(String::new())));
    OK
}

/// RaiseError: set error state and route through error handling.
/// Returns: 0 = OERN handled (advance to next instruction),
///          positive = GoTo target PC, negative = fatal.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_raise_error(ctx: *mut JitContext, code: i32, failing_pc: i32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.last_error = code;
    ctx_ref.last_error_pc = failing_pc;
    if ctx_ref.on_error_resume_next != 0 {
        ctx_ref.last_error_pc = -1; // consumed
        return OK; // 0 = OERN handled
    }
    if ctx_ref.on_error_goto_target >= 0 {
        return ctx_ref.on_error_goto_target; // positive = GoTo target PC
    }
    ERR_RUNTIME // -1 = fatal
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_clear_err(ctx: *mut JitContext) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.last_error = 0;
    ctx_ref.last_error_pc = -1;
    OK
}

/// Route an implicit error (from a helper that returned nonzero) through
/// the JIT error handling. Called from the error_dispatch_block.
/// Returns: positive = target PC to jump to, 0 = fatal.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_route_error(ctx: *mut JitContext, error_code: i32, failing_pc: i32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.last_error = error_code;
    ctx_ref.last_error_pc = failing_pc;
    if ctx_ref.on_error_resume_next != 0 {
        ctx_ref.last_error_pc = -1; // consumed
        return failing_pc + 1; // skip to next instruction
    }
    if ctx_ref.on_error_goto_target >= 0 {
        return ctx_ref.on_error_goto_target;
    }
    0 // fatal
}

/// Resume: return to the failing instruction (retry).
/// Returns: target PC >= 0, or -20 ("Resume without error").
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_resume(ctx: *mut JitContext) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.last_error_pc >= 0 {
        let target = ctx_ref.last_error_pc;
        ctx_ref.last_error = 0;
        ctx_ref.last_error_pc = -1;
        return target;
    }
    -20 // error 20: Resume without error
}

/// Resume Next: return to the instruction after the failing one.
/// Returns: target PC >= 0, or -20 ("Resume without error").
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_resume_next(ctx: *mut JitContext) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.last_error_pc >= 0 {
        let target = ctx_ref.last_error_pc + 1;
        ctx_ref.last_error = 0;
        ctx_ref.last_error_pc = -1;
        return target;
    }
    -20 // error 20: Resume without error
}

/// Resume <label>: jump to a specific label.
/// Returns: target_pc if error pending, or -20 ("Resume without error").
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_resume_label(ctx: *mut JitContext, target_pc: i32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.last_error_pc >= 0 {
        ctx_ref.last_error = 0;
        ctx_ref.last_error_pc = -1;
        return target_pc;
    }
    -20 // error 20: Resume without error
}

// ── Phase 4: Host Service Integration helpers ────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_free_file(ctx: *mut JitContext, dst: u32, range_slot: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let selector = if range_slot == u32::MAX {
        RuntimeValue::I32(0)
    } else {
        read_slot!(ctx, range_slot)
    };
    match host.fs().free_file(selector) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_open(
    ctx: *mut JitContext,
    dst: u32,
    path: u32,
    mode: u32,
    file_number: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let path_val = read_slot!(ctx, path);
    let mode_val = read_slot!(ctx, mode);
    let file_num = read_slot!(ctx, file_number);
    let mode_i32 = mode_val.to_legacy_i32().unwrap_or(0);
    let fnum_i32 = file_num.to_legacy_i32().unwrap_or(0);
    let combined_mode = RuntimeValue::I32(mode_i32 | (fnum_i32 << 16));
    match host.fs().open(path_val, combined_mode) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_close(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    match host.fs().close(handle_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_read(
    ctx: *mut JitContext,
    dst: u32,
    handle: u32,
    count: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    let count_val = read_slot!(ctx, count);
    match host.fs().read_bytes(handle_val, count_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_write(
    ctx: *mut JitContext,
    dst: u32,
    handle: u32,
    data: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    let data_val = read_slot!(ctx, data);
    match host.fs().write_bytes(handle_val, data_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_print(
    ctx: *mut JitContext,
    dst: u32,
    handle: u32,
    data: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    let data_val = read_slot!(ctx, data);
    match host.fs().print_line(handle_val, data_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_input(
    ctx: *mut JitContext,
    dst: u32,
    handle: u32,
    count: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    let count_val = read_slot!(ctx, count);
    match host.fs().input_fields(handle_val, count_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_line_input(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    match host.fs().line_input(handle_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_loc(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_slot!(ctx, handle);
    match host.fs().loc(handle_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// COM dispatch (simplified - pass through host services)
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_create_object(ctx: *mut JitContext, dst: u32, prog_id: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let prog_id_val = read_slot!(ctx, prog_id);
    match host.com().create_object(prog_id_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

/// Full dispatch invoke: reads object, member, and marshalled args.
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_dispatch_invoke(
    ctx: *mut JitContext,
    dst: u32,
    object: u32,
    member: u32,
    args_ptr: *const DispatchInvokeArg,
    args_len: u32,
) -> i32 {
    let object_val = read_slot!(ctx, object);
    // Error 91: Object variable or With block variable not set.
    if matches!(object_val, RuntimeValue::Empty) {
        return route_host_error_code(ctx, 91);
    }
    let object_handle =
        match semantics::runtime_value_to_com_object(&object_val, "dispatch_invoke.object") {
            Ok(h) => {
                if h.raw() == 0 {
                    return route_host_error_code(ctx, 91);
                }
                h
            }
            Err(_) => return route_host_error_code(ctx, 91),
        };
    let member_val = read_slot!(ctx, member);
    let member_sel = match semantics::runtime_value_to_dynamic_member_selector(
        &member_val,
        "dispatch_invoke.member",
    ) {
        Ok(m) => m,
        Err(_) => return route_host_error_code(ctx, 53053), // COM adapter fault
    };

    let args_slice = if args_ptr.is_null() || args_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, args_len as usize) }
    };

    let mut request = DynamicCallRequest {
        object: object_handle.into(),
        member: member_sel,
        args: Vec::with_capacity(args_slice.len()),
        call_kind_hint: None,
    };
    for arg in args_slice {
        let value = arg
            .slot
            .map(|slot| read_slot!(ctx, slot as u32))
            .as_ref()
            .map(ComValue::from_runtime_value);
        request.args.push(DynamicCallArg {
            value,
            name: arg.name.clone(),
        });
    }

    let host = unsafe { (*ctx).host_services() };
    let bridge = HalComDynamicBridge::new(host.profile(), host.com());
    match bridge.invoke_dynamic(&request) {
        Ok(value) => {
            // Normalize: COM dispatch may return error tags as I32 instead of ErrorCode.
            let rv = value.to_runtime_value();
            let rv = normalize_com_result(rv);
            write_slot!(ctx, dst, rv);
            OK
        }
        Err(err) => route_hal_error(ctx, err),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_com_subscribe(
    ctx: *mut JitContext,
    dst: u32,
    object: u32,
    event: u32,
) -> i32 {
    let object_val = read_slot!(ctx, object);
    let event_val = read_slot!(ctx, event);
    let object =
        match semantics::runtime_value_to_com_object(&object_val, "com_subscribe_event.object") {
            Ok(o) => o,
            Err(_) => return route_host_error(ctx),
        };
    let event =
        match semantics::runtime_value_to_com_member_token(&event_val, "com_subscribe_event.event")
        {
            Ok(e) => e,
            Err(_) => return route_host_error(ctx),
        };
    let host = unsafe { (*ctx).host_services() };
    match host.com().subscribe_event(object, event) {
        Ok(value) => {
            write_slot!(ctx, dst, RuntimeValue::I32(value.raw()));
            OK
        }
        Err(err) => route_hal_error(ctx, err),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_com_unsubscribe(
    ctx: *mut JitContext,
    dst: u32,
    subscription: u32,
) -> i32 {
    let sub_val = read_slot!(ctx, subscription);
    let sub = match semantics::runtime_value_to_com_subscription_token(
        &sub_val,
        "com_unsubscribe_event.subscription",
    ) {
        Ok(s) => s,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().unsubscribe_event(sub) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(err) => route_hal_error(ctx, err),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_com_event_callback_sub(
    ctx: *mut JitContext,
    dst: u32,
    callback: u32,
) -> i32 {
    let cb_val = read_slot!(ctx, callback);
    let cb = match semantics::runtime_value_to_com_callback_token(
        &cb_val,
        "com_event_callback_subscription.callback",
    ) {
        Ok(c) => c,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().event_callback_subscription(cb) {
        Ok(value) => {
            write_slot!(ctx, dst, RuntimeValue::I32(value.raw()));
            OK
        }
        Err(err) => route_hal_error(ctx, err),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_com_event_callback_arg(
    ctx: *mut JitContext,
    dst: u32,
    callback: u32,
    index: u32,
) -> i32 {
    let cb_val = read_slot!(ctx, callback);
    let idx_val = read_slot!(ctx, index);
    let cb = match semantics::runtime_value_to_com_callback_token(
        &cb_val,
        "com_event_callback_arg.callback",
    ) {
        Ok(c) => c,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let idx =
        match semantics::runtime_value_to_usize_index(&idx_val, "com_event_callback_arg.index") {
            Ok(i) => i,
            Err(_) => return route_host_error_code(ctx, 53053),
        };
    let host = unsafe { (*ctx).host_services() };
    match host.com().event_callback_arg(cb, idx) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(err) => route_hal_error(ctx, err),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_com_release_event_callback(
    ctx: *mut JitContext,
    dst: u32,
    callback: u32,
) -> i32 {
    let cb_val = read_slot!(ctx, callback);
    let cb = match semantics::runtime_value_to_com_callback_token(
        &cb_val,
        "com_release_event_callback.callback",
    ) {
        Ok(c) => c,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().release_event_callback(cb) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(err) => route_hal_error(ctx, err),
    }
}

// WithEvents
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_get(
    ctx: *mut JitContext,
    dst: u32,
    owner: u32,
    binding: u32,
) -> i32 {
    let owner_val = read_slot!(ctx, owner);
    let binding_val = read_slot!(ctx, binding);
    let owner = match semantics::withevents_owner_handle(&owner_val, "owner") {
        Ok(o) => o,
        Err(_) => return ERR_RUNTIME,
    };
    let binding = match semantics::withevents_binding_handle(&binding_val, "binding") {
        Ok(b) => b,
        Err(_) => return ERR_RUNTIME,
    };
    let key = semantics::withevents_binding_key(owner, binding);
    let state = unsafe { (*ctx).host_state() };
    let value = state
        .withevents_bindings
        .get(&key)
        .cloned()
        .unwrap_or(RuntimeValue::I32(0));
    write_slot!(ctx, dst, value);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_set(
    ctx: *mut JitContext,
    dst: u32,
    owner_slot: u32,
    binding_slot: u32,
    value: u32,
) -> i32 {
    let owner_val = read_slot!(ctx, owner_slot);
    let binding_val = read_slot!(ctx, binding_slot);
    let val = read_slot!(ctx, value);
    let owner = match semantics::withevents_owner_handle(&owner_val, "owner") {
        Ok(o) => o,
        Err(_) => return ERR_RUNTIME,
    };
    let binding = match semantics::withevents_binding_handle(&binding_val, "binding") {
        Ok(b) => b,
        Err(_) => return ERR_RUNTIME,
    };
    let key = semantics::withevents_binding_key(owner, binding);
    let state = unsafe { (*ctx).host_state_mut() };
    if val.to_legacy_i32().ok() == Some(0) {
        state.withevents_bindings.remove(&key);
    } else {
        state.withevents_bindings.insert(key, val.clone());
    }
    write_slot!(ctx, dst, val);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_clear_owner(
    ctx: *mut JitContext,
    dst: u32,
    owner: u32,
) -> i32 {
    let owner_val = read_slot!(ctx, owner);
    let owner = match semantics::withevents_owner_handle(&owner_val, "owner") {
        Ok(o) => o,
        Err(_) => return ERR_RUNTIME,
    };
    let state = unsafe { (*ctx).host_state_mut() };
    state
        .withevents_bindings
        .retain(|key, _| semantics::withevents_owner_from_key(*key) != owner);
    write_slot!(ctx, dst, RuntimeValue::I32(0));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_first_owner(
    ctx: *mut JitContext,
    dst: u32,
    source: u32,
    binding_slot: u32,
) -> i32 {
    let source_val = read_slot!(ctx, source);
    let binding_val = read_slot!(ctx, binding_slot);
    let binding = match semantics::withevents_binding_handle(&binding_val, "binding") {
        Ok(b) => b,
        Err(_) => return ERR_RUNTIME,
    };
    // If source is 0, no matching owners.
    if source_val.to_legacy_i32().ok() == Some(0) {
        write_slot!(ctx, dst, RuntimeValue::I32(0));
        return OK;
    }
    let state = unsafe { (*ctx).host_state_mut() };
    let mut owners: Vec<_> = state
        .withevents_bindings
        .iter()
        .filter_map(|(key, value)| {
            if value != &source_val || semantics::withevents_binding_from_key(*key) != binding {
                return None;
            }
            Some(semantics::withevents_owner_from_key(*key))
        })
        .collect();
    owners.sort_unstable();
    if owners.is_empty() {
        write_slot!(ctx, dst, RuntimeValue::I32(0));
    } else {
        let first = owners[0];
        state
            .withevents_owner_iters
            .push(crate::jit_context::WithEventsOwnerIterator {
                owners,
                next_index: 1,
            });
        write_slot!(ctx, dst, RuntimeValue::ObjectHandle(first));
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_next_owner(ctx: *mut JitContext, dst: u32) -> i32 {
    let state = unsafe { (*ctx).host_state_mut() };
    let next = if let Some(iter) = state.withevents_owner_iters.last_mut() {
        if iter.next_index < iter.owners.len() {
            let owner = iter.owners[iter.next_index];
            iter.next_index += 1;
            Some(owner)
        } else {
            None
        }
    } else {
        None
    };
    if next.is_none() {
        let _ = state.withevents_owner_iters.pop();
    }
    let result = next
        .map(RuntimeValue::ObjectHandle)
        .unwrap_or(RuntimeValue::I32(0));
    write_slot!(ctx, dst, result);
    OK
}

// UI
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_msgbox(
    ctx: *mut JitContext,
    dst: u32,
    prompt: u32,
    style_slot: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let prompt_val = read_slot!(ctx, prompt);
    let style = if style_slot == u32::MAX {
        RuntimeValue::I32(1)
    } else {
        read_slot!(ctx, style_slot)
    };
    match host.ui().msg_box(prompt_val, style) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_inputbox(
    ctx: *mut JitContext,
    dst: u32,
    prompt: u32,
    default_slot: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let prompt_val = read_slot!(ctx, prompt);
    let default_val = if default_slot == u32::MAX {
        RuntimeValue::I32(0)
    } else {
        read_slot!(ctx, default_slot)
    };
    match host.ui().input_box(prompt_val, default_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_do_events(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.events().do_events() {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// Process/Env
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_shell(ctx: *mut JitContext, dst: u32, command: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let cmd_val = read_slot!(ctx, command);
    match host.process().shell(cmd_val, RuntimeValue::I32(0)) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_environ(ctx: *mut JitContext, dst: u32, key: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let key_val = read_slot!(ctx, key);
    match host.process().environ(key_val) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_dir(ctx: *mut JitContext, dst: u32, path: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let path_val = read_slot!(ctx, path);
    match host.process().dir(path_val, RuntimeValue::I32(0)) {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// Time
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_date_now(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().date_serial_now() {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_time_now(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().time_serial_now() {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_now(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().date_serial_now() {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_timer(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().timer_ticks() {
        Ok(value) => {
            write_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// DynLink
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_invoke_symbol(
    ctx: *mut JitContext,
    dst: u32,
    symbol_raw: i32,
    descriptor_id: u32,
    args_ptr: *const usize,
    args_len: u32,
    writeback_ptr: *const [usize; 2],
    writeback_len: u32,
) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let symbol = oxvba_runtime::DynLinkSymbol::new(symbol_raw);

    let arg_slots = if args_ptr.is_null() || args_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, args_len as usize) }
    };
    let writeback_slots = if writeback_ptr.is_null() || writeback_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(writeback_ptr, writeback_len as usize) }
    };

    let arg_values: Vec<RuntimeValue> = arg_slots
        .iter()
        .map(|slot| read_slot!(ctx, *slot as u32))
        .collect();
    let first_arg = arg_values.first().cloned().unwrap_or(RuntimeValue::I32(0));

    // Fast path: no descriptors → simple invoke_symbol.
    let descriptors = unsafe { (*ctx).host_state().external_call_descriptors() };
    if descriptors.is_empty() {
        match host.dynlink().invoke_symbol(symbol, first_arg) {
            Ok(value) => {
                write_slot!(ctx, dst, value);
                return OK;
            }
            Err(err) => return route_hal_error(ctx, err),
        }
    }

    // Find descriptor by id.
    let descriptor = match descriptors
        .iter()
        .find(|e| e.descriptor_id == descriptor_id)
    {
        Some(d) => d,
        None => return route_host_error_code(ctx, 53073), // DynLink adapter fault
    };
    if descriptor.symbol != symbol {
        return route_host_error_code(ctx, 53073);
    }

    let param_type_strings: Vec<String> = descriptor
        .param_types
        .iter()
        .map(|pt| format!("{:?}", pt))
        .collect();
    let return_type_string;
    let view = oxvba_hal::traits::DynLinkDescriptorView {
        descriptor_id: descriptor.descriptor_id,
        declared_name: descriptor.declared_name.as_str(),
        library: descriptor.library.as_str(),
        alias: descriptor.alias.as_str(),
        ordinal_alias: descriptor.ordinal_alias,
        symbol: descriptor.symbol,
        marshal_lane: descriptor.marshal_lane.as_str(),
        calling_convention: descriptor.calling_convention.as_str(),
        selection_policy: descriptor.selection_policy.as_str(),
        param_count: descriptor.param_count,
        param_types: &param_type_strings,
        return_type: {
            return_type_string = descriptor
                .return_type
                .as_ref()
                .map(|rt| format!("{:?}", rt));
            return_type_string.as_deref()
        },
    };
    if let Some(_violation) = view.contract_violation() {
        return route_host_error_code(ctx, 53073);
    }

    if arg_values.len() > 1 || !writeback_slots.is_empty() {
        match host.dynlink().invoke_descriptor_multi(&view, &arg_values) {
            Ok((ret_value, wb_values)) => {
                write_slot!(ctx, dst, ret_value);
                for (wb_idx, pair) in writeback_slots.iter().enumerate() {
                    let arg_index = pair[0];
                    // pair[1] is source_slot, reserved for future use
                    if let Some(wb_val) = wb_values.get(wb_idx) {
                        if let Some(target_slot) = arg_slots.get(arg_index) {
                            write_slot!(ctx, *target_slot as u32, wb_val.clone());
                        }
                    }
                }
                OK
            }
            Err(err) => route_hal_error(ctx, err),
        }
    } else {
        match host.dynlink().invoke_descriptor(&view, first_arg) {
            Ok(value) => {
                write_slot!(ctx, dst, value);
                OK
            }
            Err(err) => route_hal_error(ctx, err),
        }
    }
}

// ── Private helper functions ─────────────────────────────────────────

/// Route a host service error through the JitContext error handling.
/// Returns OK if the error was handled (OERN or GoTo label), ERR_RUNTIME otherwise.
fn route_host_error(ctx: *mut JitContext) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.on_error_resume_next != 0 {
        return OK;
    }
    if ctx_ref.on_error_goto_target >= 0 {
        return OK;
    }
    ERR_RUNTIME
}

/// Route a specific VBA error code (e.g., error 91) through error handling.
/// Sets last_error and returns OK if handled, ERR_RUNTIME otherwise.
fn route_host_error_code(ctx: *mut JitContext, error_code: i32) -> i32 {
    let ctx_ref = unsafe { &mut *ctx };
    ctx_ref.last_error = error_code;
    if ctx_ref.on_error_resume_next != 0 {
        return OK;
    }
    if ctx_ref.on_error_goto_target >= 0 {
        return OK;
    }
    ERR_RUNTIME
}

/// Compute the HAL error code using the same formula as the VM interpreter.
fn hal_error_code(kind: HalErrorKind, capability: CapabilityId) -> i32 {
    let kind_code = match kind {
        HalErrorKind::CapabilityUnavailable => 1,
        HalErrorKind::PolicyDenied => 2,
        HalErrorKind::AdapterFault => 3,
        HalErrorKind::UnsupportedProfile => 4,
    };
    let capability_code = match capability {
        CapabilityId::UiInteraction => 1,
        CapabilityId::EventPump => 2,
        CapabilityId::FileSystemIo => 3,
        CapabilityId::ProcessEnv => 4,
        CapabilityId::ComActivationDispatch => 5,
        CapabilityId::TimeLocale => 6,
        CapabilityId::DynamicLinking => 7,
        CapabilityId::DiagnosticsTelemetry => 8,
    };
    53_000 + capability_code * 10 + kind_code
}

/// Route a HalError through the JitContext error handling.
fn route_hal_error(ctx: *mut JitContext, err: HalError) -> i32 {
    let code = hal_error_code(err.kind, err.capability);
    route_host_error_code(ctx, code)
}

/// Normalize a COM dispatch result: if it's an I32 that encodes a legacy error tag,
/// convert it to a proper ErrorCode. This matches the VM's implicit normalization
/// through from_legacy_i32.
fn normalize_com_result(value: RuntimeValue) -> RuntimeValue {
    if let RuntimeValue::I32(v) = &value {
        if runtime_is_error_tag(*v) {
            if let Some(code) = oxvba_runtime::value_tags::error_code_from_tag(*v) {
                return RuntimeValue::ErrorCode(code);
            }
        }
    }
    value
}

fn legacy_slot(ctx: *mut JitContext, slot: u32, field: &str) -> Result<i32, ()> {
    let val = read_slot!(ctx, slot);
    semantics::runtime_value_legacy_token(&val, field).map_err(|_| ())
}

fn opt_legacy_slot(ctx: *mut JitContext, slot: u32, default: i32) -> i32 {
    if slot == u32::MAX {
        default
    } else {
        legacy_slot(ctx, slot, "optional").unwrap_or(default)
    }
}

// Digit-based string helpers (legacy i32 path)
fn len_digits(value: i32) -> i32 {
    let mut n = i64::from(value);
    let mut digits = 0i32;
    if n <= 0 {
        digits += 1;
        n = -n;
    }
    while n > 0 {
        digits += 1;
        n /= 10;
    }
    digits
}

fn slice_digits(value: i32, start: usize, count: Option<i32>) -> i32 {
    let text = value.to_string();
    if start >= text.len() {
        return 0;
    }
    let end = match count {
        Some(c) if c <= 0 => start,
        Some(c) => (start + c as usize).min(text.len()),
        None => text.len(),
    };
    text[start..end].parse::<i32>().unwrap_or(0)
}

fn left_digits(value: i32, count: i32) -> i32 {
    slice_digits(value, 0, Some(count))
}

fn right_digits(value: i32, count: i32) -> i32 {
    if count <= 0 {
        return 0;
    }
    let text = value.to_string();
    let take = (count as usize).min(text.len());
    let start = text.len().saturating_sub(take);
    text[start..].parse::<i32>().unwrap_or(0)
}

fn mid_digits(value: i32, start: i32, count: Option<i32>) -> i32 {
    let zero_based_start = if start <= 1 { 0 } else { (start - 1) as usize };
    slice_digits(value, zero_based_start, count)
}

fn mid_stmt_digits(target: i32, start: i32, count: Option<i32>, value: i32) -> i32 {
    let base = target.to_string();
    let repl = value.to_string();
    let start_idx = if start <= 1 { 0 } else { (start - 1) as usize };
    if start_idx >= base.len() {
        return target;
    }
    let end_idx = match count {
        Some(c) if c <= 0 => start_idx,
        Some(c) => (start_idx + c as usize).min(base.len()),
        None => base.len(),
    };
    let replace_len = end_idx.saturating_sub(start_idx);
    let replace_text = if replace_len >= repl.len() {
        repl.as_str()
    } else {
        &repl[..replace_len]
    };
    let mut out = String::with_capacity(base.len() - replace_len + replace_text.len());
    out.push_str(&base[..start_idx]);
    out.push_str(replace_text);
    out.push_str(&base[end_idx..]);
    out.parse::<i32>().unwrap_or(0)
}

fn instr_digits(haystack: i32, needle: i32, mode: StringCompareMode) -> i32 {
    let hay = semantics::normalize_for_compare(haystack.to_string(), mode);
    let nee = semantics::normalize_for_compare(needle.to_string(), mode);
    hay.find(&nee).map_or(0, |idx| (idx + 1) as i32)
}

fn instrrev_digits(haystack: i32, needle: i32, mode: StringCompareMode) -> i32 {
    let hay = semantics::normalize_for_compare(haystack.to_string(), mode);
    let nee = semantics::normalize_for_compare(needle.to_string(), mode);
    hay.rfind(&nee).map_or(0, |idx| (idx + 1) as i32)
}

fn to_lower_digits(value: i32) -> i32 {
    value
        .to_string()
        .to_ascii_lowercase()
        .parse::<i32>()
        .unwrap_or(0)
}

fn to_upper_digits(value: i32) -> i32 {
    value
        .to_string()
        .to_ascii_uppercase()
        .parse::<i32>()
        .unwrap_or(0)
}

fn split_count_digits(value: i32, delimiter: i32) -> i32 {
    let text = value.to_string();
    let delim = delimiter.to_string();
    if delim.is_empty() {
        return 1;
    }
    text.split(&delim).count() as i32
}

fn join_digits(value: i32, _delimiter: i32) -> i32 {
    array_len_from_tag(value)
        .and_then(|c| i32::try_from(c).ok())
        .unwrap_or(value)
}

fn replace_digits(value: i32, find: i32, replace: i32) -> i32 {
    let text = value.to_string();
    let f = find.to_string();
    let r = replace.to_string();
    if f.is_empty() {
        return value;
    }
    text.replace(&f, &r).parse::<i32>().unwrap_or(0)
}

fn trim_digits(value: i32) -> i32 {
    value.to_string().trim().parse::<i32>().unwrap_or(value)
}

fn ltrim_digits(value: i32) -> i32 {
    value
        .to_string()
        .trim_start()
        .parse::<i32>()
        .unwrap_or(value)
}

fn rtrim_digits(value: i32) -> i32 {
    value.to_string().trim_end().parse::<i32>().unwrap_or(value)
}

fn strcomp_digits(lhs: i32, rhs: i32, mode: StringCompareMode) -> i32 {
    let l = semantics::normalize_for_compare(lhs.to_string(), mode);
    let r = semantics::normalize_for_compare(rhs.to_string(), mode);
    match l.cmp(&r) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

fn like_digits(lhs: i32, pattern: i32, mode: StringCompareMode) -> i32 {
    let l = semantics::normalize_for_compare(lhs.to_string(), mode);
    let p = semantics::normalize_for_compare(pattern.to_string(), mode);
    if l == p { -1 } else { 0 }
}

fn date_serial_digits(year: i32, month: i32, day: i32) -> i32 {
    year.saturating_mul(10_000)
        .saturating_add(month.saturating_mul(100))
        .saturating_add(day)
}

fn time_serial_digits(hour: i32, minute: i32, second: i32) -> i32 {
    hour.saturating_mul(3600)
        .saturating_add(minute.saturating_mul(60))
        .saturating_add(second)
}

fn date_add_digits(_interval: i32, number: i32, date: i32) -> i32 {
    date.saturating_add(number)
}

fn date_diff_digits(_interval: i32, date1: i32, date2: i32) -> i32 {
    date2.saturating_sub(date1)
}

fn day_of_week(year: i32, month: i32, day: i32) -> i32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m_idx = ((month - 1).max(0) as usize).min(11);
    (y + y / 4 - y / 100 + y / 400 + t[m_idx] + day) % 7
}

fn round_i32(value: i32, digits: i32) -> i32 {
    if digits >= 0 {
        return value;
    }
    let magnitude = (-digits) as u32;
    let factor = 10_i32.saturating_pow(magnitude);
    if factor <= 1 {
        return value;
    }
    let f = factor as f64;
    ((value as f64) / f).round() as i32 * factor
}

fn fv_i32(rate: i32, nper: i32, pmt: i32, pv: i32, due: i32) -> i32 {
    if nper == 0 {
        return 0;
    }
    if rate == 0 {
        return -(pv + pmt.saturating_mul(nper));
    }
    let r = rate as f64 / 100.0;
    let n = nper as f64;
    let growth = (1.0 + r).powf(n);
    let due_adj = if due != 0 { 1.0 + r } else { 1.0 };
    let out = -(pv as f64 * growth + pmt as f64 * due_adj * ((growth - 1.0) / r));
    out.round() as i32
}

fn pv_i32(rate: i32, nper: i32, pmt: i32, fv: i32, due: i32) -> i32 {
    if nper == 0 {
        return 0;
    }
    if rate == 0 {
        return -(fv + pmt.saturating_mul(nper));
    }
    let r = rate as f64 / 100.0;
    let n = nper as f64;
    let growth = (1.0 + r).powf(n);
    let due_adj = if due != 0 { 1.0 + r } else { 1.0 };
    let out = -(fv as f64 + pmt as f64 * due_adj * ((growth - 1.0) / r)) / growth;
    out.round() as i32
}

fn pmt_i32(rate: i32, nper: i32, pv: i32, fv: i32, due: i32) -> i32 {
    if nper == 0 {
        return 0;
    }
    if rate == 0 {
        return -((pv + fv) / nper);
    }
    let r = rate as f64 / 100.0;
    let n = nper as f64;
    let growth = (1.0 + r).powf(n);
    let due_adj = if due != 0 { 1.0 + r } else { 1.0 };
    let denom = due_adj * ((growth - 1.0) / r);
    if denom == 0.0 {
        return 0;
    }
    let out = -(pv as f64 * growth + fv as f64) / denom;
    out.round() as i32
}

fn npv_i32(rate: i32, values: &[i32]) -> i32 {
    if values.is_empty() {
        return 0;
    }
    let r = rate as f64 / 100.0;
    let mut total = 0.0f64;
    for (idx, value) in values.iter().enumerate() {
        let period = (idx + 1) as i32;
        let discount = (1.0 + r).powi(period);
        if discount == 0.0 {
            continue;
        }
        total += *value as f64 / discount;
    }
    total.round() as i32
}

fn irr_i32(value: i32, guess: i32) -> i32 {
    let mut r = guess as f64 / 100.0;
    let value = value as f64;
    for _ in 0..20 {
        let denom = 1.0 + r;
        if denom.abs() < 1e-9 {
            break;
        }
        let f = -100.0 + (value / denom);
        let fp = -value / (denom * denom);
        if fp.abs() < 1e-12 {
            break;
        }
        let next = (r - f / fp).clamp(-0.99, 10.0);
        if (next - r).abs() < 1e-10 {
            r = next;
            break;
        }
        r = next;
    }
    (r * 100.0).round() as i32
}

fn mirr_i32(value: i32, finance_rate: i32, reinvest_rate: i32) -> i32 {
    let value = value as f64;
    let fr = finance_rate as f64 / 100.0;
    let rr = reinvest_rate as f64 / 100.0;
    let pv_neg = 100.0 / (1.0 + fr).max(1e-9);
    let fv_pos = value * (1.0 + rr);
    let out = (fv_pos / pv_neg) - 1.0;
    (out * 100.0).round() as i32
}

fn rate_func(r: f64, nper: f64, pmt: f64, pv: f64, fv: f64, due: f64) -> f64 {
    if r.abs() < 1e-9 {
        pv + pmt * nper + fv
    } else {
        let growth = (1.0 + r).powf(nper);
        pv * growth + pmt * (1.0 + r * due) * ((growth - 1.0) / r) + fv
    }
}

fn rate_func_derivative(r: f64, nper: f64, pmt: f64, pv: f64, fv: f64, due: f64) -> f64 {
    if r.abs() < 1e-8 {
        let h = FIN_DERIVATIVE_STEP;
        return (rate_func(r + h, nper, pmt, pv, fv, due)
            - rate_func(r - h, nper, pmt, pv, fv, due))
            / (2.0 * h);
    }
    let base = 1.0 + r;
    if base <= 0.0 {
        return f64::NAN;
    }
    let growth = base.powf(nper);
    let growth_prime = nper * base.powf(nper - 1.0);
    let c = (growth - 1.0) / r;
    let c_prime = (growth_prime * r - (growth - 1.0)) / (r * r);
    pv * growth_prime + pmt * (due * c + (1.0 + r * due) * c_prime)
}

fn rate_i32(nper: i32, pmt: i32, pv: i32, fv: i32, due: i32, guess: i32) -> i32 {
    if nper == 0 {
        return error_tag_from_code(FIN_RATE_ERROR_CODE);
    }
    let n = nper as f64;
    let pmt = pmt as f64;
    let pv = pv as f64;
    let fv = fv as f64;
    let due = if due != 0 { 1.0 } else { 0.0 };
    let mut r = (guess as f64 / 100.0).clamp(-0.99, 10.0);
    for _ in 0..FIN_MAX_ITERS {
        let f = rate_func(r, n, pmt, pv, fv, due);
        let fp = rate_func_derivative(r, n, pmt, pv, fv, due);
        if fp.abs() < 1e-12 {
            return error_tag_from_code(FIN_RATE_ERROR_CODE);
        }
        let next = (r - f / fp).clamp(-0.99, 10.0);
        if !next.is_finite() {
            return error_tag_from_code(FIN_RATE_ERROR_CODE);
        }
        if (next - r).abs() < FIN_EPS {
            return (next * 100.0).round() as i32;
        }
        r = next;
    }
    error_tag_from_code(FIN_RATE_ERROR_CODE)
}

fn nper_i32(rate: i32, pmt: i32, pv: i32, fv: i32, due: i32) -> i32 {
    let pmt = pmt as f64;
    let pv = pv as f64;
    let fv = fv as f64;
    let due = if due != 0 { 1.0 } else { 0.0 };
    if rate == 0 {
        if pmt == 0.0 {
            return error_tag_from_code(FIN_NPER_ERROR_CODE);
        }
        return (-(pv + fv) / pmt).round() as i32;
    }
    let r = rate as f64 / 100.0;
    let numerator = pmt * (1.0 + r * due) - fv * r;
    let denominator = pv * r + pmt * (1.0 + r * due);
    if numerator <= 0.0 || denominator <= 0.0 || (1.0 + r) <= 0.0 {
        return error_tag_from_code(FIN_NPER_ERROR_CODE);
    }
    let n = (numerator / denominator).ln() / (1.0 + r).ln();
    if !n.is_finite() {
        return error_tag_from_code(FIN_NPER_ERROR_CODE);
    }
    n.round() as i32
}

// ── Symbol registration ───────────────────────────────────────────────

/// Register all runtime helper symbols with a JITBuilder.
pub fn register_symbols(builder: &mut cranelift_jit::JITBuilder) {
    let symbols: &[(&str, *const u8)] = &[
        // Phase 0-1: Arithmetic, comparison, boolean, load
        ("oxrt_add_slots", oxrt_add_slots as *const u8),
        ("oxrt_sub_slots", oxrt_sub_slots as *const u8),
        ("oxrt_mul_slots", oxrt_mul_slots as *const u8),
        ("oxrt_div_slots", oxrt_div_slots as *const u8),
        ("oxrt_intdiv_slots", oxrt_intdiv_slots as *const u8),
        ("oxrt_mod_slots", oxrt_mod_slots as *const u8),
        ("oxrt_pow_slots", oxrt_pow_slots as *const u8),
        ("oxrt_neg_slot", oxrt_neg_slot as *const u8),
        ("oxrt_concat_slots", oxrt_concat_slots as *const u8),
        ("oxrt_add_const", oxrt_add_const as *const u8),
        ("oxrt_sub_const", oxrt_sub_const as *const u8),
        ("oxrt_inc_slot", oxrt_inc_slot as *const u8),
        ("oxrt_cmp_eq", oxrt_cmp_eq as *const u8),
        ("oxrt_cmp_ne", oxrt_cmp_ne as *const u8),
        ("oxrt_cmp_lt", oxrt_cmp_lt as *const u8),
        ("oxrt_cmp_le", oxrt_cmp_le as *const u8),
        ("oxrt_cmp_gt", oxrt_cmp_gt as *const u8),
        ("oxrt_cmp_ge", oxrt_cmp_ge as *const u8),
        ("oxrt_bool_not", oxrt_bool_not as *const u8),
        ("oxrt_bool_and", oxrt_bool_and as *const u8),
        ("oxrt_bool_or", oxrt_bool_or as *const u8),
        ("oxrt_abs", oxrt_abs as *const u8),
        ("oxrt_sgn", oxrt_sgn as *const u8),
        ("oxrt_int_fix", oxrt_int_fix as *const u8),
        ("oxrt_copy_slot", oxrt_copy_slot as *const u8),
        ("oxrt_load_string", oxrt_load_string as *const u8),
        ("oxrt_load_f64", oxrt_load_f64 as *const u8),
        // Phase 2: String ops
        ("oxrt_len", oxrt_len as *const u8),
        ("oxrt_left", oxrt_left as *const u8),
        ("oxrt_right", oxrt_right as *const u8),
        ("oxrt_mid", oxrt_mid as *const u8),
        ("oxrt_mid_stmt", oxrt_mid_stmt as *const u8),
        ("oxrt_instr", oxrt_instr as *const u8),
        ("oxrt_instrrev", oxrt_instrrev as *const u8),
        ("oxrt_lower", oxrt_lower as *const u8),
        ("oxrt_upper", oxrt_upper as *const u8),
        ("oxrt_split", oxrt_split as *const u8),
        ("oxrt_join", oxrt_join as *const u8),
        ("oxrt_replace", oxrt_replace as *const u8),
        ("oxrt_trim", oxrt_trim as *const u8),
        ("oxrt_ltrim", oxrt_ltrim as *const u8),
        ("oxrt_rtrim", oxrt_rtrim as *const u8),
        ("oxrt_strcomp", oxrt_strcomp as *const u8),
        ("oxrt_like", oxrt_like as *const u8),
        ("oxrt_strconv", oxrt_strconv as *const u8),
        // Phase 2: Char/format
        ("oxrt_chr", oxrt_chr as *const u8),
        ("oxrt_asc", oxrt_asc as *const u8),
        ("oxrt_space", oxrt_space as *const u8),
        ("oxrt_string_repeat", oxrt_string_repeat as *const u8),
        ("oxrt_hex", oxrt_hex as *const u8),
        ("oxrt_oct", oxrt_oct as *const u8),
        ("oxrt_format", oxrt_format as *const u8),
        ("oxrt_strreverse", oxrt_strreverse as *const u8),
        // Phase 2: Date/time
        ("oxrt_date_serial", oxrt_date_serial as *const u8),
        ("oxrt_time_serial", oxrt_time_serial as *const u8),
        ("oxrt_date_value", oxrt_date_value as *const u8),
        ("oxrt_time_value", oxrt_time_value as *const u8),
        ("oxrt_date_add", oxrt_date_add as *const u8),
        ("oxrt_date_diff", oxrt_date_diff as *const u8),
        ("oxrt_year", oxrt_year as *const u8),
        ("oxrt_month", oxrt_month as *const u8),
        ("oxrt_day", oxrt_day as *const u8),
        ("oxrt_weekday", oxrt_weekday as *const u8),
        ("oxrt_month_name", oxrt_month_name as *const u8),
        // Phase 2: Math
        ("oxrt_round", oxrt_round as *const u8),
        ("oxrt_sqr", oxrt_sqr as *const u8),
        ("oxrt_sin", oxrt_sin as *const u8),
        ("oxrt_cos", oxrt_cos as *const u8),
        ("oxrt_log", oxrt_log as *const u8),
        ("oxrt_exp", oxrt_exp as *const u8),
        ("oxrt_atn", oxrt_atn as *const u8),
        ("oxrt_tan", oxrt_tan as *const u8),
        // Phase 2: Type checking
        ("oxrt_vartype_tag", oxrt_vartype_tag as *const u8),
        ("oxrt_vartype", oxrt_vartype as *const u8),
        ("oxrt_typename_tag", oxrt_typename_tag as *const u8),
        ("oxrt_is_numeric_tag", oxrt_is_numeric_tag as *const u8),
        ("oxrt_is_numeric", oxrt_is_numeric as *const u8),
        ("oxrt_is_error", oxrt_is_error as *const u8),
        ("oxrt_is_date_tag", oxrt_is_date_tag as *const u8),
        ("oxrt_is_object_tag", oxrt_is_object_tag as *const u8),
        ("oxrt_is_null", oxrt_is_null as *const u8),
        ("oxrt_is_empty", oxrt_is_empty as *const u8),
        ("oxrt_is_array_tag", oxrt_is_array_tag as *const u8),
        // Phase 2: Financial
        ("oxrt_fv", oxrt_fv as *const u8),
        ("oxrt_pv", oxrt_pv as *const u8),
        ("oxrt_pmt", oxrt_pmt as *const u8),
        ("oxrt_npv", oxrt_npv as *const u8),
        ("oxrt_irr", oxrt_irr as *const u8),
        ("oxrt_mirr", oxrt_mirr as *const u8),
        ("oxrt_rate", oxrt_rate as *const u8),
        ("oxrt_nper", oxrt_nper as *const u8),
        // Phase 2: Array
        ("oxrt_array_literal", oxrt_array_literal as *const u8),
        ("oxrt_lbound", oxrt_lbound as *const u8),
        ("oxrt_ubound", oxrt_ubound as *const u8),
        // Phase 2: Collection
        ("oxrt_collection_add", oxrt_collection_add as *const u8),
        ("oxrt_collection_item", oxrt_collection_item as *const u8),
        (
            "oxrt_collection_remove",
            oxrt_collection_remove as *const u8,
        ),
        ("oxrt_collection_count", oxrt_collection_count as *const u8),
        // Phase 2: Random
        ("oxrt_rnd", oxrt_rnd as *const u8),
        ("oxrt_randomize", oxrt_randomize as *const u8),
        // Phase 2: Assignment
        (
            "oxrt_validate_assignment",
            oxrt_validate_assignment as *const u8,
        ),
        // Phase 3: Error handling
        (
            "oxrt_set_on_error_resume_next",
            oxrt_set_on_error_resume_next as *const u8,
        ),
        (
            "oxrt_set_on_error_goto0",
            oxrt_set_on_error_goto0 as *const u8,
        ),
        (
            "oxrt_set_on_error_goto_label",
            oxrt_set_on_error_goto_label as *const u8,
        ),
        ("oxrt_load_err_number", oxrt_load_err_number as *const u8),
        (
            "oxrt_load_err_description",
            oxrt_load_err_description as *const u8,
        ),
        ("oxrt_load_err_source", oxrt_load_err_source as *const u8),
        ("oxrt_raise_error", oxrt_raise_error as *const u8),
        ("oxrt_clear_err", oxrt_clear_err as *const u8),
        ("oxrt_route_error", oxrt_route_error as *const u8),
        ("oxrt_resume", oxrt_resume as *const u8),
        ("oxrt_resume_next", oxrt_resume_next as *const u8),
        ("oxrt_resume_label", oxrt_resume_label as *const u8),
        // Phase 4: Host services
        ("oxrt_host_free_file", oxrt_host_free_file as *const u8),
        ("oxrt_host_file_open", oxrt_host_file_open as *const u8),
        ("oxrt_host_file_close", oxrt_host_file_close as *const u8),
        ("oxrt_host_file_read", oxrt_host_file_read as *const u8),
        ("oxrt_host_file_write", oxrt_host_file_write as *const u8),
        ("oxrt_host_file_print", oxrt_host_file_print as *const u8),
        ("oxrt_host_file_input", oxrt_host_file_input as *const u8),
        (
            "oxrt_host_file_line_input",
            oxrt_host_file_line_input as *const u8,
        ),
        ("oxrt_host_file_loc", oxrt_host_file_loc as *const u8),
        (
            "oxrt_host_create_object",
            oxrt_host_create_object as *const u8,
        ),
        (
            "oxrt_host_dispatch_invoke",
            oxrt_host_dispatch_invoke as *const u8,
        ),
        (
            "oxrt_host_com_subscribe",
            oxrt_host_com_subscribe as *const u8,
        ),
        (
            "oxrt_host_com_unsubscribe",
            oxrt_host_com_unsubscribe as *const u8,
        ),
        (
            "oxrt_host_com_event_callback_sub",
            oxrt_host_com_event_callback_sub as *const u8,
        ),
        (
            "oxrt_host_com_event_callback_arg",
            oxrt_host_com_event_callback_arg as *const u8,
        ),
        (
            "oxrt_host_com_release_event_callback",
            oxrt_host_com_release_event_callback as *const u8,
        ),
        (
            "oxrt_host_withevents_get",
            oxrt_host_withevents_get as *const u8,
        ),
        (
            "oxrt_host_withevents_set",
            oxrt_host_withevents_set as *const u8,
        ),
        (
            "oxrt_host_withevents_clear_owner",
            oxrt_host_withevents_clear_owner as *const u8,
        ),
        (
            "oxrt_host_withevents_first_owner",
            oxrt_host_withevents_first_owner as *const u8,
        ),
        (
            "oxrt_host_withevents_next_owner",
            oxrt_host_withevents_next_owner as *const u8,
        ),
        ("oxrt_host_msgbox", oxrt_host_msgbox as *const u8),
        ("oxrt_host_inputbox", oxrt_host_inputbox as *const u8),
        ("oxrt_host_do_events", oxrt_host_do_events as *const u8),
        ("oxrt_host_shell", oxrt_host_shell as *const u8),
        ("oxrt_host_environ", oxrt_host_environ as *const u8),
        ("oxrt_host_dir", oxrt_host_dir as *const u8),
        ("oxrt_host_date_now", oxrt_host_date_now as *const u8),
        ("oxrt_host_time_now", oxrt_host_time_now as *const u8),
        ("oxrt_host_now", oxrt_host_now as *const u8),
        ("oxrt_host_timer", oxrt_host_timer as *const u8),
        (
            "oxrt_host_invoke_symbol",
            oxrt_host_invoke_symbol as *const u8,
        ),
    ];
    for &(name, ptr) in symbols {
        builder.symbol(name, ptr);
    }
}
