use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Arc,
};

use oxvba_com::{
    ComCallbackToken, ComSubscriptionToken, DynamicCallArg, DynamicCallKind, DynamicCallRequest,
    DynamicMemberSelector, DynamicObjectBridge, DynamicValue,
};
use oxvba_compiler::{
    Bytecode, Instruction, ProcedureRuntimeMetadata, ProjectComWithEventsRoute,
    ProjectDynamicMemberKind, ProjectDynamicMemberRoute, ProjectDynamicObjectRoute,
    ProjectDynamicParamRoute,
    bytecode::{
        ExternalCallWriteback, ExternalCallWritebackKind, RuntimeArrayElementType,
        RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
    },
};
use oxvba_hal::{
    HalComDynamicBridge,
    adapters::builder::HostBuilder,
    error::{HalError, HalErrorKind},
    model::{CapabilityId, HostPolicy, native_host_profile},
    traits::{DynLinkDescriptorView, HostServices},
};
use oxvba_runtime::safe_array::{
    SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
    VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_UI1_VALUE,
    VT_VARIANT_VALUE, is_array_tag as runtime_is_array_tag,
};
use oxvba_runtime::value_tags::{
    EMPTY_TAG, NULL_TAG, error_code_from_tag, error_tag_from_code, is_error_tag,
};
use oxvba_runtime::{BindingHandle, ObjectRef, RuntimeValue, Variant, bstr::BStr};

use crate::register_file::{RegisterFile, RuntimeSlot};

#[derive(Debug, Default, Clone)]
struct WithEventsOwnerIterator {
    owners: Vec<ObjectRef>,
    next_index: usize,
}

#[derive(Debug, Clone)]
struct ForEachIteratorState {
    items: Vec<RuntimeSlot>,
    next_index: usize,
}

#[derive(Debug, Clone)]
struct ForEachInitError {
    code: i32,
    detail: String,
}

/// Saved error-handling state for one procedure activation.
#[derive(Debug, Clone, Default)]
struct ErrorFrame {
    on_error_resume_next: bool,
    on_error_goto_label_target: Option<usize>,
    last_error: i32,
    last_error_pc: Option<usize>,
    last_error_description: Option<String>,
    last_error_source: Option<String>,
}

#[derive(Debug, Clone)]
struct ComWithEventsSubscription {
    owner_object: ObjectRef,
    route: ProjectComWithEventsRoute,
}

#[derive(Debug, Clone)]
struct ProjectDynamicObjectState {
    object: ObjectRef,
    route: ProjectDynamicObjectRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugBreakpoint {
    pub module_name: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSourceLocation {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub statement_pc: usize,
    pub line_number: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugStopReason {
    Entry,
    Breakpoint,
    Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugStop {
    pub reason: DebugStopReason,
    pub location: DebugSourceLocation,
    pub call_stack_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugRunResult {
    Paused(DebugStop),
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugRuntimeSnapshot {
    pub last_pause: Option<DebugStop>,
    pub activation_entry_pcs: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DebugStepMode {
    Into,
    Over { depth: usize },
    Out { depth: usize },
}

#[derive(Debug, Clone)]
struct DebugRuntimeState {
    current_pc: usize,
    return_halts_when_stack_empty: bool,
    breakpoints: Vec<DebugBreakpoint>,
    pause_on_entry: bool,
    skip_pause_once_at_pc: Option<usize>,
    step_mode: Option<DebugStepMode>,
    last_pause: Option<DebugStop>,
}

pub struct Vm {
    registers: RegisterFile,
    host_services: Arc<dyn HostServices>,
    typed_fastpaths_default: bool,
    call_stack: Vec<(usize, ErrorFrame)>,
    activation_entry_pcs: Vec<usize>,
    procedure_runtime_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    project_dynamic_objects: HashMap<i32, ProjectDynamicObjectState>,
    foreach_iterators: HashMap<i32, ForEachIteratorState>,
    next_foreach_iterator_id: i32,
    project_com_withevents_routes: HashMap<i32, Vec<ProjectComWithEventsRoute>>,
    withevents_bindings: HashMap<i64, RuntimeSlot>,
    com_withevents_subscriptions: HashMap<ComSubscriptionToken, ComWithEventsSubscription>,
    com_withevents_binding_subscriptions: HashMap<i64, Vec<ComSubscriptionToken>>,
    pending_callback_tokens: VecDeque<ComCallbackToken>,
    withevents_owner_iters: Vec<WithEventsOwnerIterator>,
    on_error_resume_next: bool,
    on_error_goto_label_target: Option<usize>,
    last_error: i32,
    last_error_pc: Option<usize>,
    last_error_description: Option<String>,
    last_error_source: Option<String>,
    rnd_state: u32,
    debug_runtime: Option<DebugRuntimeState>,
}

const FIN_MAX_ITERS: usize = 60;
const FIN_EPS: f64 = 1e-10;
const FIN_DERIVATIVE_STEP: f64 = 1e-7;
const FIN_RATE_ERROR_CODE: i32 = 2001;
const FIN_NPER_ERROR_CODE: i32 = 2002;

fn default_host_services() -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(native_host_profile())
        .policy(HostPolicy::deterministic_runtime())
        .build()
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
            typed_fastpaths_default: Self::typed_fastpaths_enabled_from_env(),
            call_stack: Vec::new(),
            activation_entry_pcs: Vec::new(),
            procedure_runtime_metadata: BTreeMap::new(),
            project_dynamic_objects: HashMap::new(),
            foreach_iterators: HashMap::new(),
            next_foreach_iterator_id: 1,
            project_com_withevents_routes: HashMap::new(),
            withevents_bindings: HashMap::new(),
            com_withevents_subscriptions: HashMap::new(),
            com_withevents_binding_subscriptions: HashMap::new(),
            pending_callback_tokens: VecDeque::new(),
            withevents_owner_iters: Vec::new(),
            on_error_resume_next: false,
            on_error_goto_label_target: None,
            last_error: 0,
            last_error_pc: None,
            last_error_description: None,
            last_error_source: None,
            rnd_state: 0x50000,
            debug_runtime: None,
        }
    }

    fn clear_error_state(&mut self) {
        self.last_error = 0;
        self.last_error_pc = None;
        self.last_error_description = None;
        self.last_error_source = None;
    }

    fn route_runtime_error(
        &mut self,
        pc: usize,
        code: i32,
        detail: Option<&str>,
    ) -> Result<usize, String> {
        self.last_error = code;
        self.last_error_pc = Some(pc);
        self.last_error_description = detail.map(|s| s.to_string());
        if self.on_error_resume_next {
            // Error is handled by auto-advance; clear last_error_pc so that
            // Resume/Resume Next statements correctly detect "no pending error"
            // (VBA raises error 20 on Resume without a pending error).
            self.last_error_pc = None;
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
            CapabilityId::ProjectCatalog => 9,
            CapabilityId::ProjectReferenceProvider => 10,
            CapabilityId::ProjectMutation => 11,
            CapabilityId::ConsoleIo => 12,
        };
        53_000 + capability_code * 10 + kind_code
    }

    fn ensure_slot_count(&mut self, slot_count: usize) {
        if slot_count > self.registers.registers.len() {
            self.registers
                .registers
                .resize(slot_count, RuntimeSlot::default());
        }
    }

    pub fn snapshot_slots(&self, slot_count: usize) -> Vec<i32> {
        let end = slot_count.min(self.registers.registers.len());
        self.registers.registers[..end]
            .iter()
            .map(|value| value.project_compat_slot_i32().unwrap_or(EMPTY_TAG))
            .collect()
    }

    /// Legacy snapshot alias. Prefer `snapshot_variants` for retained
    /// value-model work.
    pub fn snapshot(&self, slot_count: usize) -> Vec<RuntimeValue> {
        self.snapshot_compat_values(slot_count)
    }

    /// Compatibility snapshot boundary that projects retained `Variant` slots
    /// to `RuntimeValue` for older tests and host surfaces.
    pub fn snapshot_compat_values(&self, slot_count: usize) -> Vec<RuntimeValue> {
        self.snapshot_variants(slot_count)
            .into_iter()
            .map(|variant| {
                variant
                    .to_runtime_value()
                    .expect("VM variant snapshot must project to RuntimeValue")
            })
            .collect()
    }

    /// Legacy snapshot alias. Prefer `snapshot_variants`.
    pub fn snapshot_values(&self, slot_count: usize) -> Vec<RuntimeValue> {
        self.snapshot_compat_values(slot_count)
    }

    /// Retained value-model snapshot API.
    pub fn snapshot_variants(&self, slot_count: usize) -> Vec<Variant> {
        let end = slot_count.min(self.registers.registers.len());
        self.registers.registers[..end]
            .iter()
            .map(|slot| match slot {
                RuntimeSlot::Variant(value) => value.clone(),
                RuntimeSlot::BindingHandle(handle) => {
                    panic!(
                        "VM register slot contains non-VBA BindingHandle {}",
                        handle.raw()
                    )
                }
            })
            .collect()
    }

    pub fn set_project_dynamic_objects(&mut self, routes: Vec<ProjectDynamicObjectRoute>) {
        self.project_dynamic_objects = routes
            .into_iter()
            .map(|route| {
                let raw = route.object_handle;
                (
                    raw,
                    ProjectDynamicObjectState {
                        object: ObjectRef::from_compat_identity(raw),
                        route,
                    },
                )
            })
            .collect();
    }

    pub fn project_dynamic_object_ref(&self, raw: i32) -> Option<ObjectRef> {
        self.project_dynamic_objects
            .get(&raw)
            .map(|state| state.object.clone())
    }

    pub fn set_project_procedure_runtime_metadata(
        &mut self,
        metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) {
        self.procedure_runtime_metadata = metadata;
    }

    pub fn set_project_com_withevents_routes(&mut self, routes: Vec<ProjectComWithEventsRoute>) {
        self.project_com_withevents_routes.clear();
        for route in routes {
            self.project_com_withevents_routes
                .entry(route.binding_token)
                .or_default()
                .push(route);
        }
    }

    pub fn debug_set_breakpoints(&mut self, breakpoints: Vec<DebugBreakpoint>) {
        if let Some(state) = &mut self.debug_runtime {
            state.breakpoints = breakpoints;
        } else {
            self.debug_runtime = Some(DebugRuntimeState {
                current_pc: 0,
                return_halts_when_stack_empty: false,
                breakpoints,
                pause_on_entry: false,
                skip_pause_once_at_pc: None,
                step_mode: None,
                last_pause: None,
            });
        }
    }

    pub fn debug_start(&mut self, bytecode: &Bytecode) -> Result<DebugRunResult, String> {
        self.reset_execution_state(bytecode.slot_count, false);
        let mut state = self.debug_runtime.take().unwrap_or(DebugRuntimeState {
            current_pc: 0,
            return_halts_when_stack_empty: false,
            breakpoints: Vec::new(),
            pause_on_entry: true,
            skip_pause_once_at_pc: None,
            step_mode: None,
            last_pause: None,
        });
        state.current_pc = 0;
        state.return_halts_when_stack_empty = false;
        state.pause_on_entry = true;
        state.skip_pause_once_at_pc = None;
        state.step_mode = None;
        state.last_pause = None;
        self.debug_runtime = Some(state);
        self.resume_debug_session(bytecode)
    }

    pub fn debug_continue(&mut self, bytecode: &Bytecode) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = None;
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_session(bytecode)
    }

    pub fn debug_step_into(&mut self, bytecode: &Bytecode) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = Some(DebugStepMode::Into);
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_session(bytecode)
    }

    pub fn debug_step_over(&mut self, bytecode: &Bytecode) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = Some(DebugStepMode::Over {
            depth: self.activation_entry_pcs.len().max(1),
        });
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_session(bytecode)
    }

    pub fn debug_step_out(&mut self, bytecode: &Bytecode) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = Some(DebugStepMode::Out {
            depth: self.activation_entry_pcs.len().saturating_sub(1),
        });
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_session(bytecode)
    }

    pub fn debug_snapshot(&self) -> Option<DebugRuntimeSnapshot> {
        let state = self.debug_runtime.as_ref()?;
        Some(DebugRuntimeSnapshot {
            last_pause: state.last_pause.clone(),
            activation_entry_pcs: self.activation_entry_pcs.clone(),
        })
    }

    pub fn execute(&mut self, bytecode: &Bytecode) -> Result<(), String> {
        self.execute_with_typed_fastpaths(bytecode, self.typed_fastpaths_default)
    }

    pub fn execute_with_typed_fastpaths(
        &mut self,
        bytecode: &Bytecode,
        typed_fastpaths: bool,
    ) -> Result<(), String> {
        self.reset_execution_state(bytecode.slot_count, false);
        self.execute_loop(bytecode, 0, 0, typed_fastpaths, false)
    }

    pub fn invoke_procedure_with_i32_args(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[i32],
    ) -> Result<(), String> {
        if arg_slots.len() != args.len() {
            return Err(format!(
                "argument shape mismatch: {} slots for {} values",
                arg_slots.len(),
                args.len()
            ));
        }
        if entry_pc >= bytecode.instructions.len() {
            return Err(format!("procedure entry out of range: {entry_pc}"));
        }
        self.reset_execution_state(bytecode.slot_count, true);
        for (slot, value) in arg_slots.iter().zip(args.iter()) {
            self.write_legacy_scalar_slot(*slot, *value)?;
        }
        self.execute_loop(
            bytecode,
            entry_pc,
            entry_pc,
            self.typed_fastpaths_default,
            true,
        )
    }

    pub fn invoke_procedure_with_values(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[RuntimeValue],
    ) -> Result<(), String> {
        let variants = args
            .iter()
            .map(RuntimeValue::to_variant)
            .collect::<Result<Vec<_>, _>>()?;
        self.invoke_procedure_with_variants(bytecode, entry_pc, arg_slots, &variants)
    }

    pub fn invoke_procedure_with_variants(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[Variant],
    ) -> Result<(), String> {
        if arg_slots.len() != args.len() {
            return Err(format!(
                "argument shape mismatch: {} slots for {} values",
                arg_slots.len(),
                args.len()
            ));
        }
        if entry_pc >= bytecode.instructions.len() {
            return Err(format!("procedure entry out of range: {entry_pc}"));
        }

        // Save caller's error frame so cross-module calls preserve the caller's
        // On Error handler. This is critical for VBA parity: On Error Resume Next
        // in the caller must catch Err.Raise from called procedures.
        let saved = ErrorFrame {
            on_error_resume_next: self.on_error_resume_next,
            on_error_goto_label_target: self.on_error_goto_label_target,
            last_error: self.last_error,
            last_error_pc: self.last_error_pc,
            last_error_description: self.last_error_description.take(),
            last_error_source: self.last_error_source.take(),
        };

        self.reset_execution_state(bytecode.slot_count, true);
        for (slot, value) in arg_slots.iter().zip(args.iter()) {
            self.write_variant_slot(*slot, value.clone())?;
        }

        let result = self.execute_loop(
            bytecode,
            entry_pc,
            entry_pc,
            self.typed_fastpaths_default,
            true,
        );

        // Restore caller's error handling mode.
        self.on_error_resume_next = saved.on_error_resume_next;
        self.on_error_goto_label_target = saved.on_error_goto_label_target;

        match result {
            Ok(()) => {
                if self.last_error == 0 {
                    self.last_error = saved.last_error;
                    self.last_error_pc = saved.last_error_pc;
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                }
                Ok(())
            }
            Err(msg) => {
                if saved.on_error_resume_next {
                    let code = msg
                        .strip_prefix("runtime error: ")
                        .and_then(|rest| {
                            rest.split(|c: char| !c.is_ascii_digit() && c != '-')
                                .next()
                                .and_then(|s| s.parse::<i32>().ok())
                        })
                        .unwrap_or(5);
                    self.last_error = code;
                    self.last_error_pc = None;
                    self.last_error_description = Some(msg);
                    self.last_error_source = None;
                    Ok(())
                } else {
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                    Err(msg)
                }
            }
        }
    }

    fn reset_execution_state(&mut self, slot_count: usize, preserve_withevents_bindings: bool) {
        self.ensure_slot_count(slot_count);
        self.call_stack.clear();
        self.activation_entry_pcs.clear();
        if !preserve_withevents_bindings {
            self.clear_all_com_withevents_state_best_effort();
            self.withevents_bindings.clear();
        }
        self.foreach_iterators.clear();
        self.next_foreach_iterator_id = 1;
        self.withevents_owner_iters.clear();
        self.on_error_resume_next = false;
        self.on_error_goto_label_target = None;
        self.clear_error_state();
    }

    fn resume_debug_session(&mut self, bytecode: &Bytecode) -> Result<DebugRunResult, String> {
        let (entry_pc, return_halts_when_stack_empty) = {
            let state = self
                .debug_runtime
                .as_ref()
                .ok_or_else(|| "debug session is not active".to_string())?;
            (state.current_pc, state.return_halts_when_stack_empty)
        };
        self.execute_loop(
            bytecode,
            entry_pc,
            self.activation_entry_pcs.last().copied().unwrap_or(0),
            self.typed_fastpaths_default,
            return_halts_when_stack_empty,
        )?;
        let Some(state) = &self.debug_runtime else {
            return Ok(DebugRunResult::Completed);
        };
        if let Some(stop) = state.last_pause.clone() {
            Ok(DebugRunResult::Paused(stop))
        } else {
            self.debug_runtime = None;
            Ok(DebugRunResult::Completed)
        }
    }

    fn debug_metadata_for_entry_pc(&self, entry_pc: usize) -> Option<&ProcedureRuntimeMetadata> {
        self.procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
    }

    fn debug_resolve_stop_location(&self, pc: usize) -> Option<DebugSourceLocation> {
        let entry_pc = self.activation_entry_pcs.last().copied().unwrap_or(0);
        let metadata = self.debug_metadata_for_entry_pc(entry_pc)?;
        let statement_index = metadata
            .statement_entry_pcs
            .iter()
            .position(|candidate| *candidate == pc)?;
        Some(DebugSourceLocation {
            module_name: metadata.module_name.clone(),
            procedure_name: metadata.procedure_name.clone(),
            entry_pc,
            statement_pc: pc,
            line_number: metadata
                .statement_line_numbers
                .get(statement_index)
                .copied(),
        })
    }

    fn debug_breakpoint_matches(
        breakpoint: &DebugBreakpoint,
        location: &DebugSourceLocation,
    ) -> bool {
        location.line_number == Some(breakpoint.line_number)
            && location
                .module_name
                .eq_ignore_ascii_case(&breakpoint.module_name)
    }

    fn maybe_pause_before_pc(&mut self, pc: usize) -> Option<DebugStop> {
        let location = self.debug_resolve_stop_location(pc)?;
        let current_depth = self.activation_entry_pcs.len().max(1);
        let state = self.debug_runtime.as_mut()?;

        if state.skip_pause_once_at_pc == Some(pc) {
            state.skip_pause_once_at_pc = None;
            return None;
        }

        let reason = if state.pause_on_entry {
            state.pause_on_entry = false;
            Some(DebugStopReason::Entry)
        } else if state
            .breakpoints
            .iter()
            .any(|breakpoint| Self::debug_breakpoint_matches(breakpoint, &location))
        {
            Some(DebugStopReason::Breakpoint)
        } else {
            match state.step_mode.clone() {
                Some(DebugStepMode::Into) => Some(DebugStopReason::Step),
                Some(DebugStepMode::Over { depth }) if current_depth <= depth => {
                    Some(DebugStopReason::Step)
                }
                Some(DebugStepMode::Out { depth }) if current_depth <= depth => {
                    Some(DebugStopReason::Step)
                }
                _ => None,
            }
        }?;

        state.current_pc = pc;
        state.step_mode = None;
        let stop = DebugStop {
            reason,
            location,
            call_stack_depth: current_depth,
        };
        state.last_pause = Some(stop.clone());
        Some(stop)
    }

