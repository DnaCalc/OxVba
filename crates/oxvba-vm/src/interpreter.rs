use std::{collections::HashMap, sync::Arc};

use oxvba_compiler::{Bytecode, Instruction, bytecode::StringCompareMode};
use oxvba_hal::{
    adapters,
    error::{HalError, HalErrorKind},
    model::{CapabilityId, HalProfileId, HostPolicy},
    traits::{DynLinkDescriptorView, HostServices},
};
use oxvba_runtime::safe_array::{
    array_len_from_tag, is_array_tag as runtime_is_array_tag, marshal_dispatch_argument,
};
use oxvba_runtime::value_tags::{
    EMPTY_TAG, NULL_TAG, error_tag_from_code, is_error_tag as runtime_is_error_tag,
};

use crate::register_file::RegisterFile;

pub struct Vm {
    registers: RegisterFile,
    host_services: Arc<dyn HostServices>,
    call_stack: Vec<usize>,
    withevents_bindings: HashMap<i64, i32>,
    on_error_resume_next: bool,
    on_error_goto_label_target: Option<usize>,
    last_error: i32,
    last_error_pc: Option<usize>,
}

const FIN_MAX_ITERS: usize = 60;
const FIN_EPS: f64 = 1e-10;
const FIN_DERIVATIVE_STEP: f64 = 1e-7;
const FIN_RATE_ERROR_CODE: i32 = 2001;
const FIN_NPER_ERROR_CODE: i32 = 2002;

fn default_host_services() -> Arc<dyn HostServices> {
    adapters::for_profile(HalProfileId::Windows, HostPolicy::deterministic_runtime())
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(default_host_services())
    }
}

impl Vm {
    pub fn new(host_services: Arc<dyn HostServices>) -> Self {
        Self {
            registers: RegisterFile::with_capacity(256),
            host_services,
            call_stack: Vec::new(),
            withevents_bindings: HashMap::new(),
            on_error_resume_next: false,
            on_error_goto_label_target: None,
            last_error: 0,
            last_error_pc: None,
        }
    }

    fn clear_error_state(&mut self) {
        self.last_error = 0;
        self.last_error_pc = None;
    }

    fn route_runtime_error(
        &mut self,
        pc: usize,
        code: i32,
        detail: Option<&str>,
    ) -> Result<usize, String> {
        self.last_error = code;
        self.last_error_pc = Some(pc);
        if self.on_error_resume_next {
            return Ok(pc + 1);
        }
        if let Some(target_pc) = self.on_error_goto_label_target {
            return Ok(target_pc);
        }
        match detail {
            Some(detail) => Err(format!("runtime error: {code} ({detail})")),
            None => Err(format!("runtime error: {code}")),
        }
    }

    fn route_host_error(&mut self, pc: usize, err: HalError) -> Result<usize, String> {
        let code = Self::hal_error_code(err.kind, err.capability);
        let detail = format!("{} [{}] {}", err.stable_code, err.operation, err.message);
        self.route_runtime_error(pc, code, Some(detail.as_str()))
    }

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

    fn ensure_slot_count(&mut self, slot_count: usize) {
        if slot_count > self.registers.registers.len() {
            self.registers.registers.resize(slot_count, 0);
        }
    }

    pub fn snapshot_slots(&self, slot_count: usize) -> Vec<i32> {
        let end = slot_count.min(self.registers.registers.len());
        self.registers.registers[..end].to_vec()
    }

    pub fn execute(&mut self, bytecode: &Bytecode) -> Result<(), String> {
        let typed_fastpaths = Self::typed_fastpaths_enabled_from_env();
        self.execute_with_typed_fastpaths(bytecode, typed_fastpaths)
    }

