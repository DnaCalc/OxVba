#![allow(clippy::not_unsafe_ptr_arg_deref)]

//! Runtime helper functions for JIT-compiled VBA code.
//!
//! Each helper is an `extern "C"` function registered with Cranelift's
//! `JITBuilder::symbol()`. They implement instruction semantics by calling
//! the same shared logic as the VM interpreter (via `oxvba_vm::semantics`).
//!
//! Return convention: 0 = success, nonzero = error code.

use std::borrow::Cow;
use std::cmp::Ordering;

use oxvba_com::{DynamicCallArg, DynamicCallRequest, DynamicObjectBridge, DynamicValue};
use oxvba_compiler::bytecode::{
    DispatchInvokeArg, ExternalCallWriteback, ExternalCallWritebackKind, RuntimeArrayElementType,
    RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
};
use oxvba_hal::HalComDynamicBridge;
use oxvba_hal::error::{HalError, HalErrorKind};
use oxvba_hal::model::CapabilityId;
use oxvba_runtime::safe_array::{
    SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
    VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_UI1_VALUE,
    VT_VARIANT_VALUE,
};
use oxvba_runtime::value_tags::{error_tag_from_code, is_error_tag as runtime_is_error_tag};
use oxvba_runtime::{F64Value, RuntimeValue, VarType, Variant, bstr::BStr};
use oxvba_vm::semantics;

use crate::jit_context::{JitContext, JitRuntimeSlot};
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

macro_rules! read_variant_slot {
    ($ctx:expr, $slot:expr) => {{
        debug_assert!(
            !$ctx.is_null(),
            "read_variant_slot: null JitContext pointer"
        );
        unsafe { (*$ctx).read_variant_slot($slot) }
    }};
}

