use rkyv::{Archive, Deserialize, Serialize};

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
    IntrinsicInStrDigits {
        dst: usize,
        haystack: usize,
        needle: usize,
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
    RaiseError {
        code: i32,
    },
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
