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
    Bytecode, Instruction, ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind,
    ProjectComWithEventsRoute, ProjectDynamicMemberKind, ProjectDynamicMemberRoute,
    ProjectDynamicObjectRoute, ProjectDynamicParamRoute,
    bytecode::{
        ExternalCallWriteback, ExternalCallWritebackKind, RuntimeArrayElementType,
        StringCompareMode,
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
    VT_VARIANT_VALUE,
};
use oxvba_runtime::{
    BindingHandle, ObjectRef, RuntimeClassDescriptor, RuntimeInterfaceDescriptor,
    RuntimeInterfaceId, RuntimeMemberDescriptor, RuntimeMemberInvokeKind, Variant, bstr::BStr,
};

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

fn runtime_invoke_kind_for_project_dynamic_member(
    kind: ProjectDynamicMemberKind,
) -> RuntimeMemberInvokeKind {
    match kind {
        ProjectDynamicMemberKind::Method | ProjectDynamicMemberKind::Function => {
            RuntimeMemberInvokeKind::Method
        }
        ProjectDynamicMemberKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        ProjectDynamicMemberKind::PropertyLet => RuntimeMemberInvokeKind::PropertyLet,
        ProjectDynamicMemberKind::PropertySet => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn leak_runtime_descriptor_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
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
                let descriptor = Self::leak_project_dynamic_class_descriptor(&route);
                (
                    raw,
                    ProjectDynamicObjectState {
                        object: ObjectRef::from_compat_identity_with_descriptor(raw, descriptor),
                        route,
                    },
                )
            })
            .collect();
    }

    fn leak_project_dynamic_class_descriptor(
        route: &ProjectDynamicObjectRoute,
    ) -> &'static RuntimeClassDescriptor {
        let members = route
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| RuntimeMemberDescriptor {
                name: leak_runtime_descriptor_str(member.member_name.clone()),
                dispatch_id: member
                    .dispatch_id
                    .or(member.known_dispatch_token)
                    .unwrap_or_else(|| (index as i32) + 1),
                vtable_slot: Some((7 + index) as u16),
                invoke_kind: runtime_invoke_kind_for_project_dynamic_member(member.kind),
                is_default_member: member.is_default_member,
            })
            .collect::<Vec<_>>();
        let interface_name = leak_runtime_descriptor_str(format!(
            "{}.{}._Default",
            route.project_name, route.module_name
        ));
        let class_name =
            leak_runtime_descriptor_str(format!("{}.{}", route.project_name, route.module_name));
        let members = Box::leak(members.into_boxed_slice());
        let interfaces = Box::leak(
            vec![RuntimeInterfaceDescriptor {
                id: RuntimeInterfaceId::IDispatch,
                name: interface_name,
                members,
                dual_dispatch: true,
            }]
            .into_boxed_slice(),
        );
        Box::leak(Box::new(RuntimeClassDescriptor {
            name: class_name,
            interfaces,
        }))
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
            self.write_i32_slot(*slot, *value)?;
        }
        self.execute_loop(
            bytecode,
            entry_pc,
            entry_pc,
            self.typed_fastpaths_default,
            true,
        )
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
        self.clear_invoked_procedure_slots(entry_pc)?;
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

    fn clear_invoked_procedure_slots(&mut self, entry_pc: usize) -> Result<(), String> {
        let slots = self
            .procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
            .map(|metadata| metadata.slots.clone())
            .unwrap_or_default();
        for slot in slots {
            if matches!(
                slot.kind,
                ProcedureRuntimeSlotKind::Parameter
                    | ProcedureRuntimeSlotKind::Local
                    | ProcedureRuntimeSlotKind::ReturnValue
            ) {
                self.write_variant_slot(slot.slot, Variant::empty())?;
            }
        }
        Ok(())
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
                    self.write_variant_slot(*slot, Variant::from_i32(*value))?;
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
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, crate::semantics::variant_int_value(&value)?)?;
                    pc += 1;
                }
                Instruction::IntrinsicFixI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, crate::semantics::variant_fix_value(&value)?)?;
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
                    let rate = self.read_i32_slot(*rate)?;
                    let nper = self.read_i32_slot(*nper)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let pv = match pv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::fv_i32(rate, nper, pmt, pv, due)),
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
                    let rate = self.read_i32_slot(*rate)?;
                    let nper = self.read_i32_slot(*nper)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::pv_i32(rate, nper, pmt, fv, due)),
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
                    let rate = self.read_i32_slot(*rate)?;
                    let nper = self.read_i32_slot(*nper)?;
                    let pv = self.read_i32_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::pmt_i32(rate, nper, pv, fv, due)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicNpvI32 { dst, rate, values } => {
                    let rate = self.read_i32_slot(*rate)?;
                    let mut cash_flows = Vec::with_capacity(values.len());
                    for slot in values {
                        cash_flows.push(self.read_i32_slot(*slot)?);
                    }
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::npv_i32(rate, &cash_flows)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIrrI32 { dst, value, guess } => {
                    let value = self.read_i32_slot(*value)?;
                    let guess = match guess {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 10,
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(Self::irr_i32(value, guess)))?;
                    pc += 1;
                }
                Instruction::IntrinsicMirrI32 {
                    dst,
                    value,
                    finance_rate,
                    reinvest_rate,
                } => {
                    let value = self.read_i32_slot(*value)?;
                    let finance_rate = self.read_i32_slot(*finance_rate)?;
                    let reinvest_rate = self.read_i32_slot(*reinvest_rate)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::mirr_i32(value, finance_rate, reinvest_rate)),
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
                    let nper = self.read_i32_slot(*nper)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let pv = self.read_i32_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let guess = match guess {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 10,
                    };
                    let value = match Self::rate_i32(nper, pmt, pv, fv, due, guess) {
                        Ok(value) => Variant::from_i32(value),
                        Err(code) => Variant::from_error_code(code),
                    };
                    self.write_variant_slot(*dst, value)?;
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
                    let rate = self.read_i32_slot(*rate)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let pv = self.read_i32_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let value = match Self::nper_i32(rate, pmt, pv, fv, due) {
                        Ok(value) => Variant::from_i32(value),
                        Err(code) => Variant::from_error_code(code),
                    };
                    self.write_variant_slot(*dst, value)?;
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
                        match crate::semantics::runtime_variant_to_i32_compat(
                            &self.read_variant_slot(*upper_bound)?,
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
                        match crate::semantics::runtime_variant_to_i32_compat(
                            &self.read_variant_slot(*upper_bound)?,
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
                        .map(|slot| self.read_variant_slot(*slot))
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
                        .map(|slot| self.read_variant_slot(*slot))
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
                            self.write_variant_slot(*iter, Variant::from_i32(id))?;
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
                    let iter_id = crate::semantics::runtime_variant_to_i32_compat(
                        &self.read_variant_slot(*iter)?,
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
                Instruction::IntrinsicCVErr { dst, src } => {
                    let code = self.read_i32_slot(*src)?.saturating_abs();
                    self.write_variant_slot(*dst, Variant::from_error_code(code))?;
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
                    let value = self.read_variant_slot(*src)?;
                    crate::semantics::validate_runtime_assignment_variant(
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
                    let count = self.read_i32_slot(*count)?;
                    let _item = self.read_i32_slot(*item)?;
                    self.write_i32_slot(*dst, (count + 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionItem { dst, count, index } => {
                    let count = self.read_i32_slot(*count)?;
                    let index = self.read_i32_slot(*index)?;
                    let out = if index >= 1 && index <= count {
                        index
                    } else {
                        0
                    };
                    self.write_i32_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionRemove { dst, count, index } => {
                    let count = self.read_i32_slot(*count)?;
                    let _index = self.read_i32_slot(*index)?;
                    self.write_i32_slot(*dst, (count - 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionCount { dst, count } => {
                    let count = self.read_i32_slot(*count)?;
                    self.write_i32_slot(*dst, count.max(0))?;
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
                    let index = match if matches!(index.vtype(), oxvba_runtime::VarType::Empty) {
                        Ok(0)
                    } else {
                        crate::semantics::variant_to_usize_index(
                            &index,
                            "com_event_callback_arg.index",
                        )
                    } {
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
                Instruction::IntrinsicWithEventsGet {
                    dst,
                    owner,
                    binding,
                } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let binding = self.read_variant_slot(*binding)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    let binding = crate::semantics::variant_to_withevents_binding_handle(
                        &binding, "binding",
                    )?;
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
                    let binding = self.read_variant_slot(*binding)?;
                    let value = self.read_variant_slot(*value)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    let binding = crate::semantics::variant_to_withevents_binding_handle(
                        &binding, "binding",
                    )?;
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
                    let binding = self.read_variant_slot(*binding)?;
                    let binding = crate::semantics::variant_to_withevents_binding_handle(
                        &binding, "binding",
                    )?;
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
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
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
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
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
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
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
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
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
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
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
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
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
                Instruction::LoadEmpty { slot } => {
                    self.write_variant_slot(*slot, Variant::empty())?;
                    pc += 1;
                }
                Instruction::BoolNot { dst, src } => {
                    let src = self.read_variant_slot(*src)?;
                    let out = !crate::semantics::variant_truthy_value(&src)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::BoolAnd { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_truthy_value(&lhs)?
                        && crate::semantics::variant_truthy_value(&rhs)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::BoolOr { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_truthy_value(&lhs)?
                        || crate::semantics::variant_truthy_value(&rhs)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::JumpIfZero {
                    cond_slot,
                    target_pc,
                } => {
                    let cond = self.read_variant_slot(*cond_slot)?;
                    pc = Self::next_pc_for_jump_if_zero_variant(&cond, *target_pc, len, pc)?;
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
                        let seed_val = self.read_variant_slot(*seed_slot)?;
                        let seed_val = crate::semantics::runtime_random_seed_variant_bounded(
                            &seed_val, "Rnd seed",
                        )?;
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
                        let seed_val = self.read_variant_slot(*seed_slot)?;
                        let seed_val = crate::semantics::runtime_random_seed_variant_bounded(
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

    fn read_i32_slot(&self, slot: usize) -> Result<i32, String> {
        crate::semantics::runtime_variant_to_i32_compat(
            &self.read_variant_slot(slot)?,
            "i32 intrinsic operand",
        )
        .map_err(|detail| format!("runtime value in slot {slot} cannot be read as i32: {detail}"))
    }

    fn write_i32_slot(&mut self, slot: usize, value: i32) -> Result<(), String> {
        self.write_variant_slot(slot, Variant::from_i32(value))
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

        if let Some(handle) = iterable.as_i32()
            && self.project_dynamic_objects.contains_key(&handle)
        {
            return self.materialize_foreach_items_from_object(
                bytecode,
                typed_fastpaths,
                ObjectRef::from_compat_identity(handle),
            );
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
                    return Err(ForEachInitError {
                        code: 13,
                        detail: format!(
                            "For Each NewEnum source on object {object} returned unsupported value {value:?}"
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
        value.clone()
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
            if !Self::withevents_source_matches(value, source_variant)
                || Self::withevents_binding_from_key(*key) != binding
            {
                continue;
            }
            owners.push(Self::withevents_owner_from_key(*key));
        }
        owners
    }

    fn withevents_source_matches(bound: &Variant, source: &Variant) -> bool {
        if bound == source {
            return true;
        }
        matches!(
            (
                Self::withevents_source_identity(bound),
                Self::withevents_source_identity(source)
            ),
            (Some(bound), Some(source)) if bound == source
        )
    }

    fn withevents_source_identity(value: &Variant) -> Option<i32> {
        if let Some(object) = value.as_object_ref() {
            let raw = object.raw();
            if raw > 0 {
                return Some(raw);
            }
        }
        value.as_i32().filter(|raw| *raw > 0)
    }

    fn clear_all_com_withevents_state_best_effort(&mut self) {
        for subscription in self
            .com_withevents_subscriptions
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            let _ = self
                .host_services
                .com()
                .unsubscribe_event_variant(subscription);
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
                .unsubscribe_event_variant(subscription)
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
                    let _ = self
                        .host_services
                        .com()
                        .release_event_callback_variant(callback);
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
                .release_event_callback_variant(callback)
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
        *dst = RuntimeSlot::Variant(Variant::from_i32(current + value));
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
        *dst = RuntimeSlot::Variant(Variant::from_i32(current - value));
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

    fn next_pc_for_jump_if_zero_variant(
        cond: &Variant,
        target_pc: usize,
        instruction_len: usize,
        current_pc: usize,
    ) -> Result<usize, String> {
        let cond = crate::semantics::variant_truthy_value(cond)?;
        Self::next_pc_for_jump_if_zero(
            if cond { -1 } else { 0 },
            target_pc,
            instruction_len,
            current_pc,
        )
    }

    fn normalize_for_compare(text: String, mode: StringCompareMode) -> String {
        crate::semantics::normalize_for_compare(text, mode)
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

    fn rate_i32(nper: i32, pmt: i32, pv: i32, fv: i32, due: i32, guess: i32) -> Result<i32, i32> {
        if nper == 0 {
            return Err(FIN_RATE_ERROR_CODE);
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
                return Err(FIN_RATE_ERROR_CODE);
            }
            let next = (r - f / fp).clamp(-0.99, 10.0);
            if !next.is_finite() {
                return Err(FIN_RATE_ERROR_CODE);
            }
            if (next - r).abs() < FIN_EPS {
                r = next;
                return Ok((r * 100.0).round() as i32);
            }
            r = next;
        }
        Err(FIN_RATE_ERROR_CODE)
    }

    fn nper_i32(rate: i32, pmt: i32, pv: i32, fv: i32, due: i32) -> Result<i32, i32> {
        let pmt = pmt as f64;
        let pv = pv as f64;
        let fv = fv as f64;
        let due = if due != 0 { 1.0 } else { 0.0 };

        if rate == 0 {
            if pmt == 0.0 {
                return Err(FIN_NPER_ERROR_CODE);
            }
            return Ok((-(pv + fv) / pmt).round() as i32);
        }

        let r = rate as f64 / 100.0;
        let numerator = pmt * (1.0 + r * due) - fv * r;
        let denominator = pv * r + pmt * (1.0 + r * due);
        if numerator <= 0.0 || denominator <= 0.0 || (1.0 + r) <= 0.0 {
            return Err(FIN_NPER_ERROR_CODE);
        }

        let n = (numerator / denominator).ln() / (1.0 + r).ln();
        if !n.is_finite() {
            return Err(FIN_NPER_ERROR_CODE);
        }
        Ok(n.round() as i32)
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
        // Keep this slice-based for V170 evidence: text[start..end] / text[start..].
        resized_values[resized_start..(resized_start + overlap)]
            .clone_from_slice(&previous_values[previous_start..(previous_start + overlap)]);
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
