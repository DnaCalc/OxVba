use oxvba_runtime::DynLinkSymbol;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum StringCompareMode {
    Binary,
    Text,
}

/// Target of a fixed-width integer narrowing coercion (`CoerceNumeric`). VBA arithmetic on
/// fixed integer types and assignment into a declared fixed integer target range-check against
/// these widths, raising run-time error 6 ("Overflow") when the value does not fit.
#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum NumericCoerceTarget {
    Byte,
    Integer,
    Long,
    LongLong,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeclareParamType {
    Long,
    Integer,
    String,
    Boolean,
    Double,
    Single,
    Currency,
    Date,
    Byte,
    LongLong,
    LongPtr,
    Variant,
    Any,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalCallDescriptor {
    pub descriptor_id: u32,
    pub declared_name: String,
    pub library: String,
    pub alias: String,
    pub ordinal_alias: bool,
    pub symbol: DynLinkSymbol,
    pub marshal_lane: String,
    pub calling_convention: String,
    pub selection_policy: String,
    pub param_count: usize,
    pub param_types: Vec<DeclareParamType>,
    pub param_by_ref: Vec<bool>,
    pub return_type: Option<DeclareParamType>,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExternalCallWritebackKind {
    ByRefValue,
    PointerByteArrayPayload,
    PointerStringPayload,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalCallWriteback {
    pub arg_index: usize,
    pub source_slot: usize,
    pub kind: ExternalCallWritebackKind,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchInvokeArg {
    pub slot: Option<usize>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProjectMemberCallKind {
    Method,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectMemberCallDescriptor {
    pub lowered_name: String,
    pub kind: ProjectMemberCallKind,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComMemberSelectorDescriptor {
    DispatchId(i32),
    Name(String),
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComMemberCallDescriptor {
    pub selector: ComMemberSelectorDescriptor,
    pub arity: usize,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeAssignmentIntent {
    Implicit,
    Let,
    Set,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeAssignmentTargetKind {
    Variant,
    Object,
    Scalar,
}

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeArrayElementType {
    Variant,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Byte,
    Single,
    Double,
    Currency,
    Date,
    String,
    Boolean,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum Instruction {
    LoadConstI32 {
        slot: usize,
        value: i32,
    },
    LoadConstI64 {
        slot: usize,
        value: i64,
    },
    LoadConstBool {
        slot: usize,
        value: bool,
    },
    LoadConstString {
        slot: usize,
        value: String,
    },
    LoadConstF64 {
        slot: usize,
        bits: u64,
    },
    LoadConstCurrency {
        slot: usize,
        scaled: i64,
    },
    LoadConstDate {
        slot: usize,
        bits: u64,
    },
    AddConstI32 {
        slot: usize,
        value: i32,
    },
    AddSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    SubConstI32 {
        slot: usize,
        value: i32,
    },
    SubSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    MulSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    DivSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    IntDivSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    ModSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    PowSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    ConcatSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    NegSlot {
        dst: usize,
        src: usize,
    },
    CopySlot {
        dst: usize,
        src: usize,
    },
    /// Narrows the numeric value in `slot` to the fixed-width integer type `target`, in place.
    /// Rounds (round-half-to-even, like `CLng`/`CInt`) if the value is fractional, then
    /// range-checks: out of range raises run-time error 6 ("Overflow"); in range, the slot is
    /// retagged to the target type. Emitted for fixed-type integer arithmetic results and for
    /// assignments into declared fixed integer targets so overflow matches VBA/Excel.
    CoerceNumeric {
        slot: usize,
        target: NumericCoerceTarget,
    },
    /// Materialises a pure-VBA project-class instance as a reference-counted `Object`
    /// Variant. `handle` is a slot holding the instance identity (`i32`); the VM resolves the
    /// route's `ObjectRef` and boxes a retained clone into `dst`. Lowered from
    /// `New <ProjectClass>` so the slot carries a real `IUnknown` reference (AddRef/Release on
    /// Set/scope) rather than a bare integer handle.
    LoadProjectObjectRef {
        dst: usize,
        handle: usize,
    },
    IntrinsicLenDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicLeftDigits {
        dst: usize,
        src: usize,
        count: usize,
    },
    IntrinsicRightDigits {
        dst: usize,
        src: usize,
        count: usize,
    },
    IntrinsicMidDigits {
        dst: usize,
        src: usize,
        start: usize,
        count: Option<usize>,
    },
    IntrinsicMidStmtDigits {
        target: usize,
        start: usize,
        count: Option<usize>,
        value: usize,
    },
    IntrinsicInStrDigits {
        dst: usize,
        haystack: usize,
        needle: usize,
        mode: StringCompareMode,
    },
    IntrinsicInStrRevDigits {
        dst: usize,
        haystack: usize,
        needle: usize,
        mode: StringCompareMode,
    },
    IntrinsicLowerDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicUpperDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicSplitCountDigits {
        dst: usize,
        src: usize,
        delimiter: usize,
    },
    IntrinsicJoinDigits {
        dst: usize,
        src: usize,
        delimiter: usize,
    },
    IntrinsicReplaceDigits {
        dst: usize,
        src: usize,
        find: usize,
        replace: usize,
    },
    IntrinsicTrimDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicLTrimDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicRTrimDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicStrCompDigits {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    IntrinsicLikeDigits {
        dst: usize,
        lhs: usize,
        pattern: usize,
        mode: StringCompareMode,
    },
    IntrinsicDateSerialDigits {
        dst: usize,
        year: usize,
        month: usize,
        day: usize,
    },
    IntrinsicTimeSerialDigits {
        dst: usize,
        hour: usize,
        minute: usize,
        second: usize,
    },
    IntrinsicDateValueDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicTimeValueDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicDateAddDigits {
        dst: usize,
        interval: usize,
        number: usize,
        date: usize,
    },
    IntrinsicDateDiffDigits {
        dst: usize,
        interval: usize,
        date1: usize,
        date2: usize,
    },
    IntrinsicYearDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicMonthDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicDayDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicDateNowHost {
        dst: usize,
    },
    IntrinsicTimeNowHost {
        dst: usize,
    },
    IntrinsicNowHost {
        dst: usize,
    },
    IntrinsicTimerHost {
        dst: usize,
    },
    IntrinsicFreeFileHost {
        dst: usize,
        range_selector: Option<usize>,
    },
    IntrinsicFileOpenHost {
        dst: usize,
        path: usize,
        mode: usize,
        file_number: usize,
    },
    IntrinsicFileCloseHost {
        dst: usize,
        handle: usize,
    },
    IntrinsicFileKillHost {
        dst: usize,
        path: usize,
    },
    IntrinsicFileReadHost {
        dst: usize,
        handle: usize,
        count: usize,
    },
    IntrinsicFileWriteHost {
        dst: usize,
        handle: usize,
        data: usize,
    },
    IntrinsicFilePrintHost {
        dst: usize,
        handle: usize,
        data: usize,
    },
    IntrinsicConsolePrintHost {
        dst: usize,
        data: usize,
    },
    IntrinsicFileInputHost {
        dst: usize,
        handle: usize,
        count: usize,
    },
    IntrinsicConsoleInputHost {
        dst: usize,
        count: usize,
    },
    IntrinsicFileLineInputHost {
        dst: usize,
        handle: usize,
    },
    IntrinsicConsoleLineInputHost {
        dst: usize,
    },
    IntrinsicFileEofHost {
        dst: usize,
        handle: usize,
    },
    IntrinsicFileLofHost {
        dst: usize,
        handle: usize,
    },
    IntrinsicFileSeekHost {
        dst: usize,
        handle: usize,
    },
    IntrinsicFileLocHost {
        dst: usize,
        handle: usize,
    },
    IntrinsicMsgBoxHost {
        dst: usize,
        prompt: usize,
        style: Option<usize>,
    },
    IntrinsicInputBoxHost {
        dst: usize,
        prompt: usize,
        default_value: Option<usize>,
    },
    IntrinsicBeepHost {
        dst: usize,
    },
    IntrinsicDebugPrintHost {
        dst: usize,
        data: usize,
    },
    IntrinsicStrPtr {
        dst: usize,
        src: usize,
    },
    IntrinsicVarPtr {
        dst: usize,
        src: usize,
    },
    IntrinsicVarPtrStringVar {
        dst: usize,
        src: usize,
    },
    IntrinsicVarPtrVariantVar {
        dst: usize,
        src: usize,
    },
    IntrinsicObjPtr {
        dst: usize,
        src: usize,
    },
    IntrinsicDoEventsHost {
        dst: usize,
    },
    IntrinsicAbsI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicIntI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicFixI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicSgnI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicRoundI32 {
        dst: usize,
        src: usize,
        digits: Option<usize>,
    },
    IntrinsicSqrI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicSinI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicCosI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicLogI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicExpI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicAtnI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicTanI32 {
        dst: usize,
        src: usize,
    },
    IntrinsicChrDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicAscDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicSpaceDigits {
        dst: usize,
        count: usize,
    },
    IntrinsicStringRepeatDigits {
        dst: usize,
        count: usize,
        ch: usize,
    },
    IntrinsicHexDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicOctDigits {
        dst: usize,
        src: usize,
    },
    /// CStr() — convert value to string (no leading space).
    IntrinsicCStrDigits {
        dst: usize,
        src: usize,
    },
    /// Str() — convert numeric value to string (leading space for positive).
    IntrinsicStrFuncDigits {
        dst: usize,
        src: usize,
    },
    /// Val() — parse numeric value from string.
    IntrinsicValDigits {
        dst: usize,
        src: usize,
    },
    /// CDate() — convert value to a Date-subtyped runtime value.
    IntrinsicCDateValue {
        dst: usize,
        src: usize,
    },
    IntrinsicStrConvDigits {
        dst: usize,
        src: usize,
        conversion: usize,
    },
    IntrinsicRndDigits {
        dst: usize,
        seed: Option<usize>,
    },
    IntrinsicRandomizeDigits {
        dst: usize,
        seed: Option<usize>,
    },
    IntrinsicFormatDigits {
        dst: usize,
        value: usize,
        format_string: Option<usize>,
    },
    IntrinsicStrReverseDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicWeekdayDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicMonthNameDigits {
        dst: usize,
        src: usize,
    },
    IntrinsicFvI32 {
        dst: usize,
        rate: usize,
        nper: usize,
        pmt: usize,
        pv: Option<usize>,
        due: Option<usize>,
    },
    IntrinsicPvI32 {
        dst: usize,
        rate: usize,
        nper: usize,
        pmt: usize,
        fv: Option<usize>,
        due: Option<usize>,
    },
    IntrinsicPmtI32 {
        dst: usize,
        rate: usize,
        nper: usize,
        pv: usize,
        fv: Option<usize>,
        due: Option<usize>,
    },
    IntrinsicNpvI32 {
        dst: usize,
        rate: usize,
        values: Vec<usize>,
    },
    IntrinsicIrrI32 {
        dst: usize,
        value: usize,
        guess: Option<usize>,
    },
    IntrinsicMirrI32 {
        dst: usize,
        value: usize,
        finance_rate: usize,
        reinvest_rate: usize,
    },
    IntrinsicRateI32 {
        dst: usize,
        nper: usize,
        pmt: usize,
        pv: usize,
        fv: Option<usize>,
        due: Option<usize>,
        guess: Option<usize>,
    },
    IntrinsicNPerI32 {
        dst: usize,
        rate: usize,
        pmt: usize,
        pv: usize,
        fv: Option<usize>,
        due: Option<usize>,
    },
    IntrinsicArrayLiteral {
        dst: usize,
        values: Vec<usize>,
    },
    IntrinsicArrayAppend {
        dst: usize,
        array: usize,
        item: usize,
    },
    IntrinsicArrayResize {
        dst: usize,
        upper_bounds: Vec<usize>,
        lower_bounds: Vec<i32>,
        element_type: RuntimeArrayElementType,
    },
    IntrinsicArrayResizePreserve {
        dst: usize,
        upper_bounds: Vec<usize>,
        lower_bounds: Vec<i32>,
        element_type: RuntimeArrayElementType,
    },
    IntrinsicArrayGet {
        dst: usize,
        array: usize,
        indices: Vec<usize>,
    },
    IntrinsicArraySet {
        array: usize,
        indices: Vec<usize>,
        src: usize,
    },
    IntrinsicForEachInit {
        iter: usize,
        src: usize,
    },
    IntrinsicForEachNext {
        iter: usize,
        item: usize,
        has_value: usize,
    },
    IntrinsicLBoundArray {
        dst: usize,
        src: usize,
    },
    IntrinsicUBoundArray {
        dst: usize,
        src: usize,
    },
    IntrinsicIsArrayTag {
        dst: usize,
        src: usize,
    },
    IntrinsicVarTypeTag {
        dst: usize,
        src: usize,
    },
    /// Typed VarType: returns standard VBA type codes from the retained value slot.
    IntrinsicVarType {
        dst: usize,
        src: usize,
    },
    IntrinsicTypeNameTag {
        dst: usize,
        src: usize,
    },
    IntrinsicIsNumericTag {
        dst: usize,
        src: usize,
    },
    /// Typed IsNumeric: checks whether the retained value slot is numeric.
    IntrinsicIsNumeric {
        dst: usize,
        src: usize,
    },
    /// Typed IsError: checks whether the retained value slot is an error value.
    IntrinsicIsError {
        dst: usize,
        src: usize,
    },
    /// CVErr(value): materializes a retained Error variant from a numeric code.
    IntrinsicCVErr {
        dst: usize,
        src: usize,
    },
    IntrinsicIsDateTag {
        dst: usize,
        src: usize,
    },
    IntrinsicIsObjectTag {
        dst: usize,
        src: usize,
    },
    ValidateRuntimeAssignment {
        src: usize,
        intent: RuntimeAssignmentIntent,
        target_kind: RuntimeAssignmentTargetKind,
        target_name: String,
        target_type_name: String,
    },
    IntrinsicShellHost {
        dst: usize,
        command: usize,
    },
    IntrinsicEnvironHost {
        dst: usize,
        key: usize,
    },
    IntrinsicDirHost {
        dst: usize,
        path: usize,
    },
    IntrinsicCollectionAdd {
        dst: usize,
        count: usize,
        item: usize,
    },
    IntrinsicCollectionItem {
        dst: usize,
        count: usize,
        index: usize,
    },
    IntrinsicCollectionRemove {
        dst: usize,
        count: usize,
        index: usize,
    },
    IntrinsicCollectionCount {
        dst: usize,
        count: usize,
    },
    IntrinsicCreateObjectHost {
        dst: usize,
        prog_id: usize,
    },
    IntrinsicDispatchInvokeHost {
        dst: usize,
        object: usize,
        member: usize,
        args: Vec<DispatchInvokeArg>,
        early_bound: bool,
        com_member: Option<ComMemberCallDescriptor>,
        call_kind_hint: Option<ProjectMemberCallKind>,
    },
    IntrinsicComSubscribeEventHost {
        dst: usize,
        object: usize,
        event: usize,
    },
    IntrinsicComUnsubscribeEventHost {
        dst: usize,
        subscription: usize,
    },
    IntrinsicComEventCallbackSubscriptionHost {
        dst: usize,
        callback: usize,
    },
    IntrinsicComEventCallbackArgHost {
        dst: usize,
        callback: usize,
        index: usize,
    },
    IntrinsicComReleaseEventCallbackHost {
        dst: usize,
        callback: usize,
    },
    IntrinsicInvokeSymbolHost {
        dst: usize,
        descriptor_id: u32,
        symbol: DynLinkSymbol,
        args: Vec<usize>,
        writeback_slots: Vec<ExternalCallWriteback>,
    },
    IntrinsicWithEventsGet {
        dst: usize,
        owner: usize,
        binding: usize,
    },
    IntrinsicWithEventsSet {
        dst: usize,
        owner: usize,
        binding: usize,
        value: usize,
    },
    IntrinsicWithEventsClearOwner {
        dst: usize,
        owner: usize,
    },
    IntrinsicWithEventsFirstOwner {
        dst: usize,
        source: usize,
        binding: usize,
    },
    IntrinsicWithEventsNextOwner {
        dst: usize,
    },
    CmpEqSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    CmpNeSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    CmpLtSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    CmpLeSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    CmpGtSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    CmpGeSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: StringCompareMode,
    },
    /// VBA object identity comparison (`lhs Is rhs`). This is distinct from
    /// value equality and treats `Nothing` as the null object identity.
    CmpObjectIsSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    LoadErrNumber {
        slot: usize,
    },
    LoadErrDescription {
        slot: usize,
    },
    LoadErrSource {
        slot: usize,
    },
    /// TypeOf x Is TypeName — checks class identity and Implements list.
    IntrinsicTypeOfIs {
        dst: usize,
        object_slot: usize,
        type_name: String,
    },
    /// Identity check: tests whether a value is Null (not equality — Null = Null
    /// yields Null under VBA semantics, so IsNull needs a dedicated path).
    IntrinsicIsNull {
        dst: usize,
        src: usize,
    },
    /// Identity check: tests whether a value is Empty (uninitialized Variant).
    IntrinsicIsEmpty {
        dst: usize,
        src: usize,
    },
    /// Loads a retained Null value into a slot — distinct from `LoadConstI32 { value: -1 }`
    /// which produces an integer -1 carrier.
    LoadNull {
        slot: usize,
    },
    /// Loads an uninitialized Variant/Empty value into a slot.
    LoadEmpty {
        slot: usize,
    },
    BoolNot {
        dst: usize,
        src: usize,
    },
    BoolAnd {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    BoolOr {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    SetOnErrorResumeNext,
    SetOnErrorGoto0,
    SetOnErrorGotoLabel {
        target_pc: usize,
    },
    ResumeNext,
    Resume,
    ResumeLabel {
        target_pc: usize,
    },
    RaiseError {
        code: i32,
    },
    ClearErr,
    CallProc {
        target_pc: usize,
        project_member: Option<ProjectMemberCallDescriptor>,
    },
    Return,
    JumpIfZero {
        cond_slot: usize,
        target_pc: usize,
    },
    Jump {
        target_pc: usize,
    },
    IncSlot {
        slot: usize,
    },
    Halt,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct Bytecode {
    pub instructions: Vec<Instruction>,
    pub external_call_descriptors: Vec<ExternalCallDescriptor>,
    pub slot_count: usize,
    pub user_slot_count: usize,
}

impl Bytecode {
    pub fn zero_copy_hint_enabled() -> bool {
        cfg!(feature = "mach_zero_copy_bytecode")
    }
}

#[cfg(test)]
mod tests {
    use super::Bytecode;

    #[test]
    fn zero_copy_hint_flag_is_stable() {
        let _ = Bytecode::zero_copy_hint_enabled();
    }
}