    fn execute_loop(
        &mut self,
        bytecode: &Bytecode,
        start_pc: usize,
        activation_entry_pc: usize,
        typed_fastpaths: bool,
        return_halts_when_stack_empty: bool,
    ) -> Result<(), String> {
        let activation_depth = self.activation_entry_pcs.len();
        if self.activation_entry_pcs.last().copied() != Some(activation_entry_pc) {
            self.activation_entry_pcs.push(activation_entry_pc);
        }
        let mut pc = start_pc;
        let len = bytecode.instructions.len();
        while pc < len {
            if self.maybe_pause_before_pc(pc).is_some() {
                return Ok(());
            }
            match &bytecode.instructions[pc] {
                Instruction::LoadConstI32 { slot, value } => {
                    // Use compat-slot decoding for the special tag values (0=Empty,
                    // error tags, array tags) but NOT for -1 which is now
                    // properly represented via LoadNull.
                    let value = if *value == NULL_TAG {
                        Variant::from_i32(*value)
                    } else {
                        Variant::try_from_compat_slot_i32(*value)?
                    };
                    self.write_variant_slot(*slot, value)?;
                    pc += 1;
                }
                Instruction::LoadConstBool { slot, value } => {
                    self.write_variant_slot(*slot, Variant::from_bool(*value))?;
                    pc += 1;
                }
                Instruction::LoadConstString { slot, value } => {
                    self.write_variant_slot(
                        *slot,
                        Variant::from_string(BStr::from(value.clone())),
                    )?;
                    pc += 1;
                }
                Instruction::LoadConstF64 { slot, bits } => {
                    self.write_variant_slot(*slot, Variant::from_f64(f64::from_bits(*bits)))?;
                    pc += 1;
                }
                Instruction::AddConstI32 { slot, value } => {
                    if typed_fastpaths && self.fast_add_const(*slot, *value) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*slot)?;
                    let out = crate::semantics::variant_add_const_value(
                        &lhs,
                        *value,
                        "add-const operand",
                    )?;
                    self.write_variant_slot(*slot, out)?;
                    pc += 1;
                }
                Instruction::AddSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_add_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::SubConstI32 { slot, value } => {
                    if typed_fastpaths && self.fast_sub_const(*slot, *value) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*slot)?;
                    let out = crate::semantics::variant_add_const_value(
                        &lhs,
                        -*value,
                        "sub-const operand",
                    )?;
                    self.write_variant_slot(*slot, out)?;
                    pc += 1;
                }
                Instruction::SubSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_sub_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::MulSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_mul_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::DivSlots { dst, lhs, rhs } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    match crate::semantics::variant_div_values(&lhs_val, &rhs_val)? {
                        Ok(out) => {
                            self.write_variant_slot(*dst, out)?;
                            pc += 1;
                        }
                        Err(11) => {
                            pc = self.route_runtime_error(pc, 11, Some("Division by zero"))?;
                        }
                        Err(code) => {
                            pc = self.route_runtime_error(pc, code, Some("Division failed"))?;
                        }
                    }
                }
                Instruction::IntDivSlots { dst, lhs, rhs } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    match crate::semantics::variant_intdiv_values(&lhs_val, &rhs_val)? {
                        Ok(out) => {
                            self.write_variant_slot(*dst, out)?;
                            pc += 1;
                        }
                        Err(11) => {
                            pc = self.route_runtime_error(pc, 11, Some("Division by zero"))?;
                        }
                        Err(code) => {
                            pc = self.route_runtime_error(
                                pc,
                                code,
                                Some("Integer division failed"),
                            )?;
                        }
                    }
                }
                Instruction::ModSlots { dst, lhs, rhs } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    match crate::semantics::variant_mod_values(&lhs_val, &rhs_val)? {
                        Ok(out) => {
                            self.write_variant_slot(*dst, out)?;
                            pc += 1;
                        }
                        Err(11) => {
                            pc = self.route_runtime_error(pc, 11, Some("Division by zero"))?;
                        }
                        Err(code) => {
                            pc = self.route_runtime_error(pc, code, Some("Modulo failed"))?;
                        }
                    }
                }
                Instruction::PowSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_pow_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::ConcatSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_concat_values(&lhs, &rhs);
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::NegSlot { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    let out = crate::semantics::variant_neg_value(&val)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CopySlot { dst, src } => {
                    if typed_fastpaths && self.fast_copy_slot(*dst, *src) {
                        pc += 1;
                        continue;
                    }
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicLenDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "Len operand")?;
                    self.write_variant_slot(*dst, Variant::from_i32(text.len() as i32))?;
                    pc += 1;
                }
                Instruction::IntrinsicLeftDigits { dst, src, count } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&src_val, "Left src")?;
                    let count_val = self.read_variant_slot(*count)?;
                    let n =
                        crate::semantics::runtime_variant_to_i32_compat(&count_val, "Left count")
                            .and_then(|value| {
                            usize::try_from(value)
                                .map_err(|_| format!("Left count cannot be negative: {value}"))
                        })?;
                    let result = if n >= text.len() {
                        text
                    } else {
                        text[..n].to_string()
                    };
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicRightDigits { dst, src, count } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&src_val, "Right src")?;
                    let count_val = self.read_variant_slot(*count)?;
                    let n =
                        crate::semantics::runtime_variant_to_i32_compat(&count_val, "Right count")
                            .and_then(|value| {
                                usize::try_from(value)
                                    .map_err(|_| format!("Right count cannot be negative: {value}"))
                            })?;
                    let len = text.len();
                    let result = if n >= len {
                        text
                    } else {
                        text[len - n..].to_string()
                    };
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidDigits {
                    dst,
                    src,
                    start,
                    count,
                } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&src_val, "Mid src")?;
                    let start_val = self.read_variant_slot(*start)?;
                    let st =
                        crate::semantics::runtime_variant_to_i32_compat(&start_val, "Mid start")
                            .and_then(|value| {
                                usize::try_from(value)
                                    .map_err(|_| format!("Mid start cannot be negative: {value}"))
                            })?;
                    let cnt = match count {
                        Some(slot) => {
                            let cv = self.read_variant_slot(*slot)?;
                            Some(
                                crate::semantics::runtime_variant_to_i32_compat(&cv, "Mid count")
                                    .and_then(|value| {
                                    usize::try_from(value).map_err(|_| {
                                        format!("Mid count cannot be negative: {value}")
                                    })
                                })?,
                            )
                        }
                        None => None,
                    };
                    let len = text.len();
                    let begin = if st == 0 { 0 } else { (st - 1).min(len) };
                    let end = match cnt {
                        Some(c) => (begin + c).min(len),
                        None => len,
                    };
                    let result = text[begin..end].to_string();
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidStmtDigits {
                    target,
                    start,
                    count,
                    value,
                } => {
                    let target_value = self.read_variant_slot(*target)?;
                    let start_value = self.read_variant_slot(*start)?;
                    let count_value = match count {
                        Some(slot) => Some(self.read_variant_slot(*slot)?),
                        None => None,
                    };
                    let value_value = self.read_variant_slot(*value)?;
                    self.write_variant_slot(
                        *target,
                        crate::semantics::runtime_mid_stmt_variant_bounded(
                            &target_value,
                            &start_value,
                            count_value.as_ref(),
                            &value_value,
                        )?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrDigits {
                    dst,
                    haystack,
                    needle,
                    mode,
                } => {
                    let hay_val = self.read_variant_slot(*haystack)?;
                    let nee_val = self.read_variant_slot(*needle)?;
                    let h = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&hay_val, "InStr haystack")?,
                        *mode,
                    );
                    let n = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&nee_val, "InStr needle")?,
                        *mode,
                    );
                    let pos = h.find(&n).map_or(0, |idx| (idx + 1) as i32);
                    self.write_variant_slot(*dst, Variant::from_i32(pos))?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrRevDigits {
                    dst,
                    haystack,
                    needle,
                    mode,
                } => {
                    let hay_val = self.read_variant_slot(*haystack)?;
                    let nee_val = self.read_variant_slot(*needle)?;
                    let h = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&hay_val, "InStrRev haystack")?,
                        *mode,
                    );
                    let n = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&nee_val, "InStrRev needle")?,
                        *mode,
                    );
                    let pos = h.rfind(&n).map_or(0, |idx| (idx + 1) as i32);
                    self.write_variant_slot(*dst, Variant::from_i32(pos))?;
                    pc += 1;
                }
                Instruction::IntrinsicLowerDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "LCase operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.to_ascii_lowercase())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicUpperDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "UCase operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.to_ascii_uppercase())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSplitCountDigits {
                    dst,
                    src,
                    delimiter,
                } => {
                    let value = self.read_variant_slot(*src)?;
                    let delimiter = self.read_variant_slot(*delimiter)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_split_count_variant_bounded(&value, &delimiter)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicJoinDigits {
                    dst,
                    src,
                    delimiter,
                } => {
                    let value = self.read_variant_slot(*src)?;
                    let delimiter = self.read_variant_slot(*delimiter)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_join_variant_bounded(&value, &delimiter)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicReplaceDigits {
                    dst,
                    src,
                    find,
                    replace,
                } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let find_val = self.read_variant_slot(*find)?;
                    let replace_val = self.read_variant_slot(*replace)?;
                    let src_text =
                        crate::semantics::runtime_variant_to_text(&src_val, "Replace src")?;
                    let find_text =
                        crate::semantics::runtime_variant_to_text(&find_val, "Replace find")?;
                    let replace_text =
                        crate::semantics::runtime_variant_to_text(&replace_val, "Replace replace")?;
                    let result = src_text.replace(&find_text, &replace_text);
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicTrimDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "Trim operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.trim().to_string())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicLTrimDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "LTrim operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.trim_start().to_string())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRTrimDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "RTrim operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.trim_end().to_string())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicStrCompDigits {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    let l = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&lhs_val, "StrComp lhs")?,
                        *mode,
                    );
                    let r = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&rhs_val, "StrComp rhs")?,
                        *mode,
                    );
                    let result = match l.cmp(&r) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(result))?;
                    pc += 1;
                }
                Instruction::IntrinsicLikeDigits {
                    dst,
                    lhs,
                    pattern,
                    mode,
                } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let pattern = self.read_variant_slot(*pattern)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_like_variant_bounded(&lhs, &pattern, *mode)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicDateSerialDigits {
                    dst,
                    year,
                    month,
                    day,
                } => {
                    let year = self.read_variant_slot(*year)?;
                    let month = self.read_variant_slot(*month)?;
                    let day = self.read_variant_slot(*day)?;
                    let out =
                        crate::semantics::runtime_date_serial_variant_bounded(&year, &month, &day)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimeSerialDigits {
                    dst,
                    hour,
                    minute,
                    second,
                } => {
                    let hour = self.read_variant_slot(*hour)?;
                    let minute = self.read_variant_slot(*minute)?;
                    let second = self.read_variant_slot(*second)?;
                    let out = crate::semantics::runtime_time_serial_variant_bounded(
                        &hour, &minute, &second,
                    )?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateValueDigits { dst, src } => {
                    let src = self.read_variant_slot(*src)?;
                    let out = crate::semantics::runtime_variant_to_datevalue(&src)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimeValueDigits { dst, src } => {
                    let src = self.read_variant_slot(*src)?;
                    let out = crate::semantics::runtime_variant_to_timevalue(&src)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateAddDigits {
                    dst,
                    interval,
                    number,
                    date,
                } => {
                    let interval = self.read_variant_slot(*interval)?;
                    let number = self.read_variant_slot(*number)?;
                    let date = self.read_variant_slot(*date)?;
                    let out = crate::semantics::runtime_date_add_variant_bounded(
                        &interval, &number, &date,
                    )?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateDiffDigits {
                    dst,
                    interval,
                    date1,
                    date2,
                } => {
                    let interval = self.read_variant_slot(*interval)?;
                    let date1 = self.read_variant_slot(*date1)?;
                    let date2 = self.read_variant_slot(*date2)?;
                    let out = crate::semantics::runtime_date_diff_variant_bounded(
                        &interval, &date1, &date2,
                    )?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicYearDigits { dst, src } => {
                    let v = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_variant_date_year(&v)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicMonthDigits { dst, src } => {
                    let v = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_variant_date_month(&v)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicDayDigits { dst, src } => {
                    let v = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, crate::semantics::runtime_variant_date_day(&v)?)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateNowHost { dst } => {
                    match self.host_services.time_locale().date_serial_now_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicTimeNowHost { dst } => {
                    match self.host_services.time_locale().time_serial_now_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicNowHost { dst } => {
                    let date = match self.host_services.time_locale().date_serial_now_variant() {
                        Ok(value) => value,
                        Err(err) => {
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let time = match self.host_services.time_locale().time_serial_now_variant() {
                        Ok(value) => value,
                        Err(err) => {
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let value = crate::semantics::variant_host_now_value(&date, &time)?;
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimerHost { dst } => {
                    match self.host_services.time_locale().timer_ticks_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileOpenHost {
                    dst,
                    path,
                    mode,
                    file_number,
                } => {
                    let path = self.read_variant_slot(*path)?;
                    let mode_val = self.read_variant_slot(*mode)?;
                    let file_num = self.read_variant_slot(*file_number)?;
                    // Encode file_number into upper 16 bits of mode so the HAL
                    // can allocate the specific handle requested by the VBA source.
                    let mode_i32 = crate::semantics::variant_to_i32_compat(&mode_val, "Open mode")?;
                    let fnum_i32 =
                        crate::semantics::variant_to_i32_compat(&file_num, "Open file number")?;
                    let combined_mode = Variant::from_i32(mode_i32 | (fnum_i32 << 16));
                    match self.host_services.fs().open_variant(path, combined_mode) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileCloseHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().close_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileKillHost { dst, path } => {
                    let path = self.read_variant_slot(*path)?;
                    match self.host_services.fs().kill_variant(path) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
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
                        self.read_variant_slot(*slot)?
                    } else {
                        Variant::from_i32(0)
                    };
                    match self.host_services.fs().free_file_variant(selector) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileReadHost { dst, handle, count } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let count = self.read_variant_slot(*count)?;
                    match self.host_services.fs().read_bytes_variant(handle, count) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileWriteHost { dst, handle, data } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.fs().write_bytes_variant(handle, data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFilePrintHost { dst, handle, data } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.fs().print_line_variant(handle, data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicConsolePrintHost { dst, data } => {
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.console().print_line_variant(data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileInputHost { dst, handle, count } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let count = self.read_variant_slot(*count)?;
                    match self.host_services.fs().input_fields_variant(handle, count) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicConsoleInputHost { dst, count } => {
                    let count = self.read_variant_slot(*count)?;
                    match self.host_services.console().input_fields_variant(count) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileLineInputHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().line_input_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicConsoleLineInputHost { dst } => {
                    match self.host_services.console().line_input_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileEofHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().eof_variant(handle) {
                        Ok(value) => {
                            let value = crate::semantics::variant_truthy_value(&value)?;
                            self.write_variant_slot(*dst, Variant::from_bool(value))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileLofHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().lof_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileSeekHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().loc_variant(handle) {
                        Ok(value) => {
                            let value = crate::semantics::variant_to_i32_compat(&value, "Loc")?;
                            self.write_variant_slot(*dst, Variant::from_i32(value + 1))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileLocHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().loc_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicMsgBoxHost { dst, prompt, style } => {
                    let prompt = self.read_variant_slot(*prompt)?;
                    let style = if let Some(slot) = style {
                        self.read_variant_slot(*slot)?
                    } else {
                        Variant::from_i32(1)
                    };
                    match self.host_services.ui().msg_box_variant(prompt, style) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
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
                    let prompt = self.read_variant_slot(*prompt)?;
                    let default_value = if let Some(slot) = default_value {
                        self.read_variant_slot(*slot)?
                    } else {
                        Variant::from_i32(0)
                    };
                    match self
                        .host_services
                        .ui()
                        .input_box_variant(prompt, default_value)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicBeepHost { dst } => {
                    match self
                        .host_services
                        .diag()
                        .emit_variant(Variant::from_i32(7), Variant::from_i32(0))
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDebugPrintHost { dst, data } => {
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.diag().debug_print_variant(data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicStrPtr { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer = match value.vtype() {
                        oxvba_runtime::VarType::Empty | oxvba_runtime::VarType::Null => 0,
                        oxvba_runtime::VarType::String => {
                            let text = value.as_bstr().ok_or_else(|| {
                                "runtime error: invalid String Variant payload".to_string()
                            })?;
                            let utf8 = text.as_str();
                            oxvba_runtime::pointer_helpers::register_utf16_string(&utf8)?
                        }
                        _ => return Err("runtime error: 13 (Type mismatch)".to_string()),
                    };
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicVarPtr { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer = if let Some(array) = value.as_safearray() {
                        oxvba_runtime::pointer_helpers::register_array_payload_pointer(&array)?
                    } else {
                        oxvba_runtime::pointer_helpers::register_variant_pointer(&value)?
                    };
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicVarPtrStringVar { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer =
                        oxvba_runtime::pointer_helpers::register_string_variant_pointer(&value)?;
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicVarPtrVariantVar { dst, src } => {
                    let pointer = self.variant_cell_pointer(*src)?;
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicObjPtr { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer =
                        oxvba_runtime::pointer_helpers::register_object_variant_pointer(&value)?;
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicDoEventsHost { dst } => {
                    if let Some(callback) = self.pending_callback_tokens.pop_front() {
                        self.write_variant_slot(*dst, Variant::from_i32(callback.raw()))?;
                        pc += 1;
                        continue;
                    }
                    match self.host_services.events().do_events_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicAbsI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_abs_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIntI32 { dst, src } => {
                    let value = self.read_value_slot(*src)?;
                    match &value {
                        RuntimeValue::F64(f) => {
                            let floored = f.as_f64().floor() as i32;
                            self.write_variant_slot(*dst, Variant::from_i32(floored))?;
                        }
                        _ => {
                            let v = crate::semantics::runtime_value_to_i32_compat(&value, "Int")?;
                            self.write_legacy_scalar_slot(*dst, v)?;
                        }
                    }
                    pc += 1;
                }
                Instruction::IntrinsicFixI32 { dst, src } => {
                    let value = self.read_legacy_scalar_slot(*src)?;
                    self.write_legacy_scalar_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicSgnI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_sgn_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRoundI32 { dst, src, digits } => {
                    let value = self.read_variant_slot(*src)?;
                    let digits = match digits {
                        Some(slot) => Some(self.read_variant_slot(*slot)?),
                        None => None,
                    };
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_round_variant_bounded(&value, digits.as_ref())?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSqrI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_sqr_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSinI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_sin_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicCosI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_cos_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicLogI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_log_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicExpI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_exp_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicAtnI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_atn_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicTanI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_tan_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicChrDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_chr_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicAscDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_asc_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSpaceDigits { dst, count } => {
                    let count = self.read_variant_slot(*count)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_space_variant_bounded(&count)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicStringRepeatDigits { dst, count, ch } => {
                    let count_val = self.read_variant_slot(*count)?;
                    let ch_val = self.read_variant_slot(*ch)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_string_repeat_variant_bounded(
                            &count_val, &ch_val,
                        )?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicCStrDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    match oxvba_runtime::variant_to_vba_string(&src_val) {
                        Ok(result) => {
                            self.write_variant_slot(*dst, Variant::from_string(result))?;
                            pc += 1;
                        }
                        Err(msg) => {
                            pc = self.route_runtime_error(pc, 13, Some(&msg))?;
                        }
                    }
                }
                Instruction::IntrinsicStrFuncDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    match crate::semantics::runtime_variant_to_vba_str(&src_val) {
                        Ok(result) => {
                            self.write_variant_slot(*dst, result)?;
                            pc += 1;
                        }
                        Err(msg) => {
                            pc = self.route_runtime_error(pc, 13, Some(&msg))?;
                        }
                    }
                }
                Instruction::IntrinsicValDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let result = crate::semantics::runtime_val_variant_bounded(&src_val)?;
                    self.write_variant_slot(*dst, result)?;
                    pc += 1;
                }
                Instruction::IntrinsicCDateValue { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let result = crate::semantics::runtime_variant_to_cdate(&src_val)?;
                    self.write_variant_slot(*dst, result)?;
                    pc += 1;
                }
                Instruction::IntrinsicHexDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_hex_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicOctDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_oct_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicWeekdayDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_variant_date_weekday(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicMonthNameDigits { dst, src } => {
                    let month = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_month_name_variant_bounded(&month)?,
                    )?;
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
                    let rate = self.read_legacy_scalar_slot(*rate)?;
                    let nper = self.read_legacy_scalar_slot(*nper)?;
                    let pmt = self.read_legacy_scalar_slot(*pmt)?;
                    let pv = match pv {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::fv_i32(rate, nper, pmt, pv, due)),
                    )?;
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
                    let rate = self.read_legacy_scalar_slot(*rate)?;
                    let nper = self.read_legacy_scalar_slot(*nper)?;
                    let pmt = self.read_legacy_scalar_slot(*pmt)?;
                    let fv = match fv {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::pv_i32(rate, nper, pmt, fv, due)),
                    )?;
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
                    let rate = self.read_legacy_scalar_slot(*rate)?;
                    let nper = self.read_legacy_scalar_slot(*nper)?;
                    let pv = self.read_legacy_scalar_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::pmt_i32(rate, nper, pv, fv, due)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicNpvI32 { dst, rate, values } => {
                    let rate = self.read_legacy_scalar_slot(*rate)?;
                    let mut cash_flows = Vec::with_capacity(values.len());
                    for slot in values {
                        cash_flows.push(self.read_legacy_scalar_slot(*slot)?);
                    }
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::npv_i32(rate, &cash_flows)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIrrI32 { dst, value, guess } => {
                    let value = self.read_legacy_scalar_slot(*value)?;
                    let guess = match guess {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 10,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::irr_i32(value, guess)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicMirrI32 {
                    dst,
                    value,
                    finance_rate,
                    reinvest_rate,
                } => {
                    let value = self.read_legacy_scalar_slot(*value)?;
                    let finance_rate = self.read_legacy_scalar_slot(*finance_rate)?;
                    let reinvest_rate = self.read_legacy_scalar_slot(*reinvest_rate)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::mirr_i32(
                            value,
                            finance_rate,
                            reinvest_rate,
                        )),
                    )?;
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
                    let nper = self.read_legacy_scalar_slot(*nper)?;
                    let pmt = self.read_legacy_scalar_slot(*pmt)?;
                    let pv = self.read_legacy_scalar_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    let guess = match guess {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 10,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::rate_i32(
                            nper, pmt, pv, fv, due, guess,
                        )),
                    )?;
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
                    let rate = self.read_legacy_scalar_slot(*rate)?;
                    let pmt = self.read_legacy_scalar_slot(*pmt)?;
                    let pv = self.read_legacy_scalar_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_legacy_scalar_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_compat_slot_i32(Self::nper_i32(rate, pmt, pv, fv, due)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayLiteral { dst, values } => {
                    let elements = values
                        .iter()
                        .map(|slot| self.read_variant_slot(*slot))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_safearray(SafeArray::from_variants(elements)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayAppend { dst, array, item } => {
                    let current = self.read_variant_slot(*array)?;
                    let item = self.read_variant_slot(*item)?;
                    let mut elements = if let Some(array) = current.as_safearray() {
                        array.variant_elements().unwrap_or_default()
                    } else if current.vtype() == oxvba_runtime::VarType::Empty
                        || current.as_i32() == Some(0)
                    {
                        Vec::new()
                    } else {
                        pc = self.route_runtime_error(
                            pc,
                            13,
                            Some(&format!(
                                "__oxvba_array_append expects an array or empty source, got {current:?}"
                            )),
                        )?;
                        continue;
                    };
                    elements.push(item);
                    self.write_variant_slot(
                        *dst,
                        Variant::from_safearray(SafeArray::from_variants(elements)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayResize {
                    dst,
                    upper_bounds,
                    lower_bounds,
                    element_type,
                } => {
                    let mut resolved_upper_bounds = Vec::with_capacity(upper_bounds.len());
                    let mut upper_error = None;
                    for upper_bound in upper_bounds {
                        match crate::semantics::runtime_value_to_i32_compat(
                            &self.read_value_slot(*upper_bound)?,
                            "ReDim upper bound",
                        ) {
                            Ok(value) => resolved_upper_bounds.push(value),
                            Err(detail) => {
                                upper_error = Some(detail);
                                break;
                            }
                        }
                    }
                    if let Some(detail) = upper_error {
                        pc = self.route_runtime_error(pc, 13, Some(&detail))?;
                        continue;
                    }
                    let array = match runtime_resized_array(
                        lower_bounds,
                        &resolved_upper_bounds,
                        *element_type,
                    ) {
                        Ok(array) => array,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*dst, Variant::from_safearray(array))?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayResizePreserve {
                    dst,
                    upper_bounds,
                    lower_bounds,
                    element_type,
                } => {
                    let mut resolved_upper_bounds = Vec::with_capacity(upper_bounds.len());
                    let mut upper_error = None;
                    for upper_bound in upper_bounds {
                        match crate::semantics::runtime_value_to_i32_compat(
                            &self.read_value_slot(*upper_bound)?,
                            "ReDim Preserve upper bound",
                        ) {
                            Ok(value) => resolved_upper_bounds.push(value),
                            Err(detail) => {
                                upper_error = Some(detail);
                                break;
                            }
                        }
                    }
                    if let Some(detail) = upper_error {
                        pc = self.route_runtime_error(pc, 13, Some(&detail))?;
                        continue;
                    }
                    let current = self.read_variant_slot(*dst)?;
                    let array = match runtime_resized_array_preserve(
                        &current,
                        lower_bounds,
                        &resolved_upper_bounds,
                        *element_type,
                    ) {
                        Ok(array) => array,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*dst, Variant::from_safearray(array))?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayGet {
                    dst,
                    array,
                    indices,
                } => {
                    let array_value = self.read_variant_slot(*array)?;
                    let index_values = indices
                        .iter()
                        .map(|slot| self.read_value_slot(*slot))
                        .collect::<Result<Vec<_>, _>>()?;
                    let value = match crate::semantics::runtime_array_get_variant(
                        &array_value,
                        &index_values,
                        "array index",
                    ) {
                        Ok(value) => value,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicArraySet {
                    array,
                    indices,
                    src,
                } => {
                    let array_value = self.read_variant_slot(*array)?;
                    let index_values = indices
                        .iter()
                        .map(|slot| self.read_value_slot(*slot))
                        .collect::<Result<Vec<_>, _>>()?;
                    let src_value = self.read_variant_slot(*src)?;
                    let value = match crate::semantics::runtime_array_set_variant(
                        &array_value,
                        &index_values,
                        &src_value,
                        "array index",
                    ) {
                        Ok(value) => value,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*array, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicForEachInit { iter, src } => {
                    let iterable = self.read_variant_slot(*src)?;
                    match self.materialize_foreach_items(bytecode, typed_fastpaths, &iterable) {
                        Ok(items) => {
                            let id = self.next_foreach_iterator_id;
                            self.next_foreach_iterator_id =
                                self.next_foreach_iterator_id.saturating_add(1);
                            self.foreach_iterators.insert(
                                id,
                                ForEachIteratorState {
                                    items,
                                    next_index: 0,
                                },
                            );
                            self.write_value_slot(*iter, RuntimeValue::I32(id))?;
                            pc += 1;
                        }
                        Err(err) => {
                            pc = self.route_runtime_error(pc, err.code, Some(&err.detail))?
                        }
                    }
                }
                Instruction::IntrinsicForEachNext {
                    iter,
                    item,
                    has_value,
                } => {
                    let iter_id = crate::semantics::runtime_value_to_i32_compat(
                        &self.read_value_slot(*iter)?,
                        "For Each iterator slot",
                    )?;
                    let next = if let Some(state) = self.foreach_iterators.get_mut(&iter_id) {
                        if state.next_index < state.items.len() {
                            let value = state.items[state.next_index].clone();
                            state.next_index += 1;
                            Some(value)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if next.is_none() {
                        self.foreach_iterators.remove(&iter_id);
                    }
                    self.write_variant_slot(*has_value, Variant::from_bool(next.is_some()))?;
                    self.write_runtime_slot(*item, next.unwrap_or_default())?;
                    pc += 1;
                }
                Instruction::IntrinsicLBoundArray { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out =
                        crate::semantics::runtime_array_lbound_variant(&value, "LBound operand")
                            .map_err(|detail| format!("runtime error: 13 ({detail})"))?;
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::IntrinsicUBoundArray { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out =
                        crate::semantics::runtime_array_ubound_variant(&value, "UBound operand")
                            .map_err(|detail| format!("runtime error: 13 ({detail})"))?;
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsArrayTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(
                            if matches!(value.vtype(), oxvba_runtime::VarType::ArrayVariant) {
                                1
                            } else {
                                0
                            },
                        ),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicVarTypeTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(crate::semantics::runtime_vartype_tag_bounded_variant(
                            &value,
                        )),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicVarType { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    let code = crate::semantics::runtime_vartype_compat_bounded_variant(&val);
                    self.write_variant_slot(*dst, Variant::from_i32(code))?;
                    pc += 1;
                }
                Instruction::IntrinsicTypeNameTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(crate::semantics::runtime_typename_tag_bounded_variant(
                            &value,
                        )),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNumericTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(
                            crate::semantics::runtime_is_numeric_tag_bounded_variant(&value),
                        ),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNumeric { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    let is_numeric = matches!(
                        val.vtype(),
                        oxvba_runtime::VarType::Integer
                            | oxvba_runtime::VarType::Long
                            | oxvba_runtime::VarType::LongLong
                            | oxvba_runtime::VarType::Single
                            | oxvba_runtime::VarType::Double
                            | oxvba_runtime::VarType::Date
                            | oxvba_runtime::VarType::Currency
                            | oxvba_runtime::VarType::Decimal
                            | oxvba_runtime::VarType::Boolean
                            | oxvba_runtime::VarType::Byte
                    );
                    self.write_variant_slot(*dst, Variant::from_bool(is_numeric))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsError { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_bool(matches!(val.vtype(), oxvba_runtime::VarType::Error)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsDateTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out = if crate::semantics::runtime_variant_is_date(&value) {
                        1
                    } else {
                        0
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsObjectTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out = if crate::semantics::runtime_variant_is_object(&value) {
                        1
                    } else {
                        0
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::ValidateRuntimeAssignment {
                    src,
                    intent,
                    target_kind,
                    target_name,
                    target_type_name,
                } => {
                    let value = self.read_value_slot(*src)?;
                    Self::validate_runtime_assignment(
                        &value,
                        *intent,
                        *target_kind,
                        target_name,
                        target_type_name,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicShellHost { dst, command } => {
                    let command = self.read_variant_slot(*command)?;
                    match self
                        .host_services
                        .process()
                        .shell_variant(command, Variant::from_i32(0))
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicEnvironHost { dst, key } => {
                    let key = self.read_variant_slot(*key)?;
                    match self.host_services.process().environ_variant(key) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDirHost { dst, path } => {
                    let path = self.read_variant_slot(*path)?;
                    match self
                        .host_services
                        .process()
                        .dir_variant(path, Variant::from_i32(0))
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicCollectionAdd { dst, count, item } => {
                    let count = self.read_legacy_scalar_slot(*count)?;
                    let _item = self.read_legacy_scalar_slot(*item)?;
                    self.write_legacy_scalar_slot(*dst, (count + 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionItem { dst, count, index } => {
                    let count = self.read_legacy_scalar_slot(*count)?;
                    let index = self.read_legacy_scalar_slot(*index)?;
                    let out = if index >= 1 && index <= count {
                        index
                    } else {
                        0
                    };
                    self.write_legacy_scalar_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionRemove { dst, count, index } => {
                    let count = self.read_legacy_scalar_slot(*count)?;
                    let _index = self.read_legacy_scalar_slot(*index)?;
                    self.write_legacy_scalar_slot(*dst, (count - 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionCount { dst, count } => {
                    let count = self.read_legacy_scalar_slot(*count)?;
                    self.write_legacy_scalar_slot(*dst, count.max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCreateObjectHost { dst, prog_id } => {
                    let prog_id = self.read_variant_slot(*prog_id)?;
                    match self.host_services.com().create_object_variant(prog_id) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDispatchInvokeHost {
                    dst,
                    object,
                    member,
                    args,
                } => {
                    let object_value = self.read_variant_slot(*object)?;
                    // VBA error 91: Object variable or With block variable not set.
                    if matches!(object_value.vtype(), oxvba_runtime::VarType::Empty) {
                        pc = self.route_runtime_error(
                            pc,
                            91,
                            Some("Object variable or With block variable not set"),
                        )?;
                        continue;
                    }
                    let object = match crate::semantics::variant_to_com_object(
                        &object_value,
                        "dispatch_invoke.object",
                    ) {
                        Ok(object) => {
                            // Also check for Nothing (ObjectRef raw identity 0).
                            if object.raw() == 0 {
                                pc = self.route_runtime_error(
                                    pc,
                                    91,
                                    Some("Object variable or With block variable not set"),
                                )?;
                                continue;
                            }
                            object
                        }
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "dispatch_invoke",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let member_value = self.read_variant_slot(*member)?;
                    let mut request = DynamicCallRequest {
                        object,
                        member: match crate::semantics::variant_to_dynamic_member_selector(
                            &member_value,
                            "dispatch_invoke.member",
                        ) {
                            Ok(member) => member,
                            Err(detail) => {
                                let err = HalError::adapter_fault(
                                    self.host_services.profile(),
                                    CapabilityId::ComActivationDispatch,
                                    "dispatch_invoke",
                                    detail,
                                );
                                pc = self.route_host_error(pc, err)?;
                                continue;
                            }
                        },
                        args: Vec::new(),
                        call_kind_hint: None,
                    };
                    for arg in args {
                        request.args.push(DynamicCallArg {
                            value: arg
                                .slot
                                .map(|slot| self.read_variant_slot(slot))
                                .transpose()?
                                .map(DynamicValue::from_variant),
                            name: arg.name.clone(),
                        });
                    }
                    match self.try_invoke_project_dynamic(bytecode, typed_fastpaths, &request) {
                        Ok(Some(value)) => {
                            self.write_runtime_slot(*dst, value)?;
                            pc += 1;
                            continue;
                        }
                        Ok(None) => {}
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "dispatch_invoke",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    }
                    let bridge = HalComDynamicBridge::new(
                        self.host_services.profile(),
                        self.host_services.com(),
                    );
                    match bridge.invoke_dynamic(&request) {
                        Ok(value) => {
                            self.write_variant_slot(
                                *dst,
                                Self::normalize_dynamic_result_variant(value.variant()),
                            )?;
                            self.pump_project_com_withevents_callbacks(bytecode, typed_fastpaths)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComSubscribeEventHost { dst, object, event } => {
                    let object = self.read_variant_slot(*object)?;
                    let event = self.read_variant_slot(*event)?;
                    let object = match crate::semantics::variant_to_com_object(
                        &object,
                        "com_subscribe_event.object",
                    ) {
                        Ok(object) => object,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "subscribe_event",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let event = match crate::semantics::variant_to_com_member_token(
                        &event,
                        "com_subscribe_event.event",
                    ) {
                        Ok(event) => event,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "subscribe_event",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self.host_services.com().subscribe_event(object, event) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, Variant::from_i32(value.raw()))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComUnsubscribeEventHost { dst, subscription } => {
                    let subscription = self.read_variant_slot(*subscription)?;
                    let subscription = match crate::semantics::variant_to_com_subscription_token(
                        &subscription,
                        "com_unsubscribe_event.subscription",
                    ) {
                        Ok(subscription) => subscription,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "unsubscribe_event",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .unsubscribe_event_variant(subscription)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComEventCallbackSubscriptionHost { dst, callback } => {
                    let callback = self.read_variant_slot(*callback)?;
                    let callback = match crate::semantics::variant_to_com_callback_token(
                        &callback,
                        "com_event_callback_subscription.callback",
                    ) {
                        Ok(callback) => callback,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "event_callback_subscription",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .event_callback_subscription(callback)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, Variant::from_i32(value.raw()))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComEventCallbackArgHost {
                    dst,
                    callback,
                    index,
                } => {
                    let callback = self.read_variant_slot(*callback)?;
                    let index = self.read_variant_slot(*index)?;
                    let callback = match crate::semantics::variant_to_com_callback_token(
                        &callback,
                        "com_event_callback_arg.callback",
                    ) {
                        Ok(callback) => callback,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "event_callback_arg",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let index = match crate::semantics::variant_to_usize_index(
                        &index,
                        "com_event_callback_arg.index",
                    ) {
                        Ok(index) => index,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "event_callback_arg",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .event_callback_variant(callback, index)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComReleaseEventCallbackHost { dst, callback } => {
                    let callback = self.read_variant_slot(*callback)?;
                    let callback = match crate::semantics::variant_to_com_callback_token(
                        &callback,
                        "com_release_event_callback.callback",
                    ) {
                        Ok(callback) => callback,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "release_event_callback",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .release_event_callback_variant(callback)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicInvokeSymbolHost {
                    dst,
                    descriptor_id,
                    symbol,
                    args,
                    writeback_slots,
                } => {
                    let arg_variants: Vec<Variant> = args
                        .iter()
                        .map(|slot| self.read_variant_slot(*slot))
                        .collect::<Result<_, _>>()?;

                    if bytecode.external_call_descriptors.is_empty() {
                        let first_arg = arg_variants
                            .first()
                            .cloned()
                            .unwrap_or_else(|| Variant::from_i32(0));
                        match self
                            .host_services
                            .dynlink()
                            .invoke_symbol_variant(*symbol, &first_arg)
                        {
                            Ok(value) => {
                                self.write_variant_slot(*dst, value)?;
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

                    let param_type_strings: Vec<String> = descriptor
                        .param_types
                        .iter()
                        .map(|pt| format!("{:?}", pt))
                        .collect();
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
                        param_count: descriptor.param_count,
                        param_types: &param_type_strings,
                        param_by_ref: &descriptor.param_by_ref,
                        return_type: descriptor
                            .return_type
                            .as_ref()
                            .map(|rt| Cow::Owned(format!("{:?}", rt))),
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

                    if arg_variants.len() > 1 || !writeback_slots.is_empty() {
                        match self
                            .host_services
                            .dynlink()
                            .invoke_descriptor_variants(&view, &arg_variants)
                        {
                            Ok((ret_value, wb_values)) => {
                                self.write_variant_slot(*dst, ret_value)?;
                                if let Err(detail) = self.apply_external_writebacks(
                                    writeback_slots,
                                    &arg_variants,
                                    &wb_values,
                                ) {
                                    let err = HalError::adapter_fault(
                                        self.host_services.profile(),
                                        CapabilityId::DynamicLinking,
                                        "invoke_descriptor",
                                        detail,
                                    );
                                    pc = self.route_host_error(pc, err)?;
                                    continue;
                                }
                                pc += 1;
                            }
                            Err(err) => pc = self.route_host_error(pc, err)?,
                        }
                    } else {
                        match self
                            .host_services
                            .dynlink()
                            .invoke_descriptor_variants(&view, &arg_variants)
                        {
                            Ok((ret_value, wb_values)) => {
                                self.write_variant_slot(*dst, ret_value)?;
                                if let Err(detail) = self.apply_external_writebacks(
                                    writeback_slots,
                                    &arg_variants,
                                    &wb_values,
                                ) {
                                    let err = HalError::adapter_fault(
                                        self.host_services.profile(),
                                        CapabilityId::DynamicLinking,
                                        "invoke_descriptor",
                                        detail,
                                    );
                                    pc = self.route_host_error(pc, err)?;
                                    continue;
                                }
                                pc += 1;
                            }
                            Err(err) => pc = self.route_host_error(pc, err)?,
                        }
                    }
                }
                Instruction::IntrinsicWithEventsGet {
                    dst,
                    owner,
                    binding,
                } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let binding = self.read_value_slot(*binding)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    let binding = Self::withevents_binding_handle(&binding, "binding")?;
                    let key = Self::withevents_binding_key(&owner, binding);
                    let value = self
                        .withevents_bindings
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| RuntimeSlot::Variant(Variant::from_i32(0)));
                    self.write_runtime_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsSet {
                    dst,
                    owner,
                    binding,
                    value,
                } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let binding = self.read_value_slot(*binding)?;
                    let value = self.read_variant_slot(*value)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    let binding = Self::withevents_binding_handle(&binding, "binding")?;
                    let key = Self::withevents_binding_key(&owner, binding);
                    self.clear_com_withevents_binding_subscriptions(key)?;
                    if value.as_i32() == Some(0) {
                        self.withevents_bindings.remove(&key);
                    } else {
                        self.withevents_bindings
                            .insert(key, RuntimeSlot::Variant(value.clone()));
                        self.sync_project_com_withevents_binding(owner, binding, &value)?;
                    }
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsClearOwner { dst, owner } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    self.clear_com_withevents_owner_subscriptions(owner.clone())?;
                    self.withevents_bindings.retain(|key, _| {
                        Self::withevents_owner_from_key(*key).raw() != owner.raw()
                    });
                    self.write_variant_slot(*dst, Variant::from_i32(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsFirstOwner {
                    dst,
                    source,
                    binding,
                } => {
                    let source_slot = *source;
                    let source = self.read_variant_slot(source_slot)?;
                    let source_variant = if source.as_i32() == Some(0)
                        || source.as_i64() == Some(0)
                        || source.as_bool() == Some(false)
                    {
                        None
                    } else {
                        Some(source)
                    };
                    let binding = self.read_value_slot(*binding)?;
                    let binding = Self::withevents_binding_handle(&binding, "binding")?;
                    let mut owners =
                        self.withevents_matching_owners(source_variant.as_ref(), binding);
                    owners.sort_unstable_by_key(|owner| owner.raw());
                    if owners.is_empty() {
                        self.write_variant_slot(*dst, Variant::from_i32(0))?;
                    } else {
                        let first = owners[0].clone();
                        self.withevents_owner_iters.push(WithEventsOwnerIterator {
                            owners,
                            next_index: 1,
                        });
                        self.write_variant_slot(*dst, Variant::from_object_ref(first))?;
                    }
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsNextOwner { dst } => {
                    let next = if let Some(iter) = self.withevents_owner_iters.last_mut() {
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
                        let _ = self.withevents_owner_iters.pop();
                    }
                    let result = next
                        .map(Variant::from_object_ref)
                        .unwrap_or_else(|| Variant::from_i32(0));
                    self.write_variant_slot(*dst, result)?;
                    pc += 1;
                }
                Instruction::CmpEqSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l == r) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::typed_compare_values(&lhs, &rhs, *mode, |ord| {
                        ord == std::cmp::Ordering::Equal
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpNeSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l != r) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::typed_compare_values(&lhs, &rhs, *mode, |ord| {
                        ord != std::cmp::Ordering::Equal
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpLtSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l < r) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::typed_compare_values(&lhs, &rhs, *mode, |ord| {
                        ord == std::cmp::Ordering::Less
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpLeSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l <= r) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::typed_compare_values(&lhs, &rhs, *mode, |ord| {
                        ord != std::cmp::Ordering::Greater
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpGtSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l > r) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::typed_compare_values(&lhs, &rhs, *mode, |ord| {
                        ord == std::cmp::Ordering::Greater
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpGeSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if typed_fastpaths && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l >= r) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::typed_compare_values(&lhs, &rhs, *mode, |ord| {
                        ord != std::cmp::Ordering::Less
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::LoadErrNumber { slot } => {
                    self.write_variant_slot(*slot, Variant::from_i32(self.last_error))?;
                    pc += 1;
                }
                Instruction::LoadErrDescription { slot } => {
                    let text = self
                        .last_error_description
                        .as_deref()
                        .unwrap_or("")
                        .to_string();
                    self.write_variant_slot(*slot, Variant::from_string(BStr::from(text)))?;
                    pc += 1;
                }
                Instruction::LoadErrSource { slot } => {
                    let text = self.last_error_source.as_deref().unwrap_or("").to_string();
                    self.write_variant_slot(*slot, Variant::from_string(BStr::from(text)))?;
                    pc += 1;
                }
                Instruction::IntrinsicTypeOfIs {
                    dst,
                    object_slot,
                    type_name,
                } => {
                    let val = self.read_variant_slot(*object_slot)?;
                    let is_match = match val.as_object_ref() {
                        Some(handle) => {
                            if let Some(state) = self.project_dynamic_objects.get(&handle.raw()) {
                                state.route.module_name.eq_ignore_ascii_case(type_name)
                                    || state
                                        .route
                                        .implements_interfaces
                                        .iter()
                                        .any(|iface| iface.eq_ignore_ascii_case(type_name))
                            } else {
                                false
                            }
                        }
                        None => false,
                    };
                    self.write_variant_slot(*dst, Variant::from_bool(is_match))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNull { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_bool(matches!(val.vtype(), oxvba_runtime::VarType::Null)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsEmpty { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_bool(matches!(val.vtype(), oxvba_runtime::VarType::Empty)),
                    )?;
                    pc += 1;
                }
                Instruction::LoadNull { slot } => {
                    self.write_variant_slot(*slot, Variant::null())?;
                    pc += 1;
                }
                Instruction::BoolNot { dst, src } => {
                    let src = self.read_value_slot(*src)?;
                    let out = !Self::legacy_truthy_value(&src)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::BoolAnd { dst, lhs, rhs } => {
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::legacy_truthy_value(&lhs)? && Self::legacy_truthy_value(&rhs)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::BoolOr { dst, lhs, rhs } => {
                    let lhs = self.read_value_slot(*lhs)?;
                    let rhs = self.read_value_slot(*rhs)?;
                    let out = Self::legacy_truthy_value(&lhs)? || Self::legacy_truthy_value(&rhs)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::JumpIfZero {
                    cond_slot,
                    target_pc,
                } => {
                    let cond = self.read_value_slot(*cond_slot)?;
                    pc = Self::next_pc_for_jump_if_zero_value(&cond, *target_pc, len, pc)?;
                }
                Instruction::Jump { target_pc } => {
                    pc = Self::next_pc_for_jump(*target_pc, len)?;
                }
                Instruction::CallProc { target_pc } => {
                    if *target_pc >= bytecode.instructions.len() {
                        return Err(format!("call target out of range: {target_pc}"));
                    }
                    // Save caller's error frame and clear for new procedure.
                    let saved = ErrorFrame {
                        on_error_resume_next: self.on_error_resume_next,
                        on_error_goto_label_target: self.on_error_goto_label_target,
                        last_error: self.last_error,
                        last_error_pc: self.last_error_pc,
                        last_error_description: self.last_error_description.take(),
                        last_error_source: self.last_error_source.take(),
                    };
                    self.call_stack.push((pc + 1, saved));
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = None;
                    self.clear_error_state();
                    self.activation_entry_pcs.push(*target_pc);
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
                    if self.last_error_pc.is_none() {
                        // VBA error 20: Resume without error.
                        pc = self.route_runtime_error(pc, 20, Some("Resume without error"))?;
                    } else {
                        // Jump to the statement after the one that caused the error.
                        let resume_target = self.last_error_pc.unwrap() + 1;
                        self.clear_error_state();
                        pc = resume_target;
                    }
                }
                Instruction::Resume => {
                    if let Some(target_pc) = self.last_error_pc {
                        self.clear_error_state();
                        pc = target_pc;
                    } else {
                        // VBA error 20: Resume without error.
                        pc = self.route_runtime_error(pc, 20, Some("Resume without error"))?;
                    }
                }
                Instruction::ResumeLabel { target_pc } => {
                    if *target_pc >= len {
                        return Err(format!("resume target out of range: {target_pc}"));
                    }
                    if self.last_error_pc.is_none() {
                        // VBA error 20: Resume without error.
                        pc = self.route_runtime_error(pc, 20, Some("Resume without error"))?;
                    } else {
                        self.clear_error_state();
                        pc = *target_pc;
                    }
                }
                Instruction::RaiseError { code } => {
                    pc = self.route_runtime_error(pc, *code, None)?;
                }
                Instruction::ClearErr => {
                    self.clear_error_state();
                    pc += 1;
                }
                Instruction::Return => {
                    if let Some((return_pc, saved_frame)) = self.call_stack.pop() {
                        if !self.activation_entry_pcs.is_empty() {
                            self.activation_entry_pcs.pop();
                        }
                        // Restore caller's error-handling state.
                        self.on_error_resume_next = saved_frame.on_error_resume_next;
                        self.on_error_goto_label_target = saved_frame.on_error_goto_label_target;
                        self.last_error = saved_frame.last_error;
                        self.last_error_pc = saved_frame.last_error_pc;
                        self.last_error_description = saved_frame.last_error_description;
                        self.last_error_source = saved_frame.last_error_source;
                        pc = return_pc;
                    } else if return_halts_when_stack_empty {
                        self.activation_entry_pcs.truncate(activation_depth);
                        break;
                    } else {
                        return Err("return with empty call stack".to_string());
                    }
                }
                Instruction::IntrinsicStrConvDigits {
                    dst,
                    src,
                    conversion,
                } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let conv_val = self.read_variant_slot(*conversion)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_strconv_variant_bounded(&src_val, &conv_val)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRndDigits { dst, seed } => {
                    if let Some(seed_slot) = seed {
                        let seed_val = self.read_value_slot(*seed_slot)?;
                        let seed_val =
                            crate::semantics::runtime_random_seed_bounded(&seed_val, "Rnd seed")?;
                        if seed_val < 0 {
                            self.rnd_state = (seed_val as u32) & 0x00FF_FFFF;
                        } else if seed_val == 0 {
                            let result = self.rnd_state as f64 / 16_777_216.0;
                            self.write_variant_slot(*dst, Variant::from_f64(result))?;
                            pc += 1;
                            continue;
                        }
                    }
                    self.rnd_state = self
                        .rnd_state
                        .wrapping_mul(0x43FD_43FD)
                        .wrapping_add(0x0026_9EC3)
                        & 0x00FF_FFFF;
                    let result = self.rnd_state as f64 / 16_777_216.0;
                    self.write_variant_slot(*dst, Variant::from_f64(result))?;
                    pc += 1;
                }
                Instruction::IntrinsicRandomizeDigits { dst, seed } => {
                    if let Some(seed_slot) = seed {
                        let seed_val = self.read_value_slot(*seed_slot)?;
                        let seed_val = crate::semantics::runtime_random_seed_bounded(
                            &seed_val,
                            "Randomize seed",
                        )?;
                        self.rnd_state = (seed_val as u32) & 0x00FF_FFFF;
                    } else {
                        self.rnd_state = 0x50000;
                    }
                    self.write_variant_slot(*dst, Variant::from_i32(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicFormatDigits {
                    dst,
                    value,
                    format_string,
                } => {
                    let val = self.read_variant_slot(*value)?;
                    let fmt_variant = if let Some(fmt_slot) = format_string {
                        Some(self.read_variant_slot(*fmt_slot)?)
                    } else {
                        None
                    };
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_format_variant_bounded(
                            &val,
                            fmt_variant.as_ref(),
                        )?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicStrReverseDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let s =
                        crate::semantics::runtime_variant_to_text(&src_val, "StrReverse source")?;
                    let result: String = s.chars().rev().collect();
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IncSlot { slot } => {
                    if typed_fastpaths && self.fast_add_const(*slot, 1) {
                        pc += 1;
                        continue;
                    }
                    let value = self.read_variant_slot(*slot)?;
                    let out = crate::semantics::variant_increment_value(&value)?;
                    self.write_variant_slot(*slot, out)?;
                    pc += 1;
                }
                Instruction::Halt => break,
            }
        }
        self.activation_entry_pcs.truncate(activation_depth);
        Ok(())
    }

    fn read_legacy_scalar_slot(&self, slot: usize) -> Result<i32, String> {
        self.read_value_slot(slot)?
            .project_compat_slot_i32()
            .map_err(|detail| {
                format!("runtime value in slot {slot} cannot enter legacy i32 lane: {detail}")
            })
    }

    fn write_legacy_scalar_slot(&mut self, slot: usize, value: i32) -> Result<(), String> {
        self.write_variant_slot(slot, Variant::from_compat_slot_i32(value))
    }

    fn read_value_slot(&self, slot: usize) -> Result<RuntimeValue, String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot].to_runtime_value()
    }

    fn read_variant_slot(&self, slot: usize) -> Result<Variant, String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        match &self.registers.registers[slot] {
            RuntimeSlot::Variant(value) => Ok(value.clone()),
            RuntimeSlot::BindingHandle(handle) => Err(format!(
                "slot {slot} contains internal BindingHandle {} and cannot be passed as a Variant",
                handle.raw()
            )),
        }
    }

    fn variant_cell_pointer(&self, slot: usize) -> Result<i64, String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot].variant_cell_pointer()
    }

    fn write_value_slot(&mut self, slot: usize, value: RuntimeValue) -> Result<(), String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = RuntimeSlot::from_runtime_value(value)?;
        Ok(())
    }

    fn write_semantic_value_slot(
        &mut self,
        slot: usize,
        value: RuntimeValue,
    ) -> Result<(), String> {
        let value = Variant::try_from_runtime_value(&value)?;
        self.write_variant_slot(slot, value)
    }

    fn write_variant_slot(&mut self, slot: usize, value: Variant) -> Result<(), String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = RuntimeSlot::Variant(value);
        Ok(())
    }

    fn write_runtime_slot(&mut self, slot: usize, value: RuntimeSlot) -> Result<(), String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = value;
        Ok(())
    }

    fn apply_external_writebacks(
        &mut self,
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
                    Variant::try_from_runtime_value(
                        &oxvba_runtime::pointer_helpers::read_back_byte_array_payload(pointer)?,
                    )?
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
                    Variant::try_from_runtime_value(
                        &oxvba_runtime::pointer_helpers::read_back_string_payload(pointer)?,
                    )?
                }
            };
            self.write_variant_slot(writeback.source_slot, value)?;
        }
        Ok(())
    }

    fn typed_fastpaths_enabled_from_env() -> bool {
        std::env::var("OXVBA_DISABLE_TYPED_FASTPATH")
            .map(|value| value != "1")
            .unwrap_or(true)
    }

    fn withevents_binding_key(owner: &ObjectRef, binding: BindingHandle) -> i64 {
        crate::semantics::withevents_binding_key(owner, binding)
    }

    fn withevents_binding_from_key(key: i64) -> BindingHandle {
        crate::semantics::withevents_binding_from_key(key)
    }

    fn withevents_owner_from_key(key: i64) -> ObjectRef {
        crate::semantics::withevents_owner_from_key(key)
    }

    fn proper_case(s: &str) -> String {
        crate::semantics::proper_case(s)
    }

    fn format_number(n: f64, fmt: Option<&str>) -> String {
        crate::semantics::format_number(n, fmt)
    }

    /// Returns `true` if either operand is Null — the VBA rule is that any
    /// comparison involving Null yields Null (which is falsy).
    fn either_null(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
        crate::semantics::either_null(lhs, rhs)
    }

    fn typed_compare_values(
        lhs: &RuntimeValue,
        rhs: &RuntimeValue,
        mode: StringCompareMode,
        pred: fn(std::cmp::Ordering) -> bool,
    ) -> Result<bool, String> {
        crate::semantics::typed_compare_values(lhs, rhs, mode, pred)
    }

    fn runtime_value_as_f64(value: &RuntimeValue) -> Result<f64, String> {
        crate::semantics::runtime_value_as_f64(value)
    }

    fn runtime_value_to_usize(value: &RuntimeValue) -> Result<usize, String> {
        crate::semantics::runtime_value_to_usize(value)
    }

    fn legacy_truthy_value(value: &RuntimeValue) -> Result<bool, String> {
        crate::semantics::legacy_truthy_value(value)
    }

    fn legacy_increment_value(value: &RuntimeValue) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_increment_value(value)
    }

    fn legacy_add_const_value(
        value: &RuntimeValue,
        delta: i32,
        field: &str,
    ) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_add_const_value(value, delta, field)
    }

    fn legacy_add_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_add_values(lhs, rhs)
    }

    fn legacy_sub_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_sub_values(lhs, rhs)
    }

    fn legacy_mul_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_mul_values(lhs, rhs)
    }

    fn legacy_pow_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_pow_values(lhs, rhs)
    }

    fn legacy_concat_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> RuntimeValue {
        crate::semantics::legacy_concat_values(lhs, rhs)
    }

    fn legacy_neg_value(val: &RuntimeValue) -> Result<RuntimeValue, String> {
        crate::semantics::legacy_neg_value(val)
    }

    fn materialize_foreach_items(
        &mut self,
        bytecode: &Bytecode,
        typed_fastpaths: bool,
        iterable: &Variant,
    ) -> Result<Vec<RuntimeSlot>, ForEachInitError> {
        if let Some(array) = iterable.as_safearray() {
            let values = array.variant_elements().ok_or_else(|| ForEachInitError {
                code: 13,
                detail: "For Each array source is missing materialized element payload".to_string(),
            })?;
            return Ok(Self::variants_to_slots(values));
        }

        if let Some(object) = iterable.as_object_ref() {
            return self.materialize_foreach_items_from_object(bytecode, typed_fastpaths, object);
        }

        Err(ForEachInitError {
            code: 13,
            detail: format!("For Each expects an array or object source, got {iterable:?}"),
        })
    }

    fn materialize_foreach_items_from_object(
        &mut self,
        bytecode: &Bytecode,
        typed_fastpaths: bool,
        object: oxvba_runtime::ObjectRef,
    ) -> Result<Vec<RuntimeSlot>, ForEachInitError> {
        let request = DynamicCallRequest {
            object: object.clone(),
            member: DynamicMemberSelector::Token(-4),
            args: Vec::new(),
            call_kind_hint: Some(DynamicCallKind::PropertyGet),
        };
        let result_slot = match self.try_invoke_project_dynamic(bytecode, typed_fastpaths, &request)
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                let bridge = HalComDynamicBridge::new(
                    self.host_services.profile(),
                    self.host_services.com(),
                );
                bridge
                    .invoke_dynamic(&request)
                    .map(|value| RuntimeSlot::Variant(value.variant().clone()))
                    .map_err(|err| ForEachInitError {
                        code: 438,
                        detail: err.message,
                    })?
            }
            Err(detail) => return Err(ForEachInitError { code: 438, detail }),
        };

        match &result_slot {
            RuntimeSlot::Variant(value) => {
                let Some(array) = value.as_safearray() else {
                    let other = result_slot
                        .to_runtime_value()
                        .map_err(|detail| ForEachInitError { code: 13, detail })?;
                    return Err(ForEachInitError {
                        code: 13,
                        detail: format!(
                            "For Each NewEnum source on object {object} returned unsupported value {other:?}"
                        ),
                    });
                };
                let values = array.variant_elements().ok_or_else(|| ForEachInitError {
                    code: 13,
                    detail: format!(
                        "For Each NewEnum source on object {object} is missing element payload"
                    ),
                })?;
                Ok(Self::variants_to_slots(values))
            }
            RuntimeSlot::BindingHandle(handle) => Err(ForEachInitError {
                code: 13,
                detail: format!(
                    "For Each NewEnum source on object {object} returned unsupported BindingHandle {}",
                    handle.raw()
                ),
            }),
        }
    }

    fn variants_to_slots(values: Vec<Variant>) -> Vec<RuntimeSlot> {
        values.into_iter().map(RuntimeSlot::Variant).collect()
    }

    fn normalize_dynamic_result_variant(value: &Variant) -> Variant {
        if let Some(value) = value.as_i32()
            && is_error_tag(value)
            && let Some(code) = error_code_from_tag(value)
        {
            return Variant::from_error_code(code);
        }
        value.clone()
    }

    fn validate_runtime_assignment(
        value: &RuntimeValue,
        intent: RuntimeAssignmentIntent,
        target_kind: RuntimeAssignmentTargetKind,
        target_name: &str,
        target_type_name: &str,
    ) -> Result<(), String> {
        crate::semantics::validate_runtime_assignment(
            value,
            intent,
            target_kind,
            target_name,
            target_type_name,
        )
    }

    fn project_dynamic_member_matches_hint(
        member: &ProjectDynamicMemberRoute,
        hint: DynamicCallKind,
    ) -> bool {
        match hint {
            DynamicCallKind::Method => matches!(
                member.kind,
                ProjectDynamicMemberKind::Method | ProjectDynamicMemberKind::Function
            ),
            DynamicCallKind::PropertyGet => member.kind == ProjectDynamicMemberKind::PropertyGet,
            DynamicCallKind::PropertyLet => member.kind == ProjectDynamicMemberKind::PropertyLet,
            DynamicCallKind::PropertySet => member.kind == ProjectDynamicMemberKind::PropertySet,
        }
    }

    fn default_project_dynamic_param_slot(param: &ProjectDynamicParamRoute) -> RuntimeSlot {
        RuntimeSlot::Variant(Variant::from_i32(param.default_value.unwrap_or(0)))
    }

    fn bind_project_dynamic_member_args(
        member: &ProjectDynamicMemberRoute,
        request_args: &[DynamicCallArg],
    ) -> Result<Vec<RuntimeSlot>, String> {
        if member.params.len() != member.visible_param_count {
            return Err(format!(
                "member metadata mismatch: {} visible params but {} param routes",
                member.visible_param_count,
                member.params.len()
            ));
        }

        let mut bound: Vec<Option<RuntimeSlot>> = vec![None; member.params.len()];
        let mut param_array_items = Vec::new();
        let param_array_index = member.params.iter().position(|param| param.param_array);
        let mut next_positional = 0usize;
        let mut named_seen = false;

        for arg in request_args {
            if let Some(name) = &arg.name {
                named_seen = true;
                let Some(index) = member
                    .params
                    .iter()
                    .position(|param| param.name.eq_ignore_ascii_case(name))
                else {
                    return Err(format!("unknown named argument `{name}`"));
                };
                let param = &member.params[index];
                if param.param_array {
                    return Err(format!(
                        "named arguments are not supported for ParamArray parameter `{}`",
                        param.name
                    ));
                }
                if bound[index].is_some() {
                    return Err(format!("argument `{}` is bound more than once", param.name));
                }
                bound[index] = Some(match &arg.value {
                    Some(value) => RuntimeSlot::Variant(value.variant().clone()),
                    None if param.optional => Self::default_project_dynamic_param_slot(param),
                    None => {
                        return Err(format!(
                            "required argument `{}` cannot be omitted",
                            param.name
                        ));
                    }
                });
                continue;
            }

            if named_seen {
                return Err("positional argument cannot follow named argument".to_string());
            }

            let Some(index) = (next_positional..member.params.len())
                .find(|&idx| member.params[idx].param_array || bound[idx].is_none())
            else {
                return Err(format!(
                    "too many arguments: request supplied {} visible args for {} parameters",
                    request_args.len(),
                    member.params.len()
                ));
            };
            let param = &member.params[index];
            if param.param_array {
                let Some(value) = &arg.value else {
                    return Err(format!(
                        "ParamArray parameter `{}` does not support omitted elements",
                        param.name
                    ));
                };
                param_array_items.push(value.variant().clone());
                next_positional = index;
                continue;
            }

            bound[index] = Some(match &arg.value {
                Some(value) => RuntimeSlot::Variant(value.variant().clone()),
                None if param.optional => Self::default_project_dynamic_param_slot(param),
                None => {
                    return Err(format!(
                        "required argument `{}` cannot be omitted",
                        param.name
                    ));
                }
            });
            next_positional = index + 1;
        }

        if let Some(index) = param_array_index {
            bound[index] = Some(RuntimeSlot::Variant(Variant::from_safearray(
                SafeArray::from_variants(param_array_items),
            )));
        }

        for (index, param) in member.params.iter().enumerate() {
            if bound[index].is_some() {
                continue;
            }
            bound[index] = Some(if param.param_array {
                RuntimeSlot::Variant(Variant::from_safearray(
                    SafeArray::from_variants(Vec::new()),
                ))
            } else if param.optional {
                Self::default_project_dynamic_param_slot(param)
            } else {
                return Err(format!("missing required argument `{}`", param.name));
            });
        }

        Ok(bound.into_iter().flatten().collect())
    }

    fn try_invoke_project_dynamic(
        &mut self,
        bytecode: &Bytecode,
        typed_fastpaths: bool,
        request: &DynamicCallRequest,
    ) -> Result<Option<RuntimeSlot>, String> {
        let object = request.object.clone();
        let Some(state) = self.project_dynamic_objects.get(&object.raw()).cloned() else {
            return Ok(None);
        };
        let route = state.route;
        let mut candidates = match &request.member {
            DynamicMemberSelector::Name(name) => route
                .members
                .iter()
                .filter(|member| member.member_name.eq_ignore_ascii_case(name))
                .cloned()
                .collect::<Vec<_>>(),
            DynamicMemberSelector::Token(token) => route
                .members
                .iter()
                .filter(|member| {
                    member.known_dispatch_token == Some(*token)
                        || member.dispatch_id == Some(*token)
                })
                .cloned()
                .collect::<Vec<_>>(),
            DynamicMemberSelector::DefaultMember => route
                .members
                .iter()
                .filter(|member| member.is_default_member)
                .cloned()
                .collect::<Vec<_>>(),
        };
        if let Some(hint) = request.call_kind_hint {
            candidates.retain(|member| Self::project_dynamic_member_matches_hint(member, hint));
        }
        let selector_label = match &request.member {
            DynamicMemberSelector::Name(name) => {
                format!("name `{}`", name.trim().to_ascii_lowercase())
            }
            DynamicMemberSelector::Token(token) => format!("token {}", token),
            DynamicMemberSelector::DefaultMember => "default member".to_string(),
        };
        candidates.sort_by(|lhs, rhs| {
            lhs.lowered_name
                .cmp(&rhs.lowered_name)
                .then(lhs.entry_pc.cmp(&rhs.entry_pc))
        });
        let mut bound_candidates = Vec::new();
        let mut binding_failures = Vec::new();
        for member in candidates {
            match Self::bind_project_dynamic_member_args(&member, &request.args) {
                Ok(values) => bound_candidates.push((member, values)),
                Err(detail) => binding_failures.push(format!("{:?}: {detail}", member.kind)),
            }
        }
        let (member, mut values) = match bound_candidates.as_slice() {
            [] => {
                let available = route
                    .members
                    .iter()
                    .map(|member| {
                        format!(
                            "{}/{}:{:?}/arity={}/default={}",
                            member
                                .known_dispatch_token
                                .map(|token| token.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            member
                                .dispatch_id
                                .map(|token| token.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            member.kind,
                            member.visible_param_count,
                            member.is_default_member
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let binding_context = if binding_failures.is_empty() {
                    String::new()
                } else {
                    format!("; binding failures: [{}]", binding_failures.join("; "))
                };
                return Err(format!(
                    "project dynamic dispatch target {} on `{}` object {} is unresolved for {} explicit args (available: [{}]{})",
                    selector_label,
                    route.module_name,
                    object.raw(),
                    request.args.len(),
                    available,
                    binding_context
                ));
            }
            [(member, values)] => (member.clone(), values.clone()),
            _ => {
                return Err(format!(
                    "project dynamic dispatch target {} on `{}` object {} is ambiguous for {} explicit args",
                    selector_label,
                    route.module_name,
                    object.raw(),
                    request.args.len()
                ));
            }
        };
        values.insert(
            0,
            RuntimeSlot::Variant(Variant::from_object_ref(object.clone())),
        );
        if member.param_slots.len() != values.len() {
            return Err(format!(
                "project dynamic dispatch target {} on `{}` object {} expects {} runtime slots but request built {} values",
                selector_label,
                route.module_name,
                object,
                member.param_slots.len(),
                values.len()
            ));
        }
        self.invoke_procedure_inline_with_slots(
            bytecode,
            member.entry_pc,
            &member.param_slots,
            &values,
            typed_fastpaths,
        )?;
        Ok(Some(
            member
                .return_slot
                .map(|slot| self.registers.registers[slot].clone())
                .unwrap_or_default(),
        ))
    }

    fn invoke_procedure_inline_with_slots(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[RuntimeSlot],
        typed_fastpaths: bool,
    ) -> Result<(), String> {
        if arg_slots.len() != args.len() {
            return Err(format!(
                "argument shape mismatch: {} slots for {} values",
                arg_slots.len(),
                args.len()
            ));
        }
        if entry_pc >= bytecode.instructions.len() {
            return Err(format!("procedure entry out of range: {entry_pc}"));
        }

        // Save caller's error frame — each procedure has its own error context.
        let saved = ErrorFrame {
            on_error_resume_next: self.on_error_resume_next,
            on_error_goto_label_target: self.on_error_goto_label_target,
            last_error: self.last_error,
            last_error_pc: self.last_error_pc,
            last_error_description: self.last_error_description.take(),
            last_error_source: self.last_error_source.take(),
        };
        let call_stack_depth = self.call_stack.len();
        self.call_stack
            .push((bytecode.instructions.len(), saved.clone()));

        // Callee starts with no error handler (VBA semantics).
        self.on_error_resume_next = false;
        self.on_error_goto_label_target = None;
        self.clear_error_state();

        for (slot, value) in arg_slots.iter().zip(args.iter()) {
            self.write_runtime_slot(*slot, value.clone())?;
        }

        let result = self.execute_loop(bytecode, entry_pc, entry_pc, typed_fastpaths, true);
        self.call_stack.truncate(call_stack_depth);

        // Restore caller's error handling mode.
        self.on_error_resume_next = saved.on_error_resume_next;
        self.on_error_goto_label_target = saved.on_error_goto_label_target;

        match result {
            Ok(()) => {
                // Callee succeeded. If the callee set an error (e.g., via On Error
                // Resume Next + Err.Raise inside the callee), preserve the callee's
                // error state for the caller's Err.Number. Otherwise restore the
                // caller's prior error state.
                if self.last_error == 0 {
                    self.last_error = saved.last_error;
                    self.last_error_pc = saved.last_error_pc;
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                }
                Ok(())
            }
            Err(msg) => {
                // Callee raised an unhandled error. Check if the caller can catch it.
                if saved.on_error_resume_next {
                    // Caller's On Error Resume Next absorbs the error.
                    let code = msg
                        .strip_prefix("runtime error: ")
                        .and_then(|rest| {
                            rest.split(|c: char| !c.is_ascii_digit() && c != '-')
                                .next()
                                .and_then(|s| s.parse::<i32>().ok())
                        })
                        .unwrap_or(5);
                    self.last_error = code;
                    self.last_error_pc = None;
                    self.last_error_description = Some(msg);
                    self.last_error_source = None;
                    Ok(())
                } else {
                    // No error handler in caller — propagate.
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                    Err(msg)
                }
            }
        }
    }

    fn withevents_binding_handle(
        value: &RuntimeValue,
        field: &str,
    ) -> Result<BindingHandle, String> {
        crate::semantics::withevents_binding_handle(value, field)
    }

    fn withevents_matching_owners(
        &self,
        source_variant: Option<&Variant>,
        binding: BindingHandle,
    ) -> Vec<ObjectRef> {
        let Some(source_variant) = source_variant else {
            return Vec::new();
        };
        let mut owners = Vec::new();
        for (key, value) in &self.withevents_bindings {
            let RuntimeSlot::Variant(value) = value else {
                continue;
            };
            if value != source_variant || Self::withevents_binding_from_key(*key) != binding {
                continue;
            }
            owners.push(Self::withevents_owner_from_key(*key));
        }
        owners
    }

    fn clear_all_com_withevents_state_best_effort(&mut self) {
        for subscription in self
            .com_withevents_subscriptions
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            let _ = self.host_services.com().unsubscribe_event(subscription);
        }
        self.com_withevents_subscriptions.clear();
        self.com_withevents_binding_subscriptions.clear();
        self.pending_callback_tokens.clear();
    }

    fn clear_com_withevents_binding_subscriptions(&mut self, key: i64) -> Result<(), String> {
        let Some(subscriptions) = self.com_withevents_binding_subscriptions.remove(&key) else {
            return Ok(());
        };
        for subscription in subscriptions {
            self.com_withevents_subscriptions.remove(&subscription);
            self.host_services
                .com()
                .unsubscribe_event(subscription)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    fn clear_com_withevents_owner_subscriptions(&mut self, owner: ObjectRef) -> Result<(), String> {
        let keys = self
            .com_withevents_binding_subscriptions
            .keys()
            .copied()
            .filter(|key| Self::withevents_owner_from_key(*key).raw() == owner.raw())
            .collect::<Vec<_>>();
        for key in keys {
            self.clear_com_withevents_binding_subscriptions(key)?;
        }
        Ok(())
    }

    fn sync_project_com_withevents_binding(
        &mut self,
        owner: ObjectRef,
        binding: BindingHandle,
        value: &Variant,
    ) -> Result<(), String> {
        let Some(routes) = self
            .project_com_withevents_routes
            .get(&binding.raw())
            .cloned()
        else {
            return Ok(());
        };
        if value.as_i32() == Some(0) {
            return Ok(());
        }
        let object = value
            .as_object_ref()
            .ok_or_else(|| "withevents.value is not an object Variant".to_string())?;
        if object.raw() == 0 {
            return Ok(());
        }
        let descriptor = self
            .host_services
            .com()
            .describe_object(object.clone())
            .map_err(|err| err.to_string())?;
        let key = Self::withevents_binding_key(&owner, binding);
        let Some(descriptor) = descriptor else {
            return Ok(());
        };
        for route in routes {
            if !descriptor
                .prog_id_name
                .eq_ignore_ascii_case(&route.prog_id_name)
            {
                continue;
            }
            let subscription = self
                .host_services
                .com()
                .subscribe_event(object.clone(), route.event_token.into())
                .map_err(|err| err.to_string())?;
            self.com_withevents_subscriptions.insert(
                subscription,
                ComWithEventsSubscription {
                    owner_object: owner.clone(),
                    route,
                },
            );
            self.com_withevents_binding_subscriptions
                .entry(key)
                .or_default()
                .push(subscription);
        }
        Ok(())
    }

    fn poll_host_callback_token(&self) -> Result<Option<ComCallbackToken>, String> {
        let value = self
            .host_services
            .events()
            .do_events_variant()
            .map_err(|err| err.to_string())?;
        if value.as_i32() == Some(0) || value.as_i64() == Some(0) || value.as_bool() == Some(false)
        {
            return Ok(None);
        }
        if let Some(raw) = value.as_i32() {
            return Ok(Some(ComCallbackToken::new(raw)));
        }
        if let Some(raw) = value.as_i64() {
            let raw = i32::try_from(raw).map_err(|_| {
                format!("do_events callback exceeds i32 callback-token range: {raw}")
            })?;
            return Ok(Some(ComCallbackToken::new(raw)));
        }
        Err(format!(
            "do_events callback requires callback-token-compatible Variant, got {:?}",
            value.vtype()
        ))
    }

    fn invoke_project_symbol_inline_with_variants(
        &mut self,
        bytecode: &Bytecode,
        symbol: &str,
        args: &[Variant],
        typed_fastpaths: bool,
    ) -> Result<(), String> {
        let normalized = symbol.trim().to_ascii_lowercase();
        let metadata = self
            .procedure_runtime_metadata
            .get(&normalized)
            .cloned()
            .ok_or_else(|| format!("project procedure metadata missing for `{normalized}`"))?;
        let args = args
            .iter()
            .cloned()
            .map(RuntimeSlot::Variant)
            .collect::<Vec<_>>();
        self.invoke_procedure_inline_with_slots(
            bytecode,
            metadata.entry_pc,
            &metadata.param_slots,
            &args,
            typed_fastpaths,
        )
    }

    fn pump_project_com_withevents_callbacks(
        &mut self,
        bytecode: &Bytecode,
        typed_fastpaths: bool,
    ) -> Result<(), String> {
        if self.com_withevents_subscriptions.is_empty() {
            return Ok(());
        }
        loop {
            let Some(callback) = self.poll_host_callback_token()? else {
                return Ok(());
            };
            let subscription = self
                .host_services
                .com()
                .event_callback_subscription(callback)
                .map_err(|err| err.to_string())?;
            let Some(bound) = self
                .com_withevents_subscriptions
                .get(&subscription)
                .cloned()
            else {
                self.pending_callback_tokens.push_back(callback);
                return Ok(());
            };
            let callback_arity = self
                .host_services
                .com()
                .event_callback_arity(callback)
                .map_err(|err| err.to_string())?;
            let mut args = vec![Variant::from_object_ref(bound.owner_object.clone())];
            let target_symbol = match callback_arity {
                0 => bound.route.handler_symbol.clone(),
                1 => {
                    let arg0 = self
                        .host_services
                        .com()
                        .event_callback_variant(callback, 0)
                        .map_err(|err| err.to_string())?;
                    args.push(arg0);
                    bound.route.handler_symbol.clone()
                }
                _ => {
                    let _ = self.host_services.com().release_event_callback(callback);
                    return Err(format!(
                        "project COM WithEvents handler `{}` supports at most 1 callback argument, got {}",
                        bound.route.event_name, callback_arity
                    ));
                }
            };
            let invoke_result = self.invoke_project_symbol_inline_with_variants(
                bytecode,
                &target_symbol,
                &args,
                typed_fastpaths,
            );
            let release_result = self
                .host_services
                .com()
                .release_event_callback(callback)
                .map_err(|err| err.to_string());
            invoke_result?;
            release_result?;
        }
    }

    fn fast_read_slot(&self, slot: usize) -> Option<i32> {
        self.registers.registers.get(slot)?.as_i32_lossy()
    }

    fn fast_add_const(&mut self, slot: usize, value: i32) -> bool {
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        // Null propagation: Null + anything = Null.
        if dst.is_null() {
            return true;
        }
        let Some(current) = dst.as_i32_lossy() else {
            return false;
        };
        let Ok(value) = RuntimeSlot::from_compat_slot_i32(current + value) else {
            return false;
        };
        *dst = value;
        true
    }

    fn fast_sub_const(&mut self, slot: usize, value: i32) -> bool {
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        // Null propagation: Null - anything = Null.
        if dst.is_null() {
            return true;
        }
        let Some(current) = dst.as_i32_lossy() else {
            return false;
        };
        let Ok(value) = RuntimeSlot::from_compat_slot_i32(current - value) else {
            return false;
        };
        *dst = value;
        true
    }

    fn fast_copy_slot(&mut self, dst: usize, src: usize) -> bool {
        let Some(value) = self.registers.registers.get(src).cloned() else {
            return false;
        };
        let Some(dst) = self.registers.registers.get_mut(dst) else {
            return false;
        };
        *dst = value;
        true
    }

    fn fast_cmp_slots<F>(&mut self, dst: usize, lhs: usize, rhs: usize, pred: F) -> bool
    where
        F: FnOnce(i32, i32) -> bool,
    {
        // Null comparisons yield Null (falsy) — bail to slow path which handles this.
        if let (Some(l), Some(r)) = (
            self.registers.registers.get(lhs),
            self.registers.registers.get(rhs),
        ) && (l.is_null() || r.is_null())
        {
            return false; // fall through to legacy_compare_values
        }
        let (Some(lhs), Some(rhs)) = (self.fast_read_slot(lhs), self.fast_read_slot(rhs)) else {
            return false;
        };
        self.write_variant_slot(dst, Variant::from_bool(pred(lhs, rhs)))
            .is_ok()
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

    fn next_pc_for_jump_if_zero_value(
        cond: &RuntimeValue,
        target_pc: usize,
        instruction_len: usize,
        current_pc: usize,
    ) -> Result<usize, String> {
        let cond = Self::legacy_truthy_value(cond)?;
        Self::next_pc_for_jump_if_zero(
            if cond { -1 } else { 0 },
            target_pc,
            instruction_len,
            current_pc,
        )
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
        crate::semantics::normalize_for_compare(text, mode)
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
}

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
    let previous = current.as_safearray().ok_or_else(|| {
        "runtime ReDim Preserve requires an existing runtime array value".to_string()
    })?;
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

#[cfg(test)]
mod tests {
    use crate::register_file::RuntimeSlot;

    use super::{
        DebugBreakpoint, DebugRunResult, DebugStopReason, Vm, runtime_resized_array_preserve,
    };
    use oxvba_com::{
        DynamicCallArg, DynamicCallKind, DynamicCallRequest, DynamicMemberSelector, DynamicValue,
    };
    use oxvba_compiler::{
        Bytecode, Instruction, ProcedureRuntimeMetadata, ProjectComWithEventsRoute,
        ProjectDynamicMemberKind, ProjectDynamicMemberRoute, ProjectDynamicObjectRoute,
        ProjectDynamicParamRoute,
        bytecode::{DispatchInvokeArg, RuntimeArrayElementType, StringCompareMode},
    };
    use oxvba_hal::{
        error::{HalError, HalErrorKind},
        model::CapabilityId,
    };
    use oxvba_runtime::value_tags::{EMPTY_TAG, NULL_TAG, error_tag_from_code};
    use oxvba_runtime::{
        F64Value, ObjectRef, RuntimeValue, VarType, Variant,
        bstr::BStr,
        safe_array::{ARRAY_TAG_BASE, SafeArray, SafeArrayBound},
    };
    use std::collections::BTreeMap;

    fn debug_metadata(
        module_name: &str,
        procedure_name: &str,
        entry_pc: usize,
        statement_line_numbers: Vec<usize>,
        statement_entry_pcs: Vec<usize>,
    ) -> ProcedureRuntimeMetadata {
        ProcedureRuntimeMetadata {
            module_name: module_name.to_string(),
            procedure_name: procedure_name.to_string(),
            entry_pc,
            source_line_start: statement_line_numbers.first().copied().unwrap_or(1),
            source_line_end: statement_line_numbers.last().copied().unwrap_or(1),
            statement_line_numbers,
            statement_entry_pcs,
            slots: Vec::new(),
            param_slots: Vec::new(),
            return_slot: None,
        }
    }

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
    fn debug_start_pauses_on_entry_and_breakpoint_then_completes() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::LoadConstI32 { slot: 1, value: 20 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "main".to_string(),
            debug_metadata("MainModule", "Main", 0, vec![2, 3], vec![0, 1]),
        );

        let mut vm = Vm::default();
        vm.set_project_procedure_runtime_metadata(metadata);
        vm.debug_set_breakpoints(vec![DebugBreakpoint {
            module_name: "MainModule".to_string(),
            line_number: 3,
        }]);

        let start = vm.debug_start(&bytecode).expect("debug start should pause");
        let DebugRunResult::Paused(start_pause) = start else {
            panic!("expected initial pause");
        };
        assert_eq!(start_pause.reason, DebugStopReason::Entry);
        assert_eq!(start_pause.location.statement_pc, 0);
        assert_eq!(start_pause.location.line_number, Some(2));

        let next = vm
            .debug_continue(&bytecode)
            .expect("continue should hit breakpoint");
        let DebugRunResult::Paused(breakpoint_pause) = next else {
            panic!("expected breakpoint pause");
        };
        assert_eq!(breakpoint_pause.reason, DebugStopReason::Breakpoint);
        assert_eq!(breakpoint_pause.location.statement_pc, 1);
        assert_eq!(breakpoint_pause.location.line_number, Some(3));

        assert_eq!(
            vm.debug_continue(&bytecode)
                .expect("final continue should complete"),
            DebugRunResult::Completed
        );
    }

    #[test]
    fn debug_step_over_skips_nested_call_and_step_into_and_out_track_depth() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::CallProc { target_pc: 3 },
                Instruction::LoadConstI32 { slot: 0, value: 7 },
                Instruction::Halt,
                Instruction::LoadConstI32 { slot: 1, value: 9 },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "main".to_string(),
            debug_metadata("MainModule", "Main", 0, vec![2, 3], vec![0, 1]),
        );
        metadata.insert(
            "foo".to_string(),
            debug_metadata("HelperModule", "Foo", 3, vec![10], vec![3]),
        );

        let mut vm = Vm::default();
        vm.set_project_procedure_runtime_metadata(metadata.clone());
        let DebugRunResult::Paused(entry_pause) =
            vm.debug_start(&bytecode).expect("debug start should pause")
        else {
            panic!("expected entry pause");
        };
        assert_eq!(entry_pause.location.statement_pc, 0);

        let DebugRunResult::Paused(step_over_pause) = vm
            .debug_step_over(&bytecode)
            .expect("step over should pause")
        else {
            panic!("expected step-over pause");
        };
        assert_eq!(step_over_pause.reason, DebugStopReason::Step);
        assert_eq!(step_over_pause.location.statement_pc, 1);
        assert_eq!(step_over_pause.call_stack_depth, 1);

        let mut vm = Vm::default();
        vm.set_project_procedure_runtime_metadata(metadata);
        let DebugRunResult::Paused(_) =
            vm.debug_start(&bytecode).expect("debug start should pause")
        else {
            panic!("expected entry pause");
        };
        let DebugRunResult::Paused(step_into_pause) = vm
            .debug_step_into(&bytecode)
            .expect("step into should pause in callee")
        else {
            panic!("expected step-into pause");
        };
        assert_eq!(step_into_pause.reason, DebugStopReason::Step);
        assert_eq!(step_into_pause.location.statement_pc, 3);
        assert_eq!(step_into_pause.location.procedure_name, "Foo");
        assert_eq!(step_into_pause.call_stack_depth, 2);

        let DebugRunResult::Paused(step_out_pause) = vm
            .debug_step_out(&bytecode)
            .expect("step out should return to caller")
        else {
            panic!("expected step-out pause");
        };
        assert_eq!(step_out_pause.reason, DebugStopReason::Step);
        assert_eq!(step_out_pause.location.statement_pc, 1);
        assert_eq!(step_out_pause.location.procedure_name, "Main");
        assert_eq!(step_out_pause.call_stack_depth, 1);
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
    fn snapshot_values_preserve_non_legacy_runtime_values() {
        let mut vm = Vm::default();
        vm.reset_execution_state(1, false);
        vm.write_value_slot(0, RuntimeValue::String(BStr::from("ABC")))
            .expect("write string runtime value");

        assert_eq!(
            vm.snapshot_values(1),
            vec![RuntimeValue::String(BStr::from("ABC"))]
        );
        assert_eq!(vm.snapshot_slots(1), vec![EMPTY_TAG]);
    }

    #[test]
    fn snapshot_variants_exposes_variant_cells_before_projection() {
        let mut vm = Vm::default();
        vm.reset_execution_state(1, false);
        vm.write_value_slot(0, RuntimeValue::String(BStr::from("ABC")))
            .expect("write string runtime value");

        let variants = vm.snapshot_variants(1);

        assert_eq!(variants[0].vtype(), oxvba_runtime::VarType::String);
        assert_eq!(variants[0].as_bstr(), Some(BStr::from("ABC")));
    }

    #[test]
    fn read_value_slot_returns_runtime_value_shape() {
        let mut vm = Vm::default();
        vm.reset_execution_state(1, false);
        vm.write_value_slot(0, RuntimeValue::Bool(true))
            .expect("write bool runtime value");

        assert_eq!(
            vm.read_value_slot(0).expect("read runtime value"),
            RuntimeValue::Bool(true)
        );
    }

    #[test]
    fn variant_varptr_returns_actual_register_variant_cell() {
        let mut vm = Vm::default();
        vm.reset_execution_state(2, false);
        vm.write_value_slot(0, RuntimeValue::String(BStr::from("ABC")))
            .expect("write variant value");

        let pointer = vm.variant_cell_pointer(0).expect("variant cell pointer");
        assert_ne!(pointer, 0);
        assert!(vm.registers.registers[0].is_variant_cell_pointer(pointer));
    }

    #[test]
    fn load_const_i32_preserves_tagged_runtime_value_shape() {
        let bytecode = Bytecode {
            instructions: vec![
                // LoadNull is now the canonical way to produce RuntimeValue::Null.
                Instruction::LoadNull { slot: 0 },
                Instruction::LoadConstI32 {
                    slot: 1,
                    value: error_tag_from_code(17),
                },
                Instruction::LoadConstI32 {
                    slot: 2,
                    value: EMPTY_TAG,
                },
            ],
            external_call_descriptors: vec![],
            slot_count: 3,
            user_slot_count: 3,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("vm should execute tagged constants");

        assert_eq!(
            vm.read_value_slot(0).expect("null slot"),
            RuntimeValue::Null
        );
        assert_eq!(
            vm.read_value_slot(1).expect("error slot"),
            RuntimeValue::ErrorCode(17)
        );
        assert_eq!(
            vm.read_value_slot(2).expect("empty slot"),
            RuntimeValue::Empty
        );
    }

    #[test]
    fn msg_box_host_accepts_string_runtime_prompt() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::IntrinsicMsgBoxHost {
                    dst: 1,
                    prompt: 0,
                    style: None,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let host = oxvba_hal::adapters::builder::HostBuilder::new()
            .profile(oxvba_hal::model::HalProfileId::Wasm)
            .policy(oxvba_hal::model::HostPolicy {
                allow_interaction: true,
                ui_virtualization: oxvba_hal::UiVirtualizationMode::ScriptedResponses,
                ..oxvba_hal::model::HostPolicy::interactive_dev()
            })
            .build();
        let mut vm = Vm::new(host);
        vm.invoke_procedure_with_values(
            &bytecode,
            0,
            &[0],
            &[RuntimeValue::String(BStr::from("Prompt"))],
        )
        .expect("vm should execute msg_box host intrinsic");

        assert_eq!(vm.snapshot_values(2)[1], RuntimeValue::I32(1));
    }

    #[test]
    fn input_box_host_preserves_string_runtime_defaults() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::IntrinsicInputBoxHost {
                    dst: 2,
                    prompt: 0,
                    default_value: Some(1),
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };

        let host = oxvba_hal::adapters::builder::HostBuilder::new()
            .profile(oxvba_hal::model::HalProfileId::Wasm)
            .policy(oxvba_hal::model::HostPolicy {
                allow_interaction: true,
                ui_virtualization: oxvba_hal::UiVirtualizationMode::ScriptedResponses,
                ..oxvba_hal::model::HostPolicy::interactive_dev()
            })
            .build();
        let mut vm = Vm::new(host);
        vm.invoke_procedure_with_values(
            &bytecode,
            0,
            &[0, 1],
            &[
                RuntimeValue::String(BStr::from("Prompt")),
                RuntimeValue::String(BStr::from("Default")),
            ],
        )
        .expect("vm should execute input_box host intrinsic");

        assert_eq!(
            vm.snapshot_values(3)[2],
            RuntimeValue::String(BStr::from("Default"))
        );
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
                    mode: StringCompareMode::Binary,
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
        let out = vm.snapshot_slots(10);
        let values = vm.snapshot_values(10);
        assert_eq!(out, vec![12345, 2, 3, 5, 0, 0, 0, 3, 0, 0]);
        assert_eq!(values[4], RuntimeValue::String(BStr::from("12")));
        assert_eq!(values[5], RuntimeValue::String(BStr::from("45")));
        assert_eq!(values[6], RuntimeValue::String(BStr::from("234")));
        assert_eq!(values[8], RuntimeValue::String(BStr::from("12345")));
        assert_eq!(values[9], RuntimeValue::String(BStr::from("12345")));
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
    fn executes_mid_statement_string_mutation_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "ABCDE".to_string(),
                },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::LoadConstI32 { slot: 2, value: 2 },
                Instruction::LoadConstString {
                    slot: 3,
                    value: "99".to_string(),
                },
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
        assert_eq!(
            vm.snapshot_values(4)[0],
            RuntimeValue::String(BStr::from("A99DE"))
        );
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
        let out = vm.snapshot_slots(15);
        let values = vm.snapshot_values(15);
        assert_eq!(
            out,
            vec![
                123231, 23, 12345, 67, 12, 123, 3, 12345, 0, 0, 0, 0, -1, 4, -1
            ]
        );
        assert_eq!(values[8], RuntimeValue::String(BStr::from("16745")));
        assert_eq!(values[9], RuntimeValue::String(BStr::from("12345")));
        assert_eq!(values[10], RuntimeValue::String(BStr::from("12345")));
        assert_eq!(values[11], RuntimeValue::String(BStr::from("12345")));
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
                Instruction::LoadConstString {
                    slot: 34,
                    value: "OxVba.TestDispatch".to_string(),
                },
                Instruction::IntrinsicCreateObjectHost {
                    dst: 20,
                    prog_id: 34,
                },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 21,
                    object: 20,
                    member: 4,
                    args: vec![DispatchInvokeArg {
                        slot: Some(6),
                        name: None,
                    }],
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
            slot_count: 35,
            user_slot_count: 35,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let out = vm.snapshot_slots(34);
        let values = vm.snapshot_values(34);
        assert_eq!(
            values[8],
            RuntimeValue::F64(F64Value::from_date_f64(46081.0))
        );
        assert_eq!(
            values[9],
            RuntimeValue::F64(F64Value::from_date_f64(46082.0))
        );
        assert_eq!(out[10], 1);
        assert_eq!(out[11], 1);
        assert_eq!(out[12], 1);
        assert_eq!(values[13], RuntimeValue::I32(46081));
        assert_eq!(out[15], 0);
        assert_eq!(out[16], 2);
        assert_eq!(out[17], 1);
        assert_eq!(out[18], 2);
        assert_eq!(out[19], 2);
        let object_handle = match &values[20] {
            RuntimeValue::Object(handle) => handle.raw(),
            other => panic!("expected object handle result, got {other:?}"),
        };
        assert_eq!(out[20], object_handle);
        assert_eq!(out[21], object_handle + 3);
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
        let out = vm.snapshot_variants(15);
        assert_eq!(out[5].as_i32(), Some(0));
        assert_eq!(out[6].as_i32(), Some(3));
        assert_eq!(out[7].as_i32(), Some(10));
        assert_eq!(out[8].as_i32(), Some(3));
        assert_eq!(out[9].as_i32(), Some(8192 + 12));
        assert_eq!(out[10].as_i32(), Some(0));
        assert_eq!(out[11].as_i32(), Some(1));
        assert_eq!(out[12].as_i32(), Some(0));
        assert_eq!(out[13].as_i32(), Some(1));
        assert_eq!(out[14].as_i32(), Some(0));
    }

    #[test]
    fn dispatch_invoke_preserves_array_argument_intent() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "OxVba.TestDispatch".to_string(),
                },
                Instruction::IntrinsicCreateObjectHost { dst: 1, prog_id: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 6 },
                Instruction::LoadConstI32 { slot: 3, value: 11 },
                Instruction::LoadConstI32 { slot: 4, value: 14 },
                Instruction::LoadConstI32 { slot: 5, value: 17 },
                Instruction::IntrinsicArrayLiteral {
                    dst: 6,
                    values: vec![3, 4, 5],
                },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 7,
                    object: 1,
                    member: 2,
                    args: vec![DispatchInvokeArg {
                        slot: Some(6),
                        name: None,
                    }],
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
        let values = vm.snapshot_values(8);
        let variants = vm.snapshot_variants(8);
        let object_handle = match &values[1] {
            RuntimeValue::Object(handle) => handle.raw(),
            other => panic!("expected object handle result, got {other:?}"),
        };
        assert_eq!(out[1], object_handle);
        assert_eq!(
            variants[1]
                .as_object_ref()
                .expect("CreateObject should retain object Variant")
                .raw(),
            object_handle
        );
        assert_eq!(out[7], object_handle + 6 + (ARRAY_TAG_BASE + 3));
        assert_eq!(
            values[6],
            RuntimeValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(11),
                RuntimeValue::I32(14),
                RuntimeValue::I32(17),
            ]))
        );
    }

    #[test]
    fn intrinsic_array_literal_and_append_preserve_retained_variant_payloads() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "alpha".to_string(),
                },
                Instruction::LoadConstI32 { slot: 1, value: 7 },
                Instruction::IntrinsicArrayLiteral {
                    dst: 2,
                    values: vec![0, 1],
                },
                Instruction::LoadConstBool {
                    slot: 3,
                    value: true,
                },
                Instruction::IntrinsicArrayAppend {
                    dst: 4,
                    array: 2,
                    item: 3,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 5,
            user_slot_count: 5,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let variants = vm.snapshot_variants(5);
        let array = variants[4]
            .as_safearray()
            .expect("array append should produce a retained SAFEARRAY Variant");
        let elements = array
            .variant_elements()
            .expect("array append should preserve Variant elements");

        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].vtype(), oxvba_runtime::VarType::String);
        assert_eq!(elements[0].as_bstr(), Some(BStr::from("alpha")));
        assert_eq!(elements[1].vtype(), oxvba_runtime::VarType::Long);
        assert_eq!(elements[1].as_i32(), Some(7));
        assert_eq!(elements[2].vtype(), oxvba_runtime::VarType::Boolean);
        assert_eq!(elements[2].as_bool(), Some(true));
    }

    #[test]
    fn intrinsic_array_resize_1d_materializes_zeroed_byte_payload() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicArrayResize {
                    dst: 1,
                    upper_bounds: vec![0],
                    lower_bounds: vec![0],
                    element_type: RuntimeArrayElementType::Byte,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let variants = vm.snapshot_variants(2);
        let variant_array = variants[1]
            .as_safearray()
            .expect("ReDim should retain a SAFEARRAY Variant");
        assert_eq!(
            variant_array,
            SafeArray::from_typed_values_nd(
                vec![SafeArrayBound { lower: 0, count: 4 }],
                0x0011,
                vec![
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                ],
            )
            .expect("byte SAFEARRAY expected")
        );
        let values = vm.snapshot_values(2);
        assert_eq!(
            values[1],
            RuntimeValue::ArrayIntent(
                SafeArray::from_typed_values_nd(
                    vec![SafeArrayBound { lower: 0, count: 4 }],
                    0x0011,
                    vec![
                        RuntimeValue::I32(0),
                        RuntimeValue::I32(0),
                        RuntimeValue::I32(0),
                        RuntimeValue::I32(0),
                    ],
                )
                .expect("byte SAFEARRAY expected")
            )
        );
    }

    #[test]
    fn runtime_redim_preserve_1d_retains_overlapping_byte_values() {
        let current = Variant::from_safearray(
            SafeArray::from_typed_values_nd(
                vec![SafeArrayBound { lower: 0, count: 2 }],
                0x0011,
                vec![RuntimeValue::I32(90), RuntimeValue::I32(91)],
            )
            .expect("byte SAFEARRAY expected"),
        );
        let resized =
            runtime_resized_array_preserve(&current, &[0], &[3], RuntimeArrayElementType::Byte)
                .expect("runtime preserve should succeed");
        assert_eq!(
            resized,
            SafeArray::from_typed_values_nd(
                vec![SafeArrayBound { lower: 0, count: 4 }],
                0x0011,
                vec![
                    RuntimeValue::I32(90),
                    RuntimeValue::I32(91),
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                ],
            )
            .expect("byte SAFEARRAY expected")
        );
    }

    #[test]
    fn intrinsic_array_get_set_preserve_retained_variant_payloads() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::IntrinsicArrayResize {
                    dst: 1,
                    upper_bounds: vec![0],
                    lower_bounds: vec![0],
                    element_type: RuntimeArrayElementType::Variant,
                },
                Instruction::LoadConstI32 { slot: 2, value: 0 },
                Instruction::LoadConstString {
                    slot: 3,
                    value: "payload".to_string(),
                },
                Instruction::IntrinsicArraySet {
                    array: 1,
                    indices: vec![2],
                    src: 3,
                },
                Instruction::IntrinsicArrayGet {
                    dst: 4,
                    array: 1,
                    indices: vec![2],
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 5,
            user_slot_count: 5,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let variants = vm.snapshot_variants(5);
        let array = variants[1]
            .as_safearray()
            .expect("array set should retain SAFEARRAY Variant");
        let elements = array
            .variant_elements()
            .expect("array set should preserve Variant element payload");

        assert_eq!(elements[0].vtype(), oxvba_runtime::VarType::String);
        assert_eq!(elements[0].as_bstr(), Some(BStr::from("payload")));
        assert_eq!(variants[4].vtype(), oxvba_runtime::VarType::String);
        assert_eq!(variants[4].as_bstr(), Some(BStr::from("payload")));
    }

    #[test]
    fn intrinsic_for_each_array_init_preserves_retained_variant_items() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "first".to_string(),
                },
                Instruction::LoadConstI32 { slot: 1, value: 22 },
                Instruction::IntrinsicArrayLiteral {
                    dst: 2,
                    values: vec![0, 1],
                },
                Instruction::IntrinsicForEachInit { iter: 3, src: 2 },
                Instruction::IntrinsicForEachNext {
                    iter: 3,
                    item: 4,
                    has_value: 5,
                },
                Instruction::IntrinsicForEachNext {
                    iter: 3,
                    item: 6,
                    has_value: 7,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 8,
            user_slot_count: 8,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        let variants = vm.snapshot_variants(8);
        let compat = vm.snapshot_values(8);

        assert_eq!(compat[5], RuntimeValue::Bool(true));
        assert_eq!(compat[7], RuntimeValue::Bool(true));
        assert_eq!(variants[4].vtype(), oxvba_runtime::VarType::String);
        assert_eq!(variants[4].as_bstr(), Some(BStr::from("first")));
        assert_eq!(variants[6].vtype(), oxvba_runtime::VarType::Long);
        assert_eq!(variants[6].as_i32(), Some(22));
    }

    #[test]
    fn project_dynamic_dispatch_binds_named_args() {
        let object = 71;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::CopySlot { dst: 2, src: 1 },
                Instruction::AddConstI32 { slot: 2, value: 1 },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let member = ProjectDynamicMemberRoute {
            member_name: "ping".to_string(),
            lowered_name: "ping".to_string(),
            known_dispatch_token: Some(77),
            dispatch_id: None,
            member_flags: None,
            is_default_member: false,
            kind: ProjectDynamicMemberKind::Function,
            visible_param_count: 1,
            params: vec![ProjectDynamicParamRoute {
                name: "n".to_string(),
                optional: false,
                param_array: false,
                default_value: None,
            }],
            entry_pc: 0,
            param_slots: vec![0, 1],
            return_slot: Some(2),
        };
        let route = ProjectDynamicObjectRoute {
            object_handle: object,
            project_name: "ProjectA".to_string(),
            module_name: "Widget".to_string(),
            members: vec![member],
            implements_interfaces: Vec::new(),
        };
        let request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(object),
            member: DynamicMemberSelector::Name("Ping".to_string()),
            args: vec![DynamicCallArg {
                value: Some(DynamicValue::from_runtime_value(&RuntimeValue::I32(7))),
                name: Some("n".to_string()),
            }],
            call_kind_hint: None,
        };

        let mut vm = Vm::default();
        vm.reset_execution_state(bytecode.slot_count, false);
        vm.set_project_dynamic_objects(vec![route]);
        let value = vm
            .try_invoke_project_dynamic(&bytecode, false, &request)
            .expect("dispatch should bind")
            .expect("project-dynamic route should match")
            .to_runtime_value()
            .expect("project-dynamic slot should project for assertion");

        assert_eq!(value, RuntimeValue::I32(8));
    }

    #[test]
    fn project_dynamic_dispatch_matches_dispatch_id_tokens_for_newenum() {
        let object = 74;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 1, value: 41 },
                Instruction::LoadConstI32 { slot: 2, value: 42 },
                Instruction::IntrinsicArrayLiteral {
                    dst: 3,
                    values: vec![1, 2],
                },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 4,
            user_slot_count: 4,
        };
        let member = ProjectDynamicMemberRoute {
            member_name: "newenum".to_string(),
            lowered_name: "newenum".to_string(),
            known_dispatch_token: None,
            dispatch_id: Some(-4),
            member_flags: Some(0x40),
            is_default_member: false,
            kind: ProjectDynamicMemberKind::PropertyGet,
            visible_param_count: 0,
            params: Vec::new(),
            entry_pc: 0,
            param_slots: vec![0],
            return_slot: Some(3),
        };
        let route = ProjectDynamicObjectRoute {
            object_handle: object,
            project_name: "ProjectA".to_string(),
            module_name: "Widget".to_string(),
            members: vec![member],
            implements_interfaces: Vec::new(),
        };
        let request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(object),
            member: DynamicMemberSelector::Token(-4),
            args: Vec::new(),
            call_kind_hint: Some(DynamicCallKind::PropertyGet),
        };

        let mut vm = Vm::default();
        vm.reset_execution_state(bytecode.slot_count, false);
        vm.set_project_dynamic_objects(vec![route]);
        let value = vm
            .try_invoke_project_dynamic(&bytecode, false, &request)
            .expect("dispatch should bind")
            .expect("project-dynamic route should match")
            .to_runtime_value()
            .expect("project-dynamic slot should project for assertion");

        assert_eq!(
            value,
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(41),
                RuntimeValue::I32(42),
            ]))
        );
    }

    #[test]
    fn project_dynamic_dispatch_applies_optional_defaults_for_missing_and_omitted_args() {
        let object = 72;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::CopySlot { dst: 2, src: 1 },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let member = ProjectDynamicMemberRoute {
            member_name: "ping".to_string(),
            lowered_name: "ping".to_string(),
            known_dispatch_token: Some(77),
            dispatch_id: None,
            member_flags: None,
            is_default_member: false,
            kind: ProjectDynamicMemberKind::Function,
            visible_param_count: 1,
            params: vec![ProjectDynamicParamRoute {
                name: "n".to_string(),
                optional: true,
                param_array: false,
                default_value: Some(7),
            }],
            entry_pc: 0,
            param_slots: vec![0, 1],
            return_slot: Some(2),
        };
        let route = ProjectDynamicObjectRoute {
            object_handle: object,
            project_name: "ProjectA".to_string(),
            module_name: "Widget".to_string(),
            members: vec![member],
            implements_interfaces: Vec::new(),
        };
        let mut vm = Vm::default();
        vm.reset_execution_state(bytecode.slot_count, false);
        vm.set_project_dynamic_objects(vec![route.clone()]);

        let omitted_request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(object),
            member: DynamicMemberSelector::Name("Ping".to_string()),
            args: vec![],
            call_kind_hint: None,
        };
        let omitted_value = vm
            .try_invoke_project_dynamic(&bytecode, false, &omitted_request)
            .expect("dispatch should bind omitted optional")
            .expect("project-dynamic route should match");
        assert!(matches!(
            &omitted_value,
            RuntimeSlot::Variant(value) if value.as_i32() == Some(7)
        ));
        let omitted_value = omitted_value
            .to_runtime_value()
            .expect("project-dynamic slot should project for assertion");
        assert_eq!(omitted_value, RuntimeValue::I32(7));

        vm.set_project_dynamic_objects(vec![route]);
        let explicit_omitted_request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(object),
            member: DynamicMemberSelector::Name("Ping".to_string()),
            args: vec![DynamicCallArg {
                value: None,
                name: None,
            }],
            call_kind_hint: None,
        };
        let explicit_omitted_value = vm
            .try_invoke_project_dynamic(&bytecode, false, &explicit_omitted_request)
            .expect("dispatch should bind explicit omitted optional")
            .expect("project-dynamic route should match");
        assert!(matches!(
            &explicit_omitted_value,
            RuntimeSlot::Variant(value) if value.as_i32() == Some(7)
        ));
        let explicit_omitted_value = explicit_omitted_value
            .to_runtime_value()
            .expect("project-dynamic slot should project for assertion");
        assert_eq!(explicit_omitted_value, RuntimeValue::I32(7));
    }

    #[test]
    fn project_dynamic_dispatch_packs_paramarray_values() {
        let object = 73;
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::CopySlot { dst: 2, src: 1 },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let member = ProjectDynamicMemberRoute {
            member_name: "capture".to_string(),
            lowered_name: "capture".to_string(),
            known_dispatch_token: Some(78),
            dispatch_id: None,
            member_flags: None,
            is_default_member: false,
            kind: ProjectDynamicMemberKind::Function,
            visible_param_count: 1,
            params: vec![ProjectDynamicParamRoute {
                name: "items".to_string(),
                optional: false,
                param_array: true,
                default_value: None,
            }],
            entry_pc: 0,
            param_slots: vec![0, 1],
            return_slot: Some(2),
        };
        let route = ProjectDynamicObjectRoute {
            object_handle: object,
            project_name: "ProjectA".to_string(),
            module_name: "Widget".to_string(),
            members: vec![member],
            implements_interfaces: Vec::new(),
        };
        let request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(object),
            member: DynamicMemberSelector::Name("Capture".to_string()),
            args: vec![
                DynamicCallArg {
                    value: Some(DynamicValue::from_runtime_value(&RuntimeValue::I32(11))),
                    name: None,
                },
                DynamicCallArg {
                    value: Some(DynamicValue::from_runtime_value(&RuntimeValue::I32(14))),
                    name: None,
                },
            ],
            call_kind_hint: None,
        };

        let mut vm = Vm::default();
        vm.reset_execution_state(bytecode.slot_count, false);
        vm.set_project_dynamic_objects(vec![route]);
        let value_slot = vm
            .try_invoke_project_dynamic(&bytecode, false, &request)
            .expect("dispatch should bind paramarray")
            .expect("project-dynamic route should match");
        let RuntimeSlot::Variant(variant) = &value_slot else {
            panic!("expected retained Variant paramarray result, got {value_slot:?}");
        };
        let array = variant
            .as_safearray()
            .expect("ParamArray should remain a retained SAFEARRAY Variant");
        assert_eq!(
            array
                .variant_elements()
                .expect("ParamArray should retain Variant element payload"),
            vec![Variant::from_i32(11), Variant::from_i32(14)]
        );
        let value = value_slot
            .to_runtime_value()
            .expect("project-dynamic slot should project for assertion");

        assert_eq!(
            value,
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(11),
                RuntimeValue::I32(14),
            ]))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_event_subscription_intrinsics_roundtrip_through_vm_host_lane() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "OxVba.TestDispatch".to_string(),
                },
                Instruction::IntrinsicCreateObjectHost { dst: 1, prog_id: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 1 },
                Instruction::IntrinsicComSubscribeEventHost {
                    dst: 3,
                    object: 1,
                    event: 2,
                },
                Instruction::LoadConstI32 { slot: 4, value: 3 },
                Instruction::LoadConstI32 { slot: 5, value: 77 },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 6,
                    object: 1,
                    member: 4,
                    args: vec![DispatchInvokeArg {
                        slot: Some(5),
                        name: None,
                    }],
                },
                Instruction::IntrinsicDoEventsHost { dst: 7 },
                Instruction::IntrinsicComEventCallbackSubscriptionHost {
                    dst: 8,
                    callback: 7,
                },
                Instruction::LoadConstI32 { slot: 9, value: 0 },
                Instruction::IntrinsicComEventCallbackArgHost {
                    dst: 10,
                    callback: 7,
                    index: 9,
                },
                Instruction::IntrinsicComReleaseEventCallbackHost {
                    dst: 11,
                    callback: 7,
                },
                Instruction::IntrinsicComUnsubscribeEventHost {
                    dst: 12,
                    subscription: 3,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 13,
            user_slot_count: 13,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy::interactive_dev())
                .build(),
        );
        vm.execute(&bytecode)
            .expect("vm should execute COM event subscribe/unsubscribe flow");
        let out = vm.snapshot_slots(13);
        assert!(out[1] >= 20_001, "expected native COM object handle");
        assert!(out[3] >= 40_001, "expected native COM subscription handle");
        assert_eq!(out[6], 77, "expected FireChanged return value");
        assert!(
            out[7] >= 60_001,
            "expected DoEvents callback pump to return callback token"
        );
        assert_eq!(
            out[8], out[3],
            "expected callback subscription lookup to return subscription token"
        );
        assert_eq!(
            out[10], 77,
            "expected callback arg lookup to return event payload"
        );
        assert_eq!(out[11], 1, "expected callback release token");
        assert_eq!(out[12], 1, "expected unsubscribe success token");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_event_subscription_intrinsics_roundtrip_multi_arg_callback_lane() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "OxVba.TestDispatch".to_string(),
                },
                Instruction::IntrinsicCreateObjectHost { dst: 1, prog_id: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 3 },
                Instruction::IntrinsicComSubscribeEventHost {
                    dst: 3,
                    object: 1,
                    event: 2,
                },
                Instruction::LoadConstI32 { slot: 4, value: 4 },
                Instruction::LoadConstI32 { slot: 5, value: 90 },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 6,
                    object: 1,
                    member: 4,
                    args: vec![DispatchInvokeArg {
                        slot: Some(5),
                        name: None,
                    }],
                },
                Instruction::IntrinsicDoEventsHost { dst: 7 },
                Instruction::IntrinsicComEventCallbackSubscriptionHost {
                    dst: 8,
                    callback: 7,
                },
                Instruction::LoadConstI32 { slot: 9, value: 0 },
                Instruction::IntrinsicComEventCallbackArgHost {
                    dst: 10,
                    callback: 7,
                    index: 9,
                },
                Instruction::LoadConstI32 { slot: 11, value: 1 },
                Instruction::IntrinsicComEventCallbackArgHost {
                    dst: 12,
                    callback: 7,
                    index: 11,
                },
                Instruction::IntrinsicComReleaseEventCallbackHost {
                    dst: 13,
                    callback: 7,
                },
                Instruction::IntrinsicComUnsubscribeEventHost {
                    dst: 14,
                    subscription: 3,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 15,
            user_slot_count: 15,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy::interactive_dev())
                .build(),
        );
        vm.execute(&bytecode)
            .expect("vm should execute COM event subscribe/unsubscribe flow");
        let out = vm.snapshot_slots(15);
        assert!(out[1] >= 20_001, "expected native COM object handle");
        assert!(out[3] >= 40_001, "expected native COM subscription handle");
        assert_eq!(out[6], 91, "expected FireChangedPair return value");
        assert!(
            out[7] >= 60_001,
            "expected DoEvents callback pump to return callback token"
        );
        assert_eq!(
            out[8], out[3],
            "expected callback subscription lookup to return subscription token"
        );
        assert_eq!(out[10], 90, "expected callback arg0 payload");
        assert_eq!(out[12], 91, "expected callback arg1 payload");
        assert_eq!(out[13], 1, "expected callback release token");
        assert_eq!(out[14], 1, "expected unsubscribe success token");
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires registered external COM typelib lane (run explicitly on Windows host with OxVba.TestEventServer registered)"]
    fn project_com_withevents_routes_auto_pump_registered_event_server_callbacks() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstString {
                    slot: 0,
                    value: "OxVba.TestEventServer".to_string(),
                },
                Instruction::IntrinsicCreateObjectHost { dst: 1, prog_id: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 1 },
                Instruction::LoadConstI32 { slot: 3, value: 77 },
                Instruction::IntrinsicWithEventsSet {
                    dst: 4,
                    owner: 2,
                    binding: 3,
                    value: 1,
                },
                Instruction::LoadConstI32 {
                    slot: 5,
                    value: 102,
                },
                Instruction::LoadConstI32 { slot: 6, value: 7 },
                Instruction::IntrinsicDispatchInvokeHost {
                    dst: 7,
                    object: 1,
                    member: 5,
                    args: vec![DispatchInvokeArg {
                        slot: Some(6),
                        name: None,
                    }],
                },
                Instruction::Halt,
                Instruction::CopySlot { dst: 8, src: 20 },
                Instruction::CopySlot { dst: 9, src: 21 },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 22,
            user_slot_count: 10,
        };

        let mut metadata = BTreeMap::new();
        metadata.insert(
            "pmr_projecta_sink_src_onvaluechanged".to_string(),
            ProcedureRuntimeMetadata {
                module_name: "sink".to_string(),
                procedure_name: "src_onvaluechanged".to_string(),
                entry_pc: 9,
                source_line_start: 1,
                source_line_end: 1,
                statement_line_numbers: vec![1],
                statement_entry_pcs: vec![10],
                slots: vec![],
                param_slots: vec![20, 21],
                return_slot: None,
            },
        );

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy::interactive_dev())
                .build(),
        );
        vm.set_project_procedure_runtime_metadata(metadata);
        vm.set_project_com_withevents_routes(vec![ProjectComWithEventsRoute {
            binding_token: 77,
            prog_id_name: "OxVba.TestEventServer".to_string(),
            event_name: "onvaluechanged".to_string(),
            event_token: 2,
            handler_symbol: "pmr_projecta_sink_src_onvaluechanged".to_string(),
            guard_symbol_zero_arg: String::new(),
            guard_symbol_one_arg: String::new(),
        }]);
        vm.execute(&bytecode)
            .expect("vm should auto-pump registered COM WithEvents callback");
        let out = vm.snapshot_values(10);
        assert_eq!(out[8], RuntimeValue::I32(1));
        assert_eq!(out[9], RuntimeValue::I32(7));
    }

    #[test]
    fn declare_invoke_routes_through_dynlink_host_service() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 3 },
                Instruction::IntrinsicInvokeSymbolHost {
                    dst: 1,
                    descriptor_id: 1_234,
                    symbol: 1_234.into(),
                    args: vec![0],
                    writeback_slots: Vec::new(),
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy {
                    allow_dynamic_link: true,
                    ..oxvba_hal::model::HostPolicy::deterministic_runtime()
                })
                .build(),
        );
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
                    symbol: symbol.into(),
                    args: vec![0],
                    writeback_slots: Vec::new(),
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: "host".to_string(),
                alias: "ping".to_string(),
                ordinal_alias: false,
                symbol: symbol.into(),
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
                param_count: 1,
                param_types: vec![oxvba_compiler::bytecode::DeclareParamType::Long],
                param_by_ref: vec![false],
                return_type: Some(oxvba_compiler::bytecode::DeclareParamType::Long),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy {
                    allow_dynamic_link: true,
                    ..oxvba_hal::model::HostPolicy::deterministic_runtime()
                })
                .build(),
        );
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
                    symbol: symbol.into(),
                    args: vec![0],
                    writeback_slots: Vec::new(),
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: "host".to_string(),
                alias: "ping".to_string(),
                ordinal_alias: false,
                symbol: symbol.into(),
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
                param_count: 1,
                param_types: vec![oxvba_compiler::bytecode::DeclareParamType::Long],
                param_by_ref: vec![false],
                return_type: Some(oxvba_compiler::bytecode::DeclareParamType::Long),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy {
                    allow_dynamic_link: true,
                    ..oxvba_hal::model::HostPolicy::deterministic_runtime()
                })
                .build(),
        );
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
                    symbol: symbol.into(),
                    args: vec![0],
                    writeback_slots: Vec::new(),
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: " ".to_string(),
                alias: "ping".to_string(),
                ordinal_alias: false,
                symbol: symbol.into(),
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
                param_count: 1,
                param_types: vec![oxvba_compiler::bytecode::DeclareParamType::Long],
                param_by_ref: vec![false],
                return_type: Some(oxvba_compiler::bytecode::DeclareParamType::Long),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy {
                    allow_dynamic_link: true,
                    ..oxvba_hal::model::HostPolicy::deterministic_runtime()
                })
                .build(),
        );
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
                    symbol: symbol.into(),
                    args: vec![0],
                    writeback_slots: Vec::new(),
                },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![oxvba_compiler::bytecode::ExternalCallDescriptor {
                descriptor_id: symbol as u32,
                declared_name: "hostping".to_string(),
                library: "host".to_string(),
                alias: "7".to_string(),
                ordinal_alias: true,
                symbol: symbol.into(),
                marshal_lane: "m0-deterministic".to_string(),
                calling_convention: "platform-default".to_string(),
                selection_policy: "case-insensitive-canonical".to_string(),
                param_count: 1,
                param_types: vec![oxvba_compiler::bytecode::DeclareParamType::Long],
                param_by_ref: vec![false],
                return_type: Some(oxvba_compiler::bytecode::DeclareParamType::Long),
            }],
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::new(
            oxvba_hal::adapters::builder::HostBuilder::new()
                .profile(oxvba_hal::model::HalProfileId::Windows)
                .policy(oxvba_hal::model::HostPolicy {
                    allow_dynamic_link: true,
                    ..oxvba_hal::model::HostPolicy::deterministic_runtime()
                })
                .build(),
        );
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
                    mode: StringCompareMode::Binary,
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
                    mode: StringCompareMode::Binary,
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
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpLtSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpNeSlots {
                    dst: 4,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
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
        let values = vm.snapshot_values(8);
        assert_eq!(values[2], RuntimeValue::Bool(true));
        assert_eq!(values[3], RuntimeValue::Bool(false));
        assert_eq!(values[4], RuntimeValue::Bool(true));
        assert_eq!(values[5], RuntimeValue::Bool(true));
        assert_eq!(values[6], RuntimeValue::Bool(true));
        assert_eq!(values[7], RuntimeValue::Bool(true));
        assert_eq!(vm.snapshot_slots(8), vec![5, 3, 1, 0, 1, 1, 1, 1]);
    }

    #[test]
    fn intrinsic_is_object_tag_distinguishes_object_slots() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::IntrinsicIsObjectTag { dst: 2, src: 0 },
                Instruction::IntrinsicIsObjectTag { dst: 3, src: 1 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 4,
            user_slot_count: 4,
        };

        let mut vm = Vm::default();
        vm.write_value_slot(0, RuntimeValue::Object(ObjectRef::from_compat_identity(42)))
            .expect("object slot should be writable");
        vm.write_value_slot(1, RuntimeValue::I32(7))
            .expect("scalar slot should be writable");

        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(4)[2], 1);
        assert_eq!(vm.snapshot_slots(4)[3], 0);
    }

    #[test]
    fn intrinsic_variant_classifiers_read_retained_carriers() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::IntrinsicIsArrayTag { dst: 7, src: 0 },
                Instruction::IntrinsicVarTypeTag { dst: 8, src: 0 },
                Instruction::IntrinsicVarType { dst: 9, src: 6 },
                Instruction::IntrinsicTypeNameTag { dst: 10, src: 0 },
                Instruction::IntrinsicIsNumericTag { dst: 11, src: 0 },
                Instruction::IntrinsicIsNumeric { dst: 12, src: 6 },
                Instruction::IntrinsicIsError { dst: 13, src: 2 },
                Instruction::IntrinsicIsDateTag { dst: 14, src: 1 },
                Instruction::IntrinsicIsObjectTag { dst: 15, src: 3 },
                Instruction::IntrinsicIsNull { dst: 16, src: 4 },
                Instruction::IntrinsicIsEmpty { dst: 17, src: 5 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 18,
            user_slot_count: 18,
        };

        let mut vm = Vm::default();
        vm.write_variant_slot(
            0,
            Variant::from_safearray(oxvba_runtime::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(1),
                RuntimeValue::I32(2),
            ])),
        )
        .expect("array slot should be writable");
        vm.write_variant_slot(1, Variant::from_date_f64(42.0))
            .expect("date slot should be writable");
        vm.write_variant_slot(2, Variant::from_error_code(9))
            .expect("error slot should be writable");
        vm.write_variant_slot(
            3,
            Variant::from_object_ref(ObjectRef::from_compat_identity(42)),
        )
        .expect("object slot should be writable");
        vm.write_variant_slot(4, Variant::null())
            .expect("null slot should be writable");
        vm.write_variant_slot(5, Variant::empty())
            .expect("empty slot should be writable");
        vm.write_variant_slot(6, Variant::from_u8(7))
            .expect("byte slot should be writable");

        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(18)[7], 1);
        assert_eq!(vm.snapshot_slots(18)[8], 8204);
        assert_eq!(vm.snapshot_slots(18)[9], 2);
        assert_eq!(vm.snapshot_slots(18)[10], 1001);
        assert_eq!(vm.snapshot_slots(18)[11], 0);
        assert_eq!(vm.snapshot_slots(18)[12], 1);
        assert_eq!(vm.snapshot_slots(18)[13], 1);
        assert_eq!(vm.snapshot_slots(18)[14], 1);
        assert_eq!(vm.snapshot_slots(18)[15], 1);
        assert_eq!(vm.snapshot_slots(18)[16], 1);
        assert_eq!(vm.snapshot_slots(18)[17], 1);
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
    fn invoke_procedure_with_i32_args_dispatches_into_existing_vm_state() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 11 },
                Instruction::LoadConstI32 { slot: 1, value: 7 },
                Instruction::LoadConstI32 { slot: 2, value: 99 },
                Instruction::IntrinsicWithEventsSet {
                    dst: 3,
                    owner: 0,
                    binding: 1,
                    value: 2,
                },
                Instruction::Halt,
                Instruction::IntrinsicWithEventsGet {
                    dst: 4,
                    owner: 0,
                    binding: 1,
                },
                Instruction::AddSlots {
                    dst: 5,
                    lhs: 4,
                    rhs: 6,
                },
                Instruction::Return,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 7,
            user_slot_count: 7,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("initial run should seed WithEvents bindings");
        vm.invoke_procedure_with_i32_args(&bytecode, 5, &[6], &[1])
            .expect("procedure invoke should execute against existing VM state");
        assert_eq!(vm.snapshot_slots(7)[5], 100);
    }

    #[test]
    fn invoke_procedure_with_i32_args_rejects_mismatched_shape() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        let err = vm
            .invoke_procedure_with_i32_args(&bytecode, 0, &[0], &[])
            .expect_err("invoke should reject mismatched arg slots and values");
        assert!(err.contains("argument shape mismatch"));
    }

    #[test]
    fn invoke_procedure_with_values_dispatches_into_existing_vm_state() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::Jump { target_pc: 4 },
                Instruction::LoadConstI32 { slot: 0, value: 0 },
                Instruction::Halt,
                Instruction::Halt,
                Instruction::CopySlot { dst: 0, src: 1 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::default();
        vm.invoke_procedure_with_values(&bytecode, 4, &[1], &[RuntimeValue::Bool(true)])
            .expect("invoke with runtime values");
        assert_eq!(vm.snapshot_values(2)[0], RuntimeValue::Bool(true));
        assert_eq!(vm.snapshot_values(2)[1], RuntimeValue::Bool(true));
        assert_eq!(vm.snapshot_slots(2), vec![1, 1]);
    }

    #[test]
    fn invoke_procedure_with_variants_preserves_exact_carrier() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::Jump { target_pc: 4 },
                Instruction::Halt,
                Instruction::Halt,
                Instruction::Halt,
                Instruction::CopySlot { dst: 0, src: 1 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::default();
        vm.invoke_procedure_with_variants(&bytecode, 4, &[1], &[Variant::from_string("ABC")])
            .expect("invoke with variant value");

        let variants = vm.snapshot_variants(2);
        assert_eq!(variants[0].vtype(), VarType::String);
        assert_eq!(variants[0].as_bstr(), Some(BStr::from("ABC")));
        assert_eq!(variants[1].vtype(), VarType::String);
        assert_eq!(variants[1].as_bstr(), Some(BStr::from("ABC")));
    }

    #[test]
    fn copy_slot_preserves_non_legacy_runtime_shape() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::Jump { target_pc: 4 },
                Instruction::Halt,
                Instruction::Halt,
                Instruction::Halt,
                Instruction::CopySlot { dst: 0, src: 1 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };

        let mut vm = Vm::default();
        vm.invoke_procedure_with_values(
            &bytecode,
            4,
            &[1],
            &[RuntimeValue::String(BStr::from("ABC"))],
        )
        .expect("invoke with string runtime value");

        assert_eq!(
            vm.snapshot_values(2)[0],
            RuntimeValue::String(BStr::from("ABC"))
        );
        assert_eq!(vm.snapshot_slots(2), vec![EMPTY_TAG, EMPTY_TAG]);
    }

    #[test]
    fn withevents_bindings_are_owner_scoped() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 11 },
                Instruction::LoadConstI32 { slot: 1, value: 22 },
                Instruction::LoadConstI32 { slot: 2, value: 7 },
                Instruction::LoadConstI32 {
                    slot: 3,
                    value: 101,
                },
                Instruction::LoadConstI32 {
                    slot: 4,
                    value: 202,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 5,
                    owner: 0,
                    binding: 2,
                    value: 3,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 6,
                    owner: 1,
                    binding: 2,
                    value: 4,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 7,
                    owner: 0,
                    binding: 2,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 8,
                    owner: 1,
                    binding: 2,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 9,
            user_slot_count: 9,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(9)[7], 101);
        assert_eq!(vm.snapshot_slots(9)[8], 202);
    }

    #[test]
    fn withevents_clear_only_removes_matching_owner_binding() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 11 },
                Instruction::LoadConstI32 { slot: 1, value: 22 },
                Instruction::LoadConstI32 { slot: 2, value: 7 },
                Instruction::LoadConstI32 {
                    slot: 3,
                    value: 101,
                },
                Instruction::LoadConstI32 {
                    slot: 4,
                    value: 202,
                },
                Instruction::LoadConstI32 { slot: 5, value: 0 },
                Instruction::IntrinsicWithEventsSet {
                    dst: 6,
                    owner: 0,
                    binding: 2,
                    value: 3,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 7,
                    owner: 1,
                    binding: 2,
                    value: 4,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 8,
                    owner: 0,
                    binding: 2,
                    value: 5,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 9,
                    owner: 0,
                    binding: 2,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 10,
                    owner: 1,
                    binding: 2,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 11,
            user_slot_count: 11,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(11)[9], 0);
        assert_eq!(vm.snapshot_slots(11)[10], 202);
    }

    #[test]
    fn withevents_clear_owner_removes_all_bindings_for_owner() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 11 },
                Instruction::LoadConstI32 { slot: 1, value: 22 },
                Instruction::LoadConstI32 { slot: 2, value: 7 },
                Instruction::LoadConstI32 { slot: 3, value: 8 },
                Instruction::LoadConstI32 {
                    slot: 4,
                    value: 101,
                },
                Instruction::LoadConstI32 {
                    slot: 5,
                    value: 202,
                },
                Instruction::LoadConstI32 {
                    slot: 6,
                    value: 303,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 7,
                    owner: 0,
                    binding: 2,
                    value: 4,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 8,
                    owner: 0,
                    binding: 3,
                    value: 5,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 9,
                    owner: 1,
                    binding: 2,
                    value: 6,
                },
                Instruction::IntrinsicWithEventsClearOwner { dst: 10, owner: 0 },
                Instruction::IntrinsicWithEventsGet {
                    dst: 11,
                    owner: 0,
                    binding: 2,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 12,
                    owner: 0,
                    binding: 3,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 13,
                    owner: 1,
                    binding: 2,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 14,
            user_slot_count: 14,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(14)[11], 0);
        assert_eq!(vm.snapshot_slots(14)[12], 0);
        assert_eq!(vm.snapshot_slots(14)[13], 303);
    }

    #[test]
    fn withevents_owner_iteration_intrinsics_yield_deterministic_owner_order() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 11 },
                Instruction::LoadConstI32 { slot: 1, value: 22 },
                Instruction::LoadConstI32 { slot: 2, value: 33 },
                Instruction::LoadConstI32 { slot: 3, value: 7 },
                Instruction::LoadConstI32 { slot: 4, value: 8 },
                Instruction::LoadConstI32 { slot: 5, value: 5 },
                Instruction::IntrinsicWithEventsSet {
                    dst: 6,
                    owner: 0,
                    binding: 3,
                    value: 5,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 7,
                    owner: 1,
                    binding: 3,
                    value: 5,
                },
                Instruction::IntrinsicWithEventsSet {
                    dst: 8,
                    owner: 2,
                    binding: 4,
                    value: 5,
                },
                Instruction::IntrinsicWithEventsFirstOwner {
                    dst: 9,
                    source: 5,
                    binding: 3,
                },
                Instruction::IntrinsicWithEventsNextOwner { dst: 10 },
                Instruction::IntrinsicWithEventsNextOwner { dst: 11 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 12,
            user_slot_count: 12,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(12)[9], 11);
        assert_eq!(vm.snapshot_slots(12)[10], 22);
        assert_eq!(vm.snapshot_slots(12)[11], 0);
    }

    #[test]
    fn withevents_bindings_preserve_runtime_value_shape() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::IntrinsicWithEventsSet {
                    dst: 3,
                    owner: 0,
                    binding: 1,
                    value: 2,
                },
                Instruction::IntrinsicWithEventsGet {
                    dst: 4,
                    owner: 0,
                    binding: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 5,
            user_slot_count: 5,
        };

        let mut vm = Vm::default();
        vm.write_value_slot(0, RuntimeValue::I32(11))
            .expect("owner slot should be writable");
        vm.write_value_slot(1, RuntimeValue::I32(7))
            .expect("binding slot should be writable");
        vm.write_value_slot(2, RuntimeValue::Bool(true))
            .expect("value slot should be writable");

        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_values(5)[4], RuntimeValue::Bool(true));
        assert_eq!(vm.snapshot_slots(5)[4], 1);
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
    fn resume_next_without_pending_error_raises_error_20() {
        // Under OERN, Resume Next with no pending error raises VBA error 20.
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorResumeNext,
                Instruction::RaiseError { code: 5 },
                // Error 5 was handled by OERN (no pending error remains).
                Instruction::ResumeNext,
                // Resume Next raises error 20; OERN swallows it.
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("resume next should raise error 20 under OERN");
        assert_eq!(vm.snapshot_slots(1), vec![20]);
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
                CapabilityId::ConsoleIo,
                CapabilityId::UiInteraction,
                CapabilityId::EventPump,
                CapabilityId::FileSystemIo,
                CapabilityId::ProcessEnv,
                CapabilityId::ComActivationDispatch,
                CapabilityId::TimeLocale,
                CapabilityId::DynamicLinking,
                CapabilityId::DiagnosticsTelemetry,
                CapabilityId::ProjectCatalog,
                CapabilityId::ProjectReferenceProvider,
                CapabilityId::ProjectMutation,
            ] {
                let code = Vm::hal_error_code(kind, capability);
                assert!(
                    (53_011..=53_124).contains(&code),
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
    use oxvba_compiler::{Bytecode, Instruction, bytecode::StringCompareMode};
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
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpNeSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpLtSlots {
                    dst: 4,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpLeSlots {
                    dst: 5,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpGtSlots {
                    dst: 6,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
                },
                Instruction::CmpGeSlots {
                    dst: 7,
                    lhs: 0,
                    rhs: 1,
                    mode: StringCompareMode::Binary,
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

    #[test]
    fn mul_slots_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 6 },
                Instruction::LoadConstI32 { slot: 1, value: 7 },
                Instruction::MulSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(3)[2], 42);
    }

    #[test]
    fn div_slots_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 15 },
                Instruction::LoadConstI32 { slot: 1, value: 3 },
                Instruction::DivSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(3)[2], 5);
    }

    #[test]
    fn div_slots_zero_error() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::LoadConstI32 { slot: 1, value: 0 },
                Instruction::DivSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        let result = vm.execute(&bytecode);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Division by zero"));
    }

    #[test]
    fn intdiv_slots_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 17 },
                Instruction::LoadConstI32 { slot: 1, value: 3 },
                Instruction::IntDivSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(3)[2], 5);
    }

    #[test]
    fn mod_slots_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 17 },
                Instruction::LoadConstI32 { slot: 1, value: 3 },
                Instruction::ModSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(3)[2], 2);
    }

    #[test]
    fn pow_slots_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 2 },
                Instruction::LoadConstI32 { slot: 1, value: 10 },
                Instruction::PowSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(3)[2], 1024);
    }

    #[test]
    fn sub_slots_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::LoadConstI32 { slot: 1, value: 3 },
                Instruction::SubSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(3)[2], 7);
    }

    #[test]
    fn neg_slot_basic() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 5 },
                Instruction::NegSlot { dst: 1, src: 0 },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 2,
            user_slot_count: 2,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute");
        assert_eq!(vm.snapshot_slots(2)[1], -5);
    }

    // --- Feature 2: Currency/Decimal f64 promotion (v521-v522) ---

    #[test]
    fn formal_v521_currency_add_promotes_to_f64() {
        use oxvba_runtime::CurrencyValue;
        let mut vm = Vm::default();
        vm.reset_execution_state(3, false);
        // Currency 1.5 = scaled 15000
        vm.write_value_slot(
            0,
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(15_000)),
        )
        .unwrap();
        vm.write_value_slot(1, RuntimeValue::I32(1)).unwrap();
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::AddSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        vm.execute(&bytecode).expect("vm should execute");
        let result = vm.snapshot_values(3)[2].clone();
        assert_eq!(
            result,
            RuntimeValue::F64(oxvba_runtime::F64Value::from_f64(2.5))
        );
    }

    #[test]
    fn formal_v522_currency_div_promotes_to_f64() {
        use oxvba_runtime::CurrencyValue;
        let mut vm = Vm::default();
        vm.reset_execution_state(3, false);
        // Currency 10.0 = scaled 100000
        vm.write_value_slot(
            0,
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(100_000)),
        )
        .unwrap();
        vm.write_value_slot(1, RuntimeValue::I32(3)).unwrap();
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::DivSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            external_call_descriptors: Vec::new(),
            slot_count: 3,
            user_slot_count: 3,
        };
        vm.execute(&bytecode).expect("vm should execute");
        let result = vm.snapshot_values(3)[2].clone();
        match result {
            RuntimeValue::F64(v) => {
                let diff = (v.as_f64() - 10.0 / 3.0).abs();
                assert!(diff < 1e-10, "expected ~3.333..., got {}", v.as_f64());
            }
            other => panic!("expected F64, got {:?}", other),
        }
    }
}
