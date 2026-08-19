//! Shared JIT constants, descriptors, and entry ABI.

use super::*;

pub const JIT_NOT_IMPLEMENTED_MESSAGE: &str =
    "JIT execution is not implemented for this OxIR shape";

pub(crate) const AREA_GLOBAL: u32 = 0;
pub(crate) const AREA_LOCAL: u32 = 1;
pub(crate) const AREA_TEMP: u32 = 2;
pub(crate) const MAX_JIT_FRAMES: usize = 50_000;
pub(crate) const MISSING_ARG: i32 = 0x8002_0004u32 as i32;
pub(crate) const JIT_CALL_ARG_DESC_SIZE: u32 = 24;
pub(crate) const JIT_CALL_ARG_AUX_OFFSET: i32 = 4;
pub(crate) const JIT_CALL_ARG_VALUE_OFFSET: i32 = 8;
pub(crate) const JIT_CALL_ARG_AREA_OFFSET: i32 = 16;
pub(crate) const JIT_CALL_ARG_INDEX_OFFSET: i32 = 20;
pub(crate) const JIT_CALL_ARG_NAME_DESC_SIZE: u32 = 16;
pub(crate) const JIT_CALL_ARG_NAME_LEN_OFFSET: i32 = 8;
pub(crate) const JIT_SLOT_ALIAS_DESC_SIZE: u32 = 8;
pub(crate) const JIT_SLOT_ALIAS_INDEX_OFFSET: i32 = 4;
pub(crate) const JIT_I32_STACK_ELEM_SIZE: u32 = 4;
pub(crate) const JIT_CALL_ARG_BYVAL_SCALAR: i32 = 0;
pub(crate) const JIT_CALL_ARG_BYREF_ALIAS: i32 = 1;
pub(crate) const JIT_CALL_ARG_BYVAL_VARIANT: i32 = 2;
pub(crate) const JIT_CALL_ARG_OMITTED: i32 = 3;
pub(crate) const JIT_CALL_ARG_BYREF_COPY: i32 = 4;
pub(crate) const JIT_PROC_REF_RET_NONE: i32 = 0;
pub(crate) const JIT_PROC_REF_RET_LONG: i32 = 1;
pub(crate) const JIT_PROC_REF_RET_STRING: i32 = 2;
pub(crate) const JIT_PROC_REF_RET_VARIANT: i32 = 3;
pub(crate) const JIT_PROC_REF_RET_EXACT_LONGLONG: i32 = 100;
pub(crate) const JIT_PROC_REF_RET_EXACT_CURRENCY: i32 = 101;
pub(crate) const JIT_PROC_REF_RET_EXACT_SINGLE: i32 = 102;
pub(crate) const JIT_PROC_REF_RET_EXACT_DOUBLE: i32 = 103;
pub(crate) const JIT_PROC_REF_RET_EXACT_DATE: i32 = 104;
pub(crate) const JIT_PROC_REF_RET_EXACT_BYTE: i32 = 105;
pub(crate) const JIT_PROC_REF_RET_EXACT_INTEGER: i32 = 106;
pub(crate) const VBA_COLLECTION_ROUTE_KEY: i32 = i32::MIN;

pub(crate) static VBA_COLLECTION_DESCRIPTOR: RuntimeClassDescriptor = RuntimeClassDescriptor {
    name: "Collection",
    project_identity: None,
    predeclared: false,
    lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
    fields: &[],
    as_new_fields: &[],
    implements: &[],
    interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR],
};
pub(crate) const JIT_PROC_REF_RET_EXACT_BOOL: i32 = 107;
pub(crate) const JIT_VARIANT_OPERAND_PLACE: i32 = 0;
pub(crate) const JIT_VARIANT_OPERAND_EMPTY: i32 = 1;
pub(crate) const JIT_VARIANT_OPERAND_NULL: i32 = 2;
pub(crate) const JIT_VARIANT_OPERAND_BOOL: i32 = 3;
pub(crate) const JIT_VARIANT_OPERAND_I16: i32 = 4;
pub(crate) const JIT_VARIANT_OPERAND_I32: i32 = 5;
pub(crate) const JIT_VARIANT_OPERAND_I64: i32 = 6;
pub(crate) const JIT_VARIANT_OPERAND_F32: i32 = 7;
pub(crate) const JIT_VARIANT_OPERAND_F64: i32 = 8;
pub(crate) const JIT_VARIANT_OPERAND_CURRENCY: i32 = 9;
pub(crate) const JIT_VARIANT_OPERAND_DATE: i32 = 10;
pub(crate) const JIT_VARIANT_OPERAND_STR_UTF8: i32 = 11;
pub(crate) const JIT_VARIANT_OPERAND_NOTHING: i32 = 12;
pub(crate) const JIT_ASSIGN_INTENT_IMPLICIT: i32 = 0;
pub(crate) const JIT_ASSIGN_INTENT_LET: i32 = 1;
pub(crate) const JIT_ASSIGN_INTENT_SET: i32 = 2;
pub(crate) const JIT_ASSIGN_TARGET_VARIANT: i32 = 0;
pub(crate) const JIT_ASSIGN_TARGET_OBJECT: i32 = 1;
pub(crate) const JIT_ASSIGN_TARGET_SCALAR: i32 = 2;

pub(crate) type JitEntryFn = unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32;