macro_rules! write_variant_slot {
    ($ctx:expr, $slot:expr, $value:expr) => {{
        debug_assert!(
            !$ctx.is_null(),
            "write_variant_slot: null JitContext pointer"
        );
        unsafe { (*$ctx).write_variant_slot($slot, $value) }
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
    let result = match semantics::runtime_abs_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, result);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sgn(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let result = match semantics::runtime_sgn_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
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

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_i32(ctx: *mut JitContext, dst: u32, value: i32) -> i32 {
    let value = if value == oxvba_runtime::value_tags::NULL_TAG {
        // LoadNull is the only bytecode instruction that materializes VT_NULL.
        RuntimeValue::I32(value)
    } else {
        RuntimeValue::from_compat_slot_i32(value)
    };
    write_slot!(ctx, dst, value);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_bool(ctx: *mut JitContext, dst: u32, value: i32) -> i32 {
    write_slot!(ctx, dst, RuntimeValue::Bool(value != 0));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_null(ctx: *mut JitContext, dst: u32) -> i32 {
    write_slot!(ctx, dst, RuntimeValue::Null);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_jump_if_zero(ctx: *mut JitContext, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    match semantics::legacy_truthy_value(&val) {
        Ok(truthy) => i32::from(!truthy),
        Err(_) => ERR_RUNTIME,
    }
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
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(s)));
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
    let text = match semantics::runtime_value_to_text(&val, "Len operand") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(text.len() as i32));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_left(ctx: *mut JitContext, dst: u32, src: u32, count: u32) -> i32 {
    let src_val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&src_val, "Left src") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    let count_val = read_slot!(ctx, count);
    let n = match semantics::runtime_value_to_usize(&count_val) {
        Ok(n) => n,
        Err(_) => return ERR_RUNTIME,
    };
    let result = if n >= text.len() {
        text
    } else {
        text[..n].to_string()
    };
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_right(ctx: *mut JitContext, dst: u32, src: u32, count: u32) -> i32 {
    let src_val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&src_val, "Right src") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    let count_val = read_slot!(ctx, count);
    let n = match semantics::runtime_value_to_usize(&count_val) {
        Ok(n) => n,
        Err(_) => return ERR_RUNTIME,
    };
    let len = text.len();
    let result = if n >= len {
        text
    } else {
        text[len - n..].to_string()
    };
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(result)));
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
    let text = match semantics::runtime_value_to_text(&src_val, "Mid src") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
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
    let len = text.len();
    let begin = if st == 0 { 0 } else { (st - 1).min(len) };
    let end = match cnt {
        Some(c) => (begin + c).min(len),
        None => len,
    };
    let result = text[begin..end].to_string();
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(result)));
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
    let st_val = read_slot!(ctx, start);
    let cnt_val = if count_slot == u32::MAX {
        None
    } else {
        Some(read_slot!(ctx, count_slot))
    };
    let val = read_slot!(ctx, value);
    let out =
        match semantics::runtime_mid_stmt_bounded(&target_val, &st_val, cnt_val.as_ref(), &val) {
            Ok(v) => v,
            Err(_) => return ERR_RUNTIME,
        };
    write_slot!(ctx, target, out);
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
    let h = match semantics::runtime_value_to_text(&hay_val, "InStr haystack") {
        Ok(text) => semantics::normalize_for_compare(text, scm),
        Err(_) => return ERR_RUNTIME,
    };
    let n = match semantics::runtime_value_to_text(&nee_val, "InStr needle") {
        Ok(text) => semantics::normalize_for_compare(text, scm),
        Err(_) => return ERR_RUNTIME,
    };
    let pos = h.find(&n).map_or(0, |idx| (idx + 1) as i32);
    write_slot!(ctx, dst, RuntimeValue::I32(pos));
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
    let h = match semantics::runtime_value_to_text(&hay_val, "InStrRev haystack") {
        Ok(text) => semantics::normalize_for_compare(text, scm),
        Err(_) => return ERR_RUNTIME,
    };
    let n = match semantics::runtime_value_to_text(&nee_val, "InStrRev needle") {
        Ok(text) => semantics::normalize_for_compare(text, scm),
        Err(_) => return ERR_RUNTIME,
    };
    let pos = h.rfind(&n).map_or(0, |idx| (idx + 1) as i32);
    write_slot!(ctx, dst, RuntimeValue::I32(pos));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_lower(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&val, "LCase operand") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::String(BStr::from(text.to_ascii_lowercase()))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_upper(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&val, "UCase operand") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::String(BStr::from(text.to_ascii_uppercase()))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_split(ctx: *mut JitContext, dst: u32, src: u32, delimiter: u32) -> i32 {
    let v_val = read_slot!(ctx, src);
    let d_val = read_slot!(ctx, delimiter);
    let out = match semantics::runtime_split_count_bounded(&v_val, &d_val) {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_join(ctx: *mut JitContext, dst: u32, src: u32, delimiter: u32) -> i32 {
    let v_val = read_slot!(ctx, src);
    let d_val = read_slot!(ctx, delimiter);
    let out = match semantics::runtime_join_bounded(&v_val, &d_val) {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
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
    let src_text = match semantics::runtime_value_to_text(&src_val, "Replace src") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    let find_text = match semantics::runtime_value_to_text(&find_val, "Replace find") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    let replace_text = match semantics::runtime_value_to_text(&replace_val, "Replace replace") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    let result = src_text.replace(&find_text, &replace_text);
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_trim(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&val, "Trim operand") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::String(BStr::from(text.trim().to_string()))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_ltrim(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&val, "LTrim operand") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::String(BStr::from(text.trim_start().to_string()))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_rtrim(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let text = match semantics::runtime_value_to_text(&val, "RTrim operand") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::String(BStr::from(text.trim_end().to_string()))
    );
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
    let l = match semantics::runtime_value_to_text(&lhs_val, "StrComp lhs") {
        Ok(text) => semantics::normalize_for_compare(text, scm),
        Err(_) => return ERR_RUNTIME,
    };
    let r = match semantics::runtime_value_to_text(&rhs_val, "StrComp rhs") {
        Ok(text) => semantics::normalize_for_compare(text, scm),
        Err(_) => return ERR_RUNTIME,
    };
    let result = match l.cmp(&r) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(result));
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
    let out = match semantics::runtime_like_bounded(&lhs_val, &pat_val, scm) {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_strconv(ctx: *mut JitContext, dst: u32, src: u32, conversion: u32) -> i32 {
    let src_val = read_slot!(ctx, src);
    let conv_val = read_slot!(ctx, conversion);
    let result = match semantics::runtime_strconv_bounded(&src_val, &conv_val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, result);
    OK
}

// ── Char/format ops ──────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_chr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_chr_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_asc(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_asc_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_space(ctx: *mut JitContext, dst: u32, count: u32) -> i32 {
    let val = read_slot!(ctx, count);
    let out = match semantics::runtime_space_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_string_repeat(ctx: *mut JitContext, dst: u32, count: u32, ch: u32) -> i32 {
    let n_val = read_slot!(ctx, count);
    let ch_val = read_slot!(ctx, ch);
    let out = match semantics::runtime_string_repeat_bounded(&n_val, &ch_val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_hex(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_hex_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_oct(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_oct_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
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
            RuntimeValue::String(s) => Some(s.as_str().to_string()),
            _ => None,
        }
    };
    let result = semantics::format_number(n, fmt_str.as_deref());
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(result)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_strreverse(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let s = match semantics::runtime_value_to_text(&val, "StrReverse source") {
        Ok(text) => text,
        Err(_) => return ERR_RUNTIME,
    };
    let result: String = s.chars().rev().collect();
    write_slot!(ctx, dst, RuntimeValue::String(BStr::from(result)));
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
    let y = read_slot!(ctx, year);
    let m = read_slot!(ctx, month);
    let d = read_slot!(ctx, day);
    let out = match semantics::runtime_date_serial_bounded(&y, &m, &d) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
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
    let h = read_slot!(ctx, hour);
    let m = read_slot!(ctx, minute);
    let s = read_slot!(ctx, second);
    let out = match semantics::runtime_time_serial_bounded(&h, &m, &s) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_date_value(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_to_datevalue(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, v);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cdate(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_to_cdate(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, v);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_time_value(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_value_to_timevalue(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, v);
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
    let i = read_slot!(ctx, interval);
    let n = read_slot!(ctx, number);
    let date_value = read_slot!(ctx, date);
    let out = match semantics::runtime_date_add_bounded(&i, &n, &date_value) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
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
    let i = read_slot!(ctx, interval);
    let date1_value = read_slot!(ctx, date1);
    let date2_value = read_slot!(ctx, date2);
    let out = match semantics::runtime_date_diff_bounded(&i, &date1_value, &date2_value) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_year(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_date_year(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_month(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_date_month(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_day(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let v = match semantics::runtime_date_day(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_weekday(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let weekday = match semantics::runtime_date_weekday(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(weekday));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_month_name(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_month_name_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

// ── Math ops ─────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_round(ctx: *mut JitContext, dst: u32, src: u32, digits_slot: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let digits = if digits_slot == u32::MAX {
        None
    } else {
        Some(read_slot!(ctx, digits_slot))
    };
    let out = match semantics::runtime_round_bounded(&val, digits.as_ref()) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sqr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_sqr_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_sin(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_sin_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_cos(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_cos_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_log(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_log_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_exp(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_exp_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_atn(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_atn_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_tan(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_slot!(ctx, src);
    let out = match semantics::runtime_tan_bounded(&val) {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, out);
    OK
}

// ── Type checking ops ────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_vartype_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let code = semantics::runtime_vartype_tag_bounded_variant(&val);
    write_slot!(ctx, dst, RuntimeValue::I32(code));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_vartype(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let code = semantics::runtime_vartype_compat_bounded_variant(&val);
    write_slot!(ctx, dst, RuntimeValue::I32(code));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_strptr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let value = read_variant_slot!(ctx, src);
    let pointer = match value.vtype() {
        oxvba_runtime::VarType::Empty | oxvba_runtime::VarType::Null => 0,
        oxvba_runtime::VarType::String => {
            let Some(value) = value.as_bstr() else {
                return ERR_RUNTIME;
            };
            let utf8 = value.as_str();
            match oxvba_runtime::pointer_helpers::register_utf16_string(&utf8) {
                Ok(pointer) => pointer,
                Err(_) => return ERR_RUNTIME,
            }
        }
        _ => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I64(pointer));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_varptr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let value = read_variant_slot!(ctx, src);
    let pointer = match oxvba_runtime::pointer_helpers::register_variant_pointer(&value) {
        Ok(pointer) => pointer,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I64(pointer));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_varptr_string_var(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let value = read_variant_slot!(ctx, src);
    let pointer = match oxvba_runtime::pointer_helpers::register_string_variant_pointer(&value) {
        Ok(pointer) => pointer,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I64(pointer));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_varptr_variant_var(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let pointer = unsafe { (*ctx).variant_cell_pointer(src) };
    write_slot!(ctx, dst, RuntimeValue::I64(pointer));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_objptr(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let value = read_variant_slot!(ctx, src);
    let pointer = match oxvba_runtime::pointer_helpers::register_object_variant_pointer(&value) {
        Ok(pointer) => pointer,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I64(pointer));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_typename_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let out = semantics::runtime_typename_tag_bounded_variant(&val);
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_numeric_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let out = semantics::runtime_is_numeric_tag_bounded_variant(&val);
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_numeric(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let is_numeric = match val.vtype() {
        oxvba_runtime::VarType::Integer
        | oxvba_runtime::VarType::Long
        | oxvba_runtime::VarType::LongLong
        | oxvba_runtime::VarType::Single
        | oxvba_runtime::VarType::Double
        | oxvba_runtime::VarType::Date
        | oxvba_runtime::VarType::Currency
        | oxvba_runtime::VarType::Decimal
        | oxvba_runtime::VarType::Boolean
        | oxvba_runtime::VarType::Byte => true,
        _ => false,
    };
    write_slot!(ctx, dst, RuntimeValue::Bool(is_numeric));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_error(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let is_error = matches!(val.vtype(), oxvba_runtime::VarType::Error);
    write_slot!(ctx, dst, RuntimeValue::Bool(is_error));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_date_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let out = if semantics::runtime_variant_is_date(&val) {
        1
    } else {
        0
    };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_object_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let out = if semantics::runtime_variant_is_object(&val) {
        1
    } else {
        0
    };
    write_slot!(ctx, dst, RuntimeValue::I32(out));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_null(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::Bool(matches!(val.vtype(), oxvba_runtime::VarType::Null))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_empty(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::Bool(matches!(val.vtype(), oxvba_runtime::VarType::Empty))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_is_array_tag(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::I32(
            if matches!(val.vtype(), oxvba_runtime::VarType::ArrayVariant) {
                1
            } else {
                0
            }
        )
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
    let r = compat_i32_slot(ctx, rate, "FV rate");
    let n = compat_i32_slot(ctx, nper, "FV nper");
    let p = compat_i32_slot(ctx, pmt, "FV pmt");
    let (r, n, p) = match (r, n, p) {
        (Ok(r), Ok(n), Ok(p)) => (r, n, p),
        _ => return ERR_RUNTIME,
    };
    let pv = opt_compat_i32_slot(ctx, pv_slot, 0);
    let due = opt_compat_i32_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(fv_i32(r, n, p, pv, due))
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
    let r = compat_i32_slot(ctx, rate, "PV rate");
    let n = compat_i32_slot(ctx, nper, "PV nper");
    let p = compat_i32_slot(ctx, pmt, "PV pmt");
    let (r, n, p) = match (r, n, p) {
        (Ok(r), Ok(n), Ok(p)) => (r, n, p),
        _ => return ERR_RUNTIME,
    };
    let fv = opt_compat_i32_slot(ctx, fv_slot, 0);
    let due = opt_compat_i32_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(pv_i32(r, n, p, fv, due))
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
    let r = compat_i32_slot(ctx, rate, "PMT rate");
    let n = compat_i32_slot(ctx, nper, "PMT nper");
    let p = compat_i32_slot(ctx, pv, "PMT pv");
    let (r, n, p) = match (r, n, p) {
        (Ok(r), Ok(n), Ok(p)) => (r, n, p),
        _ => return ERR_RUNTIME,
    };
    let fv = opt_compat_i32_slot(ctx, fv_slot, 0);
    let due = opt_compat_i32_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(pmt_i32(r, n, p, fv, due))
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
    let r = match compat_i32_slot(ctx, rate, "NPV rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let slot_indices = unsafe { std::slice::from_raw_parts(slots_ptr, slots_len as usize) };
    let mut cash_flows = Vec::with_capacity(slots_len as usize);
    for &slot in slot_indices {
        match compat_i32_slot(ctx, slot, "NPV value") {
            Ok(v) => cash_flows.push(v),
            Err(_) => return ERR_RUNTIME,
        }
    }
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(npv_i32(r, &cash_flows))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_irr(ctx: *mut JitContext, dst: u32, value: u32, guess_slot: u32) -> i32 {
    let v = match compat_i32_slot(ctx, value, "IRR value") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let guess = opt_compat_i32_slot(ctx, guess_slot, 10);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(irr_i32(v, guess))
    );
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
    let v = match compat_i32_slot(ctx, value, "MIRR value") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fr = match compat_i32_slot(ctx, finance_rate, "MIRR finance_rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let rr = match compat_i32_slot(ctx, reinvest_rate, "MIRR reinvest_rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(mirr_i32(v, fr, rr))
    );
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
    let n = match compat_i32_slot(ctx, nper, "Rate nper") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let p = match compat_i32_slot(ctx, pmt, "Rate pmt") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let pv = match compat_i32_slot(ctx, pv, "Rate pv") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fv = opt_compat_i32_slot(ctx, fv_slot, 0);
    let due = opt_compat_i32_slot(ctx, due_slot, 0);
    let guess = opt_compat_i32_slot(ctx, guess_slot, 10);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(rate_i32(n, p, pv, fv, due, guess))
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
    let r = match compat_i32_slot(ctx, rate, "NPer rate") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let p = match compat_i32_slot(ctx, pmt, "NPer pmt") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let pv = match compat_i32_slot(ctx, pv, "NPer pv") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let fv = opt_compat_i32_slot(ctx, fv_slot, 0);
    let due = opt_compat_i32_slot(ctx, due_slot, 0);
    write_slot!(
        ctx,
        dst,
        RuntimeValue::from_compat_slot_i32(nper_i32(r, p, pv, fv, due))
    );
    OK
}

// ── Array ops ────────────────────────────────────────────────────────

fn runtime_resized_array(
    lower_bounds: &[i32],
    upper_bounds: &[i32],
    element_type: RuntimeArrayElementType,
) -> Result<SafeArray, String> {
    if lower_bounds.is_empty() || lower_bounds.len() != upper_bounds.len() {
        return Err("runtime ReDim requires at least one dimension".to_string());
    }
    let mut len = 1usize;
    let mut bounds = Vec::with_capacity(lower_bounds.len());
    for (&lower_bound, &upper_bound) in lower_bounds.iter().zip(upper_bounds.iter()) {
        if upper_bound < lower_bound {
            return Err(format!(
                "runtime ReDim upper bound {upper_bound} is below lower bound {lower_bound}"
            ));
        }
        let count = i64::from(upper_bound) - i64::from(lower_bound) + 1;
        let width = usize::try_from(count)
            .map_err(|_| format!("runtime ReDim bound span {count} cannot fit in host memory"))?;
        len = len
            .checked_mul(width)
            .ok_or_else(|| "runtime ReDim total array length overflowed".to_string())?;
        bounds.push(SafeArrayBound {
            lower: lower_bound,
            count: u32::try_from(width)
                .map_err(|_| format!("runtime ReDim length {width} exceeds SAFEARRAY capacity"))?,
        });
    }
    let default = runtime_array_default_variant(element_type);
    let values = vec![default; len];
    SafeArray::from_typed_variants_nd(bounds, runtime_array_element_vartype(element_type), values)
}

fn runtime_resized_array_preserve(
    current: &Variant,
    lower_bounds: &[i32],
    upper_bounds: &[i32],
    element_type: RuntimeArrayElementType,
) -> Result<SafeArray, String> {
    let Some(previous) = current.as_safearray() else {
        return Err("runtime ReDim Preserve requires an existing runtime array value".to_string());
    };
    if previous.dimensions() as usize != lower_bounds.len()
        || lower_bounds.len() != upper_bounds.len()
    {
        return Err(
            "runtime ReDim Preserve requires the existing and resized array to have the same rank"
                .to_string(),
        );
    }
    let previous_bounds_binding = previous.bounds();
    let previous_bounds = previous_bounds_binding
        .as_ref()
        .ok_or_else(|| "runtime ReDim Preserve requires bounds metadata".to_string())?;
    let previous_values_binding = previous.variant_elements();
    let previous_values = previous_values_binding
        .as_ref()
        .ok_or_else(|| "runtime ReDim Preserve requires an owned array payload".to_string())?;
    let resized = runtime_resized_array(lower_bounds, upper_bounds, element_type)?;
    let resized_bounds = resized
        .bounds()
        .as_ref()
        .ok_or_else(|| "runtime ReDim Preserve failed to materialize bounds metadata".to_string())?
        .clone();
    let mut resized_values = resized.variant_elements().ok_or_else(|| {
        "runtime ReDim Preserve failed to materialize an owned array payload".to_string()
    })?;
    for dim in 0..previous_bounds.len() {
        let previous_bound = &previous_bounds[dim];
        let resized_bound = &resized_bounds[dim];
        if previous_bound.lower != resized_bound.lower {
            return Err(
                "runtime ReDim Preserve requires lower bounds to remain unchanged".to_string(),
            );
        }
        if dim + 1 != previous_bounds.len() && previous_bound.count != resized_bound.count {
            return Err("runtime ReDim Preserve only supports resizing the upper bound of the last dimension".to_string());
        }
    }
    let last = previous_bounds.len() - 1;
    let previous_last = previous_bounds[last].count as usize;
    let resized_last = resized_bounds[last].count as usize;
    let overlap = previous_last.min(resized_last);
    let mut block_count = 1usize;
    for bound in &previous_bounds[..last] {
        block_count = block_count
            .checked_mul(bound.count as usize)
            .ok_or_else(|| "runtime ReDim Preserve block count overflowed".to_string())?;
    }
    for block in 0..block_count.max(1) {
        let previous_start = block
            .checked_mul(previous_last)
            .ok_or_else(|| "runtime ReDim Preserve previous block offset overflowed".to_string())?;
        let resized_start = block
            .checked_mul(resized_last)
            .ok_or_else(|| "runtime ReDim Preserve resized block offset overflowed".to_string())?;
        for offset in 0..overlap {
            resized_values[resized_start + offset] =
                previous_values[previous_start + offset].clone();
        }
    }
    resized.replace_variant_elements(resized_values)
}

fn decode_runtime_array_element_type(element_type: i32) -> Option<RuntimeArrayElementType> {
    Some(match element_type {
        x if x == RuntimeArrayElementType::Variant as i32 => RuntimeArrayElementType::Variant,
        x if x == RuntimeArrayElementType::Integer as i32 => RuntimeArrayElementType::Integer,
        x if x == RuntimeArrayElementType::Long as i32 => RuntimeArrayElementType::Long,
        x if x == RuntimeArrayElementType::LongLong as i32 => RuntimeArrayElementType::LongLong,
        x if x == RuntimeArrayElementType::LongPtr as i32 => RuntimeArrayElementType::LongPtr,
        x if x == RuntimeArrayElementType::Byte as i32 => RuntimeArrayElementType::Byte,
        x if x == RuntimeArrayElementType::Single as i32 => RuntimeArrayElementType::Single,
        x if x == RuntimeArrayElementType::Double as i32 => RuntimeArrayElementType::Double,
        x if x == RuntimeArrayElementType::Currency as i32 => RuntimeArrayElementType::Currency,
        x if x == RuntimeArrayElementType::Date as i32 => RuntimeArrayElementType::Date,
        x if x == RuntimeArrayElementType::String as i32 => RuntimeArrayElementType::String,
        x if x == RuntimeArrayElementType::Boolean as i32 => RuntimeArrayElementType::Boolean,
        _ => return None,
    })
}

fn runtime_array_default_variant(element_type: RuntimeArrayElementType) -> Variant {
    match element_type {
        RuntimeArrayElementType::Variant => Variant::empty(),
        RuntimeArrayElementType::Integer => Variant::from_i16(0),
        RuntimeArrayElementType::Long => Variant::from_i32(0),
        RuntimeArrayElementType::Byte => Variant::from_u8(0),
        RuntimeArrayElementType::LongLong | RuntimeArrayElementType::LongPtr => {
            Variant::from_i64(0)
        }
        RuntimeArrayElementType::Single => Variant::from_f32(0.0),
        RuntimeArrayElementType::Double => Variant::from_f64(0.0),
        RuntimeArrayElementType::Currency => Variant::from_currency_scaled_i64(0),
        RuntimeArrayElementType::Date => Variant::from_date_f64(0.0),
        RuntimeArrayElementType::String => Variant::from_string(BStr::empty()),
        RuntimeArrayElementType::Boolean => Variant::from_bool(false),
    }
}

fn runtime_array_element_vartype(element_type: RuntimeArrayElementType) -> u16 {
    match element_type {
        RuntimeArrayElementType::Variant => VT_VARIANT_VALUE,
        RuntimeArrayElementType::Integer => VT_I2_VALUE,
        RuntimeArrayElementType::Long => VT_I4_VALUE,
        RuntimeArrayElementType::LongLong | RuntimeArrayElementType::LongPtr => VT_I8_VALUE,
        RuntimeArrayElementType::Byte => VT_UI1_VALUE,
        RuntimeArrayElementType::Single => VT_R4_VALUE,
        RuntimeArrayElementType::Double => VT_R8_VALUE,
        RuntimeArrayElementType::Currency => VT_CY_VALUE,
        RuntimeArrayElementType::Date => VT_DATE_VALUE,
        RuntimeArrayElementType::String => VT_BSTR_VALUE,
        RuntimeArrayElementType::Boolean => VT_BOOL_VALUE,
    }
}

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
        elements.push(read_variant_slot!(ctx, slot));
    }
    write_variant_slot!(
        ctx,
        dst,
        Variant::from_safearray(oxvba_runtime::safe_array::SafeArray::from_variants(
            elements
        ))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_array_append(ctx: *mut JitContext, dst: u32, array: u32, item: u32) -> i32 {
    let current = read_variant_slot!(ctx, array);
    let item = read_variant_slot!(ctx, item);
    let mut elements = match current.as_safearray() {
        Some(array) => array.variant_elements().unwrap_or_default(),
        None if matches!(current.vtype(), VarType::Empty) || current.as_i32() == Some(0) => {
            Vec::new()
        }
        _ => return ERR_RUNTIME,
    };
    elements.push(item);
    write_variant_slot!(
        ctx,
        dst,
        Variant::from_safearray(oxvba_runtime::safe_array::SafeArray::from_variants(
            elements
        ))
    );
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_array_resize(
    ctx: *mut JitContext,
    dst: u32,
    upper_bound_slots_ptr: *const u32,
    lower_bounds_ptr: *const i32,
    bounds_len: u32,
    element_type: i32,
) -> i32 {
    let upper_bound_slots =
        unsafe { std::slice::from_raw_parts(upper_bound_slots_ptr, bounds_len as usize) };
    let lower_bounds = unsafe { std::slice::from_raw_parts(lower_bounds_ptr, bounds_len as usize) };
    let mut upper_bounds = Vec::with_capacity(upper_bound_slots.len());
    for &upper_bound_slot in upper_bound_slots {
        match semantics::runtime_value_to_i32_compat(
            &read_slot!(ctx, upper_bound_slot),
            "ReDim upper bound",
        ) {
            Ok(value) => upper_bounds.push(value),
            Err(_) => return ERR_RUNTIME,
        }
    }
    let Some(element_type) = decode_runtime_array_element_type(element_type) else {
        return ERR_RUNTIME;
    };
    let array = match runtime_resized_array(lower_bounds, &upper_bounds, element_type) {
        Ok(array) => array,
        Err(_) => return ERR_RUNTIME,
    };
    write_variant_slot!(ctx, dst, Variant::from_safearray(array));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_array_resize_preserve(
    ctx: *mut JitContext,
    dst: u32,
    upper_bound_slots_ptr: *const u32,
    lower_bounds_ptr: *const i32,
    bounds_len: u32,
    element_type: i32,
) -> i32 {
    let upper_bound_slots =
        unsafe { std::slice::from_raw_parts(upper_bound_slots_ptr, bounds_len as usize) };
    let lower_bounds = unsafe { std::slice::from_raw_parts(lower_bounds_ptr, bounds_len as usize) };
    let mut upper_bounds = Vec::with_capacity(upper_bound_slots.len());
    for &upper_bound_slot in upper_bound_slots {
        match semantics::runtime_value_to_i32_compat(
            &read_slot!(ctx, upper_bound_slot),
            "ReDim Preserve upper bound",
        ) {
            Ok(value) => upper_bounds.push(value),
            Err(_) => return ERR_RUNTIME,
        }
    }
    let current = read_variant_slot!(ctx, dst);
    let Some(element_type) = decode_runtime_array_element_type(element_type) else {
        return ERR_RUNTIME;
    };
    let array =
        match runtime_resized_array_preserve(&current, lower_bounds, &upper_bounds, element_type) {
            Ok(array) => array,
            Err(_) => return ERR_RUNTIME,
        };
    write_variant_slot!(ctx, dst, Variant::from_safearray(array));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_array_get(
    ctx: *mut JitContext,
    dst: u32,
    array_slot: u32,
    index_slots_ptr: *const u32,
    index_slots_len: u32,
) -> i32 {
    let array_value = read_variant_slot!(ctx, array_slot);
    let index_slots =
        unsafe { std::slice::from_raw_parts(index_slots_ptr, index_slots_len as usize) };
    let index_values = index_slots
        .iter()
        .map(|slot| read_slot!(ctx, *slot))
        .collect::<Vec<_>>();
    let value =
        match semantics::runtime_array_get_variant(&array_value, &index_values, "array index") {
            Ok(value) => value,
            Err(_) => return ERR_RUNTIME,
        };
    write_variant_slot!(ctx, dst, value);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_array_set(
    ctx: *mut JitContext,
    array_slot: u32,
    index_slots_ptr: *const u32,
    index_slots_len: u32,
    src_slot: u32,
) -> i32 {
    let array_value = read_variant_slot!(ctx, array_slot);
    let index_slots =
        unsafe { std::slice::from_raw_parts(index_slots_ptr, index_slots_len as usize) };
    let index_values = index_slots
        .iter()
        .map(|slot| read_slot!(ctx, *slot))
        .collect::<Vec<_>>();
    let src_value = read_variant_slot!(ctx, src_slot);
    let value = match semantics::runtime_array_set_variant(
        &array_value,
        &index_values,
        &src_value,
        "array index",
    ) {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    write_variant_slot!(ctx, array_slot, value);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_lbound(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let v = match semantics::runtime_array_lbound_variant(&val, "LBound operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_ubound(ctx: *mut JitContext, dst: u32, src: u32) -> i32 {
    let val = read_variant_slot!(ctx, src);
    let v = match semantics::runtime_array_ubound_variant(&val, "UBound operand") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    write_slot!(ctx, dst, RuntimeValue::I32(v));
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
    let c = match compat_i32_slot(ctx, count, "Collection.Add count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let _item = compat_i32_slot(ctx, item, "Collection.Add item");
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
    let c = match compat_i32_slot(ctx, count, "Collection.Item count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let i = match compat_i32_slot(ctx, index, "Collection.Item index") {
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
    let c = match compat_i32_slot(ctx, count, "Collection.Remove count") {
        Ok(v) => v,
        Err(_) => return ERR_RUNTIME,
    };
    let _i = compat_i32_slot(ctx, index, "Collection.Remove index");
    write_slot!(ctx, dst, RuntimeValue::I32((c - 1).max(0)));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_collection_count(ctx: *mut JitContext, dst: u32, count: u32) -> i32 {
    let c = match compat_i32_slot(ctx, count, "Collection.Count count") {
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
        let seed = match semantics::runtime_random_seed_bounded(&seed_val, "Rnd seed") {
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
        let seed = match semantics::runtime_random_seed_bounded(&seed_val, "Randomize seed") {
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
    write_slot!(ctx, slot, RuntimeValue::String(BStr::empty()));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_load_err_source(ctx: *mut JitContext, slot: u32) -> i32 {
    write_slot!(ctx, slot, RuntimeValue::String(BStr::empty()));
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
        Variant::from_i32(0)
    } else {
        read_variant_slot!(ctx, range_slot)
    };
    match host.fs().free_file_variant(selector) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let path_val = read_variant_slot!(ctx, path);
    let mode_val = read_variant_slot!(ctx, mode);
    let file_num = read_variant_slot!(ctx, file_number);
    let mode_val = match mode_val.to_runtime_value() {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    let file_num = match file_num.to_runtime_value() {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    let mode_i32 = match semantics::runtime_value_to_i32_compat(&mode_val, "Open mode") {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    let fnum_i32 = match semantics::runtime_value_to_i32_compat(&file_num, "Open file number") {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    let combined_mode = Variant::from_i32(mode_i32 | (fnum_i32 << 16));
    match host.fs().open_variant(path_val, combined_mode) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_close(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_variant_slot!(ctx, handle);
    match host.fs().close_variant(handle_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_kill(ctx: *mut JitContext, dst: u32, path: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let path_val = read_variant_slot!(ctx, path);
    match host.fs().kill_variant(path_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let handle_val = read_variant_slot!(ctx, handle);
    let count_val = read_variant_slot!(ctx, count);
    match host.fs().read_bytes_variant(handle_val, count_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let handle_val = read_variant_slot!(ctx, handle);
    let data_val = read_variant_slot!(ctx, data);
    match host.fs().write_bytes_variant(handle_val, data_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let handle_val = read_variant_slot!(ctx, handle);
    let data_val = read_variant_slot!(ctx, data);
    match host.fs().print_line_variant(handle_val, data_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_console_print(ctx: *mut JitContext, dst: u32, data: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let data_val = read_variant_slot!(ctx, data);
    match host.console().print_line_variant(data_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let handle_val = read_variant_slot!(ctx, handle);
    let count_val = read_variant_slot!(ctx, count);
    match host.fs().input_fields_variant(handle_val, count_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_console_input(ctx: *mut JitContext, dst: u32, count: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let count_val = read_variant_slot!(ctx, count);
    match host.console().input_fields_variant(count_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_line_input(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_variant_slot!(ctx, handle);
    match host.fs().line_input_variant(handle_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_console_line_input(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.console().line_input_variant() {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_beep(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host
        .diag()
        .emit_variant(Variant::from_i32(7), Variant::from_i32(0))
    {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_eof(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_variant_slot!(ctx, handle);
    match host.fs().eof_variant(handle_val) {
        Ok(value) if value.as_i32().is_some() => {
            write_variant_slot!(
                ctx,
                dst,
                Variant::from_bool(value.as_i32().unwrap_or(0) != 0)
            );
            OK
        }
        Ok(value) if value.as_bool().is_some() => {
            write_variant_slot!(ctx, dst, Variant::from_bool(value.as_bool().unwrap_or(false)));
            OK
        }
        Ok(_) => ERR_RUNTIME,
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_lof(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_variant_slot!(ctx, handle);
    match host.fs().lof_variant(handle_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_seek(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_variant_slot!(ctx, handle);
    match host.fs().loc_variant(handle_val) {
        Ok(value) if value.as_i32().is_some() => {
            write_variant_slot!(ctx, dst, Variant::from_i32(value.as_i32().unwrap_or(0) + 1));
            OK
        }
        Ok(value) => match semantics::variant_to_i32_compat(&value, "Loc") {
            Ok(value) => {
                write_variant_slot!(ctx, dst, Variant::from_i32(value + 1));
                OK
            }
            Err(_) => ERR_RUNTIME,
        },
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_file_loc(ctx: *mut JitContext, dst: u32, handle: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let handle_val = read_variant_slot!(ctx, handle);
    match host.fs().loc_variant(handle_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// COM dispatch (simplified - pass through host services)
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_create_object(ctx: *mut JitContext, dst: u32, prog_id: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let prog_id_val = read_variant_slot!(ctx, prog_id);
    match host.com().create_object_variant(prog_id_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let object_val = read_variant_slot!(ctx, object);
    // Error 91: Object variable or With block variable not set.
    if matches!(object_val.vtype(), VarType::Empty) {
        return route_host_error_code(ctx, 91);
    }
    let object_ref = match semantics::variant_to_com_object(&object_val, "dispatch_invoke.object") {
        Ok(h) => {
            if h.raw() == 0 {
                return route_host_error_code(ctx, 91);
            }
            h
        }
        Err(_) => return route_host_error_code(ctx, 91),
    };
    let member_val = read_variant_slot!(ctx, member);
    let member_sel = match semantics::variant_to_dynamic_member_selector(
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
        object: object_ref,
        member: member_sel,
        args: Vec::with_capacity(args_slice.len()),
        call_kind_hint: None,
    };
    for arg in args_slice {
        let value = arg
            .slot
            .map(|slot| read_variant_slot!(ctx, slot as u32))
            .map(DynamicValue::from_variant);
        request.args.push(DynamicCallArg {
            value,
            name: arg.name.clone(),
        });
    }

    let host = unsafe { (*ctx).host_services() };
    let bridge = HalComDynamicBridge::new(host.profile(), host.com());
    match bridge.invoke_dynamic(&request) {
        Ok(value) => {
            // Normalize: COM dispatch may return error tags as I32 instead of VT_ERROR.
            write_variant_slot!(ctx, dst, normalize_com_result_variant(value.variant()));
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
    let object_val = read_variant_slot!(ctx, object);
    let event_val = read_variant_slot!(ctx, event);
    let object =
        match semantics::variant_to_com_object(&object_val, "com_subscribe_event.object") {
            Ok(o) => o,
            Err(_) => return route_host_error(ctx),
        };
    let event = match semantics::variant_to_com_member_token(
        &event_val,
        "com_subscribe_event.event",
    ) {
        Ok(e) => e,
        Err(_) => return route_host_error(ctx),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().subscribe_event(object, event) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, Variant::from_i32(value.raw()));
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
    let sub_val = read_variant_slot!(ctx, subscription);
    let sub = match semantics::variant_to_com_subscription_token(
        &sub_val,
        "com_unsubscribe_event.subscription",
    ) {
        Ok(s) => s,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().unsubscribe_event_variant(sub) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let cb_val = read_variant_slot!(ctx, callback);
    let cb = match semantics::variant_to_com_callback_token(
        &cb_val,
        "com_event_callback_subscription.callback",
    ) {
        Ok(c) => c,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().event_callback_subscription(cb) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, Variant::from_i32(value.raw()));
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
    let cb_val = read_variant_slot!(ctx, callback);
    let idx_val = read_variant_slot!(ctx, index);
    let cb = match semantics::variant_to_com_callback_token(
        &cb_val,
        "com_event_callback_arg.callback",
    ) {
        Ok(c) => c,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let idx = match semantics::variant_to_usize_index(&idx_val, "com_event_callback_arg.index") {
        Ok(i) => i,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().event_callback_variant(cb, idx) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let cb_val = read_variant_slot!(ctx, callback);
    let cb = match semantics::variant_to_com_callback_token(
        &cb_val,
        "com_release_event_callback.callback",
    ) {
        Ok(c) => c,
        Err(_) => return route_host_error_code(ctx, 53053),
    };
    let host = unsafe { (*ctx).host_services() };
    match host.com().release_event_callback_variant(cb) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let owner_val = read_variant_slot!(ctx, owner);
    let binding_val = read_slot!(ctx, binding);
    let owner = match semantics::variant_to_withevents_owner_handle(&owner_val, "owner") {
        Ok(o) => o,
        Err(_) => return ERR_RUNTIME,
    };
    let binding = match semantics::withevents_binding_handle(&binding_val, "binding") {
        Ok(b) => b,
        Err(_) => return ERR_RUNTIME,
    };
    let key = semantics::withevents_binding_key(&owner, binding);
    let state = unsafe { (*ctx).host_state() };
    let value = state
        .withevents_bindings
        .get(&key)
        .cloned()
        .unwrap_or_else(|| JitRuntimeSlot::Variant(Variant::from_i32(0)));
    match value {
        JitRuntimeSlot::Variant(value) => write_variant_slot!(ctx, dst, value),
        JitRuntimeSlot::BindingHandle(handle) => {
            write_slot!(ctx, dst, RuntimeValue::BindingHandle(handle))
        }
    };
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
    let owner_val = read_variant_slot!(ctx, owner_slot);
    let binding_val = read_slot!(ctx, binding_slot);
    let val = read_variant_slot!(ctx, value);
    let owner = match semantics::variant_to_withevents_owner_handle(&owner_val, "owner") {
        Ok(o) => o,
        Err(_) => return ERR_RUNTIME,
    };
    let binding = match semantics::withevents_binding_handle(&binding_val, "binding") {
        Ok(b) => b,
        Err(_) => return ERR_RUNTIME,
    };
    let key = semantics::withevents_binding_key(&owner, binding);
    let state = unsafe { (*ctx).host_state_mut() };
    if val.as_i32() == Some(0) {
        state.withevents_bindings.remove(&key);
    } else {
        state
            .withevents_bindings
            .insert(key, JitRuntimeSlot::Variant(val.clone()));
    }
    write_variant_slot!(ctx, dst, val);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_clear_owner(
    ctx: *mut JitContext,
    dst: u32,
    owner: u32,
) -> i32 {
    let owner_val = read_variant_slot!(ctx, owner);
    let owner = match semantics::variant_to_withevents_owner_handle(&owner_val, "owner") {
        Ok(o) => o,
        Err(_) => return ERR_RUNTIME,
    };
    let state = unsafe { (*ctx).host_state_mut() };
    state
        .withevents_bindings
        .retain(|key, _| semantics::withevents_owner_from_key(*key) != owner);
    write_variant_slot!(ctx, dst, Variant::from_i32(0));
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_first_owner(
    ctx: *mut JitContext,
    dst: u32,
    source: u32,
    binding_slot: u32,
) -> i32 {
    let source_val = read_variant_slot!(ctx, source);
    let binding_val = read_slot!(ctx, binding_slot);
    let binding = match semantics::withevents_binding_handle(&binding_val, "binding") {
        Ok(b) => b,
        Err(_) => return ERR_RUNTIME,
    };
    // If source is 0, no matching owners.
    if source_val.as_i32() == Some(0)
        || source_val.as_i64() == Some(0)
        || source_val.as_bool() == Some(false)
    {
        write_variant_slot!(ctx, dst, Variant::from_i32(0));
        return OK;
    }
    let state = unsafe { (*ctx).host_state_mut() };
    let mut owners: Vec<_> = state
        .withevents_bindings
        .iter()
        .filter_map(|(key, value)| {
            let JitRuntimeSlot::Variant(value) = value else {
                return None;
            };
            if value != &source_val || semantics::withevents_binding_from_key(*key) != binding {
                return None;
            }
            Some(semantics::withevents_owner_from_key(*key))
        })
        .collect();
    owners.sort_unstable_by_key(|owner| owner.raw());
    if owners.is_empty() {
        write_variant_slot!(ctx, dst, Variant::from_i32(0));
    } else {
        let first = owners[0].clone();
        state
            .withevents_owner_iters
            .push(crate::jit_context::WithEventsOwnerIterator {
                owners,
                next_index: 1,
            });
        write_variant_slot!(ctx, dst, Variant::from_object_ref(first));
    }
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_withevents_next_owner(ctx: *mut JitContext, dst: u32) -> i32 {
    let state = unsafe { (*ctx).host_state_mut() };
    let next = if let Some(iter) = state.withevents_owner_iters.last_mut() {
        if iter.next_index < iter.owners.len() {
            let owner = iter.owners[iter.next_index].clone();
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
        .map(Variant::from_object_ref)
        .unwrap_or_else(|| Variant::from_i32(0));
    write_variant_slot!(ctx, dst, result);
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
    let prompt_val = read_variant_slot!(ctx, prompt);
    let style = if style_slot == u32::MAX {
        Variant::from_i32(1)
    } else {
        read_variant_slot!(ctx, style_slot)
    };
    match host.ui().msg_box_variant(prompt_val, style) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    let prompt_val = read_variant_slot!(ctx, prompt);
    let default_val = if default_slot == u32::MAX {
        Variant::from_i32(0)
    } else {
        read_variant_slot!(ctx, default_slot)
    };
    match host.ui().input_box_variant(prompt_val, default_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_debug_print(ctx: *mut JitContext, dst: u32, data: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let data_val = read_variant_slot!(ctx, data);
    match host.diag().debug_print_variant(data_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_do_events(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.events().do_events_variant() {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// Process/Env
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_shell(ctx: *mut JitContext, dst: u32, command: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let cmd_val = read_variant_slot!(ctx, command);
    match host.process().shell_variant(cmd_val, Variant::from_i32(0)) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_environ(ctx: *mut JitContext, dst: u32, key: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let key_val = read_variant_slot!(ctx, key);
    match host.process().environ_variant(key_val) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_dir(ctx: *mut JitContext, dst: u32, path: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let path_val = read_variant_slot!(ctx, path);
    match host.process().dir_variant(path_val, Variant::from_i32(0)) {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

// Time
#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_date_now(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().date_serial_now_variant() {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_time_now(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().time_serial_now_variant() {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
            OK
        }
        Err(_) => ERR_RUNTIME,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_now(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    let date = match host.time_locale().date_serial_now_variant() {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    let time = match host.time_locale().time_serial_now_variant() {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    let value = match semantics::variant_host_now_value(&date, &time) {
        Ok(value) => value,
        Err(_) => return ERR_RUNTIME,
    };
    write_variant_slot!(ctx, dst, value);
    OK
}

#[unsafe(no_mangle)]
pub extern "C" fn oxrt_host_timer(ctx: *mut JitContext, dst: u32) -> i32 {
    let host = unsafe { (*ctx).host_services() };
    match host.time_locale().timer_ticks_variant() {
        Ok(value) => {
            write_variant_slot!(ctx, dst, value);
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
    writeback_ptr: *const ExternalCallWriteback,
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

    let arg_variants: Vec<Variant> = arg_slots
        .iter()
        .map(|slot| read_variant_slot!(ctx, *slot as u32))
        .collect();

    // Fast path: no descriptors → simple invoke_symbol.
    let descriptors = unsafe { (*ctx).host_state().external_call_descriptors() };
    if descriptors.is_empty() {
        let first_arg = arg_variants
            .first()
            .cloned()
            .unwrap_or_else(|| Variant::from_i32(0));
        match host.dynlink().invoke_symbol_variant(symbol, &first_arg) {
            Ok(value) => {
                write_variant_slot!(ctx, dst, value);
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
        param_by_ref: &descriptor.param_by_ref,
        return_type: {
            return_type_string = descriptor
                .return_type
                .as_ref()
                .map(|rt| Cow::Owned(format!("{:?}", rt)));
            return_type_string.clone()
        },
    };
    if let Some(_violation) = view.contract_violation() {
        return route_host_error_code(ctx, 53073);
    }

    if arg_variants.len() > 1 || !writeback_slots.is_empty() {
        match host
            .dynlink()
            .invoke_descriptor_variants(&view, &arg_variants)
        {
            Ok((ret_value, wb_values)) => {
                write_variant_slot!(ctx, dst, ret_value);
                if let Err(_detail) =
                    apply_external_writebacks(ctx, writeback_slots, &arg_variants, &wb_values)
                {
                    return route_host_error_code(ctx, 53073);
                }
                OK
            }
            Err(err) => route_hal_error(ctx, err),
        }
    } else {
        match host
            .dynlink()
            .invoke_descriptor_variants(&view, &arg_variants)
        {
            Ok((ret_value, wb_values)) => {
                write_variant_slot!(ctx, dst, ret_value);
                if let Err(_detail) =
                    apply_external_writebacks(ctx, writeback_slots, &arg_variants, &wb_values)
                {
                    return route_host_error_code(ctx, 53073);
                }
                OK
            }
            Err(err) => route_hal_error(ctx, err),
        }
    }
}

fn apply_external_writebacks(
    ctx: *mut JitContext,
    writebacks: &[ExternalCallWriteback],
    arg_values: &[Variant],
    wb_values: &[Variant],
) -> Result<(), String> {
    for writeback in writebacks {
        let value = match writeback.kind {
            ExternalCallWritebackKind::ByRefValue => {
                let Some(value) = wb_values.get(writeback.arg_index) else {
                    continue;
                };
                value.clone()
            }
            ExternalCallWritebackKind::PointerByteArrayPayload => {
                let Some(pointer) = arg_values
                    .get(writeback.arg_index)
                    .and_then(Variant::as_i64)
                else {
                    return Err(format!(
                        "pointer writeback arg {} is not a LongPtr value",
                        writeback.arg_index
                    ));
                };
                oxvba_runtime::pointer_helpers::read_back_byte_array_payload_variant(pointer)?
            }
            ExternalCallWritebackKind::PointerStringPayload => {
                let Some(pointer) = arg_values
                    .get(writeback.arg_index)
                    .and_then(Variant::as_i64)
                else {
                    return Err(format!(
                        "pointer writeback arg {} is not a LongPtr value",
                        writeback.arg_index
                    ));
                };
                oxvba_runtime::pointer_helpers::read_back_string_payload_variant(pointer)?
            }
        };
        write_variant_slot!(ctx, writeback.source_slot as u32, value);
    }
    Ok(())
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
        CapabilityId::ProjectCatalog => 9,
        CapabilityId::ProjectReferenceProvider => 10,
        CapabilityId::ProjectMutation => 11,
        CapabilityId::ConsoleIo => 12,
    };
    53_000 + capability_code * 10 + kind_code
}

/// Route a HalError through the JitContext error handling.
fn route_hal_error(ctx: *mut JitContext, err: HalError) -> i32 {
    let code = hal_error_code(err.kind, err.capability);
    route_host_error_code(ctx, code)
}

fn normalize_com_result_variant(value: &Variant) -> Variant {
    if let Some(value) = value.as_i32()
        && runtime_is_error_tag(value)
        && let Some(code) = oxvba_runtime::value_tags::error_code_from_tag(value)
    {
        return Variant::from_error_code(code);
    }
    value.clone()
}

fn compat_i32_slot(ctx: *mut JitContext, slot: u32, field: &str) -> Result<i32, ()> {
    let val = read_slot!(ctx, slot);
    semantics::runtime_value_to_i32_compat(&val, field).map_err(|_| ())
}

fn opt_compat_i32_slot(ctx: *mut JitContext, slot: u32, default: i32) -> i32 {
    if slot == u32::MAX {
        default
    } else {
        compat_i32_slot(ctx, slot, "optional").unwrap_or(default)
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
        ("oxrt_load_i32", oxrt_load_i32 as *const u8),
        ("oxrt_load_bool", oxrt_load_bool as *const u8),
        ("oxrt_load_null", oxrt_load_null as *const u8),
        ("oxrt_jump_if_zero", oxrt_jump_if_zero as *const u8),
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
        ("oxrt_cdate", oxrt_cdate as *const u8),
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
        ("oxrt_array_append", oxrt_array_append as *const u8),
        (
            "oxrt_array_resize_preserve",
            oxrt_array_resize_preserve as *const u8,
        ),
        ("oxrt_array_get", oxrt_array_get as *const u8),
        ("oxrt_array_set", oxrt_array_set as *const u8),
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
        ("oxrt_host_file_kill", oxrt_host_file_kill as *const u8),
        ("oxrt_host_file_read", oxrt_host_file_read as *const u8),
        ("oxrt_host_file_write", oxrt_host_file_write as *const u8),
        ("oxrt_host_file_print", oxrt_host_file_print as *const u8),
        (
            "oxrt_host_console_print",
            oxrt_host_console_print as *const u8,
        ),
        ("oxrt_host_file_input", oxrt_host_file_input as *const u8),
        (
            "oxrt_host_console_input",
            oxrt_host_console_input as *const u8,
        ),
        (
            "oxrt_host_file_line_input",
            oxrt_host_file_line_input as *const u8,
        ),
        (
            "oxrt_host_console_line_input",
            oxrt_host_console_line_input as *const u8,
        ),
        ("oxrt_host_beep", oxrt_host_beep as *const u8),
        ("oxrt_host_file_eof", oxrt_host_file_eof as *const u8),
        ("oxrt_host_file_lof", oxrt_host_file_lof as *const u8),
        ("oxrt_host_file_seek", oxrt_host_file_seek as *const u8),
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
        ("oxrt_host_debug_print", oxrt_host_debug_print as *const u8),
        ("oxrt_array_resize", oxrt_array_resize as *const u8),
        (
            "oxrt_array_resize_preserve",
            oxrt_array_resize_preserve as *const u8,
        ),
        ("oxrt_array_get", oxrt_array_get as *const u8),
        ("oxrt_array_set", oxrt_array_set as *const u8),
        ("oxrt_strptr", oxrt_strptr as *const u8),
        ("oxrt_varptr", oxrt_varptr as *const u8),
        (
            "oxrt_varptr_string_var",
            oxrt_varptr_string_var as *const u8,
        ),
        (
            "oxrt_varptr_variant_var",
            oxrt_varptr_variant_var as *const u8,
        ),
        ("oxrt_objptr", oxrt_objptr as *const u8),
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
