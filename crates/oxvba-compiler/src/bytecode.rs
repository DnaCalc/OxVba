use rkyv::{Archive, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum StringCompareMode {
    Binary,
    Text,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalCallDescriptor {
    pub descriptor_id: u32,
    pub declared_name: String,
    pub library: String,
    pub alias: String,
    pub ordinal_alias: bool,
    pub symbol: i32,
    pub marshal_lane: String,
    pub calling_convention: String,
    pub selection_policy: String,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize, PartialEq, Eq)]
pub enum Instruction {
    LoadConstI32 {
        slot: usize,
        value: i32,
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
    CopySlot {
        dst: usize,
        src: usize,
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
    IntrinsicTypeNameTag {
        dst: usize,
        src: usize,
    },
    IntrinsicIsNumericTag {
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
        arg: usize,
    },
    IntrinsicInvokeSymbolHost {
        dst: usize,
        descriptor_id: u32,
        symbol: i32,
        arg: usize,
    },
    CmpEqSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    CmpNeSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    CmpLtSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    CmpLeSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    CmpGtSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    CmpGeSlots {
        dst: usize,
        lhs: usize,
        rhs: usize,
    },
    LoadErrNumber {
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