    pub fn execute_with_typed_fastpaths(
        &mut self,
        bytecode: &Bytecode,
        typed_fastpaths: bool,
    ) -> Result<(), String> {
        self.ensure_slot_count(bytecode.slot_count);
        self.withevents_bindings.clear();
        let mut pc = 0usize;
        let len = bytecode.instructions.len();

        while pc < len {
            match &bytecode.instructions[pc] {
                Instruction::LoadConstI32 { slot, value } => {
                    self.write_slot(*slot, *value)?;
                    pc += 1;
                }
                Instruction::AddConstI32 { slot, value } => {
                    if typed_fastpaths && self.fast_add_const(*slot, *value) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_slot(*slot)?;
                    self.write_slot(*slot, lhs + *value)?;
                    pc += 1;
                }
                Instruction::AddSlots { dst, lhs, rhs } => {
                    let lhs = self.read_slot(*lhs)?;
                    let rhs = self.read_slot(*rhs)?;
                    self.write_slot(*dst, lhs + rhs)?;
                    pc += 1;
                }
                Instruction::SubConstI32 { slot, value } => {
                    if typed_fastpaths && self.fast_sub_const(*slot, *value) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_slot(*slot)?;
                    self.write_slot(*slot, lhs - *value)?;
                    pc += 1;
                }
                Instruction::CopySlot { dst, src } => {
                    if typed_fastpaths && self.fast_copy_slot(*dst, *src) {
                        pc += 1;
                        continue;
                    }
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicLenDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::len_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicLeftDigits { dst, src, count } => {
                    let value = self.read_slot(*src)?;
                    let count = self.read_slot(*count)?;
                    self.write_slot(*dst, Self::left_digits(value, count))?;
                    pc += 1;
                }
                Instruction::IntrinsicRightDigits { dst, src, count } => {
                    let value = self.read_slot(*src)?;
                    let count = self.read_slot(*count)?;
                    self.write_slot(*dst, Self::right_digits(value, count))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidDigits {
                    dst,
                    src,
                    start,
                    count,
                } => {
                    let value = self.read_slot(*src)?;
                    let start = self.read_slot(*start)?;
                    let count = match count {
                        Some(slot) => Some(self.read_slot(*slot)?),
                        None => None,
                    };
                    self.write_slot(*dst, Self::mid_digits(value, start, count))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidStmtDigits {
                    target,
                    start,
                    count,
                    value,
                } => {
                    let target_value = self.read_slot(*target)?;
                    let start = self.read_slot(*start)?;
                    let count = match count {
                        Some(slot) => Some(self.read_slot(*slot)?),
                        None => None,
                    };
                    let value = self.read_slot(*value)?;
                    self.write_slot(
                        *target,
                        Self::mid_stmt_digits(target_value, start, count, value),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrDigits {
                    dst,
                    haystack,
                    needle,
                    mode,
                } => {
                    let haystack = self.read_slot(*haystack)?;
                    let needle = self.read_slot(*needle)?;
                    self.write_slot(*dst, Self::instr_digits(haystack, needle, *mode))?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrRevDigits {
                    dst,
                    haystack,
                    needle,
                    mode,
                } => {
                    let haystack = self.read_slot(*haystack)?;
                    let needle = self.read_slot(*needle)?;
                    self.write_slot(*dst, Self::instrrev_digits(haystack, needle, *mode))?;
                    pc += 1;
                }
                Instruction::IntrinsicLowerDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::to_lower_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicUpperDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::to_upper_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicSplitCountDigits {
                    dst,
                    src,
                    delimiter,
                } => {
                    let value = self.read_slot(*src)?;
                    let delimiter = self.read_slot(*delimiter)?;
                    self.write_slot(*dst, Self::split_count_digits(value, delimiter))?;
                    pc += 1;
                }
                Instruction::IntrinsicJoinDigits {
                    dst,
                    src,
                    delimiter,
                } => {
                    let value = self.read_slot(*src)?;
                    let delimiter = self.read_slot(*delimiter)?;
                    self.write_slot(*dst, Self::join_digits(value, delimiter))?;
                    pc += 1;
                }
                Instruction::IntrinsicReplaceDigits {
                    dst,
                    src,
                    find,
                    replace,
                } => {
                    let value = self.read_slot(*src)?;
                    let find = self.read_slot(*find)?;
                    let replace = self.read_slot(*replace)?;
                    self.write_slot(*dst, Self::replace_digits(value, find, replace))?;
                    pc += 1;
                }
                Instruction::IntrinsicTrimDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::trim_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicLTrimDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::ltrim_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicRTrimDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::rtrim_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicStrCompDigits {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    let lhs = self.read_slot(*lhs)?;
                    let rhs = self.read_slot(*rhs)?;
                    self.write_slot(*dst, Self::strcomp_digits(lhs, rhs, *mode))?;
                    pc += 1;
                }
                Instruction::IntrinsicLikeDigits {
                    dst,
                    lhs,
                    pattern,
                    mode,
                } => {
                    let lhs = self.read_slot(*lhs)?;
                    let pattern = self.read_slot(*pattern)?;
                    self.write_slot(*dst, Self::like_digits(lhs, pattern, *mode))?;
                    pc += 1;
                }
                Instruction::IntrinsicDateSerialDigits {
                    dst,
                    year,
                    month,
                    day,
                } => {
                    let year = self.read_slot(*year)?;
                    let month = self.read_slot(*month)?;
                    let day = self.read_slot(*day)?;
                    self.write_slot(*dst, Self::date_serial_digits(year, month, day))?;
                    pc += 1;
                }
                Instruction::IntrinsicTimeSerialDigits {
                    dst,
                    hour,
                    minute,
                    second,
                } => {
                    let hour = self.read_slot(*hour)?;
                    let minute = self.read_slot(*minute)?;
                    let second = self.read_slot(*second)?;
                    self.write_slot(*dst, Self::time_serial_digits(hour, minute, second))?;
                    pc += 1;
                }
                Instruction::IntrinsicDateValueDigits { dst, src } => {
                    let src = self.read_slot(*src)?;
                    self.write_slot(*dst, src)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimeValueDigits { dst, src } => {
                    let src = self.read_slot(*src)?;
                    self.write_slot(*dst, src)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateAddDigits {
                    dst,
                    interval,
                    number,
                    date,
                } => {
                    let interval = self.read_slot(*interval)?;
                    let number = self.read_slot(*number)?;
                    let date = self.read_slot(*date)?;
                    self.write_slot(*dst, Self::date_add_digits(interval, number, date))?;
                    pc += 1;
                }
                Instruction::IntrinsicDateDiffDigits {
                    dst,
                    interval,
                    date1,
                    date2,
                } => {
                    let interval = self.read_slot(*interval)?;
                    let date1 = self.read_slot(*date1)?;
                    let date2 = self.read_slot(*date2)?;
                    self.write_slot(*dst, Self::date_diff_digits(interval, date1, date2))?;
                    pc += 1;
                }
                Instruction::IntrinsicDateNowHost { dst } => {
                    match self.host_services.time_locale().date_serial_now() {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicTimeNowHost { dst } => {
                    match self.host_services.time_locale().time_serial_now() {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicNowHost { dst } => {
                    // Current token model uses date projection for Now().
                    match self.host_services.time_locale().date_serial_now() {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicTimerHost { dst } => {
                    match self.host_services.time_locale().timer_ticks() {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFreeFileHost {
                    dst,
                    range_selector,
                } => {
                    let selector = if let Some(slot) = range_selector {
                        self.read_slot(*slot)?
                    } else {
                        0
                    };
                    match self.host_services.fs().free_file(selector) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicMsgBoxHost { dst, prompt, style } => {
                    let prompt = self.read_slot(*prompt)?;
                    let style = if let Some(slot) = style {
                        self.read_slot(*slot)?
                    } else {
                        1
                    };
                    match self.host_services.ui().msg_box(prompt, style) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicInputBoxHost {
                    dst,
                    prompt,
                    default_value,
                } => {
                    let prompt = self.read_slot(*prompt)?;
                    let default_value = if let Some(slot) = default_value {
                        self.read_slot(*slot)?
                    } else {
                        0
                    };
                    match self.host_services.ui().input_box(prompt, default_value) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDoEventsHost { dst } => {
                    match self.host_services.events().do_events() {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicAbsI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, value.saturating_abs())?;
                    pc += 1;
                }
                Instruction::IntrinsicIntI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicFixI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicSgnI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, value.signum())?;
                    pc += 1;
                }
                Instruction::IntrinsicRoundI32 { dst, src, digits } => {
                    let value = self.read_slot(*src)?;
                    let digits = match digits {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    self.write_slot(*dst, Self::round_i32(value, digits))?;
                    pc += 1;
                }
                Instruction::IntrinsicSqrI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, (value.saturating_abs() as f64).sqrt() as i32)?;
                    pc += 1;
                }
                Instruction::IntrinsicSinI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, (value as f64).sin().round() as i32)?;
                    pc += 1;
                }
                Instruction::IntrinsicCosI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, (value as f64).cos().round() as i32)?;
                    pc += 1;
                }
                Instruction::IntrinsicLogI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(
                        *dst,
                        if value > 0 {
                            (value as f64).ln().round() as i32
                        } else {
                            0
                        },
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicExpI32 { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, (value as f64).exp().round() as i32)?;
                    pc += 1;
                }
                Instruction::IntrinsicFvI32 {
                    dst,
                    rate,
                    nper,
                    pmt,
                    pv,
                    due,
                } => {
                    let rate = self.read_slot(*rate)?;
                    let nper = self.read_slot(*nper)?;
                    let pmt = self.read_slot(*pmt)?;
                    let pv = match pv {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    self.write_slot(*dst, Self::fv_i32(rate, nper, pmt, pv, due))?;
                    pc += 1;
                }
                Instruction::IntrinsicPvI32 {
                    dst,
                    rate,
                    nper,
                    pmt,
                    fv,
                    due,
                } => {
                    let rate = self.read_slot(*rate)?;
                    let nper = self.read_slot(*nper)?;
                    let pmt = self.read_slot(*pmt)?;
                    let fv = match fv {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    self.write_slot(*dst, Self::pv_i32(rate, nper, pmt, fv, due))?;
                    pc += 1;
                }
                Instruction::IntrinsicPmtI32 {
                    dst,
                    rate,
                    nper,
                    pv,
                    fv,
                    due,
                } => {
                    let rate = self.read_slot(*rate)?;
                    let nper = self.read_slot(*nper)?;
                    let pv = self.read_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    self.write_slot(*dst, Self::pmt_i32(rate, nper, pv, fv, due))?;
                    pc += 1;
                }
                Instruction::IntrinsicNpvI32 { dst, rate, values } => {
                    let rate = self.read_slot(*rate)?;
                    let mut cash_flows = Vec::with_capacity(values.len());
                    for slot in values {
                        cash_flows.push(self.read_slot(*slot)?);
                    }
                    self.write_slot(*dst, Self::npv_i32(rate, &cash_flows))?;
                    pc += 1;
                }
                Instruction::IntrinsicIrrI32 { dst, value, guess } => {
                    let value = self.read_slot(*value)?;
                    let guess = match guess {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 10,
                    };
                    self.write_slot(*dst, Self::irr_i32(value, guess))?;
                    pc += 1;
                }
                Instruction::IntrinsicMirrI32 {
                    dst,
                    value,
                    finance_rate,
                    reinvest_rate,
                } => {
                    let value = self.read_slot(*value)?;
                    let finance_rate = self.read_slot(*finance_rate)?;
                    let reinvest_rate = self.read_slot(*reinvest_rate)?;
                    self.write_slot(*dst, Self::mirr_i32(value, finance_rate, reinvest_rate))?;
                    pc += 1;
                }
                Instruction::IntrinsicRateI32 {
                    dst,
                    nper,
                    pmt,
                    pv,
                    fv,
                    due,
                    guess,
                } => {
                    let nper = self.read_slot(*nper)?;
                    let pmt = self.read_slot(*pmt)?;
                    let pv = self.read_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    let guess = match guess {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 10,
                    };
                    self.write_slot(*dst, Self::rate_i32(nper, pmt, pv, fv, due, guess))?;
                    pc += 1;
                }
                Instruction::IntrinsicNPerI32 {
                    dst,
                    rate,
                    pmt,
                    pv,
                    fv,
                    due,
                } => {
                    let rate = self.read_slot(*rate)?;
                    let pmt = self.read_slot(*pmt)?;
                    let pv = self.read_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_slot(*slot)?,
                        None => 0,
                    };
                    self.write_slot(*dst, Self::nper_i32(rate, pmt, pv, fv, due))?;
                    pc += 1;
                }
                Instruction::IntrinsicLBoundArray { dst, src } => {
                    let value = self.read_slot(*src)?;
                    let out = if Self::is_array_tag(value) { 0 } else { -1 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicUBoundArray { dst, src } => {
                    let value = self.read_slot(*src)?;
                    let out = if Self::is_array_tag(value) {
                        Self::array_count(value) - 1
                    } else {
                        -1
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicIsArrayTag { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, if Self::is_array_tag(value) { 1 } else { 0 })?;
                    pc += 1;
                }
                Instruction::IntrinsicVarTypeTag { dst, src } => {
                    let value = self.read_slot(*src)?;
                    let out = if Self::is_array_tag(value) {
                        8192 + 12
                    } else if value == EMPTY_TAG {
                        0
                    } else if value == NULL_TAG {
                        1
                    } else if runtime_is_error_tag(value) {
                        10
                    } else {
                        3
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicTypeNameTag { dst, src } => {
                    let value = self.read_slot(*src)?;
                    let out = if Self::is_array_tag(value) {
                        1001
                    } else {
                        1002
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNumericTag { dst, src } => {
                    let value = self.read_slot(*src)?;
                    let out = if Self::is_array_tag(value)
                        || value == EMPTY_TAG
                        || value == NULL_TAG
                        || runtime_is_error_tag(value)
                    {
                        0
                    } else {
                        1
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicIsDateTag { dst, src } => {
                    let value = self.read_slot(*src)?;
                    let out = if (1_000_000..=99_999_999).contains(&value) {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicIsObjectTag { dst, .. } => {
                    self.write_slot(*dst, 0)?;
                    pc += 1;
                }
                Instruction::IntrinsicShellHost { dst, command } => {
                    let command = self.read_slot(*command)?;
                    match self.host_services.process().shell(command, 0) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicEnvironHost { dst, key } => {
                    let key = self.read_slot(*key)?;
                    match self.host_services.process().environ(key) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDirHost { dst, path } => {
                    let path = self.read_slot(*path)?;
                    match self.host_services.process().dir(path, 0) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicCollectionAdd { dst, count, item } => {
                    let count = self.read_slot(*count)?;
                    let _item = self.read_slot(*item)?;
                    self.write_slot(*dst, (count + 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionItem { dst, count, index } => {
                    let count = self.read_slot(*count)?;
                    let index = self.read_slot(*index)?;
                    let out = if index >= 1 && index <= count {
                        index
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionRemove { dst, count, index } => {
                    let count = self.read_slot(*count)?;
                    let _index = self.read_slot(*index)?;
                    self.write_slot(*dst, (count - 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionCount { dst, count } => {
                    let count = self.read_slot(*count)?;
                    self.write_slot(*dst, count.max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCreateObjectHost { dst, prog_id } => {
                    let prog_id = self.read_slot(*prog_id)?;
                    match self.host_services.com().create_object(prog_id) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDispatchInvokeHost {
                    dst,
                    object,
                    member,
                    arg,
                } => {
                    let object = self.read_slot(*object)?;
                    let member = self.read_slot(*member)?;
                    let arg = marshal_dispatch_argument(self.read_slot(*arg)?);
                    match self
                        .host_services
                        .com()
                        .dispatch_invoke(object, member, arg)
                    {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicInvokeSymbolHost {
                    dst,
                    descriptor_id,
                    symbol,
                    arg,
                } => {
                    let arg = self.read_slot(*arg)?;
                    if bytecode.external_call_descriptors.is_empty() {
                        match self.host_services.dynlink().invoke_symbol(*symbol, arg) {
                            Ok(value) => {
                                self.write_slot(*dst, value)?;
                                pc += 1;
                            }
                            Err(err) => pc = self.route_host_error(pc, err)?,
                        }
                        continue;
                    }

                    let Some(descriptor) = bytecode
                        .external_call_descriptors
                        .iter()
                        .find(|entry| entry.descriptor_id == *descriptor_id)
                    else {
                        let err = HalError::adapter_fault(
                            self.host_services.profile(),
                            CapabilityId::DynamicLinking,
                            "invoke_descriptor",
                            format!("unknown external descriptor id {}", descriptor_id),
                        );
                        pc = self.route_host_error(pc, err)?;
                        continue;
                    };

                    if descriptor.symbol != *symbol {
                        let err = HalError::adapter_fault(
                            self.host_services.profile(),
                            CapabilityId::DynamicLinking,
                            "invoke_descriptor",
                            format!(
                                "descriptor {} symbol mismatch: instruction={}, descriptor={}",
                                descriptor_id, symbol, descriptor.symbol
                            ),
                        );
                        pc = self.route_host_error(pc, err)?;
                        continue;
                    }

                    let view = DynLinkDescriptorView {
                        descriptor_id: descriptor.descriptor_id,
                        declared_name: descriptor.declared_name.as_str(),
                        library: descriptor.library.as_str(),
                        alias: descriptor.alias.as_str(),
                        ordinal_alias: descriptor.ordinal_alias,
                        symbol: descriptor.symbol,
                        marshal_lane: descriptor.marshal_lane.as_str(),
                        calling_convention: descriptor.calling_convention.as_str(),
                        selection_policy: descriptor.selection_policy.as_str(),
                    };
                    if let Some(violation) = view.contract_violation() {
                        let err = HalError::adapter_fault(
                            self.host_services.profile(),
                            CapabilityId::DynamicLinking,
                            "invoke_descriptor",
                            format!(
                                "external descriptor contract violation for id {}: {}",
                                descriptor_id, violation
                            ),
                        );
                        pc = self.route_host_error(pc, err)?;
                        continue;
                    }
                    match self.host_services.dynlink().invoke_descriptor(&view, arg) {
                        Ok(value) => {
                            self.write_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicWithEventsGet {
                    dst,
                    owner,
                    binding,
                } => {
                    let owner = self.read_slot(*owner)?;
                    let binding = self.read_slot(*binding)?;
                    let key = Self::withevents_binding_key(owner, binding);
                    let value = self.withevents_bindings.get(&key).copied().unwrap_or(0);
                    self.write_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsSet {
                    dst,
                    owner,
                    binding,
                    value,
                } => {
                    let owner = self.read_slot(*owner)?;
                    let binding = self.read_slot(*binding)?;
                    let value = self.read_slot(*value)?;
                    let key = Self::withevents_binding_key(owner, binding);
                    if value == 0 {
                        self.withevents_bindings.remove(&key);
                    } else {
                        self.withevents_bindings.insert(key, value);
                    }
                    self.write_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::CmpEqSlots { dst, lhs, rhs } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l == r) {
                        pc += 1;
                        continue;
                    }
                    let out = if self.read_slot(*lhs)? == self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpNeSlots { dst, lhs, rhs } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l != r) {
                        pc += 1;
                        continue;
                    }
                    let out = if self.read_slot(*lhs)? != self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpLtSlots { dst, lhs, rhs } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l < r) {
                        pc += 1;
                        continue;
                    }
                    let out = if self.read_slot(*lhs)? < self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpLeSlots { dst, lhs, rhs } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l <= r) {
                        pc += 1;
                        continue;
                    }
                    let out = if self.read_slot(*lhs)? <= self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpGtSlots { dst, lhs, rhs } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l > r) {
                        pc += 1;
                        continue;
                    }
                    let out = if self.read_slot(*lhs)? > self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpGeSlots { dst, lhs, rhs } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l >= r) {
                        pc += 1;
                        continue;
                    }
                    let out = if self.read_slot(*lhs)? >= self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::LoadErrNumber { slot } => {
                    self.write_slot(*slot, self.last_error)?;
                    pc += 1;
                }
                Instruction::BoolNot { dst, src } => {
                    let out = if self.read_slot(*src)? == 0 { 1 } else { 0 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::BoolAnd { dst, lhs, rhs } => {
                    let lhs_val = self.read_slot(*lhs)?;
                    let rhs_val = self.read_slot(*rhs)?;
                    let out = if lhs_val != 0 && rhs_val != 0 { 1 } else { 0 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::BoolOr { dst, lhs, rhs } => {
                    let lhs_val = self.read_slot(*lhs)?;
                    let rhs_val = self.read_slot(*rhs)?;
                    let out = if lhs_val != 0 || rhs_val != 0 { 1 } else { 0 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::JumpIfZero {
                    cond_slot,
                    target_pc,
                } => {
                    let cond = self.read_slot(*cond_slot)?;
                    pc = Self::next_pc_for_jump_if_zero(cond, *target_pc, len, pc)?;
                }
                Instruction::Jump { target_pc } => {
                    pc = Self::next_pc_for_jump(*target_pc, len)?;
                }
                Instruction::CallProc { target_pc } => {
                    if *target_pc >= bytecode.instructions.len() {
                        return Err(format!("call target out of range: {target_pc}"));
                    }
                    self.call_stack.push(pc + 1);
                    pc = *target_pc;
                }
                Instruction::SetOnErrorResumeNext => {
                    self.on_error_resume_next = true;
                    self.on_error_goto_label_target = None;
                    pc += 1;
                }
                Instruction::SetOnErrorGoto0 => {
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = None;
                    pc += 1;
                }
                Instruction::SetOnErrorGotoLabel { target_pc } => {
                    if *target_pc >= len {
                        return Err(format!("error handler target out of range: {target_pc}"));
                    }
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = Some(*target_pc);
                    pc += 1;
                }
                Instruction::ResumeNext => {
                    self.clear_error_state();
                    pc += 1;
                }
                Instruction::Resume => {
                    let Some(target_pc) = self.last_error_pc else {
                        return Err("resume without active error".to_string());
                    };
                    self.clear_error_state();
                    pc = target_pc;
                }
                Instruction::ResumeLabel { target_pc } => {
                    if *target_pc >= len {
                        return Err(format!("resume target out of range: {target_pc}"));
                    }
                    if self.last_error_pc.is_none() {
                        return Err("resume without active error".to_string());
                    }
                    self.clear_error_state();
                    pc = *target_pc;
                }
                Instruction::RaiseError { code } => {
                    pc = self.route_runtime_error(pc, *code, None)?;
                }
                Instruction::ClearErr => {
                    self.clear_error_state();
                    pc += 1;
                }
                Instruction::Return => {
                    if let Some(return_pc) = self.call_stack.pop() {
                        pc = return_pc;
                    } else {
                        return Err("return with empty call stack".to_string());
                    }
                }
                Instruction::IncSlot { slot } => {
                    if typed_fastpaths && self.fast_add_const(*slot, 1) {
                        pc += 1;
                        continue;
                    }
                    let value = self.read_slot(*slot)?;
                    self.write_slot(*slot, value + 1)?;
                    pc += 1;
                }
                Instruction::Halt => break,
            }
        }
        Ok(())
    }

    fn read_slot(&self, slot: usize) -> Result<i32, String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        Ok(self.registers.registers[slot])
    }

    fn write_slot(&mut self, slot: usize, value: i32) -> Result<(), String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = value;
        Ok(())
    }

    fn typed_fastpaths_enabled_from_env() -> bool {
        std::env::var("OXVBA_DISABLE_TYPED_FASTPATH")
            .map(|value| value != "1")
            .unwrap_or(true)
    }

    fn withevents_binding_key(owner: i32, binding: i32) -> i64 {
        ((owner as i64) << 32) | (binding as u32 as i64)
    }

    fn fast_read_slot(&self, slot: usize) -> Option<i32> {
        self.registers.registers.get(slot).copied()
    }

    fn fast_write_slot(&mut self, slot: usize, value: i32) -> bool {
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        *dst = value;
        true
    }

    fn fast_add_const(&mut self, slot: usize, value: i32) -> bool {
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        *dst += value;
        true
    }

    fn fast_sub_const(&mut self, slot: usize, value: i32) -> bool {
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        *dst -= value;
        true
    }

    fn fast_copy_slot(&mut self, dst: usize, src: usize) -> bool {
        let Some(value) = self.fast_read_slot(src) else {
            return false;
        };
        self.fast_write_slot(dst, value)
    }

    fn fast_cmp_slots<F>(&mut self, dst: usize, lhs: usize, rhs: usize, pred: F) -> bool
    where
        F: FnOnce(i32, i32) -> bool,
    {
        let (Some(lhs), Some(rhs)) = (self.fast_read_slot(lhs), self.fast_read_slot(rhs)) else {
            return false;
        };
        self.fast_write_slot(dst, if pred(lhs, rhs) { 1 } else { 0 })
    }

    fn next_pc_for_jump(target_pc: usize, instruction_len: usize) -> Result<usize, String> {
        if target_pc > instruction_len {
            return Err(format!("jump target out of range: {target_pc}"));
        }
        Ok(target_pc)
    }

    fn next_pc_for_jump_if_zero(
        cond: i32,
        target_pc: usize,
        instruction_len: usize,
        current_pc: usize,
    ) -> Result<usize, String> {
        if cond == 0 {
            Self::next_pc_for_jump(target_pc, instruction_len)
        } else {
            Ok(current_pc + 1)
        }
    }

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

    fn left_digits(value: i32, count: i32) -> i32 {
        Self::slice_digits(value, 0, Some(count))
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
        Self::slice_digits(value, zero_based_start, count)
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

    fn normalize_for_compare(text: String, mode: StringCompareMode) -> String {
        match mode {
            StringCompareMode::Binary => text,
            StringCompareMode::Text => text.to_ascii_lowercase(),
        }
    }

    fn instr_digits(haystack: i32, needle: i32, mode: StringCompareMode) -> i32 {
        let hay = Self::normalize_for_compare(haystack.to_string(), mode);
        let nee = Self::normalize_for_compare(needle.to_string(), mode);
        hay.find(&nee).map_or(0, |idx| (idx + 1) as i32)
    }

    fn instrrev_digits(haystack: i32, needle: i32, mode: StringCompareMode) -> i32 {
        let hay = Self::normalize_for_compare(haystack.to_string(), mode);
        let nee = Self::normalize_for_compare(needle.to_string(), mode);
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
        let delimiter = delimiter.to_string();
        if delimiter.is_empty() {
            return 1;
        }
        text.split(&delimiter).count() as i32
    }

    fn join_digits(value: i32, _delimiter: i32) -> i32 {
        array_len_from_tag(value)
            .and_then(|count| i32::try_from(count).ok())
            .unwrap_or(value)
    }

    fn replace_digits(value: i32, find: i32, replace: i32) -> i32 {
        let text = value.to_string();
        let find = find.to_string();
        let replace = replace.to_string();
        if find.is_empty() {
            return value;
        }
        text.replace(&find, &replace).parse::<i32>().unwrap_or(0)
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
        let lhs = Self::normalize_for_compare(lhs.to_string(), mode);
        let rhs = Self::normalize_for_compare(rhs.to_string(), mode);
        match lhs.cmp(&rhs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    fn like_digits(lhs: i32, pattern: i32, mode: StringCompareMode) -> i32 {
        let lhs = Self::normalize_for_compare(lhs.to_string(), mode);
        let pattern = Self::normalize_for_compare(pattern.to_string(), mode);
        if lhs == pattern { -1 } else { 0 }
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
            return (Self::rate_func(r + h, nper, pmt, pv, fv, due)
                - Self::rate_func(r - h, nper, pmt, pv, fv, due))
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
            let f = Self::rate_func(r, n, pmt, pv, fv, due);
            let fp = Self::rate_func_derivative(r, n, pmt, pv, fv, due);
            if fp.abs() < 1e-12 {
                return error_tag_from_code(FIN_RATE_ERROR_CODE);
            }
            let next = (r - f / fp).clamp(-0.99, 10.0);
            if !next.is_finite() {
                return error_tag_from_code(FIN_RATE_ERROR_CODE);
            }
            if (next - r).abs() < FIN_EPS {
                r = next;
                return (r * 100.0).round() as i32;
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

    fn is_array_tag(value: i32) -> bool {
        runtime_is_array_tag(value)
    }

    fn array_count(value: i32) -> i32 {
        array_len_from_tag(value)
            .and_then(|count| i32::try_from(count).ok())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::Vm;
    use oxvba_compiler::{Bytecode, Instruction, bytecode::StringCompareMode};
    use oxvba_hal::{
        error::{HalError, HalErrorKind},
        model::CapabilityId,
    };
    use oxvba_runtime::safe_array::ARRAY_TAG_BASE;
    use oxvba_runtime::value_tags::{EMPTY_TAG, NULL_TAG, error_tag_from_code};

    #[test]
    fn executes_load_and_add_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![15]);
    }

    #[test]
    fn executes_load_and_sub_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::SubConstI32 { slot: 0, value: 3 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![7]);
    }

    #[test]
    fn typed_fastpath_toggle_preserves_hot_instruction_semantics() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::AddConstI32 { slot: 0, value: 4 },
                Instruction::SubConstI32 { slot: 1, value: 1 },
                Instruction::CopySlot { dst: 2, src: 0 },
                Instruction::CmpGtSlots {
                    dst: 3,
                    lhs: 2,
                    rhs: 1,
                },
                Instruction::IncSlot { slot: 2 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 4,
            user_slot_count: 4,
        };

        let mut fast = Vm::default();
        fast.execute_with_typed_fastpaths(&bytecode, true)
            .expect("fastpath execution should succeed");
        let mut baseline = Vm::default();
        baseline
            .execute_with_typed_fastpaths(&bytecode, false)
            .expect("baseline execution should succeed");

        assert_eq!(fast.snapshot_slots(4), baseline.snapshot_slots(4));
    }

    #[test]
    fn executes_intrinsic_digit_string_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: 12345,
                },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::LoadConstI32 { slot: 2, value: 3 },
                Instruction::IntrinsicLenDigits { dst: 3, src: 0 },
                Instruction::IntrinsicLeftDigits {
                    dst: 4,
                    src: 0,
                    count: 1,
                },
                Instruction::IntrinsicRightDigits {
                    dst: 5,
                    src: 0,
                    count: 1,
                },
                Instruction::IntrinsicMidDigits {
                    dst: 6,
                    src: 0,
                    start: 1,
                    count: Some(2),
                },
                Instruction::IntrinsicInStrDigits {
                    dst: 7,
                    haystack: 0,
                    needle: 2,
                    mode: StringCompareMode::Binary,
                },
                Instruction::IntrinsicLowerDigits { dst: 8, src: 0 },
                Instruction::IntrinsicUpperDigits { dst: 9, src: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 10,
            user_slot_count: 10,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(
            vm.snapshot_slots(10),
            vec![12345, 2, 3, 5, 12, 45, 234, 3, 12345, 12345]
        );
    }

    #[test]
    fn executes_mid_statement_mutation_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: 12345,
                },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::LoadConstI32 { slot: 2, value: 2 },
                Instruction::LoadConstI32 { slot: 3, value: 99 },
                Instruction::IntrinsicMidStmtDigits {
                    target: 0,
                    start: 1,
                    count: Some(2),
                    value: 3,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 4,
            user_slot_count: 4,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(4), vec![19945, 2, 2, 99]);
    }

    #[test]
    fn executes_intrinsic_advanced_digit_string_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: 123231,
                },
                Instruction::LoadConstI32 { slot: 1, value: 23 },
                Instruction::LoadConstI32 {
                    slot: 2,
                    value: 12345,
                },
                Instruction::LoadConstI32 { slot: 3, value: 67 },
                Instruction::LoadConstI32 { slot: 4, value: 12 },
                Instruction::LoadConstI32 {
                    slot: 5,
                    value: 123,
                },
                Instruction::IntrinsicInStrRevDigits {
                    dst: 13,
                    haystack: 0,
                    needle: 1,
                    mode: StringCompareMode::Binary,
                },
                Instruction::IntrinsicSplitCountDigits {
                    dst: 6,
                    src: 0,
                    delimiter: 1,
                },
                Instruction::IntrinsicJoinDigits {
                    dst: 7,
                    src: 2,
                    delimiter: 1,
                },
                Instruction::IntrinsicReplaceDigits {
                    dst: 8,
                    src: 2,
                    find: 1,
                    replace: 3,
                },
                Instruction::IntrinsicTrimDigits { dst: 9, src: 2 },
                Instruction::IntrinsicLTrimDigits { dst: 10, src: 2 },
                Instruction::IntrinsicRTrimDigits { dst: 11, src: 2 },
                Instruction::IntrinsicStrCompDigits {
                    dst: 12,
                    lhs: 4,
                    rhs: 5,
                    mode: StringCompareMode::Binary,
                },
                Instruction::IntrinsicLikeDigits {
                    dst: 14,
                    lhs: 4,
                    pattern: 4,
                    mode: StringCompareMode::Binary,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 15,
            user_slot_count: 15,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(
            vm.snapshot_slots(15),
            vec![
                123231, 23, 12345, 67, 12, 123, 3, 12345, 16745, 12345, 12345, 12345, -1, 4, -1
            ]
        );
    }

    #[test]
    fn join_intrinsic_maps_array_tag_to_count() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: ARRAY_TAG_BASE + 3,
                },
                Instruction::LoadConstI32 { slot: 1, value: 0 },
                Instruction::IntrinsicJoinDigits {
                    dst: 2,
                    src: 0,
                    delimiter: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(3), vec![ARRAY_TAG_BASE + 3, 0, 3]);
    }

    #[test]
    fn executes_intrinsic_runtime_expansion_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: 2026,
                },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::LoadConstI32 { slot: 2, value: 28 },
                Instruction::LoadConstI32 { slot: 3, value: 0 },
                Instruction::LoadConstI32 { slot: 4, value: 1 },
                Instruction::LoadConstI32 { slot: 5, value: 3 },
                Instruction::LoadConstI32 { slot: 6, value: 2 },
                Instruction::LoadConstI32 {
                    slot: 7,
                    value: ARRAY_TAG_BASE + 3,
                },
                Instruction::LoadConstI32 { slot: 22, value: 1 },
                Instruction::LoadConstI32 {
                    slot: 23,
                    value: 10,
                },
                Instruction::LoadConstI32 {
                    slot: 24,
                    value: 20,
                },
                Instruction::LoadConstI32 {
                    slot: 25,
                    value: 30,
                },
                Instruction::LoadConstI32 {
                    slot: 26,
                    value: 50,
                },
                Instruction::LoadConstI32 {
                    slot: 27,
                    value: 10,
                },
                Instruction::LoadConstI32 {
                    slot: 28,
                    value: 70,
                },
                Instruction::LoadConstI32 { slot: 29, value: 1 },
                Instruction::LoadConstI32 { slot: 30, value: 2 },
                Instruction::IntrinsicDateSerialDigits {
                    dst: 8,
                    year: 0,
                    month: 1,
                    day: 2,
                },
                Instruction::IntrinsicDateAddDigits {
                    dst: 9,
                    interval: 3,
                    number: 4,
                    date: 8,
                },
                Instruction::IntrinsicDateDiffDigits {
                    dst: 10,
                    interval: 3,
                    date1: 8,
                    date2: 9,
                },
                Instruction::IntrinsicAbsI32 { dst: 11, src: 10 },
                Instruction::IntrinsicSgnI32 { dst: 12, src: 10 },
                Instruction::IntrinsicRoundI32 {
                    dst: 13,
                    src: 8,
                    digits: None,
                },
                Instruction::IntrinsicFvI32 {
                    dst: 14,
                    rate: 3,
                    nper: 5,
                    pmt: 6,
                    pv: Some(6),
                    due: Some(3),
                },
                Instruction::IntrinsicLBoundArray { dst: 15, src: 7 },
                Instruction::IntrinsicUBoundArray { dst: 16, src: 7 },
                Instruction::IntrinsicIsArrayTag { dst: 17, src: 7 },
                Instruction::IntrinsicCollectionAdd {
                    dst: 18,
                    count: 4,
                    item: 6,
                },
                Instruction::IntrinsicCollectionCount { dst: 19, count: 18 },
                Instruction::IntrinsicCreateObjectHost {
                    dst: 20,
                    prog_id: 6,
                },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 21,
                    object: 20,
                    member: 4,
                    arg: 6,
                },
                Instruction::IntrinsicNpvI32 {
                    dst: 31,
                    rate: 22,
                    values: vec![23, 24, 25],
                },
                Instruction::IntrinsicIrrI32 {
                    dst: 32,
                    value: 26,
                    guess: Some(27),
                },
                Instruction::IntrinsicMirrI32 {
                    dst: 33,
                    value: 28,
                    finance_rate: 29,
                    reinvest_rate: 30,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 34,
            user_slot_count: 34,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(34);
        assert_eq!(out[8], 20260228);
        assert_eq!(out[9], 20260229);
        assert_eq!(out[10], 1);
        assert_eq!(out[11], 1);
        assert_eq!(out[12], 1);
        assert_eq!(out[13], 20260228);
        assert_eq!(out[15], 0);
        assert_eq!(out[16], 2);
        assert_eq!(out[17], 1);
        assert_eq!(out[18], 2);
        assert_eq!(out[19], 2);
        assert_eq!(out[20], 5002);
        assert_eq!(out[21], 5005);
        assert_eq!(out[31], 59);
        assert_eq!(out[32], -50);
        assert_eq!(out[33], -28);
    }

    #[test]
    fn executes_intrinsic_financial_rate_nper_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::LoadConstI32 { slot: 2, value: 99 },
                Instruction::LoadConstI32 { slot: 3, value: 1 },
                Instruction::LoadConstI32 { slot: 4, value: 88 },
                Instruction::LoadConstI32 { slot: 5, value: 3 },
                Instruction::IntrinsicRateI32 {
                    dst: 6,
                    nper: 0,
                    pmt: 1,
                    pv: 2,
                    fv: None,
                    due: None,
                    guess: None,
                },
                Instruction::IntrinsicNPerI32 {
                    dst: 7,
                    rate: 3,
                    pmt: 1,
                    pv: 4,
                    fv: Some(5),
                    due: None,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 8,
            user_slot_count: 8,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(8);
        assert_eq!(out[6], -99);
        assert_eq!(out[7], -38);
    }

    #[test]
    fn financial_non_convergence_paths_return_stable_error_tags() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 0 },
                Instruction::LoadConstI32 { slot: 1, value: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 1 },
                Instruction::IntrinsicRateI32 {
                    dst: 3,
                    nper: 0,
                    pmt: 1,
                    pv: 1,
                    fv: None,
                    due: None,
                    guess: None,
                },
                Instruction::IntrinsicNPerI32 {
                    dst: 4,
                    rate: 2,
                    pmt: 1,
                    pv: 0,
                    fv: None,
                    due: None,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 5,
            user_slot_count: 5,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(5);
        assert_eq!(out[3], error_tag_from_code(2001));
        assert_eq!(out[4], error_tag_from_code(2002));
    }

    #[test]
    fn vartype_and_isnumeric_distinguish_empty_null_error_and_array_tags() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: EMPTY_TAG,
                },
                Instruction::LoadConstI32 {
                    slot: 1,
                    value: NULL_TAG,
                },
                Instruction::LoadConstI32 {
                    slot: 2,
                    value: error_tag_from_code(17),
                },
                Instruction::LoadConstI32 {
                    slot: 3,
                    value: 123,
                },
                Instruction::LoadConstI32 {
                    slot: 4,
                    value: ARRAY_TAG_BASE + 2,
                },
                Instruction::IntrinsicVarTypeTag { dst: 5, src: 0 },
                Instruction::IntrinsicVarTypeTag { dst: 6, src: 1 },
                Instruction::IntrinsicVarTypeTag { dst: 7, src: 2 },
                Instruction::IntrinsicVarTypeTag { dst: 8, src: 3 },
                Instruction::IntrinsicVarTypeTag { dst: 9, src: 4 },
                Instruction::IntrinsicIsNumericTag { dst: 10, src: 0 },
                Instruction::IntrinsicIsNumericTag { dst: 11, src: 1 },
                Instruction::IntrinsicIsNumericTag { dst: 12, src: 2 },
                Instruction::IntrinsicIsNumericTag { dst: 13, src: 3 },
                Instruction::IntrinsicIsNumericTag { dst: 14, src: 4 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 15,
            user_slot_count: 15,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(15);
        assert_eq!(out[5], 0);
        assert_eq!(out[6], 1);
        assert_eq!(out[7], 10);
        assert_eq!(out[8], 3);
        assert_eq!(out[9], 8192 + 12);
        assert_eq!(out[10], 0);
        assert_eq!(out[11], 0);
        assert_eq!(out[12], 0);
        assert_eq!(out[13], 1);
        assert_eq!(out[14], 0);
    }

    #[test]
    fn dispatch_invoke_marshals_array_argument_payload() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 4 },
                Instruction::IntrinsicCreateObjectHost { dst: 1, prog_id: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 6 },
                Instruction::LoadConstI32 {
                    slot: 3,
                    value: ARRAY_TAG_BASE + 3,
                },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 4,
                    object: 1,
                    member: 2,
                    arg: 3,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 5,
            user_slot_count: 5,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(5);
        assert_eq!(out[1], 5004);
        assert_eq!(out[4], 25_013);
    }

    #[test]
    fn declare_invoke_routes_through_dynlink_host_service() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicInvokeSymbolHost {
                    dst: 1,
                    descriptor_id: 1_234,
                    symbol: 1_234,
                    arg: 0,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(oxvba_hal::adapters::for_profile(
            oxvba_hal::model::HalProfileId::Windows,
            oxvba_hal::model::HostPolicy {
                allow_dynamic_link: true,
                ..oxvba_hal::model::HostPolicy::deterministic_runtime()
            },
        ));
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(2);
        assert_eq!(out[1], 1_237);
    }

    #[test]
    fn declare_invoke_uses_descriptor_table_when_present() {
        let symbol = 2_345;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicInvokeSymbolHost {
                    dst: 1,
                    descriptor_id: symbol as u32,
                    symbol,
                    arg: 0,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: "host".to_string(),
                alias: "ping".to_string(),
                ordinal_alias: false,
                symbol,
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(oxvba_hal::adapters::for_profile(
            oxvba_hal::model::HalProfileId::Windows,
            oxvba_hal::model::HostPolicy {
                allow_dynamic_link: true,
                ..oxvba_hal::model::HostPolicy::deterministic_runtime()
            },
        ));
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(2);
        assert_eq!(out[1], 2_348);
    }

    #[test]
    fn declare_invoke_descriptor_id_mismatch_is_reported() {
        let symbol = 4_321;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicInvokeSymbolHost {
                    dst: 1,
                    descriptor_id: 999,
                    symbol,
                    arg: 0,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: "host".to_string(),
                alias: "ping".to_string(),
                ordinal_alias: false,
                symbol,
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(oxvba_hal::adapters::for_profile(
            oxvba_hal::model::HalProfileId::Windows,
            oxvba_hal::model::HostPolicy {
                allow_dynamic_link: true,
                ..oxvba_hal::model::HostPolicy::deterministic_runtime()
            },
        ));
        let err = vm
            .execute(&bytecode)
            .expect_err("descriptor mismatch should be reported");
        assert!(err.contains("unknown external descriptor id"));
    }

    #[test]
    fn declare_invoke_descriptor_contract_empty_library_is_reported() {
        let symbol = 4_321;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicInvokeSymbolHost {
                    dst: 1,
                    descriptor_id: symbol as u32,
                    symbol,
                    arg: 0,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: " ".to_string(),
                alias: "ping".to_string(),
                ordinal_alias: false,
                symbol,
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(oxvba_hal::adapters::for_profile(
            oxvba_hal::model::HalProfileId::Windows,
            oxvba_hal::model::HostPolicy {
                allow_dynamic_link: true,
                ..oxvba_hal::model::HostPolicy::deterministic_runtime()
            },
        ));
        let err = vm
            .execute(&bytecode)
            .expect_err("contract violation should be reported");
        assert!(err.contains("external descriptor contract violation"));
        assert!(err.contains("library is empty"));
    }

    #[test]
    fn declare_invoke_descriptor_contract_selection_policy_mismatch_is_reported() {
        let symbol = 5_432;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicInvokeSymbolHost {
                    dst: 1,
                    descriptor_id: symbol as u32,
                    symbol,
                    arg: 0,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: "host".to_string(),
                alias: "7".to_string(),
                ordinal_alias: true,
                symbol,
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(oxvba_hal::adapters::for_profile(
            oxvba_hal::model::HalProfileId::Windows,
            oxvba_hal::model::HostPolicy {
                allow_dynamic_link: true,
                ..oxvba_hal::model::HostPolicy::deterministic_runtime()
            },
        ));
        let err = vm
            .execute(&bytecode)
            .expect_err("selection policy mismatch should be reported");
        assert!(err.contains("external descriptor contract violation"));
        assert!(err.contains("selection_policy does not match ordinal_alias contract"));
    }

    #[test]
    fn executes_branch_and_loop_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 0 },
                Instruction::LoadConstI32 { slot: 1, value: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 3 },
                Instruction::CmpEqSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::JumpIfZero {
                    cond_slot: 3,
                    target_pc: 6,
                },
                Instruction::LoadConstI32 { slot: 4, value: 10 },
                Instruction::LoadConstI32 { slot: 5, value: 1 },
                Instruction::CmpLeSlots {
                    dst: 6,
                    lhs: 5,
                    rhs: 2,
                },
                Instruction::JumpIfZero {
                    cond_slot: 6,
                    target_pc: 12,
                },
                Instruction::AddConstI32 { slot: 4, value: 1 },
                Instruction::IncSlot { slot: 5 },
                Instruction::Jump { target_pc: 7 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 7,
            user_slot_count: 7,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(7), vec![0, 0, 3, 1, 13, 4, 0]);
    }

    #[test]
    fn rejects_invalid_jump_target() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Jump { target_pc: 10 }, Instruction::Halt],
            external_call_descriptors: Vec::new(),
            slot_count: 0,
            user_slot_count: 0,
        };
        let mut vm = Vm::default();
        let err = vm.execute(&bytecode).expect_err("invalid jump should fail");
        assert!(err.contains("jump target out of range"));
    }

    #[test]
    fn jump_if_zero_pc_progression_helper() {
        assert_eq!(Vm::next_pc_for_jump_if_zero(0, 3, 4, 1).expect("jump"), 3);
        assert_eq!(
            Vm::next_pc_for_jump_if_zero(1, 3, 4, 1).expect("fallthrough"),
            2
        );
        assert!(Vm::next_pc_for_jump_if_zero(0, 9, 4, 1).is_err());
    }

    #[test]
    fn executes_comparators_and_boolean_ops() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 5 },
                Instruction::LoadConstI32 { slot: 1, value: 3 },
                Instruction::CmpGtSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpLtSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpNeSlots {
                    dst: 4,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::BoolAnd {
                    dst: 5,
                    lhs: 2,
                    rhs: 4,
                },
                Instruction::BoolNot { dst: 6, src: 3 },
                Instruction::BoolOr {
                    dst: 7,
                    lhs: 3,
                    rhs: 6,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 8,
            user_slot_count: 8,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(8), vec![5, 3, 1, 0, 1, 1, 1, 1]);
    }

    #[test]
    fn executes_call_and_return_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::CallProc { target_pc: 4 },
                Instruction::AddConstI32 { slot: 0, value: 1 },
                Instruction::Halt,
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![7]);
    }

    #[test]
    fn resume_next_records_error_number_and_continues() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorResumeNext,
                Instruction::RaiseError { code: 5 },
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should continue on error");
        assert_eq!(vm.snapshot_slots(1), vec![5]);
    }

    #[test]
    fn goto_label_handler_receives_error_and_jumps() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorGotoLabel { target_pc: 4 },
                Instruction::RaiseError { code: 7 },
                Instruction::LoadConstI32 { slot: 0, value: 99 },
                Instruction::Halt,
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("vm should jump to label handler");
        assert_eq!(vm.snapshot_slots(1), vec![7]);
    }

    #[test]
    fn resume_next_clears_error_state_before_continuing() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorResumeNext,
                Instruction::RaiseError { code: 5 },
                Instruction::ResumeNext,
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("resume next should clear error state");
        assert_eq!(vm.snapshot_slots(1), vec![0]);
    }

    #[test]
    fn resume_label_clears_error_state_before_jump() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorGotoLabel { target_pc: 3 },
                Instruction::RaiseError { code: 9 },
                Instruction::Halt,
                Instruction::ResumeLabel { target_pc: 4 },
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("resume label should clear error state");
        assert_eq!(vm.snapshot_slots(1), vec![0]);
    }

    #[test]
    fn hal_error_code_mapping_is_total_and_stable() {
        for kind in [
            HalErrorKind::CapabilityUnavailable,
            HalErrorKind::PolicyDenied,
            HalErrorKind::AdapterFault,
            HalErrorKind::UnsupportedProfile,
        ] {
            for capability in [
                CapabilityId::UiInteraction,
                CapabilityId::EventPump,
                CapabilityId::FileSystemIo,
                CapabilityId::ProcessEnv,
                CapabilityId::ComActivationDispatch,
                CapabilityId::TimeLocale,
                CapabilityId::DynamicLinking,
                CapabilityId::DiagnosticsTelemetry,
            ] {
                let code = Vm::hal_error_code(kind, capability);
                assert!(
                    (53_011..=53_084).contains(&code),
                    "HAL error code out of expected deterministic band: {}",
                    code
                );
            }
        }
    }

    #[test]
    fn route_host_error_surfaces_stable_code_and_operation_in_runtime_message() {
        let mut vm = Vm::default();
        let err = HalError::policy_denied(
            oxvba_hal::model::HalProfileId::Windows,
            CapabilityId::ProcessEnv,
            "shell",
        );
        let runtime = vm
            .route_host_error(0, err)
            .expect_err("without On Error handlers, host failures must surface");
        assert!(runtime.contains("HAL-E-POLICY-DENIED"));
        assert!(runtime.contains("[shell]"));
        assert!(runtime.contains("runtime error: 53042"));
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use crate::interpreter::Vm;
    use oxvba_compiler::{Bytecode, Instruction};
    use oxvba_runtime::value_tags::error_tag_from_code;

    #[kani::proof]
    fn pc_progression_is_safe_for_valid_jump_target() {
        let instruction_len: usize = kani::any();
        kani::assume(instruction_len > 0);
        kani::assume(instruction_len < 64);

        let current_pc: usize = kani::any();
        kani::assume(current_pc < instruction_len);

        let target_pc: usize = kani::any();
        kani::assume(target_pc <= instruction_len);

        let cond: i32 = kani::any();
        let next = Vm::next_pc_for_jump_if_zero(cond, target_pc, instruction_len, current_pc)
            .expect("assumed valid target");
        assert!(next <= instruction_len);
    }

    #[kani::proof]
    fn comparator_ops_produce_boolean_values() {
        let a: i32 = kani::any();
        let b: i32 = kani::any();
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: a },
                Instruction::LoadConstI32 { slot: 1, value: b },
                Instruction::CmpEqSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpNeSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpLtSlots {
                    dst: 4,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpLeSlots {
                    dst: 5,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpGtSlots {
                    dst: 6,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpGeSlots {
                    dst: 7,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 8,
            user_slot_count: 8,
        };

        let mut vm = Vm::default();
        assert!(vm.execute(&bytecode).is_ok());
        let out = vm.snapshot_slots(8);
        for idx in 2..=7 {
            assert!(out[idx] == 0 || out[idx] == 1);
        }
    }

    #[kani::proof]
    fn financial_rate_zero_nper_yields_error_tag() {
        let out = Vm::rate_i32(0, 0, 0, 0, 0, 0);
        assert_eq!(out, error_tag_from_code(2001));
    }

    #[kani::proof]
    fn financial_nper_invalid_domain_yields_error_tag() {
        let out = Vm::nper_i32(1, 0, 0, 0, 0);
        assert_eq!(out, error_tag_from_code(2002));
    }

    #[kani::proof]
    fn vartype_intrinsic_outputs_expected_domain() {
        let value: i32 = kani::any();
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value },
                Instruction::IntrinsicVarTypeTag { dst: 1, src: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };
        let mut vm = Vm::default();
        assert!(vm.execute(&bytecode).is_ok());
        let out = vm.snapshot_slots(2)[1];
        assert!(matches!(out, 0 | 1 | 3 | 10 | 8204));
    }

    #[kani::proof]
    fn cverr_tag_encoding_stays_in_reserved_error_band() {
        let code: i32 = kani::any();
        let tag = error_tag_from_code(code);
        assert!(oxvba_runtime::value_tags::is_error_tag(tag));
    }

    #[kani::proof]
    fn resume_next_clears_err_number_after_raise() {
        let code: i32 = kani::any();
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorResumeNext,
                Instruction::RaiseError { code },
                Instruction::ResumeNext,
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut vm = Vm::default();
        assert!(vm.execute(&bytecode).is_ok());
        assert_eq!(vm.snapshot_slots(1)[0], 0);
    }
}
