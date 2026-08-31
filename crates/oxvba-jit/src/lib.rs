//! `oxvba-jit` — the Cranelift backend for typed OxIR.
//!
//! The backend compiles linked `OxProgram` sets with an explicit whole-program
//! decline boundary and no VM fallback. It currently covers a broad
//! platform-neutral language/runtime surface while Windows COM/native calls,
//! persistent package sessions, typed-primary entries and the product cache
//! remain open architecture work. See `docs/spec/OXVBA_JIT_ARCHITECTURE_V1.md`.

use cranelift_codegen::ir::{
    self, AbiParam, InstBuilder, StackSlotData, StackSlotKind, UserFuncName, Value,
    immediates::{Ieee32, Ieee64},
    types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId as ClifFuncId, Linkage, Module, default_libcall_names};
use oxvba_bundle::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, ExportTarget, NativeBody,
    NativeImplId, NumericCoerceTarget, NumericMode, ProjectMemberKind, StringCompareMode,
    array_element_type_for_vartype, default_array_element, redim_safearray_from_elements,
    safearray_vartype_for_element, vba_library_bundle, vba_record_layout_for_fields,
};
use oxvba_com::{
    DynamicCallArg, DynamicCallKind, DynamicCallRequest, DynamicMemberSelector, DynamicValue,
    TypeLibMemberInvokeKind,
};
use oxvba_eval::{
    arith,
    collection::{CollectionError, CollectionMethod, dispatch_collection},
};
use oxvba_hal::HostServices;
use oxvba_hal::traits::DynLinkDescriptorView;
use oxvba_oxir::inst::OxAsNew;
use oxvba_oxir::ty::ClassId;
use oxvba_oxir::{
    ArithOp, ArrayShape, BlockId, BoundWhich, CmpOp, ErrField, ErrorHandler, FuncId, LogicalOp,
    ObjClass, OxArg, OxCallArg, OxClass, OxClassMethod, OxCoerceTarget, OxConst, OxFunc, OxInst,
    OxLocal, OxNativeCallee, OxOperand, OxPlace, OxProgram, OxTerminator, OxTy,
};
use oxvba_rt_abi::{
    EventBinding, ExecState, Fault, LoadedProgram, RT_ARITH_ADD, RT_ARITH_DIV, RT_ARITH_INT_DIV,
    RT_ARITH_MOD, RT_ARITH_MUL, RT_ARITH_POW, RT_ARITH_SUB, RT_COMPARE_EQ, RT_COMPARE_GE,
    RT_COMPARE_GT, RT_COMPARE_LE, RT_COMPARE_LT, RT_COMPARE_NE, RT_ERR_FIELD_DESCRIPTION,
    RT_ERR_FIELD_HELP_CONTEXT, RT_ERR_FIELD_HELP_FILE, RT_ERR_FIELD_LAST_DLL_ERROR,
    RT_ERR_FIELD_NUMBER, RT_ERR_FIELD_SOURCE, RT_ERROR_HANDLER_GOTO_0, RT_ERROR_HANDLER_GOTO_LABEL,
    RT_ERROR_HANDLER_GOTO_MINUS_1, RT_ERROR_HANDLER_RESUME_NEXT, RT_FAULT_DISP_HANDLER,
    RT_FAULT_DISP_RESUME_NEXT, RT_FAULT_DISP_UNWIND, RT_LOGIC_AND, RT_LOGIC_EQV, RT_LOGIC_IMP,
    RT_LOGIC_OR, RT_LOGIC_XOR, RT_NUMERIC_CHECKED_BOOLEAN, RT_NUMERIC_CHECKED_BYTE,
    RT_NUMERIC_CHECKED_CURRENCY, RT_NUMERIC_CHECKED_DATE, RT_NUMERIC_CHECKED_DOUBLE,
    RT_NUMERIC_CHECKED_INTEGER, RT_NUMERIC_CHECKED_LONG, RT_NUMERIC_CHECKED_LONGLONG,
    RT_NUMERIC_CHECKED_SINGLE, RT_NUMERIC_WIDENING, RT_RESUME_LABEL, RT_RESUME_NEXT,
    RT_RESUME_SAME, RT_STRING_COMPARE_BINARY, RT_STRING_COMPARE_TEXT, RawExecState,
    RtSavedErrState, ST_FAULT, ST_HALT, ST_OK, exec_state_as_raw, rt_add_i16, rt_add_i32,
    rt_add_i64, rt_add_u8, rt_arith_v, rt_clear_proc_invoker, rt_coerce_fixed_string_v,
    rt_coerce_numeric_v, rt_coerce_string_v, rt_compare_v, rt_currency_add, rt_currency_mul,
    rt_currency_sub, rt_div_i32, rt_div_i64, rt_erl_get, rt_err_clear, rt_err_enter_activation,
    rt_err_i32_field, rt_err_restore_activation, rt_err_set_field, rt_err_string_field_utf8,
    rt_install_proc_invoker, rt_lib_invoke_with_policy, rt_logical_v, rt_maybe_drain, rt_mul_i16,
    rt_mul_i32, rt_mul_i64, rt_mul_u8, rt_neg_v, rt_not_v, rt_project_new_object,
    rt_project_predeclared_instance, rt_project_set_predeclared_instance,
    rt_raise_array_has_no_bounds, rt_raise_error_number, rt_raise_expected_array,
    rt_raise_fixed_or_temporarily_locked_array, rt_raise_invalid_proc_ref, rt_raise_out_of_memory,
    rt_raise_out_of_stack, rt_raise_runtime_error_number, rt_raise_subscript_out_of_range,
    rt_raise_type_mismatch, rt_rem_i32, rt_rem_i64, rt_resume, rt_route_fault,
    rt_set_error_handler, rt_sub_i16, rt_sub_i32, rt_sub_i64, rt_sub_u8, rt_truthy_v,
    runtime_class_descriptors_for_program, variant_changed,
};
use oxvba_runtime::{
    Decimal96, InteropInvokeKind, RUNTIME_CLASS_LIFECYCLE_NONE, RuntimeByRefSlot,
    RuntimeClassDescriptor, VarType, Variant, VbaRecord, VerifiedInteropPlan,
    object_ref::{ObjectRef, RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR},
    safe_array::{
        SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
        VT_DECIMAL_VALUE, VT_DISPATCH_VALUE, VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE, VT_R4_VALUE,
        VT_R8_VALUE, VT_RECORD_VALUE, VT_UI1_VALUE, VT_VARIANT_VALUE,
    },
    variant_to_vba_string,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use thiserror::Error;

mod admission;
mod consts;
mod error;
#[macro_use]
mod runtime;
mod helpers;
mod lowering;

pub use crate::consts::JIT_NOT_IMPLEMENTED_MESSAGE;
pub use crate::error::{JitError, JitFinalErr, JitOutcome};
pub use crate::runtime::{CompiledImage, JitEngine};

pub(crate) use crate::admission::*;
pub(crate) use crate::consts::*;
pub(crate) use crate::helpers::*;
pub(crate) use crate::lowering::*;
pub(crate) use crate::runtime::*;

#[cfg(test)]
mod tests;
