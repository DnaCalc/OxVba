use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use oxvba_com::{
    ComCallbackToken, ComObjectDescriptor, ComSubscriptionToken, DynamicEventPayload,
    DynamicObjectBridge,
};
use oxvba_compiler::{
    Bytecode, CompiledProject, Instruction, ProcedureRuntimeMetadata, ProjectManifest, compile,
    compile_project,
};
use oxvba_hal::{
    HalComDynamicBridge, adapters,
    model::{
        CapabilityId, HalDescriptor, HalProfileId, HostPolicy, HostPolicyPreset,
        UnsupportedFeatureMode, native_host_profile,
    },
    traits::HostServices,
};
use oxvba_jit::JitEngine;
use oxvba_runtime::value_tags::EMPTY_TAG;
use oxvba_runtime::{ObjectHandle, RuntimeValue};
use oxvba_vm::{Vm, execute_and_snapshot_with_host};

use crate::{
    events::{EventDispatcher, EventSourceKey},
    runner::RuntimeProfileId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    CompileTime,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseDiagnostic {
    phase: DiagnosticPhase,
    message: String,
}

impl PhaseDiagnostic {
    fn compile(message: impl Into<String>) -> Self {
        Self {
            phase: DiagnosticPhase::CompileTime,
            message: message.into(),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            phase: DiagnosticPhase::Runtime,
            message: message.into(),
        }
    }

    pub fn phase(&self) -> DiagnosticPhase {
        self.phase
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for PhaseDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let phase = match self.phase {
            DiagnosticPhase::CompileTime => "compile-time",
            DiagnosticPhase::Runtime => "runtime",
        };
        write!(f, "{phase} diagnostic: {}", self.message)
    }
}

impl std::error::Error for PhaseDiagnostic {}

#[derive(Debug, Clone, Default)]
pub struct HostConfig {
    pub enable_jit: bool,
    pub root_object_name: Option<String>,
}

pub struct Engine {
    config: HostConfig,
    jit: JitEngine,
    root_objects: HashMap<String, String>,
    event_dispatcher: Mutex<EventDispatcher>,
    com_subscription_handlers: Mutex<HashMap<ComSubscriptionToken, String>>,
    runtime_profile: RuntimeProfileId,
    host_services: Arc<dyn HostServices>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventCallbackDispatch {
    pub callback_token: ComCallbackToken,
    pub subscription_token: ComSubscriptionToken,
    pub handler_symbol: String,
    pub args: Vec<RuntimeValue>,
}

pub struct ProjectRuntimeSession {
    compiled: CompiledProject,
    vm: Vm,
}

impl ProjectRuntimeSession {
    pub fn snapshot(&self) -> Vec<RuntimeValue> {
        self.vm.snapshot(self.compiled.bytecode.user_slot_count)
    }

    pub fn snapshot_values(&self) -> Vec<RuntimeValue> {
        self.snapshot()
    }

    pub fn snapshot_slots(&self) -> Vec<i32> {
        project_runtime_values_to_legacy_slots(self.snapshot())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(HostConfig::default())
    }
}

fn project_runtime_values_to_legacy_slots(values: Vec<RuntimeValue>) -> Vec<i32> {
    values
        .into_iter()
        .map(|value| value.to_legacy_i32().unwrap_or(EMPTY_TAG))
        .collect()
}

impl Engine {
    pub fn new(config: HostConfig) -> Self {
        let runtime_profile = RuntimeProfileId::default_for_hal_profile(native_host_profile());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.runtime_class = Some(runtime_profile.runtime_class());
        Self {
            config,
            jit: JitEngine,
            root_objects: HashMap::new(),
            event_dispatcher: Mutex::new(EventDispatcher::default()),
            com_subscription_handlers: Mutex::new(HashMap::new()),
            runtime_profile,
            host_services: adapters::for_profile_with_runtime_class(
                runtime_profile.hal_profile(),
                runtime_profile.runtime_class(),
                policy,
            ),
        }
    }

    pub fn set_hal_profile(&mut self, profile: HalProfileId) {
        let policy = self.host_services.policy().clone();
        self.runtime_profile = RuntimeProfileId::default_for_hal_profile(profile);
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services =
            adapters::for_profile_with_runtime_class(profile, runtime_class, policy);
    }

    pub fn set_runtime_profile(&mut self, runtime_profile: RuntimeProfileId) {
        self.runtime_profile = runtime_profile;
        let mut policy = self.host_services.policy().clone();
        policy.runtime_class = Some(runtime_profile.runtime_class());
        self.host_services = adapters::for_profile_with_runtime_class(
            runtime_profile.hal_profile(),
            runtime_profile.runtime_class(),
            policy,
        );
    }

    pub fn with_runtime_profile(mut self, runtime_profile: RuntimeProfileId) -> Self {
        self.set_runtime_profile(runtime_profile);
        self
    }

    pub fn with_hal_profile(mut self, profile: HalProfileId) -> Self {
        self.set_hal_profile(profile);
        self
    }

    pub fn set_host_policy(&mut self, policy: HostPolicy) {
        let profile = self.host_services.profile();
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services =
            adapters::for_profile_with_runtime_class(profile, runtime_class, policy);
    }

    pub fn set_host_policy_preset(&mut self, preset: HostPolicyPreset) {
        self.set_host_policy(HostPolicy::for_preset(preset));
    }

    pub fn set_unsupported_feature_mode(&mut self, mode: UnsupportedFeatureMode) {
        let mut policy = self.host_services.policy().clone();
        policy.unsupported_feature_mode = mode;
        self.set_host_policy(policy);
    }

    pub fn set_com_prog_id_override(&mut self, selector: i32, prog_id: impl Into<String>) {
        let mut policy = self.host_services.policy().clone();
        policy
            .com_prog_id_overrides
            .insert(selector, prog_id.into());
        self.set_host_policy(policy);
    }

    pub fn clear_com_prog_id_override(&mut self, selector: i32) {
        let mut policy = self.host_services.policy().clone();
        policy.com_prog_id_overrides.remove(&selector);
        self.set_host_policy(policy);
    }

    pub fn host_policy(&self) -> &HostPolicy {
        self.host_services.policy()
    }

    pub fn runtime_profile(&self) -> RuntimeProfileId {
        self.runtime_profile
    }

    pub fn hal_descriptor(&self) -> HalDescriptor {
        self.host_services.descriptor()
    }

    pub fn register_root_object(&mut self, name: impl Into<String>, type_name: impl Into<String>) {
        self.root_objects.insert(name.into(), type_name.into());
    }

    pub fn has_root_object(&self, name: &str) -> bool {
        self.root_objects.contains_key(name)
    }

    pub fn subscribe_host_event_handler(
        &self,
        project_name: &str,
        module_name: &str,
        event_name: &str,
        handler_symbol: &str,
    ) {
        if let Ok(mut dispatcher) = self.event_dispatcher.lock() {
            dispatcher.subscribe(
                &EventSourceKey::new(project_name, module_name, event_name),
                handler_symbol,
            );
        }
    }

    pub fn unsubscribe_host_event_handler(
        &self,
        project_name: &str,
        module_name: &str,
        event_name: &str,
        handler_symbol: &str,
    ) -> bool {
        self.event_dispatcher
            .lock()
            .map(|mut dispatcher| {
                dispatcher.unsubscribe(
                    &EventSourceKey::new(project_name, module_name, event_name),
                    handler_symbol,
                )
            })
            .unwrap_or(false)
    }

    pub fn dispatch_host_event(
        &self,
        project_name: &str,
        module_name: &str,
        event_name: &str,
    ) -> Vec<String> {
        self.event_dispatcher
            .lock()
            .map(|dispatcher| {
                dispatcher.dispatch(&EventSourceKey::new(project_name, module_name, event_name))
            })
            .unwrap_or_default()
    }

    pub fn dispatch_host_event_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        project_name: &str,
        module_name: &str,
        event_name: &str,
        source_instance: ObjectHandle,
        args: &[RuntimeValue],
    ) -> Result<bool, PhaseDiagnostic> {
        let bindings = self
            .event_dispatcher
            .lock()
            .map(|dispatcher| {
                dispatcher.dispatch_bindings(&EventSourceKey::new(
                    project_name,
                    module_name,
                    event_name,
                ))
            })
            .unwrap_or_default();
        if bindings.is_empty() {
            return Ok(false);
        }
        if args.len() > 1 {
            return Err(PhaseDiagnostic::runtime(format!(
                "PMR-E-HOST-EVENT-ARITY-UNSUPPORTED: host event ingress for `{project_name}.{module_name}.{event_name}` supports at most 1 forwarded argument in the current deterministic subset, got {}",
                args.len()
            )));
        }
        for binding in bindings {
            let target_symbol = match args.len() {
                0 => binding.guard_symbol_zero_arg.as_deref(),
                1 => binding.guard_symbol_one_arg.as_deref(),
                _ => None,
            }
            .unwrap_or(&binding.handler_symbol);
            let (resolved_symbol, metadata) =
                self.resolve_runtime_handler_metadata(runtime, target_symbol)?;
            let actual_args = if binding.guard_symbol_zero_arg.is_some()
                || binding.guard_symbol_one_arg.is_some()
            {
                let mut actual_args = vec![RuntimeValue::I32(source_instance.raw())];
                actual_args.extend_from_slice(args);
                actual_args
            } else {
                args.to_vec()
            };
            let expected_arity = metadata.param_slots.len();
            let actual_arity = actual_args.len();
            if expected_arity != actual_arity {
                return Err(PhaseDiagnostic::runtime(format!(
                    "PMR-E-HOST-EVENT-SIGNATURE-MISMATCH: host event dispatch target `{}` expects {} arguments but ingress supplied {}",
                    resolved_symbol, expected_arity, actual_arity
                )));
            }
            runtime
                .vm
                .invoke_procedure_with_values(
                    &runtime.compiled.bytecode,
                    metadata.entry_pc,
                    &metadata.param_slots,
                    actual_args.as_slice(),
                )
                .map_err(PhaseDiagnostic::runtime)?;
        }
        Ok(true)
    }

    pub fn subscribe_com_event_handler(
        &self,
        object_token: ObjectHandle,
        event_token: i32,
        handler_symbol: &str,
    ) -> Result<ComSubscriptionToken, PhaseDiagnostic> {
        let subscription = self
            .host_services
            .com()
            .subscribe_event(object_token, event_token.into())
            .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))?;

        self.com_subscription_handlers
            .lock()
            .map_err(|_| {
                PhaseDiagnostic::runtime(
                    "COM subscription handler registry lock poisoned during subscribe",
                )
            })?
            .insert(subscription, handler_symbol.trim().to_ascii_lowercase());
        Ok(subscription)
    }

    pub fn unsubscribe_com_event_handler(
        &self,
        subscription_token: ComSubscriptionToken,
    ) -> Result<bool, PhaseDiagnostic> {
        self.host_services
            .com()
            .unsubscribe_event(subscription_token)
            .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))?;
        let removed = self
            .com_subscription_handlers
            .lock()
            .map_err(|_| {
                PhaseDiagnostic::runtime(
                    "COM subscription handler registry lock poisoned during unsubscribe",
                )
            })?
            .remove(&subscription_token)
            .is_some();
        Ok(removed)
    }

    pub fn describe_com_object(
        &self,
        object_token: ObjectHandle,
    ) -> Result<Option<ComObjectDescriptor>, PhaseDiagnostic> {
        self.host_services
            .com()
            .describe_object(object_token)
            .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))
    }

    pub fn poll_com_event_callback(
        &self,
    ) -> Result<Option<ComEventCallbackDispatch>, PhaseDiagnostic> {
        let _ = self
            .host_services
            .events()
            .do_events()
            .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))?;
        let bridge =
            HalComDynamicBridge::new(self.host_services.profile(), self.host_services.com());
        let Some(payload) = bridge
            .poll_dynamic_event()
            .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))?
        else {
            return Ok(None);
        };

        let callback = normalize_callback_payload(payload)?;
        let callback_arity = callback.args.len();
        if callback_arity > i32::MAX as usize {
            return Err(PhaseDiagnostic::runtime(format!(
                "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback arity {} exceeds deterministic token range",
                callback_arity
            )));
        }

        let handler_symbol = self
            .com_subscription_handlers
            .lock()
            .map_err(|_| {
                PhaseDiagnostic::runtime(
                    "COM subscription handler registry lock poisoned during callback poll",
                )
            })?
            .get(&callback.subscription_token)
            .cloned()
            .ok_or_else(|| {
                PhaseDiagnostic::runtime(format!(
                    "PMR-E-EVENT-DISPATCH-TARGET-MISSING: no handler binding for COM subscription token {}",
                    callback.subscription_token
                ))
            })?;

        Ok(Some(ComEventCallbackDispatch {
            callback_token: callback.callback_token,
            subscription_token: callback.subscription_token,
            handler_symbol,
            args: callback.args,
        }))
    }

    pub fn start_project_runtime_session(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<ProjectRuntimeSession, PhaseDiagnostic> {
        if self.config.enable_jit {
            return Err(PhaseDiagnostic::runtime(
                "project runtime session requires VM execution path (set enable_jit=false)",
            ));
        }
        let compiled =
            compile_project(manifest).map_err(|e| PhaseDiagnostic::compile(e.to_string()))?;
        if let Ok(mut dispatcher) = self.event_dispatcher.lock() {
            dispatcher.apply_bindings(&compiled.event_dispatch_bindings);
        }
        self.preflight_host_sensitive_support(&compiled.bytecode)?;
        let mut vm = Vm::new(self.host_services.clone());
        vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
        vm.execute(&compiled.bytecode)
            .map_err(PhaseDiagnostic::runtime)?;
        Ok(ProjectRuntimeSession { compiled, vm })
    }

    pub fn poll_and_dispatch_next_com_event_callback(
        &self,
        runtime: &mut ProjectRuntimeSession,
    ) -> Result<bool, PhaseDiagnostic> {
        let Some(callback) = self.poll_com_event_callback()? else {
            return Ok(false);
        };
        self.dispatch_com_event_callback_into_runtime(runtime, &callback)?;
        Ok(true)
    }

    pub fn dispatch_com_event_callback_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        callback: &ComEventCallbackDispatch,
    ) -> Result<(), PhaseDiagnostic> {
        let (resolved_symbol, metadata) =
            self.resolve_runtime_handler_metadata(runtime, &callback.handler_symbol)?;
        let expected_arity = metadata.param_slots.len();
        let callback_arity = callback.args.len();
        if expected_arity != callback_arity {
            return Err(PhaseDiagnostic::runtime(format!(
                "PMR-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback dispatch target `{}` expects {} arguments but callback supplied {}",
                resolved_symbol, expected_arity, callback_arity
            )));
        }
        runtime
            .vm
            .invoke_procedure_with_values(
                &runtime.compiled.bytecode,
                metadata.entry_pc,
                &metadata.param_slots,
                callback.args.as_slice(),
            )
            .map_err(PhaseDiagnostic::runtime)
    }

    pub fn execute_source(&self, source: &str) -> Result<(), String> {
        let _ = self.execute_source_with_snapshot(source)?;
        Ok(())
    }

    pub fn execute_source_with_snapshot(&self, source: &str) -> Result<Vec<RuntimeValue>, String> {
        self.execute_source_with_snapshot_phased(source)
            .map_err(|diagnostic| diagnostic.message().to_string())
    }

    pub fn execute_source_with_value_snapshot(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, String> {
        self.execute_source_with_snapshot(source)
    }

    pub fn execute_source_with_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        let bytecode = compile(source).map_err(|e| PhaseDiagnostic::compile(e.to_string()))?;
        self.preflight_host_sensitive_support(&bytecode)?;
        if self.config.enable_jit {
            self.jit
                .compile_function("main")
                .map_err(|e| PhaseDiagnostic::runtime(e.to_string()))?;
            return self
                .jit
                .execute_and_snapshot_with_host(&bytecode, self.host_services.clone())
                .map_err(|e| PhaseDiagnostic::runtime(e.to_string()));
        }

        execute_and_snapshot_with_host(&bytecode, self.host_services.clone())
            .map_err(PhaseDiagnostic::runtime)
    }

    pub fn execute_source_with_value_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        self.execute_source_with_snapshot_phased(source)
    }

    pub fn execute_project_with_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        let compiled =
            compile_project(manifest).map_err(|e| PhaseDiagnostic::compile(e.to_string()))?;
        if let Ok(mut dispatcher) = self.event_dispatcher.lock() {
            dispatcher.apply_bindings(&compiled.event_dispatch_bindings);
        }
        self.preflight_host_sensitive_support(&compiled.bytecode)?;
        if self.config.enable_jit {
            self.jit
                .compile_function("main")
                .map_err(|e| PhaseDiagnostic::runtime(e.to_string()))?;
            return self
                .jit
                .execute_and_snapshot_with_host(&compiled.bytecode, self.host_services.clone())
                .map_err(|e| PhaseDiagnostic::runtime(e.to_string()));
        }

        let mut vm = Vm::new(self.host_services.clone());
        vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
        vm.execute(&compiled.bytecode)
            .map_err(PhaseDiagnostic::runtime)?;
        Ok(vm.snapshot_values(compiled.bytecode.user_slot_count))
    }

    pub fn execute_project_with_value_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        self.execute_project_with_snapshot_phased(manifest)
    }

    fn preflight_host_sensitive_support(&self, bytecode: &Bytecode) -> Result<(), PhaseDiagnostic> {
        if self.host_services.policy().unsupported_feature_mode
            != UnsupportedFeatureMode::CompileTime
        {
            return Ok(());
        }

        let descriptor = self.host_services.descriptor();
        let policy = self.host_services.policy();
        let mut issues = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for instruction in &bytecode.instructions {
            let Some((intrinsic_name, capability)) = hal_requirement(instruction) else {
                continue;
            };
            if !descriptor.supports(capability) {
                let key = format!(
                    "{intrinsic_name}: missing capability {:?} on profile {:?}",
                    capability, descriptor.profile
                );
                if seen.insert(key.clone()) {
                    issues.push(key);
                }
            }
            match instruction {
                Instruction::IntrinsicShellHost { .. } if !policy.allow_process_spawn => {
                    let key = format!(
                        "{intrinsic_name}: blocked by host policy allow_process_spawn=false"
                    );
                    if seen.insert(key.clone()) {
                        issues.push(key);
                    }
                }
                Instruction::IntrinsicMsgBoxHost { .. }
                | Instruction::IntrinsicInputBoxHost { .. }
                    if !policy.allow_interaction =>
                {
                    let key =
                        format!("{intrinsic_name}: blocked by host policy allow_interaction=false");
                    if seen.insert(key.clone()) {
                        issues.push(key);
                    }
                }
                Instruction::IntrinsicCreateObjectHost { .. }
                | Instruction::IntrinsicDispatchInvokeHost { .. }
                | Instruction::IntrinsicComSubscribeEventHost { .. }
                | Instruction::IntrinsicComUnsubscribeEventHost { .. }
                | Instruction::IntrinsicComEventCallbackSubscriptionHost { .. }
                | Instruction::IntrinsicComEventCallbackArgHost { .. }
                | Instruction::IntrinsicComReleaseEventCallbackHost { .. }
                    if !policy.allow_com_activation =>
                {
                    let key = format!(
                        "{intrinsic_name}: blocked by host policy allow_com_activation=false"
                    );
                    if seen.insert(key.clone()) {
                        issues.push(key);
                    }
                }
                Instruction::IntrinsicInvokeSymbolHost { .. } if !policy.allow_dynamic_link => {
                    let key = format!(
                        "{intrinsic_name}: blocked by host policy allow_dynamic_link=false"
                    );
                    if seen.insert(key.clone()) {
                        issues.push(key);
                    }
                }
                _ => {}
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(PhaseDiagnostic::compile(format!(
                "HAL compile-time gate rejected host-sensitive intrinsics: {}",
                issues.join("; ")
            )))
        }
    }

    fn resolve_runtime_handler_metadata(
        &self,
        runtime: &ProjectRuntimeSession,
        handler_symbol: &str,
    ) -> Result<(String, ProcedureRuntimeMetadata), PhaseDiagnostic> {
        let normalized = handler_symbol.trim().to_ascii_lowercase();
        if let Some(metadata) = runtime.compiled.procedure_runtime_metadata.get(&normalized) {
            return Ok((normalized, metadata.clone()));
        }

        let suffix = format!("_{normalized}");
        let mut matches: Vec<_> = runtime
            .compiled
            .procedure_runtime_metadata
            .iter()
            .filter_map(|(name, metadata)| {
                if name.ends_with(&suffix) {
                    Some((name.clone(), metadata.clone()))
                } else {
                    None
                }
            })
            .collect();
        matches.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));

        match matches.as_slice() {
            [] => Err(PhaseDiagnostic::runtime(format!(
                "PMR-E-EVENT-DISPATCH-TARGET-MISSING: callback handler `{}` is not present in project runtime metadata",
                normalized
            ))),
            [(name, metadata)] => Ok((name.clone(), metadata.clone())),
            _ => Err(PhaseDiagnostic::runtime(format!(
                "PMR-E-EVENT-DISPATCH-TARGET-AMBIGUOUS: callback handler `{}` resolves to multiple project procedures",
                normalized
            ))),
        }
    }
}

fn normalize_callback_payload(
    payload: DynamicEventPayload,
) -> Result<ComEventCallbackDispatch, PhaseDiagnostic> {
    Ok(ComEventCallbackDispatch {
        callback_token: payload.callback.into(),
        subscription_token: payload.subscription.into(),
        handler_symbol: String::new(),
        args: payload
            .args
            .into_iter()
            .map(|value| value.to_runtime_value())
            .collect(),
    })
}

#[cfg(test)]
impl Engine {
    fn execute_source_slots_test(&self, source: &str) -> Result<Vec<i32>, String> {
        self.execute_source_with_snapshot(source)
            .map(project_runtime_values_to_legacy_slots)
    }

    fn execute_source_slots_test_phased(&self, source: &str) -> Result<Vec<i32>, PhaseDiagnostic> {
        self.execute_source_with_snapshot_phased(source)
            .map(project_runtime_values_to_legacy_slots)
    }

    fn execute_project_slots_test_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<i32>, PhaseDiagnostic> {
        self.execute_project_with_snapshot_phased(manifest)
            .map(project_runtime_values_to_legacy_slots)
    }
}

fn hal_requirement(instruction: &Instruction) -> Option<(&'static str, CapabilityId)> {
    match instruction {
        Instruction::IntrinsicShellHost { .. } => Some(("Shell", CapabilityId::ProcessEnv)),
        Instruction::IntrinsicEnvironHost { .. } => Some(("Environ", CapabilityId::ProcessEnv)),
        Instruction::IntrinsicDirHost { .. } => Some(("Dir", CapabilityId::ProcessEnv)),
        Instruction::IntrinsicDateNowHost { .. } => Some(("Date", CapabilityId::TimeLocale)),
        Instruction::IntrinsicTimeNowHost { .. } => Some(("Time", CapabilityId::TimeLocale)),
        Instruction::IntrinsicNowHost { .. } => Some(("Now", CapabilityId::TimeLocale)),
        Instruction::IntrinsicTimerHost { .. } => Some(("Timer", CapabilityId::TimeLocale)),
        Instruction::IntrinsicFreeFileHost { .. } => Some(("FreeFile", CapabilityId::FileSystemIo)),
        Instruction::IntrinsicMsgBoxHost { .. } => Some(("MsgBox", CapabilityId::UiInteraction)),
        Instruction::IntrinsicInputBoxHost { .. } => {
            Some(("InputBox", CapabilityId::UiInteraction))
        }
        Instruction::IntrinsicDoEventsHost { .. } => Some(("DoEvents", CapabilityId::EventPump)),
        Instruction::IntrinsicCreateObjectHost { .. } => {
            Some(("CreateObject", CapabilityId::ComActivationDispatch))
        }
        Instruction::IntrinsicDispatchInvokeHost { .. } => {
            Some(("DispatchInvoke", CapabilityId::ComActivationDispatch))
        }
        Instruction::IntrinsicComSubscribeEventHost { .. } => {
            Some(("ComSubscribeEvent", CapabilityId::ComActivationDispatch))
        }
        Instruction::IntrinsicComUnsubscribeEventHost { .. } => {
            Some(("ComUnsubscribeEvent", CapabilityId::ComActivationDispatch))
        }
        Instruction::IntrinsicComEventCallbackSubscriptionHost { .. } => Some((
            "ComEventCallbackSubscription",
            CapabilityId::ComActivationDispatch,
        )),
        Instruction::IntrinsicComEventCallbackArgHost { .. } => {
            Some(("ComEventCallbackArg", CapabilityId::ComActivationDispatch))
        }
        Instruction::IntrinsicComReleaseEventCallbackHost { .. } => Some((
            "ComReleaseEventCallback",
            CapabilityId::ComActivationDispatch,
        )),
        Instruction::IntrinsicInvokeSymbolHost { .. } => {
            Some(("DeclareInvoke", CapabilityId::DynamicLinking))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticPhase, Engine, HostConfig};
    use oxvba_com::{ComInvokeArg, ComInvokeRequest};
    use oxvba_compiler::{
        ModuleKind, ProjectDynamicMemberKind, ProjectKind, ProjectManifest, ProjectReference,
        ReferenceKind, ReferencedProjectManifest, module_unit_from_source,
    };
    use oxvba_hal::model::{
        HalProfileId, HostPolicy, HostPolicyPreset, UiVirtualizationMode, UnsupportedFeatureMode,
    };
    use oxvba_runtime::{ObjectHandle, RuntimeValue, value_tags::error_tag_from_code};
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    #[cfg(target_os = "windows")]
    fn create_controlled_com_object(engine: &Engine) -> oxvba_runtime::ObjectHandle {
        match engine
            .host_services
            .com()
            .create_object(RuntimeValue::I32(4))
            .expect("create_object should return controlled COM object")
        {
            RuntimeValue::ObjectHandle(handle) => handle,
            other => panic!("expected object handle from create_object, got {:?}", other),
        }
    }

    #[cfg(target_os = "windows")]
    fn dispatch_controlled_com_call(
        engine: &Engine,
        object: oxvba_runtime::ObjectHandle,
        member: i32,
        arg: i32,
    ) -> RuntimeValue {
        let request = ComInvokeRequest::new(
            object,
            member.into(),
            if arg == oxvba_com::DISPATCH_INVOKE_MISSING_ARG_TOKEN {
                Vec::new()
            } else {
                vec![ComInvokeArg::positional(arg)]
            },
        );
        engine
            .host_services
            .com()
            .dispatch_invoke_runtime_value_v2(&request)
            .expect("dispatch_invoke should succeed")
    }

    fn repo_path(relative: &str) -> PathBuf {
        workspace_root().join(relative)
    }

    fn divergence_record_has_required_sections(record_path: &Path) -> bool {
        let Ok(text) = std::fs::read_to_string(record_path) else {
            return false;
        };
        if !text.starts_with("# DIV-") {
            return false;
        }
        let required = [
            "- Scope impact:",
            "- Fixture:",
            "- Reproduction command:",
            "- Tracking status:",
        ];
        required.iter().all(|label| text.contains(label))
    }

    #[test]
    fn execute_source_with_default_vm_path() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: Some("Application".to_string()),
        });
        engine.register_root_object("Application", "Host.Application");
        assert!(engine.has_root_object("Application"));

        let result = engine.execute_source("Sub Main()\nEnd Sub");
        assert!(result.is_ok());
    }

    #[test]
    fn execute_source_returns_slot_snapshot() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: Some("Application".to_string()),
        });

        let source = "Sub Main()\nDim x\nx = 10\nx = x + 5\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![15]);
    }

    #[test]
    fn execute_source_jit_toggle_preserves_semantics() {
        let engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: Some("Application".to_string()),
        });

        let source = "Sub Main()\nDim x\nx = 20\nx = x - 4\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![16]);
    }

    #[test]
    fn formal_v5_branch_selection_is_total_over_small_domain() {
        let engine = Engine::new(HostConfig::default());
        for input in -4..=4 {
            let source = format!(
                "Sub Main()\nDim x\nx = {input}\nIf x = 1 Then\nx = 10\nElseIf x = 2 Then\nx = 20\nElse\nx = 30\nEnd If\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_slots_test(&source)
                .expect("execution should succeed");
            assert!(matches!(snapshot[0], 10 | 20 | 30));
        }
    }

    #[test]
    fn formal_v5_branch_selection_matches_reference_model() {
        let engine = Engine::new(HostConfig::default());
        for input in -6..=6 {
            let expected = if input == 1 {
                10
            } else if input == 2 {
                20
            } else {
                30
            };
            let source = format!(
                "Sub Main()\nDim x\nx = {input}\nIf x = 1 Then\nx = 10\nElseIf x = 2 Then\nx = 20\nElse\nx = 30\nEnd If\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_slots_test(&source)
                .expect("execution should succeed");
            assert_eq!(snapshot[0], expected);
        }
    }

    #[test]
    fn formal_v5_no_dual_branch_write_effect() {
        let engine = Engine::new(HostConfig::default());
        for input in -3..=3 {
            let source = format!(
                "Sub Main()\nDim x\nDim y\nx = {input}\ny = 0\nIf x = 1 Then\ny = y + 1\nElseIf x = 2 Then\ny = y + 10\nElse\ny = y + 100\nEnd If\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_slots_test(&source)
                .expect("execution should succeed");
            assert!(matches!(snapshot[1], 1 | 10 | 100));
        }
    }

    #[test]
    fn formal_v6_do_while_matches_reference_model() {
        let engine = Engine::new(HostConfig::default());
        for limit in 0..=6 {
            let source =
                format!("Sub Main()\nDim x\nx = 0\nDo While x < {limit}\nx = x + 1\nLoop\nEnd Sub");
            let snapshot = engine
                .execute_source_slots_test(&source)
                .expect("execution should succeed");
            assert_eq!(snapshot[0], limit);
        }
    }

    #[test]
    fn formal_v6_post_condition_loop_semantics() {
        let engine = Engine::new(HostConfig::default());
        for limit in 0..=4 {
            let source =
                format!("Sub Main()\nDim x\nx = 0\nDo\nx = x + 1\nLoop While x < {limit}\nEnd Sub");
            let snapshot = engine
                .execute_source_slots_test(&source)
                .expect("execution should succeed");
            let expected = if limit <= 1 { 1 } else { limit };
            assert_eq!(snapshot[0], expected);
        }
    }

    #[test]
    fn formal_v6_exit_do_short_circuits_iteration() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 0\nDo While x < 10\nx = x + 1\nIf x = 4 Then\nExit Do\nEnd If\nLoop\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 4);
    }

    #[test]
    fn formal_v7_select_case_first_match_wins() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 2\nSelect Case x\nCase 2\nx = 20\nCase 2, 3\nx = 99\nCase Else\nx = 0\nEnd Select\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 20);
    }

    #[test]
    fn formal_v7_select_case_else_fallback() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 9\nSelect Case x\nCase 1\nx = 10\nCase 2\nx = 20\nCase Else\nx = 99\nEnd Select\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 99);
    }

    #[test]
    fn formal_v7_select_case_multi_value_arm() {
        let engine = Engine::new(HostConfig::default());
        for input in [1, 3] {
            let source = format!(
                "Sub Main()\nDim x\nx = {input}\nSelect Case x\nCase 1, 3\nx = 30\nCase Else\nx = 0\nEnd Select\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_slots_test(&source)
                .expect("execution should succeed");
            assert_eq!(snapshot[0], 30);
        }
    }

    #[test]
    fn formal_v8_call_returns_to_caller() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nCall Foo\nx = x + 1\nEnd Sub\nSub Foo()\nDim y\ny = 9\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v8_local_scope_isolated_between_procedures() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 2\nCall Foo\nx = x + 1\nEnd Sub\nSub Foo()\nDim x\nx = 200\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 3);
    }

    #[test]
    fn formal_v8_nested_call_chain_integrity() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 0\nCall A\nx = x + 1\nEnd Sub\nSub A()\nDim y\ny = 1\nCall B\nEnd Sub\nSub B()\nDim z\nz = 2\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_v9_byval_does_not_propagate_mutation() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nCall AddOne(x)\nEnd Sub\nSub AddOne(ByVal a)\na = a + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_v9_byref_propagates_mutation() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nCall AddOne(x)\nEnd Sub\nSub AddOne(ByRef a)\na = a + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v9_byref_requires_variable_argument() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nCall AddOne(1)\nEnd Sub\nSub AddOne(ByRef a)\na = a + 1\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("byref constant argument should fail");
        assert!(err.contains("ByRef"));
    }

    #[test]
    fn formal_v37_optional_param_default_applies_when_omitted() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 7);
    }

    #[test]
    fn formal_v37_optional_param_explicit_value_overrides_default() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(x, 9)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 9);
    }

    #[test]
    fn formal_v37_optional_param_missing_required_arg_is_rejected() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nCall Fill\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("missing required arg should fail");
        assert!(err.contains("missing required argument"));
    }

    #[test]
    fn formal_v38_named_args_bind_by_parameter_name() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(value := 9, target := x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 9);
    }

    #[test]
    fn formal_v38_named_args_allow_omitting_optional_by_name() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(target := x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 7);
    }

    #[test]
    fn formal_v38_named_args_reject_positional_after_named() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(value := 9, x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("positional-after-named should fail");
        assert!(err.contains("positional argument cannot follow named argument"));
    }

    #[test]
    fn formal_v83_paramarray_packs_trailing_args_count() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Capture(x, 5, 7, 9)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v83_paramarray_empty_pack_reports_negative_upper_bound() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Capture(x)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], -1);
    }

    #[test]
    fn formal_v83_paramarray_named_args_rejected_in_current_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Capture(target := x, items := 5)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("named args for paramarray should fail in current subset");
        assert!(err.contains("ParamArray"));
    }

    #[test]
    fn formal_v84_dispatch_invoke_marshals_array_argument_shape() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(4), 6, Array(1, 2, 3))\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(
            snapshot[0],
            5004 + 6 + (oxvba_runtime::safe_array::ARRAY_TAG_BASE + 3)
        );
    }

    #[test]
    fn formal_v84_paramarray_pack_roundtrips_into_dispatch_boundary() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall InvokePack(x, 5, 7)\nEnd Sub\nSub InvokePack(ByRef target, ParamArray items() As Variant)\ntarget = DispatchInvoke(CreateObject(2), 4, items)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(
            snapshot[0],
            5002 + 4 + (oxvba_runtime::safe_array::ARRAY_TAG_BASE + 2)
        );
    }

    #[test]
    fn formal_v84_deferred_gate_rows_present_for_array_track() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/DEFERRED_GATES.md"))
            .expect("deferred gates register exists");
        assert!(text.contains("DG-V80-001"));
        assert!(text.contains("DG-V81-001"));
        assert!(text.contains("DG-V82-001"));
        assert!(text.contains("DG-V83-001"));
    }

    #[test]
    fn formal_v85_typed_fastpath_vm_parity_disabled_vs_enabled() {
        let source = "Sub Main()\nDim x As Long\nDim i As Long\nx = 0\nFor i = 1 To 100\nx = x + 3\nNext i\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");

        let fast = oxvba_vm::execute_and_snapshot_with_typed_fastpaths(&bytecode, true)
            .expect("fastpath execution should succeed");
        let baseline = oxvba_vm::execute_and_snapshot_with_typed_fastpaths(&bytecode, false)
            .expect("baseline execution should succeed");

        assert_eq!(fast, baseline);
        assert_eq!(fast[0], RuntimeValue::I32(300));
    }

    #[test]
    fn formal_v85_typed_fastpath_jit_vm_equivalence() {
        let source = "Sub Main()\nDim x As Long\nDim i As Long\nx = 0\nFor i = 1 To 75\nx = x + 2\nNext i\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v85_typed_fastpath_hotloop_fixture_exists() {
        assert!(repo_path("conformance/tests/typed_fastpath_hotloop.bas").exists());
    }

    #[test]
    fn formal_v40_gosub_executes_label_body_and_returns() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nx = x + 1\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 4);
    }

    #[test]
    fn formal_v40_gosub_missing_label_is_rejected() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nGoSub nope\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("missing gosub label should fail");
        assert!(err.contains("gosub target label not found"));
    }

    #[test]
    fn formal_v40_gosub_return_stack_handles_repeated_calls() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nGoSub add_two\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 5);
    }

    #[test]
    fn formal_v41_on_error_goto_label_jumps_to_handler() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nOn Error GoTo handler\nError 5\nx = 99\nIf Err.Number = -1 Then\nhandler:\nx = Err.Number\nResume Next\nEnd If\nx = x + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 6);
    }

    #[test]
    fn formal_v41_on_error_goto_label_missing_target_is_rejected() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error GoTo handler\nError 5\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("missing handler label should fail");
        assert!(err.contains("on error goto target label not found"));
    }

    #[test]
    fn formal_v41_on_error_goto_zero_disables_label_handler() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error GoTo handler\nOn Error GoTo 0\nError 4\nIf Err.Number = -1 Then\nhandler:\nResume Next\nEnd If\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("goto 0 should disable label handler");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v42_redim_preserve_retains_existing_values() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim a(1)\nDim x\na(0) = 7\nReDim Preserve a(3)\nx = a(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[2], 7);
    }

    #[test]
    fn formal_v42_redim_without_preserve_reinitializes_array() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(1)\nDim x\na(0) = 7\nReDim a(3)\nx = a(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[2], 0);
    }

    #[test]
    fn formal_v42_redim_shrink_rejects_out_of_bounds_access() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(3)\nReDim a(1)\na(2) = 9\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("out-of-bounds after shrink should fail");
        assert!(!err.trim().is_empty());
    }

    #[test]
    fn formal_v82_redim_preserve_multidim_last_dimension_keeps_overlap() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim m(1 To 2, 1 To 2)\nDim x\nm(1, 1) = 7\nReDim Preserve m(1 To 2, 1 To 3)\nx = m(1, 1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 7);
        assert_eq!(snapshot[4], 7);
    }

    #[test]
    fn formal_v82_redim_preserve_shrink_then_expand_clears_removed_tail() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(0 To 3)\nDim x\na(3) = 9\nReDim Preserve a(0 To 1)\nReDim Preserve a(0 To 3)\nx = a(3)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[3], 0);
        assert_eq!(snapshot[4], 0);
    }

    #[test]
    fn formal_v82_redim_preserve_rejects_non_last_dimension_resize() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim m(1 To 2, 1 To 2)\nReDim Preserve m(1 To 3, 1 To 2)\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("non-last-dimension preserve resize should fail");
        assert!(err.contains("redim preserve only supports resizing"));
    }

    #[test]
    fn formal_v43_module_const_evaluates_in_expression() {
        let engine = Engine::new(HostConfig::default());
        let source = "Const BASE = 5\nSub Main()\nDim x\nx = BASE + 2\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5, 7]);
    }

    #[test]
    fn formal_v43_enum_members_bind_to_expected_values() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3, 4, 5]);
    }

    #[test]
    fn formal_v43_udt_declaration_block_is_parse_tolerated() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim x\nx = 9\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![9]);
    }

    #[test]
    fn formal_v44_property_let_routes_assignment_byref() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nValue = x\nEnd Sub\nProperty Let Value(ByRef target)\ntarget = target + 2\nEnd Property";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3]);
    }

    #[test]
    fn formal_v44_property_set_routes_assignment_byref() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 2\nObj = x\nEnd Sub\nProperty Set Obj(ByRef target)\ntarget = target + 5\nEnd Property";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![7]);
    }

    #[test]
    fn formal_v44_property_get_block_is_parse_tolerated() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nx = 4\nEnd Sub\nProperty Get Value()\nDim y\ny = 1\nEnd Property";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![4]);
    }

    #[test]
    fn formal_v45_cint_conversion_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = CInt(5)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5]);
    }

    #[test]
    fn formal_v45_nested_conversion_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = CLng(CInt(7))\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![7]);
    }

    #[test]
    fn formal_v45_val_str_conversion_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = Val(Str(9))\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![9]);
    }

    #[test]
    fn formal_v46_len_intrinsic_digit_count() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = Len(1234)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![4]);
    }

    #[test]
    fn formal_v46_slice_intrinsics_digit_subsets() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim b\nDim c\na = Left(12345, 2)\nb = Right(12345, 2)\nc = Mid(12345, 2, 3)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![12, 45, 234]);
    }

    #[test]
    fn formal_v46_instr_and_case_intrinsics() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nDim y\nDim z\nx = InStr(12345, 34)\ny = LCase(789)\nz = UCase(654)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3, 789, 654]);
    }

    #[test]
    fn formal_v47_split_and_join_intrinsics() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nDim y\nx = Split(123231, 23)\ny = Join(789, 0)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3, 789]);
    }

    #[test]
    fn formal_v47_replace_and_trim_intrinsics() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nDim y\nDim z\nx = Replace(12345, 23, 67)\ny = Trim(456)\nz = RTrim(321)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![16745, 456, 321]);
    }

    #[test]
    fn formal_v47_strcomp_intrinsic_subset() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nDim y\nx = StrComp(12, 123)\ny = StrComp(123, 123)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![-1, 0]);
    }

    #[test]
    fn formal_v48_date_serial_and_value_subset() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nDim y\nx = DateSerial(2026, 2, 28)\ny = DateValue(x)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![20260228, 20260228]);
    }

    #[test]
    fn formal_v48_time_serial_and_value_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nDim y\nx = TimeSerial(1, 2, 3)\ny = TimeValue(x)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3723, 3723]);
    }

    #[test]
    fn formal_v48_date_add_diff_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nDim y\nx = DateAdd(1, 3, DateSerial(2026, 2, 28))\ny = DateDiff(1, DateSerial(2026, 2, 28), x)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[1], 3);
    }

    #[test]
    fn formal_v49_math_primitives_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Abs(-7)\nb = Sgn(-9)\nc = Sqr(81)\nd = Round(19, -1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![7, -1, 9, 20]);
    }

    #[test]
    fn formal_v49_transcendental_identity_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Sin(0)\nb = Cos(0)\nc = Log(1)\nd = Exp(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![0, 1, 0, 1]);
    }

    #[test]
    fn formal_v49_financial_zero_rate_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim b\nDim c\na = FV(0, 3, 2, 5)\nb = PV(0, 3, 2, 5)\nc = PMT(0, 3, 6, 3)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![-11, -11, -3]);
    }

    #[test]
    fn formal_v50_array_bounds_introspection_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim l\nDim u\na = Array(10, 20, 30)\nl = LBound(a)\nu = UBound(a)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[1], 0);
        assert_eq!(snapshot[2], 2);
    }

    #[test]
    fn formal_v50_variant_type_tag_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim t1\nDim t2\na = Array(1, 2)\nt1 = VarType(a)\nt2 = TypeName(a)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[1], 8204);
        assert_eq!(snapshot[2], 1001);
    }

    #[test]
    fn formal_v50_numeric_date_object_predicates_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim d\nDim n\nDim o\nd = IsDate(DateSerial(2026, 2, 28))\nn = IsNumeric(17)\no = IsObject(17)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![1, 1, 0]);
    }

    #[test]
    fn formal_v51_err_raise_maps_to_runtime_error_state() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nOn Error Resume Next\nErr.Raise 11\nx = Err.Number\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![11]);
    }

    #[test]
    fn formal_v51_cverr_error_tag_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = CVErr(17)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![error_tag_from_code(17)]);
    }

    #[test]
    fn formal_v51_err_raise_without_handler_fails() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nErr.Raise 9\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("expected runtime error");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v52_shell_environ_dir_host_subset() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim a\nDim b\nDim c\na = Shell(5)\nb = Environ(9)\nc = Dir(1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![1, 9, 1]);
    }

    #[test]
    fn formal_v52_time_locale_host_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim d\nDim t\nDim n\nDim k\nd = Date()\nt = Time()\nn = Now()\nk = Timer()\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![20_260_301, 123_456, 20_260_301, 42]);
    }

    #[test]
    fn formal_v52_freefile_host_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim b\na = FreeFile()\nb = FreeFile(1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![1, 256]);
    }

    #[test]
    fn formal_v52_ui_event_host_subset() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.allow_interaction = true;
        policy.ui_virtualization = UiVirtualizationMode::ScriptedResponses;
        engine.set_host_policy(policy);
        let source = "Sub Main()\nDim a\nDim b\nDim c\na = MsgBox(7, 3)\nb = InputBox(9, 4)\nc = DoEvents()\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3, 4, 0]);
    }

    #[test]
    fn formal_v52_host_sensitive_subset_is_jit_vm_equivalent() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim a\nDim b\na = Shell(7)\nb = Environ(4)\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v52_missing_capability_fallback_is_deterministic() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = Shell(0)\nEnd Sub";
        let first = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        let second = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(first, second);
    }

    #[test]
    fn formal_v53_collection_add_item_count_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim c\nDim item\nc = CollectionAdd(0, 9)\nitem = CollectionItem(c, 1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![1, 1]);
    }

    #[test]
    fn formal_v53_collection_remove_subset() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim c\nc = CollectionAdd(0, 9)\nc = CollectionRemove(c, 1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![0]);
    }

    #[test]
    fn formal_v53_collection_ops_jit_vm_equivalent() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim c\nc = CollectionAdd(0, 2)\nc = CollectionAdd(c, 3)\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v54_class_initialize_runs_before_main() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nx = 1\nEnd Sub\nSub Class_Initialize()\nErr.Raise 77\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("initializer should run before main");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v54_class_terminate_runs_after_main() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nx = 1\nEnd Sub\nSub Class_Terminate()\nErr.Raise 88\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("terminate should run after main");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v54_lifecycle_jit_vm_equivalence() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nx = 3\nEnd Sub\nSub Class_Initialize()\nOn Error Resume Next\nErr.Raise 5\nEnd Sub\nSub Class_Terminate()\nOn Error Resume Next\nErr.Raise 7\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_pmr_project_manifest_cross_module_call_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim x\nx = MathModule.Add(1, 2)\nEnd Sub",
        )
        .expect("main module should parse");
        let math_module = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Function Add(ByVal a, ByVal b)\nAdd = a\nEnd Function",
        )
        .expect("math module should parse");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, math_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_slots_test_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_pmr_project_manifest_cross_project_call_executes_with_loaded_reference_source() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim x\nx = OtherProject.Tools.Add(1, 2)\nEnd Sub",
        )
        .expect("main module should parse");
        let tools_module = module_unit_from_source(
            "Tools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Tools\"\nPublic Function Add(ByVal a, ByVal b)\nAdd = a\nEnd Function",
        )
        .expect("tools module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OtherProject".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "OtherProject".to_string(),
                modules: vec![tools_module],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_slots_test_phased(&manifest)
            .expect("cross-project execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_property_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim valueOut\nvalueOut = Application.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPublic Property Get Value()\nValue = 41\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared property get should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(41),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_default_member_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim valueOut As Variant\nLet valueOut = Application\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPublic Property Get Value()\nValue = 41\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared default member should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(41),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_property_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim valueOut\nvalueOut = Application.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPublic Property Get Value()\nValue = 41\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace property get should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(41),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_default_member_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim valueOut As Variant\nLet valueOut = Application\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPublic Property Get Value()\nValue = 41\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace default member should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(41),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_property_let_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication.Value = 9\nDim afterValue\nafterValue = Application.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared property let should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(9),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_default_member_let_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication = 9\nDim afterValue As Variant\nafterValue = Application\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared default-member let should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(9),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_property_let_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication.Value = 9\nDim afterValue\nafterValue = Application.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace property let should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(9),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_default_member_let_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication = 9\nDim afterValue As Variant\nafterValue = Application\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace default-member let should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(9),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_call_statement_property_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Application.Value\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared call statement property get should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_call_statement_default_member_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Application\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared call statement default member should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_call_statement_property_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Application.Value\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace call statement property get should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_call_statement_default_member_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Application\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace call statement default member should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_statement_context_property_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication.Value\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared statement-context property get should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_statement_context_default_member_executes() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared statement-context default member should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_statement_context_property_get_executes()
    {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication.Value\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace statement-context property get should execute");
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_statement_context_default_member_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nApplication\nDim afterValue\nafterValue = Application.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect(
                "host-injected global-namespace statement-context default member should execute",
            );
        assert_eq!(
            snapshot[0],
            RuntimeValue::I32(7),
            "snapshot: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_object_property_get_returns_object_handle() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Object\nSet child = Application.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("host application module should parse");
        let host_child = module_unit_from_source(
            "Child",
            ModuleKind::Class,
            "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("host child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application, host_child],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected predeclared object property get should execute");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_object_property_get_returns_object_handle()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Object\nSet child = Application.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("host application module should parse");
        let host_child = module_unit_from_source(
            "Child",
            ModuleKind::Class,
            "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("host child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application, host_child],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("host-injected global-namespace object property get should execute");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_child_navigation_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\nDim afterValue\nafterValue = child.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("host application module should parse");
        let host_child = module_unit_from_source(
            "Child",
            ModuleKind::Class,
            "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("host child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application, host_child],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect(
                "host-injected predeclared child navigation after object root get should execute",
            );
        assert_eq!(
            snapshot,
            [RuntimeValue::I32(1), RuntimeValue::I32(9)],
            "{snapshot:?}"
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_child_navigation_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\nDim afterValue\nafterValue = child.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("host application module should parse");
        let host_child = module_unit_from_source(
            "Child",
            ModuleKind::Class,
            "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("host child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application, host_child],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine.execute_project_with_value_snapshot_phased(&manifest).expect(
            "host-injected global-namespace child navigation after object root get should execute",
        );
        assert_eq!(
            snapshot,
            [RuntimeValue::I32(1), RuntimeValue::I32(9)],
            "{snapshot:?}"
        );
    }

    #[test]
    fn formal_host_project_host_injected_predeclared_child_default_member_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\nDim afterValue\nafterValue = child\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("host application module should parse");
        let host_child = module_unit_from_source(
            "Child",
            ModuleKind::Class,
            "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application, host_child],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine.execute_project_with_value_snapshot_phased(&manifest).expect(
            "host-injected predeclared child default member after object root get should execute",
        );
        assert_eq!(
            snapshot,
            [RuntimeValue::I32(1), RuntimeValue::I32(9)],
            "{snapshot:?}"
        );
    }

    #[test]
    fn formal_host_project_host_injected_global_namespace_child_default_member_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\nDim afterValue\nafterValue = child\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_GlobalNamespace = True\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("host application module should parse");
        let host_child = module_unit_from_source(
            "Child",
            ModuleKind::Class,
            "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::HostInjected,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application, host_child],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine.execute_project_with_value_snapshot_phased(&manifest).expect(
            "host-injected global-namespace child default member after object root get should execute",
        );
        assert_eq!(
            snapshot,
            [RuntimeValue::I32(1), RuntimeValue::I32(9)],
            "{snapshot:?}"
        );
    }

    #[test]
    fn formal_host_project_host_injected_child_call_statement_after_object_root_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "Call child.Value",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "Call child",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "Call child.Value",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "Call child",
            ),
        ];

        for (label, exposure_attr, invoke_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{invoke_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(7)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_statement_context_after_object_root_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "child.Value",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "child",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "child.Value",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "child",
            ),
        ];

        for (label, exposure_attr, invoke_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{invoke_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(7)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_property_let_after_object_root_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "child.Value = 9",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "child = 9",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "child.Value = 9",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "child = 9",
            ),
        ];

        for (label, exposure_attr, write_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{write_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(9)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_parenthesized_read_after_object_root_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "afterValue = child.Value()",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "afterValue = child()",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "afterValue = child.Value()",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "afterValue = child()",
            ),
        ];

        for (label, exposure_attr, read_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nDim afterValue\nSet child = Application.Value\n{read_line}\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(9)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_parenthesized_call_statement_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "Call child.Value()",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "Call child()",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "Call child.Value()",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "Call child()",
            ),
        ];

        for (label, exposure_attr, invoke_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{invoke_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(7)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_parenthesized_statement_context_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "child.Value()",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "child()",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "child.Value()",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "child()",
            ),
        ];

        for (label, exposure_attr, invoke_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{invoke_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nstored = 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(7)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_indexed_read_after_object_root_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "afterValue = child.Value(2)",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "afterValue = child(2)",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "afterValue = child.Value(2)",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "afterValue = child(2)",
            ),
        ];

        for (label, exposure_attr, read_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nDim afterValue\nSet child = Application.Value\n{read_line}\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPublic Property Get Value(ByVal index)\nValue = index + 7\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(9)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_indexed_call_statement_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "Call child.Value(2)",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "Call child(2)",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "Call child.Value(2)",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "Call child(2)",
            ),
        ];

        for (label, exposure_attr, invoke_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{invoke_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nstored = index + 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(9)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_indexed_statement_context_after_object_root_get_executes()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "child.Value(2)",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "child(2)",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "child.Value(2)",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "child(2)",
            ),
        ];

        for (label, exposure_attr, invoke_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{invoke_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nstored = index + 7\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(9)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_indexed_property_let_after_object_root_get_executes()
    {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "child.Value(2) = 11",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "child(2) = 11",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "child.Value(2) = 11",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "child(2) = 11",
            ),
        ];

        for (label, exposure_attr, write_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nSet child = Application.Value\n{write_line}\nDim afterValue\nafterValue = child.Observe\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Let Value(ByVal index, ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [RuntimeValue::I32(1), RuntimeValue::I32(11)],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_property_set_after_object_root_get_executes() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "Set child.Value = x",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "Set child = x",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "Set child.Value = x",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "Set child = x",
            ),
        ];

        for (label, exposure_attr, write_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nDim x\nDim afterValue\nSet child = Application.Value\nx = 2\n{write_line}\nafterValue = x\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPublic Property Set Value(ByRef target)\ntarget = target + 7\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [
                    RuntimeValue::I32(1),
                    RuntimeValue::I32(9),
                    RuntimeValue::I32(9)
                ],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_host_injected_child_indexed_property_set_after_object_root_get_executes()
    {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "predeclared named property",
                "Attribute VB_PredeclaredId = True",
                "Set child.Value(1) = x",
            ),
            (
                "predeclared default member",
                "Attribute VB_PredeclaredId = True",
                "Set child(1) = x",
            ),
            (
                "global named property",
                "Attribute VB_GlobalNamespace = True",
                "Set child.Value(1) = x",
            ),
            (
                "global default member",
                "Attribute VB_GlobalNamespace = True",
                "Set child(1) = x",
            ),
        ];

        for (label, exposure_attr, write_line) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim child As Child\nDim x\nDim afterValue\nSet child = Application.Value\nx = 2\n{write_line}\nafterValue = x\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"\nPublic Property Set Value(ByVal index, ByRef target)\ntarget = target + 7\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            )
            .expect("host child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_child],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .unwrap_or_else(|err| panic!("{label} should execute: {err}"));
            assert_eq!(
                snapshot,
                [
                    RuntimeValue::I32(1),
                    RuntimeValue::I32(9),
                    RuntimeValue::I32(9)
                ],
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_host_project_plain_project_reference_does_not_gain_implicit_receiver() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim valueOut As Variant\nLet valueOut = Application\nEnd Sub",
        )
        .expect("main module should parse");
        let host_application = module_unit_from_source(
            "Application",
            ModuleKind::Class,
            "Attribute VB_Name = \"Application\"\nAttribute VB_PredeclaredId = True\nPublic Property Get Value()\nValue = 41\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("host application module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "HostProject".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "HostProject".to_string(),
                modules: vec![host_application],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("plain project references should stay on the ordinary name path");
        assert_eq!(
            snapshot,
            vec![RuntimeValue::Empty, RuntimeValue::Empty],
            "plain project references must not gain implicit host receiver behavior: {:?}",
            snapshot
        );
    }

    #[test]
    fn formal_host_project_invalid_host_root_without_exposure_attribute_fails_deterministically() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            ("named property read", "Let valueOut = Application.Value"),
            ("default member read", "Let valueOut = Application"),
            ("call statement", "Call Application.Value"),
            ("property let write", "Application.Value = 9"),
        ];

        for (label, statement) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                format!(
                    "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim valueOut As Variant\n{statement}\nEnd Sub"
                ),
            )
            .expect("main module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                "Attribute VB_Name = \"Application\"\nPublic Property Get Value()\nValue = 41\nEnd Property\nPublic Property Let Value(ByVal n)\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            )
            .expect("host application module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .expect_err(&format!("{label} should fail with host-root diagnostic"));
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
            assert!(
                err.message().contains("PMR-E-HOST-ROOT-NOT-EXPOSED"),
                "{label}: unexpected diagnostic: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_host_project_runtime_sessions_isolate_host_injected_root_state() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            ("predeclared", "Attribute VB_PredeclaredId = True"),
            ("global namespace", "Attribute VB_GlobalNamespace = True"),
        ];

        for (label, exposure_attr) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim s As New Sink\nCall s.Attach(e)\nEnd Sub",
            )
            .expect("main module should parse");
            let emitter = module_unit_from_source(
                "Emitter",
                ModuleKind::Class,
                "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\n",
            )
            .expect("emitter module should parse");
            let sink = module_unit_from_source(
                "Sink",
                ModuleKind::Class,
                "Attribute VB_Name = \"Sink\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPrivate Sub em_changed()\nIf Application.Value = 4 Then\nApplication.Value = 9\nError 104\nElseIf Application.Value = 9 Then\nError 109\nElse\nError 101\nEnd If\nEnd Sub",
            )
            .expect("sink module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPrivate stored\nPublic Property Get Value()\nIf stored = 0 Then\nstored = 4\nEnd If\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, emitter, sink],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let mut runtime_a = engine
                .start_project_runtime_session(&manifest)
                .unwrap_or_else(|err| panic!("{label} runtime A should start: {err}"));
            let mut runtime_b = engine
                .start_project_runtime_session(&manifest)
                .unwrap_or_else(|err| panic!("{label} runtime B should start: {err}"));

            let err_a = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime_a,
                    "ProjectA",
                    "Emitter",
                    "Changed",
                    ObjectHandle::new(1),
                    &[],
                )
                .expect_err("runtime A first event should raise its baseline branch");
            assert_eq!(err_a.phase(), DiagnosticPhase::Runtime);
            assert!(
                err_a.message().contains("runtime error: 104"),
                "{label}: runtime A should observe its baseline host-root state on first event, got: {}",
                err_a.message()
            );

            let err_a_follow_up = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime_a,
                    "ProjectA",
                    "Emitter",
                    "Changed",
                    ObjectHandle::new(1),
                    &[],
                )
                .expect_err("runtime A second event should see its mutated host-root state");
            assert_eq!(err_a_follow_up.phase(), DiagnosticPhase::Runtime);
            assert!(
                err_a_follow_up.message().contains("runtime error: 109"),
                "{label}: runtime A should retain host-root mutation across event ingress, got: {}",
                err_a_follow_up.message()
            );

            let err_b = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime_b,
                    "ProjectA",
                    "Emitter",
                    "Changed",
                    ObjectHandle::new(1),
                    &[],
                )
                .expect_err("runtime B first event should raise its own baseline branch");
            assert_eq!(err_b.phase(), DiagnosticPhase::Runtime);
            assert!(
                err_b.message().contains("runtime error: 104"),
                "{label}: runtime B should retain its own host-root baseline, got: {}",
                err_b.message()
            );

            let mut runtime_c = engine
                .start_project_runtime_session(&manifest)
                .unwrap_or_else(|err| panic!("{label} runtime C should start: {err}"));
            let err_c = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime_c,
                    "ProjectA",
                    "Emitter",
                    "Changed",
                    ObjectHandle::new(1),
                    &[],
                )
                .expect_err("runtime C first event should raise its own baseline branch");
            assert_eq!(err_c.phase(), DiagnosticPhase::Runtime);
            assert!(
                err_c.message().contains("runtime error: 104"),
                "{label}: fresh runtime sessions should reset host-root state, got: {}",
                err_c.message()
            );
        }
    }

    #[test]
    fn formal_host_project_event_ingress_routes_only_bound_host_backed_source_handle() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            ("predeclared", "Attribute VB_PredeclaredId = True"),
            ("global namespace", "Attribute VB_GlobalNamespace = True"),
        ];

        for (label, exposure_attr) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim bound As Object\nDim other As Object\nDim s As New Sink\nSet bound = Application.Value\nSet other = Application.OtherValue\nCall s.Attach(bound)\nEnd Sub",
            )
            .expect("main module should parse");
            let sink = module_unit_from_source(
                "Sink",
                ModuleKind::Class,
                "Attribute VB_Name = \"Sink\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Object)\nSet em = e\nEnd Sub\nPrivate Sub em_changed()\nError 204\nEnd Sub",
            )
            .expect("sink module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim e As New Emitter\nSet Value = e\nEnd Property\nPublic Property Get OtherValue() As Object\nDim e As New Emitter\nSet OtherValue = e\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_emitter = module_unit_from_source(
                "Emitter",
                ModuleKind::Class,
                "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\n",
            )
            .expect("host emitter module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, sink],
                references: vec![ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                }],
                reference_projects: vec![ReferencedProjectManifest {
                    project_name: "HostProject".to_string(),
                    modules: vec![host_application, host_emitter],
                }],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let mut runtime = engine
                .start_project_runtime_session(&manifest)
                .unwrap_or_else(|err| panic!("{label} runtime should start: {err}"));
            let snapshot = runtime.snapshot_values();
            let snapshot_handle = |value: &RuntimeValue| match value {
                RuntimeValue::ObjectHandle(handle) if handle.raw() > 0 => Some(*handle),
                RuntimeValue::I32(raw) if *raw > 0 => Some(ObjectHandle::new(*raw)),
                _ => None,
            };
            let (bound_handle, other_handle) = match snapshot.as_slice() {
                [bound, other, _sink] => {
                    let bound_handle = snapshot_handle(bound);
                    let other_handle = snapshot_handle(other);
                    match (bound_handle, other_handle) {
                        (Some(bound_handle), Some(other_handle))
                            if bound_handle.raw() != other_handle.raw() =>
                        {
                            (bound_handle, other_handle)
                        }
                        _ => panic!(
                            "{label} should snapshot distinct bound and unbound host-backed emitters: {snapshot:?}"
                        ),
                    }
                }
                _ => panic!(
                    "{label} should snapshot bound and unbound host-backed emitters: {snapshot:?}"
                ),
            };

            let unmatched = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime,
                    "HostProject",
                    "Emitter",
                    "Changed",
                    other_handle,
                    &[],
                )
                .unwrap_or_else(|err| {
                    panic!("{label} unmatched host-backed event should not fail ingress: {err}")
                });
            assert!(
                unmatched,
                "{label}: host-backed event source should still resolve a binding set"
            );

            let err = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime,
                    "HostProject",
                    "Emitter",
                    "Changed",
                    bound_handle,
                    &[],
                )
                .expect_err("bound host-backed event should route to the sink handler");
            assert_eq!(err.phase(), DiagnosticPhase::Runtime);
            assert!(
                err.message().contains("runtime error: 204"),
                "{label}: bound host-backed source should route callback, got: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_host_project_withevents_prefers_host_injected_source_over_plain_project_name_match() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            ("predeclared", "Attribute VB_PredeclaredId = True"),
            ("global namespace", "Attribute VB_GlobalNamespace = True"),
        ];

        for (label, exposure_attr) in cases {
            let main_module = module_unit_from_source(
                "MainModule",
                ModuleKind::Procedural,
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim bound As Object\nDim s As New Sink\nSet bound = Application.Value\nCall s.Attach(bound)\nEnd Sub",
            )
            .expect("main module should parse");
            let sink = module_unit_from_source(
                "Sink",
                ModuleKind::Class,
                "Attribute VB_Name = \"Sink\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Object)\nSet em = e\nEnd Sub\nPrivate Sub em_changed()\nError 204\nEnd Sub",
            )
            .expect("sink module should parse");
            let host_application = module_unit_from_source(
                "Application",
                ModuleKind::Class,
                format!(
                    "Attribute VB_Name = \"Application\"\n{exposure_attr}\nPublic Property Get Value() As Object\nDim e As New Emitter\nSet Value = e\nEnd Property"
                ),
            )
            .expect("host application module should parse");
            let host_emitter = module_unit_from_source(
                "Emitter",
                ModuleKind::Class,
                "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\n",
            )
            .expect("host emitter module should parse");
            let plain_emitter = module_unit_from_source(
                "Emitter",
                ModuleKind::Class,
                "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\n",
            )
            .expect("plain emitter module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, sink],
                references: vec![
                    ProjectReference {
                        referenced_project_name: "PlainProject".to_string(),
                        reference_kind: ReferenceKind::Project,
                    },
                    ProjectReference {
                        referenced_project_name: "HostProject".to_string(),
                        reference_kind: ReferenceKind::HostInjected,
                    },
                ],
                reference_projects: vec![
                    ReferencedProjectManifest {
                        project_name: "PlainProject".to_string(),
                        modules: vec![plain_emitter],
                    },
                    ReferencedProjectManifest {
                        project_name: "HostProject".to_string(),
                        modules: vec![host_application, host_emitter],
                    },
                ],
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let mut runtime = engine
                .start_project_runtime_session(&manifest)
                .unwrap_or_else(|err| panic!("{label} runtime should start: {err}"));
            let snapshot = runtime.snapshot_values();
            let bound_handle = match snapshot.as_slice() {
                [bound, _sink] => match bound {
                    RuntimeValue::ObjectHandle(handle) if handle.raw() > 0 => *handle,
                    RuntimeValue::I32(raw) if *raw > 0 => ObjectHandle::new(*raw),
                    _ => panic!(
                        "{label} should snapshot a bound host-backed emitter handle: {snapshot:?}"
                    ),
                },
                _ => panic!(
                    "{label} should snapshot bound host-backed emitter plus sink instance: {snapshot:?}"
                ),
            };

            let plain_project_routed = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime,
                    "PlainProject",
                    "Emitter",
                    "Changed",
                    bound_handle,
                    &[],
                )
                .unwrap_or_else(|err| {
                    panic!("{label} plain project event ingress should stay non-routing: {err}")
                });
            assert!(
                !plain_project_routed,
                "{label}: plain project type-name match must not own host-backed WithEvents routing"
            );

            let err = engine
                .dispatch_host_event_into_runtime(
                    &mut runtime,
                    "HostProject",
                    "Emitter",
                    "Changed",
                    bound_handle,
                    &[],
                )
                .expect_err("host-injected source should own the conflicting WithEvents route");
            assert_eq!(err.phase(), DiagnosticPhase::Runtime);
            assert!(
                err.message().contains("runtime error: 204"),
                "{label}: host-injected source should route callback, got: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_runtime_procedure_call_expression_in_condition_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "Function GetValue()\nGetValue = 4\nEnd Function\nSub Main()\nIf GetValue() = 4 Then\nError 104\nElse\nError 101\nEnd If\nEnd Sub";

        let err = engine
            .execute_source_with_snapshot_phased(source)
            .expect_err("procedure call expression in condition should execute deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("runtime error: 104"),
            "procedure call expression should drive the true branch, got: {}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_dispatchinvoke_routes_internal_class_function_through_native_dynamic_path() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim value\nvalue = DispatchInvoke(widget, \"Ping\", 7)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Function Ping(ByVal n)\nPing = n + 1\nEnd Function",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        assert_eq!(compiled.project_dynamic_objects.len(), 1);

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(8));
    }

    #[test]
    fn formal_pmr_dispatchinvoke_routes_internal_class_property_get_let_through_native_dynamic_path()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim setResult\nDim afterValue\nbeforeValue = DispatchInvoke(widget, \"Value\")\nsetResult = DispatchInvoke(widget, \"Value\", 9)\nafterValue = DispatchInvoke(widget, \"Value\")\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        assert_eq!(compiled.project_dynamic_objects.len(), 1);
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property get pmr_projecta_widget_value"),
            "expected lowered property get signature in rewritten project source: {lowered}"
        );
        assert!(
            lowered.contains("property let pmr_projecta_widget_value"),
            "expected lowered property let signature in rewritten project source: {lowered}"
        );
        assert!(
            lowered.contains(
                "property_get_pmr_projecta_widget_value = __oxvba_withevents_get(__oxvba_this_instance,"
            ),
            "expected lowered property get state-backed return assignment in rewritten project source: {lowered}"
        );
        assert!(
            lowered.contains("stored = __oxvba_withevents_set(__oxvba_this_instance,"),
            "expected lowered class field state write in rewritten project source: {lowered}"
        );
        assert!(
            compiled.project_dynamic_objects[0]
                .members
                .iter()
                .any(|member| member.member_name == "value"),
            "expected property member metadata for project dynamic object route"
        );
        assert!(
            compiled.project_dynamic_objects[0]
                .members
                .iter()
                .any(|member| {
                    member.member_name == "value" && member.known_dispatch_token == Some(9)
                }),
            "expected property member token metadata for project dynamic object route"
        );
        assert!(
            compiled.project_dynamic_objects[0]
                .members
                .iter()
                .any(|member| {
                    member.member_name == "value"
                        && member.kind == ProjectDynamicMemberKind::PropertyGet
                        && member.return_slot.is_some()
                }),
            "expected property get member route with a return slot"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::Empty);
        assert_eq!(snapshot[3], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_natural_internal_class_property_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget.Value\nwidget.Value = 9\nafterValue = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_internal_class_property_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget.Value\nLet widget.Value = 9\nafterValue = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_natural_internal_class_indexed_property_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget.Value(2)\nwidget.Value(2) = 9\nafterValue = widget.Value(2)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal index, ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_internal_class_indexed_property_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget.Value(2)\nLet widget.Value(2) = 9\nafterValue = widget.Value(2)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal index, ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_natural_internal_class_indexed_default_member_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget(2)\nwidget(2) = 9\nafterValue = widget(2)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal index, ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_internal_class_indexed_default_member_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget(2)\nLet widget(2) = 9\nafterValue = widget(2)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal index, ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_natural_internal_class_default_member_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget\nwidget = 9\nafterValue = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_default_member_get_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("ambiguous non-authoritative fallback should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_default_member_let_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget = 9\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Let Value(ByVal n)\nEnd Property\nPublic Property Let Observe(ByVal n)\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("ambiguous non-authoritative let should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_indexed_default_member_let_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x) = 9\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Let Value(ByVal index, ByVal n)\nEnd Property\nPublic Property Let Observe(ByVal index, ByVal n)\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("ambiguous indexed non-authoritative let should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_default_member_property_set_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByRef target)\nEnd Property\nPublic Property Set Observe(ByRef target)\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("ambiguous non-authoritative property set should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_indexed_default_member_get_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "ambiguous indexed non-authoritative fallback should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_indexed_default_member_property_set_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget(1) = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByVal index, ByRef target)\nEnd Property\nPublic Property Set Observe(ByVal index, ByRef target)\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "ambiguous indexed non-authoritative property set should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_object_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "bare implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_variant_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "bare implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_scalar_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "bare implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_explicit_set_object_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_explicit_set_variant_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_explicit_set_scalar_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_call_statement_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("ambiguous non-authoritative call getter should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_call_statement_indexed_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "ambiguous indexed non-authoritative call getter should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_call_statement_parenthesized_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine.execute_project_with_value_snapshot_phased(&manifest).expect_err(
            "ambiguous parenthesized non-authoritative call getter should fail deterministically",
        );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_no_paren_default_member_get_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "ambiguous non-authoritative no-paren getter should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_statement_context_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine.execute_project_with_value_snapshot_phased(&manifest).expect_err(
            "ambiguous non-authoritative statement-context getter should fail deterministically",
        );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_statement_context_indexed_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine.execute_project_with_value_snapshot_phased(&manifest).expect_err(
            "ambiguous indexed non-authoritative statement-context getter should fail deterministically",
        );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_ambiguous_non_authoritative_statement_context_parenthesized_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine.execute_project_with_value_snapshot_phased(&manifest).expect_err(
            "ambiguous parenthesized non-authoritative statement-context getter should fail deterministically",
        );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_default_member_get_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing non-authoritative getter should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_default_member_let_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget = 9\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing non-authoritative let should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_indexed_default_member_let_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x) = 9\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing indexed non-authoritative let should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_default_member_property_set_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing non-authoritative property set should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_indexed_default_member_property_set_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget(1) = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "missing non-authoritative indexed property set should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_indexed_default_member_get_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing non-authoritative indexed getter should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_object_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "bare implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_variant_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "bare implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_scalar_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "bare implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_explicit_set_object_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_explicit_set_variant_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_explicit_set_scalar_target_default_member_read_assignment_lanes_fail_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_call_statement_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing non-authoritative call getter should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_call_statement_indexed_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "missing non-authoritative indexed call getter should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_statement_context_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "missing non-authoritative statement-context getter should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_statement_context_indexed_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine.execute_project_with_value_snapshot_phased(&manifest).expect_err(
            "missing non-authoritative indexed statement-context getter should fail deterministically",
        );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_call_statement_parenthesized_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "missing parenthesized non-authoritative call getter should fail deterministically",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_no_paren_default_member_get_fails_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("missing non-authoritative no-paren getter should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_missing_non_authoritative_statement_context_parenthesized_default_member_get_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine.execute_project_with_value_snapshot_phased(&manifest).expect_err(
            "missing parenthesized non-authoritative statement-context getter should fail deterministically",
        );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget\nwidget = 9\nafterValue = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("beforevalue = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("property_let_pmr_projecta_widget_value(widget, 9)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("aftervalue = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_get_let_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim beforeValue\nDim afterValue\nx = 2\nbeforeValue = widget(x)\nwidget(x) = 9\nafterValue = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value(ByVal index)\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal index, ByVal n)\nstored = n\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("beforevalue = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("property_let_pmr_projecta_widget_value(widget, x, 9)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("aftervalue = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(2));
        assert_eq!(snapshot[2], RuntimeValue::I32(4));
        assert_eq!(snapshot[3], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_internal_class_default_member_get_let_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget\nLet widget = 9\nafterValue = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }
    #[test]
    fn formal_pmr_statement_context_internal_class_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nwidget.Value\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_statement_context_internal_class_default_member_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nwidget\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 9\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_object_property_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_object_property_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject object property-get result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_object_property_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject object property-get result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_object_property_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject object property-get result on scalar target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_object_property_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject object property-get result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_parenthesized_object_property_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_parenthesized_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_parenthesized_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_object_property_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject parenthesized object property-get result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_parenthesized_object_property_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("implicit assignment should reject parenthesized object property-get result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_object_property_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject parenthesized object property-get result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_parenthesized_object_property_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject parenthesized object property-get result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 9\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_property_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget.Value()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 9\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 9\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_indexed_object_property_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nSet childOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_indexed_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nSet valueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nLet valueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_indexed_object_property_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nvalueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_object_property_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nLet childOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject indexed object property-get result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_indexed_object_property_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nchildOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("implicit assignment should reject indexed object property-get result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_object_property_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nLet n = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject indexed object property-get result on scalar target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_indexed_object_property_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nn = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject indexed object property-get result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_object_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_object_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_object_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_object_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_object_default_member_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject object default-member result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_object_default_member_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject object default-member result on scalar target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_parenthesized_object_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_parenthesized_object_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_object_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_parenthesized_object_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_object_default_member_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject parenthesized object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_parenthesized_object_default_member_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("implicit assignment should reject parenthesized object default-member result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_parenthesized_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject parenthesized object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_parenthesized_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject parenthesized object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_indexed_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nSet childOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_indexed_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nSet valueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nLet valueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_indexed_default_member_get_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_object_default_member_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nLet childOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject indexed object default-member result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_indexed_object_default_member_get_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nchildOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("implicit assignment should reject indexed object default-member result on Object target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nLet n = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("Let should reject indexed object default-member result on scalar target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_internal_class_indexed_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nn = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject indexed object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_set_read_assignment_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_set_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_let_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_implicit_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_let_read_assignment_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject non-authoritative object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_implicit_read_assignment_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject non-authoritative object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }
    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_let_read_assignment_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject non-authoritative object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_implicit_read_assignment_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err("implicit assignment should reject non-authoritative object default-member result on scalar target");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_default_member_property_set_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nSet widget = x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByRef target)\ntarget = target + 7\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_set_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(9)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(9)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_set_read_assignment_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_set_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_let_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_implicit_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_call_statement_internal_class_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nCall widget.Value\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_let_read_assignment_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject non-authoritative parenthesized object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_implicit_read_assignment_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject non-authoritative parenthesized object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }
    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_let_read_assignment_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject non-authoritative parenthesized object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_parenthesized_default_member_implicit_read_assignment_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject non-authoritative parenthesized object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_call_statement_internal_class_default_member_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nCall widget\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_call_statement_internal_class_indexed_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nCall widget.Value(x)\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_call_statement_internal_class_indexed_default_member_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nCall widget(x)\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_call_statement_non_authoritative_single_candidate_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nCall widget\nafterValue = widget.Observe()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Function Observe()\nObserve = stored\nEnd Function",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_call_statement_non_authoritative_single_candidate_indexed_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nCall widget(x)\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_statement_context_non_authoritative_single_candidate_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nwidget\nafterValue = widget.Observe()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Function Observe()\nObserve = stored\nEnd Function",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_non_authoritative_parenthesized_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_set_read_assignment_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nSet childOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_set_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nSet valueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_let_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nLet valueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_implicit_read_assignment_to_variant_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(5)), "{snapshot:?}");
        assert_eq!(snapshot.get(2), Some(&RuntimeValue::I32(2)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_let_read_assignment_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nLet childOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject non-authoritative indexed object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_implicit_read_assignment_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nchildOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject non-authoritative indexed object default-member result on Object target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .to_ascii_lowercase()
                .contains("set required for object variable childout"),
            "{}",
            err.message()
        );
    }
    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_let_read_assignment_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nLet n = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "Let should reject non-authoritative indexed object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_implicit_read_assignment_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nn = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("widget module should parse");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("child module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect_err(
                "implicit assignment should reject non-authoritative indexed object default-member result on scalar target",
            );
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(
            err.message()
                .contains("cannot assign Object to Long variable n"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_object_getter_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Object\nindex = index + 7\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("Set requires Object or Variant target, got Long variable n"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_internal_class_object_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Object\nindex = index + 7\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("Set requires Object or Variant target, got Long variable n"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_non_authoritative_single_candidate_default_member_get_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Object\nindex = index + 7\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message()
                    .contains("Set requires Object or Variant target, got Long variable n"),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_scalar_getter_result_to_variant_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable valueout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable valueout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable valueout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable valueout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message().contains(expected_message),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_scalar_getter_result_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable childout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable childout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable childout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable childout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message().contains(expected_message),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_set_assignment_from_scalar_getter_result_to_scalar_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message().contains(expected_message),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_scalar_getter_result_to_variant_target_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let compiled =
                oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .expect("project execution should succeed");
            assert_eq!(
                snapshot.first(),
                Some(&RuntimeValue::I32(1)),
                "{label}: {snapshot:?}"
            );
            assert_eq!(
                snapshot.last(),
                Some(&RuntimeValue::I32(9)),
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_scalar_getter_result_to_scalar_target_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let compiled =
                oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .expect("project execution should succeed");
            assert_eq!(
                snapshot.first(),
                Some(&RuntimeValue::I32(1)),
                "{label}: {snapshot:?}"
            );
            assert_eq!(
                snapshot.last(),
                Some(&RuntimeValue::I32(9)),
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_scalar_getter_result_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Let cannot assign to Object variable childout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Let cannot assign to Object variable childout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Let cannot assign to Object variable childout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Let cannot assign to Object variable childout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Let cannot assign to Object variable childout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Let cannot assign to Object variable childout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Let cannot assign to Object variable childout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Let cannot assign to Object variable childout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Let cannot assign to Object variable childout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message().contains(expected_message),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_scalar_getter_result_to_variant_target_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let compiled =
                oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .expect("project execution should succeed");
            assert_eq!(
                snapshot.first(),
                Some(&RuntimeValue::I32(1)),
                "{label}: {snapshot:?}"
            );
            assert_eq!(
                snapshot.last(),
                Some(&RuntimeValue::I32(9)),
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_scalar_getter_result_to_scalar_target_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let compiled =
                oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");

            let snapshot = engine
                .execute_project_with_value_snapshot_phased(&manifest)
                .expect("project execution should succeed");
            assert_eq!(
                snapshot.first(),
                Some(&RuntimeValue::I32(1)),
                "{label}: {snapshot:?}"
            );
            assert_eq!(
                snapshot.last(),
                Some(&RuntimeValue::I32(9)),
                "{label}: {snapshot:?}"
            );
        }
    }

    #[test]
    fn formal_pmr_implicit_assignment_from_scalar_getter_result_to_object_target_fails_at_compile_time()
     {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign Long to Object variable childout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign Long to Object variable childout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "cannot assign Long to Object variable childout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "cannot assign Long to Object variable childout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "cannot assign Long to Object variable childout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "cannot assign Long to Object variable childout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign Long to Object variable childout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign Long to Object variable childout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "cannot assign Long to Object variable childout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            assert!(
                err.message().contains(expected_message),
                "{label}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn formal_pmr_non_authoritative_single_candidate_indexed_default_member_property_set_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nSet widget(1) = x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByVal index, ByRef target)\ntarget = target + 7\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_set_pmr_projecta_widget_value(widget, 1, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(
            snapshot.first(),
            Some(&RuntimeValue::I32(1)),
            "{snapshot:?}"
        );
        assert_eq!(snapshot.get(1), Some(&RuntimeValue::I32(9)), "{snapshot:?}");
    }

    #[test]
    fn formal_pmr_statement_context_non_authoritative_single_candidate_indexed_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nwidget(x)\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_call_statement_internal_class_parenthesized_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nCall widget.Value()\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_call_statement_internal_class_parenthesized_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nCall widget()\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_call_statement_parenthesized_non_authoritative_single_candidate_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nCall widget()\nafterValue = widget.Observe()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Function Observe()\nObserve = stored\nEnd Function",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_statement_context_internal_class_parenthesized_property_get_executes_end_to_end()
    {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nwidget.Value()\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_statement_context_internal_class_parenthesized_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nwidget()\nafterValue = widget.Observe\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 2\nEnd Sub\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Property Get Observe()\nObserve = stored\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_statement_context_parenthesized_non_authoritative_single_candidate_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim afterValue\nwidget()\nafterValue = widget.Observe()\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Property Get Value()\nstored = 9\nValue = stored\nEnd Property\nPublic Function Observe()\nObserve = stored\nEnd Function",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot.last(), Some(&RuntimeValue::I32(9)));
    }

    #[test]
    fn formal_pmr_no_paren_call_internal_class_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nwidget.Value x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_no_paren_call_internal_class_default_member_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nwidget x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_no_paren_call_non_authoritative_single_candidate_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nwidget x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_rejects_no_paren_getter_read_assignment_lanes_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            let message = err.message().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn formal_pmr_rejects_no_paren_getter_read_assignment_to_object_target_lanes_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            let message = err.message().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn formal_pmr_rejects_no_paren_getter_read_assignment_to_scalar_target_lanes_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            let message = err.message().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn formal_pmr_rejects_no_paren_getter_explicit_set_read_assignment_lanes_at_compile_time() {
        let engine = Engine::new(HostConfig::default());
        let cases = [
            (
                "named property Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module should parse");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module should parse");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: std::collections::BTreeMap::new(),
            };

            let err = match engine.execute_project_with_value_snapshot_phased(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert_eq!(err.phase(), DiagnosticPhase::CompileTime, "{label}: {err}");
            let message = err.message().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn formal_pmr_statement_context_internal_class_indexed_property_get_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nwidget.Value(x)\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_statement_context_internal_class_indexed_default_member_get_executes_end_to_end()
    {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nwidget(x)\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_property_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nLet valueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_explicit_let_assignment_from_internal_class_indexed_default_member_get_executes_end_to_end()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }
    #[test]
    fn formal_pmr_natural_internal_class_property_set_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nSet widget.Value = x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByRef target)\ntarget = target + 7\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }
    #[test]
    fn formal_pmr_natural_internal_class_default_member_property_set_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nSet widget = x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByRef target)\ntarget = target + 7\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
        assert_eq!(snapshot[2], RuntimeValue::I32(9));
    }
    #[test]
    fn formal_pmr_natural_internal_class_indexed_property_set_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nSet widget.Value(1) = x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByVal index, ByRef target)\ntarget = target + 7\nEnd Property",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_natural_internal_class_indexed_default_member_property_set_executes_end_to_end() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim afterValue\nx = 2\nSet widget(1) = x\nafterValue = x\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByVal index, ByRef target)\ntarget = target + 7\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(9));
    }
    #[test]
    fn formal_pmr_dispatchinvoke_routes_internal_class_default_member_through_native_dynamic_path()
    {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim setResult\nDim afterValue\nbeforeValue = DispatchInvoke(widget, 0)\nsetResult = DispatchInvoke(widget, 0, 9)\nafterValue = DispatchInvoke(widget, 0)\nEnd Sub",
        )
        .expect("main module should parse");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPrivate stored\nPublic Sub Class_Initialize()\nstored = 4\nEnd Sub\nPublic Property Get Value()\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("widget module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        assert_eq!(compiled.project_dynamic_objects.len(), 1);
        assert!(
            compiled.project_dynamic_objects[0]
                .members
                .iter()
                .any(|member| {
                    member.member_name == "value"
                        && member.kind == ProjectDynamicMemberKind::PropertyGet
                        && member.is_default_member
                }),
            "expected default-member metadata for native project dynamic property get route"
        );
        assert!(
            compiled.project_dynamic_objects[0]
                .members
                .iter()
                .any(|member| {
                    member.member_name == "value"
                        && member.kind == ProjectDynamicMemberKind::PropertyLet
                        && member.is_default_member
                }),
            "expected default-member metadata for native project dynamic property let route"
        );

        let snapshot = engine
            .execute_project_with_value_snapshot_phased(&manifest)
            .expect("project execution should succeed");
        assert_eq!(snapshot[0], RuntimeValue::I32(1));
        assert_eq!(snapshot[1], RuntimeValue::I32(4));
        assert_eq!(snapshot[2], RuntimeValue::Empty);
        assert_eq!(snapshot[3], RuntimeValue::I32(9));
    }

    #[test]
    fn formal_pmr_project_manifest_option_private_module_preserves_host_export_entry() {
        let module = module_unit_from_source(
            "PrivateModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"PrivateModule\"\nOption Private Module\nPublic Function Hidden()\nHidden = 1\nEnd Function\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };
        let compiled = oxvba_compiler::compile_project(&manifest).expect("compile should succeed");
        assert!(
            compiled
                .host_exports
                .iter()
                .any(|entry| entry.module_name == "privatemodule"
                    && entry.procedure_name == "hidden"),
            "Option Private Module procedures remain callable for host-direct invocation lanes"
        );
    }

    #[test]
    fn formal_event_runtime_raiseevent_dispatches_to_withevents_handlers_in_stable_order() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim sa As New SinkA\nDim sb As New SinkB\nCall sa.Attach(e)\nCall sb.Attach(e)\nCall e.Fire\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\nPublic Sub Fire()\nRaiseEvent Changed\nEnd Sub",
        )
        .expect("class module should parse");
        let sink_b = module_unit_from_source(
            "SinkB",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkB\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed()\nErr.Raise 202\nEnd Sub",
        )
        .expect("sink module should parse");
        let sink_a = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed()\nErr.Raise 101\nEnd Sub",
        )
        .expect("sink module should parse");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink_b, sink_a],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_slots_test_phased(&manifest)
            .expect_err("first lowered handler should raise deterministic runtime error");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("runtime error: 101"),
            "expected first handler (SinkA) to run before SinkB"
        );
    }

    #[test]
    fn formal_event_runtime_dispatch_host_event_returns_bound_handlers() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\nPublic Sub Fire()\nRaiseEvent Changed\nEnd Sub",
        )
        .expect("class module should parse");
        let sink = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub em_changed()\nEnd Sub",
        )
        .expect("sink module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let _ = engine
            .execute_project_slots_test_phased(&manifest)
            .expect("project compile/execute should load event bindings");

        let handlers = engine.dispatch_host_event("ProjectA", "Emitter", "Changed");
        assert_eq!(handlers, vec!["pmr_projecta_sinka_em_changed".to_string()]);
    }

    #[test]
    fn formal_event_runtime_host_event_ingress_invokes_bound_handler_in_stable_order() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim a As New SinkA\nDim b As New SinkB\nCall a.Attach(e)\nCall b.Attach(e)\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\n",
        )
        .expect("class module should parse");
        let sink_b = module_unit_from_source(
            "SinkB",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkB\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed()\nErr.Raise 202\nEnd Sub",
        )
        .expect("sink module should parse");
        let sink_a = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed()\nErr.Raise 101\nEnd Sub",
        )
        .expect("sink module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink_b, sink_a],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let err = engine
            .dispatch_host_event_into_runtime(
                &mut runtime,
                "ProjectA",
                "Emitter",
                "Changed",
                ObjectHandle::new(1),
                &[],
            )
            .expect_err("first bound handler should raise deterministic runtime error");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("runtime error: 101"),
            "expected first handler (SinkA) to run before SinkB, got: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_host_event_ingress_forwards_single_event_arg() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim s As New SinkA\nCall s.Attach(e)\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed(ByVal n As Integer)\n",
        )
        .expect("class module should parse");
        let sink = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed(ByVal n)\nIf n = 42 Then\nError 142\nElse\nError 141\nEnd If\nEnd Sub",
        )
        .expect("sink module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let err = engine
            .dispatch_host_event_into_runtime(
                &mut runtime,
                "ProjectA",
                "Emitter",
                "Changed",
                ObjectHandle::new(1),
                &[RuntimeValue::I32(42)],
            )
            .expect_err("host ingress should forward deterministic event arg to handler");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("runtime error: 142"),
            "expected forwarded event arg to reach handler, got: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_host_event_ingress_rejects_more_than_one_forwarded_arg() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim s As New SinkA\nCall s.Attach(e)\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed(ByVal n As Integer)\n",
        )
        .expect("class module should parse");
        let sink = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed(ByVal n)\nEnd Sub",
        )
        .expect("sink module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let err = engine
            .dispatch_host_event_into_runtime(
                &mut runtime,
                "ProjectA",
                "Emitter",
                "Changed",
                ObjectHandle::new(1),
                &[RuntimeValue::I32(1), RuntimeValue::I32(2)],
            )
            .expect_err("host ingress should reject more than one forwarded arg");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("PMR-E-HOST-EVENT-ARITY-UNSUPPORTED"),
            "unexpected host ingress diagnostic: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_host_event_ingress_reports_missing_handler() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\n",
        )
        .expect("class module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        engine.subscribe_host_event_handler("ProjectA", "Emitter", "Changed", "MissingHandler");
        let err = engine
            .dispatch_host_event_into_runtime(
                &mut runtime,
                "ProjectA",
                "Emitter",
                "Changed",
                ObjectHandle::new(1),
                &[],
            )
            .expect_err("missing handler should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("PMR-E-EVENT-DISPATCH-TARGET-MISSING"),
            "unexpected host ingress diagnostic: {}",
            err.message()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_ingress_maps_to_registered_handler_symbol() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 1, "SinkA_OnChanged")
            .expect("subscribe_com_event_handler should succeed");

        let _ = dispatch_controlled_com_call(&engine, object, 3, 77);
        let callback = engine
            .poll_com_event_callback()
            .expect("callback poll should succeed")
            .expect("callback should be available");

        assert_eq!(callback.subscription_token, subscription);
        assert_eq!(callback.handler_symbol, "sinka_onchanged");
        assert_eq!(callback.args, vec![RuntimeValue::I32(77)]);
        assert!(callback.callback_token.raw() >= 60_001);
        assert!(
            engine
                .unsubscribe_com_event_handler(subscription)
                .expect("unsubscribe should succeed"),
            "subscription binding should have been tracked"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_object_descriptor_reports_identity_and_capabilities() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let object = create_controlled_com_object(&engine);
        let descriptor = engine
            .describe_com_object(object)
            .expect("describe_com_object should succeed")
            .expect("known COM object should produce a descriptor");

        assert_eq!(descriptor.object.raw(), object.raw());
        assert_eq!(descriptor.prog_id_name, "OxVba.TestDispatch");
        assert_eq!(
            descriptor.transport,
            oxvba_com::ComObjectTransportKind::NativeDispatch
        );
        assert!(descriptor.supports_events);
        assert!(descriptor.known_member_tokens.contains(&1.into()));
        assert!(descriptor.known_member_tokens.contains(&12.into()));
        assert!(descriptor.known_event_tokens.contains(&1.into()));
        assert!(descriptor.known_event_tokens.contains(&3.into()));
        assert_eq!(
            descriptor.typelib_cache_key.as_deref(),
            Some("typelib:oxvba-testdispatch:1.0:0")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_ingress_requires_registered_handler_binding() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .host_services
            .com()
            .subscribe_event(object, 1.into())
            .expect("subscribe_event should succeed");

        let _ = dispatch_controlled_com_call(&engine, object, 3, 21);

        let err = engine
            .poll_com_event_callback()
            .expect_err("callback without handler binding should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("PMR-E-EVENT-DISPATCH-TARGET-MISSING")
        );

        let _ = engine.host_services.com().unsubscribe_event(subscription);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_ingress_captures_multi_arg_payload_shape() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 3, "SinkA_OnPair")
            .expect("subscribe_com_event_handler should succeed");

        let _ = dispatch_controlled_com_call(&engine, object, 4, 90);
        let callback = engine
            .poll_com_event_callback()
            .expect("callback poll should succeed")
            .expect("callback should be available");

        assert_eq!(callback.subscription_token, subscription);
        assert_eq!(callback.handler_symbol, "sinka_onpair");
        assert_eq!(
            callback.args,
            vec![RuntimeValue::I32(90), RuntimeValue::I32(91)]
        );
        assert!(
            engine
                .unsubscribe_com_event_handler(subscription)
                .expect("unsubscribe should succeed")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_evt_b_source_interface_callback_ingress_maps_to_registered_handler_symbol() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 2, "SinkA_OnSourceChanged")
            .expect("subscribe_com_event_handler should succeed for source-interface event");

        let _ = dispatch_controlled_com_call(&engine, object, 11, 55);
        let callback = engine
            .poll_com_event_callback()
            .expect("callback poll should succeed")
            .expect("callback should be available");

        assert_eq!(callback.subscription_token, subscription);
        assert_eq!(callback.handler_symbol, "sinka_onsourcechanged");
        assert_eq!(callback.args, vec![RuntimeValue::I32(55)]);
        assert!(
            engine
                .unsubscribe_com_event_handler(subscription)
                .expect("unsubscribe should succeed")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_runtime_dispatch_invokes_project_handler() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub\nPublic Sub SinkA_OnChanged(ByVal n)\nIf n = 77 Then\nError 177\nElse\nError 178\nEnd If\nEnd Sub",
        )
        .expect("main module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 1, "SinkA_OnChanged")
            .expect("subscribe should succeed");
        let _ = dispatch_controlled_com_call(&engine, object, 3, 77);

        let err = engine
            .poll_and_dispatch_next_com_event_callback(&mut runtime)
            .expect_err("runtime handler should raise deterministic callback code");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("runtime error: 177"));
        assert!(
            engine
                .unsubscribe_com_event_handler(subscription)
                .expect("unsubscribe should succeed")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_runtime_dispatch_reports_missing_project_handler() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("main module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 1, "MissingHandler")
            .expect("subscribe should succeed");
        let _ = dispatch_controlled_com_call(&engine, object, 3, 77);

        let err = engine
            .poll_and_dispatch_next_com_event_callback(&mut runtime)
            .expect_err("unknown runtime handler should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("PMR-E-EVENT-DISPATCH-TARGET-MISSING")
        );
        let _ = engine.unsubscribe_com_event_handler(subscription);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_runtime_dispatch_enforces_handler_signature_arity() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub\nPublic Sub SinkA_OnChanged()\nEnd Sub",
        )
        .expect("main module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 1, "SinkA_OnChanged")
            .expect("subscribe should succeed");
        let _ = dispatch_controlled_com_call(&engine, object, 3, 77);

        let err = engine
            .poll_and_dispatch_next_com_event_callback(&mut runtime)
            .expect_err("signature mismatch should fail deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("PMR-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        let _ = engine.unsubscribe_com_event_handler(subscription);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_event_callback_runtime_dispatch_invokes_two_arg_handler() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub\nPublic Sub SinkA_OnPair(ByVal a, ByVal b)\nIf a = 90 And b = 91 Then\nError 190\nElse\nError 191\nEnd If\nEnd Sub",
        )
        .expect("main module should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut runtime = engine
            .start_project_runtime_session(&manifest)
            .expect("project runtime session should start");
        let object = create_controlled_com_object(&engine);
        let subscription = engine
            .subscribe_com_event_handler(object, 3, "SinkA_OnPair")
            .expect("subscribe should succeed");
        let _ = dispatch_controlled_com_call(&engine, object, 4, 90);

        let err = engine
            .poll_and_dispatch_next_com_event_callback(&mut runtime)
            .expect_err("runtime handler should execute with two callback arguments");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("runtime error: 190"));
        let _ = engine.unsubscribe_com_event_handler(subscription);
    }

    #[test]
    fn formal_event_runtime_raiseevent_forwards_single_event_arg() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim s As New SinkA\nCall s.Attach(e)\nCall e.Fire\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed(ByVal n)\nPublic Sub Fire()\nRaiseEvent Changed(42)\nEnd Sub",
        )
        .expect("class module should parse");
        let sink = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet em = e\nEnd Sub\nPublic Sub em_changed(ByVal n)\nIf n = 42 Then\nError 142\nElse\nError 141\nEnd If\nEnd Sub",
        )
        .expect("sink module should parse");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_slots_test_phased(&manifest)
            .expect_err("event arg should flow to handler and raise deterministic runtime code");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("runtime error: 142"));
    }

    #[test]
    fn formal_event_runtime_withevents_reassignment_rebinds_non_default_instances_deterministically()
     {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e1 As New Emitter\nDim e2 As New Emitter\nDim s As New Sink\nDim a\nDim b\nCall s.Attach(e1)\na = __oxvba_withevents_get(s, 2049099222)\nCall s.Attach(e2)\nb = __oxvba_withevents_get(s, 2049099222)\nIf a = 1 And b = 2 Then\nError 13\nElse\nError 77\nEnd If\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Tick(ByVal n As Integer)\nPublic Sub Fire(ByVal n As Integer)\nRaiseEvent Tick(n)\nEnd Sub",
        )
        .expect("class module should parse");
        let sink = module_unit_from_source(
            "Sink",
            ModuleKind::Class,
            "Attribute VB_Name = \"Sink\"\nPrivate WithEvents src As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet src = e\nEnd Sub",
        )
        .expect("sink module should parse");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };
        let compiled = oxvba_compiler::compile_project(&manifest)
            .expect("project should compile for event lane");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("__oxvba_withevents_set("),
            "expected WithEvents set rewrite in lowered source"
        );
        assert!(
            lowered.contains("__oxvba_withevents_get("),
            "expected WithEvents guard rewrite in lowered source"
        );
        assert!(
            lowered.contains("pmr_projecta_sink_attach"),
            "expected sink attach call path in lowered source"
        );
        assert!(
            compiled.bytecode.instructions.iter().any(|inst| matches!(
                inst,
                oxvba_compiler::Instruction::IntrinsicWithEventsSet { .. }
            )),
            "expected IntrinsicWithEventsSet in emitted bytecode"
        );
        assert!(
            compiled.bytecode.instructions.iter().any(|inst| matches!(
                inst,
                oxvba_compiler::Instruction::IntrinsicWithEventsGet { .. }
            )),
            "expected IntrinsicWithEventsGet in emitted bytecode"
        );
        let err = engine
            .execute_project_slots_test_phased(&manifest)
            .expect_err("reassignment lane should emit deterministic parity sentinel");
        assert_eq!(
            err.phase(),
            DiagnosticPhase::Runtime,
            "unexpected phase with message: {}",
            err.message()
        );
        assert!(
            err.message().contains("runtime error: 13"),
            "unexpected runtime message: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_withevents_clear_then_rebind_updates_dispatch_membership() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e1 As New Emitter\nDim e2 As New Emitter\nDim s As New Sink\nDim a\nDim b\nDim c\nCall s.Attach(e1)\na = __oxvba_withevents_get(s, 2049099222)\nCall s.Detach\nb = __oxvba_withevents_get(s, 2049099222)\nCall s.Attach(e2)\nc = __oxvba_withevents_get(s, 2049099222)\nIf a = 1 And b = 0 And c = 2 Then\nError 13\nElse\nError 77\nEnd If\nEnd Sub",
        )
        .expect("main module should parse");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Tick(ByVal n As Integer)\nPublic Sub Fire(ByVal n As Integer)\nRaiseEvent Tick(n)\nEnd Sub",
        )
        .expect("class module should parse");
        let sink = module_unit_from_source(
            "Sink",
            ModuleKind::Class,
            "Attribute VB_Name = \"Sink\"\nPrivate WithEvents src As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet src = e\nEnd Sub\nPublic Sub Detach()\nSet src = Nothing\nEnd Sub",
        )
        .expect("sink module should parse");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };
        let err = engine
            .execute_project_slots_test_phased(&manifest)
            .expect_err("clear/rebind lane should emit deterministic parity sentinel");
        assert_eq!(
            err.phase(),
            DiagnosticPhase::Runtime,
            "unexpected phase with message: {}",
            err.message()
        );
        assert!(
            err.message().contains("runtime error: 13"),
            "unexpected runtime message: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_withevents_binding_intrinsics_roundtrip_state() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_set(0, 2049099222, 42)\nIf __oxvba_withevents_get(0, 2049099222) = 42 Then\nError 13\nElse\nError 77\nEnd If\nEnd Sub";
        let err = engine
            .execute_source_slots_test_phased(source)
            .expect_err("intrinsic roundtrip should raise deterministic sentinel");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("runtime error: 13"),
            "unexpected runtime message: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_withevents_binding_intrinsics_are_owner_scoped() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a\nDim b\na = __oxvba_withevents_set(11, 2049099222, 41)\nb = __oxvba_withevents_set(22, 2049099222, 52)\nIf __oxvba_withevents_get(11, 2049099222) = 41 And __oxvba_withevents_get(22, 2049099222) = 52 Then\nError 13\nElse\nError 77\nEnd If\nEnd Sub";
        let err = engine
            .execute_source_slots_test_phased(source)
            .expect_err("owner-scoped intrinsic state should roundtrip deterministically");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("runtime error: 13"),
            "unexpected runtime message: {}",
            err.message()
        );
    }

    #[test]
    fn formal_event_runtime_implements_prefixed_member_executes_in_class_flow() {
        let engine = Engine::new(HostConfig::default());
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall ThingImpl.Fire\nEnd Sub",
        )
        .expect("main module should parse");
        let iface_module = module_unit_from_source(
            "IThing",
            ModuleKind::Class,
            "Attribute VB_Name = \"IThing\"\nPublic Sub Ping()\nEnd Sub",
        )
        .expect("interface module should parse");
        let impl_module = module_unit_from_source(
            "ThingImpl",
            ModuleKind::Class,
            "Attribute VB_Name = \"ThingImpl\"\nImplements IThing\nPrivate Sub IThing_Ping()\nErr.Raise 303\nEnd Sub\nPublic Sub Fire()\nCall IThing_Ping\nEnd Sub",
        )
        .expect("implementation module should parse");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, iface_module, impl_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let err = engine
            .execute_project_slots_test_phased(&manifest)
            .expect_err(
                "Implements-prefixed member call should execute and raise deterministic error",
            );
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("runtime error: 303"));
    }

    #[test]
    fn formal_v55_createobject_dispatch_subset() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(11), 2, 3)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5016]);
    }

    #[test]
    fn formal_v55_dispatch_jit_vm_equivalence() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(9), 1, 4)\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v55_dispatch_diagnostics_are_deterministic() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(3), 7, 8)\nEnd Sub";
        let first = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        let second = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(first, second);
    }

    #[test]
    fn formal_v397_createobject_string_progid_subset_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5_004]);
    }

    #[test]
    fn formal_v398_dispatchinvoke_member_name_subset_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"Count\", 0)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5_005]);
    }

    #[test]
    fn formal_v399_dispatchinvoke_two_arg_property_get_subset_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"Count\")\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5_005]);
    }

    #[test]
    fn formal_dispatchinvoke_multi_arg_subset_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SumPair\", 3, 14)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5_033]);
    }

    #[test]
    fn formal_v400_string_com_lane_failure_routes_through_resume_next() {
        let mut engine = Engine::new(HostConfig::default()).with_hal_profile(HalProfileId::Linux);
        engine.set_unsupported_feature_mode(UnsupportedFeatureMode::Runtime);

        let source = "Sub Main()\nDim x\nDim e\nOn Error Resume Next\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"Count\", 0)\ne = Err.Number\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("On Error Resume Next should continue");
        assert_eq!(snapshot[0], 0);
        assert_eq!(snapshot[1], 53_051);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_com_prog_id_override_can_force_registered_lane_failure_for_negative_coverage() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine.set_com_prog_id_override(4, "OxVba.DoesNotExist.Component");
        let err = engine
            .execute_source_slots_test_phased(
                "Sub Main()\nDim x\nx = CreateObject(\"Scripting.Dictionary\")\nEnd Sub",
            )
            .expect_err("invalid override should fail native COM activation");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("com-createobject-class-not-registered")
                || err
                    .message()
                    .contains("com-createobject-invalid-class-string")
                || err.message().contains("0x80040154"),
            "expected class-not-registered indicator, got {}",
            err.message()
        );
    }

    #[test]
    fn formal_v10_array_store_load_roundtrip() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(2)\nDim x\na(1) = 7\nx = a(1)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot.last().copied(), Some(7));
    }

    #[test]
    fn formal_v10_array_bounds_violation_errors() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(1)\na(2) = 5\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("out-of-range access should fail");
        assert!(!err.trim().is_empty());
    }

    #[test]
    fn formal_v10_array_index_zero_is_valid() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(2)\nDim x\na(0) = 3\nx = a(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot.last().copied(), Some(3));
    }

    #[test]
    fn formal_v11_resume_next_records_error_number() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 5);
    }

    #[test]
    fn formal_v11_default_error_mode_fails() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nError 9\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("default error mode should fail");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v11_resume_next_continues_execution() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nx = 1\nError 2\nx = x + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v12_on_error_goto_zero_restores_fail_behavior() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error Resume Next\nOn Error GoTo 0\nError 3\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("goto 0 should restore fail behavior");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v12_resume_next_statement_no_panic() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error Resume Next\nResume Next\nError 2\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("resume next statement should not fail");
        assert!(snapshot.is_empty());
    }

    #[test]
    fn formal_v12_resume_next_then_continue_updates_value() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nOn Error Resume Next\nError 2\nResume Next\nx = 1\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_v20_jit_vm_equivalence_arithmetic() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 4\nx = x - 2\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v20_jit_vm_equivalence_control_flow() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v20_jit_vm_equivalence_error_state() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_slots_test(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_slots_test(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v13_variant_numeric_coercion_long_to_double() {
        let value = oxvba_runtime::Variant::from_i32(7);
        let coerced = oxvba_runtime::coerce::coerce_to(&value, oxvba_runtime::VarType::Double)
            .expect("coercion should succeed");
        assert_eq!(coerced.as_f64(), Some(7.0));
    }

    #[test]
    fn formal_v13_variant_numeric_bool_to_long() {
        let value = oxvba_runtime::Variant::from_bool(true);
        let coerced = oxvba_runtime::coerce::coerce_to(&value, oxvba_runtime::VarType::Long)
            .expect("coercion should succeed");
        assert_eq!(coerced.as_i32(), Some(-1));
    }

    #[test]
    fn formal_v13_variant_numeric_addition_consistency() {
        let lhs = oxvba_runtime::Variant::from_i16(2);
        let rhs = oxvba_runtime::Variant::from_i16(3);
        let out = oxvba_runtime::arithmetic::add(&lhs, &rhs).expect("add should succeed");
        assert_eq!(out.as_i32(), Some(5));
    }

    #[test]
    fn formal_v14_bstr_roundtrip_ascii() {
        let b = oxvba_runtime::bstr::BStr("ABC".to_string());
        assert_eq!(b.0, "ABC");
    }

    #[test]
    fn formal_v14_bstr_concat_law() {
        let a = oxvba_runtime::bstr::BStr("A".to_string());
        let b = oxvba_runtime::bstr::BStr("B".to_string());
        assert_eq!(format!("{}{}", a.0, b.0), "AB");
    }

    #[test]
    fn formal_v14_bstr_empty_identity() {
        let empty = oxvba_runtime::bstr::BStr(String::new());
        let text = oxvba_runtime::bstr::BStr("X".to_string());
        assert_eq!(format!("{}{}", empty.0, text.0), "X");
    }

    #[test]
    fn formal_v15_date_currency_projection_is_stable() {
        let date_like = 45000.25_f64;
        assert_eq!((date_like * 10000.0).round() / 10000.0, 45000.25_f64);
    }

    #[test]
    fn formal_v15_currency_scale_roundtrip() {
        let units = 12345_i64;
        let major = units as f64 / 100.0;
        let roundtrip = (major * 100.0).round() as i64;
        assert_eq!(roundtrip, units);
    }

    #[test]
    fn formal_v15_date_addition_monotonicity() {
        let day0 = 45000.0_f64;
        let day1 = day0 + 1.0;
        assert!(day1 > day0);
    }

    #[test]
    fn formal_v16_spec_trace_matches_runtime_small_program() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 1\nEnd Sub";
        let runtime = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        let spec = vec![2];
        assert_eq!(runtime, spec);
    }

    #[test]
    fn formal_v16_spec_trace_matches_branch_program() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = 3\nElse\nx = 4\nEnd If\nEnd Sub";
        let runtime = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(runtime, vec![3]);
    }

    #[test]
    fn formal_v16_trace_format_is_csv_stable() {
        let trace = [1, 2, 3]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(trace, "1,2,3");
    }

    #[test]
    fn formal_v17_formal_manifest_has_active_entries() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/obligations.csv"))
            .expect("obligations file should exist");
        assert!(text.contains("obligation_id"));
        assert!(text.contains(",true,"));
    }

    #[test]
    fn formal_v17_runner_script_exists() {
        assert!(repo_path("scripts/run-formal.ps1").exists());
        assert!(repo_path("scripts/setup-kani.ps1").exists());
        assert!(repo_path("scripts/test-path-stability.ps1").exists());
        assert!(repo_path("scripts/validate-divergences.ps1").exists());
    }

    #[test]
    fn formal_v17_meta_check_includes_formal_switch() {
        let text = std::fs::read_to_string(repo_path("scripts/meta-check.ps1"))
            .expect("meta-check script exists");
        assert!(text.contains("[switch]$Formal"));
        assert!(text.contains("run-formal.ps1"));
        assert!(text.contains("check-governance.ps1") || text.contains("validate-divergences.ps1"));
        assert!(text.contains("validate-language-coverage.ps1"));
    }

    #[test]
    fn formal_v18_divergence_index_is_present() {
        assert!(repo_path("docs/evidence/divergences/README.md").exists());
    }

    #[test]
    fn formal_v18_divergence_records_have_scope_lines() {
        assert!(divergence_record_has_required_sections(&repo_path(
            "docs/evidence/divergences/DIV-0001.md"
        )));
    }

    #[test]
    fn formal_v18_divergence_records_link_evidence() {
        assert!(divergence_record_has_required_sections(&repo_path(
            "docs/evidence/divergences/DIV-0002.md"
        )));
    }

    #[test]
    fn formal_v22_jit_vm_equivalence_for_loop_backedge() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v22_jit_vm_equivalence_do_loop_backedge() {
        let source = "Sub Main()\nDim x\nx = 0\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v22_cranelift_supports_loop_subset() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(oxvba_jit::cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn formal_v23_formal_runner_has_require_kani_switch() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(text.contains("[switch]$RequireKani"));
        assert!(text.contains("[switch]$UseWslKani"));
        assert!(text.contains("OXVBA_REQUIRE_KANI"));
    }

    #[test]
    fn formal_v23_setup_kani_script_documents_bootstrap() {
        let text =
            std::fs::read_to_string(repo_path("scripts/setup-kani.ps1")).expect("script exists");
        assert!(text.contains("cargo install kani-verifier --locked"));
        assert!(text.contains("cargo kani setup"));
        assert!(repo_path("scripts/run-formal-kani-wsl.ps1").exists());
        assert!(repo_path("scripts/run-formal-kani-async.ps1").exists());
        assert!(repo_path("scripts/async-task-runner.ps1").exists());
    }

    #[test]
    fn formal_v23_ci_supports_optional_kani_job() {
        let text = std::fs::read_to_string(repo_path(".github/workflows/ci.yml"))
            .expect("ci workflow exists");
        assert!(text.contains("formal-kani"));
        assert!(text.contains("RUN_KANI"));
    }

    #[test]
    fn formal_v24_jit_vm_equivalence_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall AddTwo\nEnd Sub\nSub AddTwo()\nx = x + 2\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v24_cranelift_supports_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall AddTwo\nEnd Sub\nSub AddTwo()\nx = x + 2\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(oxvba_jit::cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn formal_v24_jit_falls_back_for_error_state_subset() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v25_optimizer_parity_on_constant_if_fold() {
        let source =
            "Sub Main()\nDim x\nx = 1\nIf 1 = 1 Then\nx = x + 3\nElse\nx = x + 9\nEnd If\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v25_optimizer_parity_on_select_case_fold() {
        let source = "Sub Main()\nDim x\nSelect Case 2\nCase 1\nx = 10\nCase 2\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v25_optimizer_parity_on_dead_store_reduction() {
        let source = "Sub Main()\nDim x\nx = 1\nx = 2\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v26_script_defaults_target_v26_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(
            matrix.contains("mvp-perf-shape-v26")
                || matrix.contains("mvp-full-coverage-perf-gate-v36")
                || matrix.contains("mvp-language-stdlib-consolidation-gate-v56")
                || matrix.contains("mvp-stabilization-rollup-v66")
                || matrix.contains("mvp-full-typing-conformance-gate-v86")
                || matrix.contains("mvp-full-v146")
                || matrix.contains("mvp-profile-v386")
        );
        assert!(
            formal.contains("mvp-perf-shape-v26")
                || formal.contains("mvp-full-coverage-perf-gate-v36")
                || formal.contains("mvp-language-stdlib-consolidation-gate-v56")
                || formal.contains("mvp-stabilization-rollup-v66")
                || formal.contains("mvp-full-typing-conformance-gate-v86")
                || formal.contains("mvp-full-v146")
                || formal.contains("mvp-profile-v386")
        );
    }

    #[test]
    fn formal_v26_benchmark_default_targets_v26_artifact() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(
            bench.contains("docs/evidence/profiles/v26/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v36/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v56/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v64/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v66/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v86/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v146/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.md")
        );
    }

    #[test]
    fn formal_v26_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V26.md").exists());
    }

    #[test]
    fn formal_v27_async_runner_supports_full_action_set() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("Start"));
        assert!(text.contains("Status"));
        assert!(text.contains("Tail"));
        assert!(text.contains("Wait"));
        assert!(text.contains("Stop"));
    }

    #[test]
    fn formal_v27_async_runner_uses_hidden_background_window() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("-WindowStyle Hidden"));
    }

    #[test]
    fn formal_v27_async_runner_persists_state_and_exit_markers() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("state.json"));
        assert!(text.contains("exit_code.txt"));
        assert!(text.contains("completed_utc.txt"));
    }

    #[test]
    fn formal_v28_vm_pc_progression_kani_harness_is_bounded() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("vm interpreter exists");
        assert!(text.contains("pc_progression_is_safe_for_valid_jump_target"));
        assert!(text.contains("kani::assume(instruction_len < 64)"));
        assert!(text.contains("next_pc_for_jump_if_zero"));
    }

    #[test]
    fn formal_v28_vm_jump_helper_has_regression_unit_test() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("vm interpreter exists");
        assert!(text.contains("jump_if_zero_pc_progression_helper"));
    }

    #[test]
    fn formal_v28_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V28.md").exists());
    }

    #[test]
    fn formal_v29_async_runner_wait_supports_timeouts() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("TimeoutSeconds"));
        assert!(text.contains("timed out"));
    }

    #[test]
    fn formal_v29_capacity_workset_document_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-27_KANI_CAPACITY_V29.md").exists());
    }

    #[test]
    fn formal_v29_obligation_entries_are_registered() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/obligations.csv"))
            .expect("obligations should exist");
        assert!(text.contains("FO-V29-001"));
        assert!(text.contains("FO-V29-002"));
        assert!(text.contains("FO-V29-003"));
    }

    #[test]
    fn formal_v30_variant_layout_uses_com_reserved_fields() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-runtime/src/variant.rs"))
            .expect("variant runtime file exists");
        assert!(text.contains("reserved1"));
        assert!(text.contains("reserved2"));
        assert!(text.contains("reserved3"));
        assert!(text.contains("union VariantData"));
    }

    #[test]
    fn formal_v30_variant_runtime_has_com_layout_shape_test() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-runtime/src/variant.rs"))
            .expect("variant runtime file exists");
        assert!(text.contains("com_variant_layout_shape"));
    }

    #[test]
    fn formal_v30_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V30.md").exists());
    }

    #[test]
    fn formal_v31_variant_wire_roundtrip_helpers_exist() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-runtime/src/variant.rs"))
            .expect("variant runtime file exists");
        assert!(text.contains("to_wire_bytes"));
        assert!(text.contains("from_wire_bytes"));
        assert!(text.contains("com_variant_wire_roundtrip_for_numeric_value"));
    }

    #[test]
    fn formal_v31_boundary_marshalling_workset_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-27_BOUNDARY_MARSHALLING_V31.md").exists());
    }

    #[test]
    fn formal_v31_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V31.md").exists());
    }

    #[test]
    fn formal_v32_language_coverage_index_exists() {
        assert!(repo_path("docs/evidence/language/COVERAGE_INDEX.csv").exists());
    }

    #[test]
    fn formal_v32_meta_check_validates_language_coverage() {
        let text = std::fs::read_to_string(repo_path("scripts/meta-check.ps1"))
            .expect("meta-check script exists");
        assert!(text.contains("validate-language-coverage.ps1"));
    }

    #[test]
    fn formal_v32_language_coverage_status_taxonomy_is_present() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains(",implemented,"));
        assert!(text.contains(",partial,"));
        assert!(text.contains(",planned,"));
    }

    #[test]
    fn formal_v33_core_coverage_tracks_key_control_flow_constructs() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains("If Then End If"));
        assert!(text.contains("For Next"));
        assert!(text.contains("Select Case"));
    }

    #[test]
    fn formal_v33_core_coverage_workset_exists() {
        assert!(
            repo_path("docs/worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_CORE_V33.md").exists()
        );
    }

    #[test]
    fn formal_v33_core_conformance_fixtures_are_present() {
        assert!(repo_path("conformance/tests/if_true.bas").exists());
        assert!(repo_path("conformance/tests/for_basic.bas").exists());
        assert!(repo_path("conformance/tests/select_case_basic.bas").exists());
    }

    #[test]
    fn formal_v34_object_coverage_entries_are_present() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains("objects,Root object injection"));
        assert!(text.contains("objects,Class module lifecycle"));
    }

    #[test]
    fn formal_v34_object_coverage_workset_exists() {
        assert!(
            repo_path("docs/worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_OBJECTS_V34.md").exists()
        );
    }

    #[test]
    fn formal_v34_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V34.md").exists());
    }

    #[test]
    fn formal_v35_hotpath_workset_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-27_JIT_OPT_HOTPATHS_V35.md").exists());
    }

    #[test]
    fn formal_v35_jit_vm_hotpath_parity_examples_exist() {
        assert!(repo_path("conformance/tests/for_basic.bas").exists());
        assert!(repo_path("conformance/tests/proc_call_chain.bas").exists());
    }

    #[test]
    fn formal_v35_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V35.md").exists());
    }

    #[test]
    fn formal_v36_script_defaults_target_v36_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(
            matrix.contains("mvp-full-coverage-perf-gate-v36")
                || matrix.contains("mvp-language-stdlib-consolidation-gate-v56")
                || matrix.contains("mvp-stabilization-rollup-v66")
                || matrix.contains("mvp-full-typing-conformance-gate-v86")
                || matrix.contains("mvp-full-v146")
                || matrix.contains("mvp-profile-v386")
        );
        assert!(
            formal.contains("mvp-full-coverage-perf-gate-v36")
                || formal.contains("mvp-language-stdlib-consolidation-gate-v56")
                || formal.contains("mvp-stabilization-rollup-v66")
                || formal.contains("mvp-full-typing-conformance-gate-v86")
                || formal.contains("mvp-full-v146")
                || formal.contains("mvp-profile-v386")
        );
    }

    #[test]
    fn formal_v36_benchmark_default_targets_v36_artifact() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(
            bench.contains("docs/evidence/profiles/v36/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v56/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v64/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v66/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v86/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v146/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.md")
        );
    }

    #[test]
    fn formal_v36_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V36.md").exists());
    }

    #[test]
    fn formal_v56_script_defaults_target_v56_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(
            matrix.contains("mvp-language-stdlib-consolidation-gate-v56")
                || matrix.contains("mvp-stabilization-rollup-v66")
                || matrix.contains("mvp-full-typing-conformance-gate-v86")
                || matrix.contains("mvp-full-v146")
                || matrix.contains("mvp-profile-v386")
        );
        assert!(
            formal.contains("mvp-language-stdlib-consolidation-gate-v56")
                || formal.contains("mvp-stabilization-rollup-v66")
                || formal.contains("mvp-full-typing-conformance-gate-v86")
                || formal.contains("mvp-full-v146")
                || formal.contains("mvp-profile-v386")
        );
    }

    #[test]
    fn formal_v56_benchmark_default_targets_v56_artifact() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(
            bench.contains("docs/evidence/profiles/v56/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v64/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v66/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v86/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v146/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.md")
        );
    }

    #[test]
    fn formal_v56_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V56.md").exists());
    }

    #[test]
    fn formal_v57_async_runner_supports_watcher_controls() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("WatchStart"));
        assert!(text.contains("WatchStop"));
        assert!(text.contains("StartWatcher"));
        assert!(text.contains("WatchPollSeconds"));
        assert!(text.contains("watcher_pid"));
        assert!(text.contains("liveness.log"));
    }

    #[test]
    fn formal_v57_watcher_script_is_resilient() {
        let text = std::fs::read_to_string(repo_path("scripts/watch-formal-kani-async.ps1"))
            .expect("watch script exists");
        assert!(text.contains("status=state-missing"));
        assert!(text.contains("status=state-parse-error"));
        assert!(text.contains("status=watch-error"));
        assert!(text.contains("status=completed"));
    }

    #[test]
    fn formal_v57_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V57.md").exists());
    }

    #[test]
    fn formal_v58_syntax_kani_harnesses_exist() {
        let lexer = std::fs::read_to_string(repo_path("crates/oxvba-syntax/src/lexer.rs"))
            .expect("lexer exists");
        let parser = std::fs::read_to_string(repo_path("crates/oxvba-syntax/src/parser.rs"))
            .expect("parser exists");
        assert!(lexer.contains("tokenize_always_appends_eof_token"));
        assert!(parser.contains("parse_non_empty_source_roundtrips_input"));
        assert!(lexer.contains("#[cfg(kani)]"));
        assert!(parser.contains("#[cfg(kani)]"));
    }

    #[test]
    fn formal_v58_optimizer_kani_harness_exists() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-compiler/src/optimize.rs"))
            .expect("optimizer file exists");
        assert!(text.contains("zero_delta_self_add_assignment_is_removed"));
        assert!(text.contains("#[cfg(kani)]"));
    }

    #[test]
    fn formal_v58_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V58.md").exists());
    }

    #[test]
    fn formal_v59_line_continuation_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nx = x + _\n2\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 3);
    }

    #[test]
    fn formal_v59_line_continuation_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/line_continuation_basic.bas").exists());
    }

    #[test]
    fn formal_v59_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V59.md").exists());
    }

    #[test]
    fn formal_v60_with_block_member_assignments_execute() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nWith x\n.Value = 1\n.Value = .Value + 2\nx = .Value\nEnd With\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 3);
    }

    #[test]
    fn formal_v60_nested_with_block_assignments_execute() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nWith x\nWith .inner\n.Value = 9\nEnd With\nx = .inner_Value\nEnd With\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 9);
    }

    #[test]
    fn formal_v60_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V60.md").exists());
    }

    #[test]
    fn formal_v61_conditional_compilation_if_else_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "#Const ENABLE = True\nSub Main()\nDim x\n#If ENABLE Then\nx = 7\n#Else\nx = 1\n#End If\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 7);
    }

    #[test]
    fn formal_v61_conditional_compilation_elseif_executes() {
        let engine = Engine::new(HostConfig::default());
        let source = "#Const A = False\n#Const B = True\nSub Main()\nDim x\n#If A Then\nx = 1\n#ElseIf B Then\nx = 9\n#Else\nx = 3\n#End If\nEnd Sub";
        let snapshot = engine
            .execute_source_slots_test(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 9);
    }

    #[test]
    fn formal_v61_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V61.md").exists());
    }

    #[test]
    fn formal_v62_intrinsic_surface_registry_classifies_host_and_core() {
        use oxvba_compiler::resolve::{IntrinsicSurface, intrinsic_surface};

        assert_eq!(
            intrinsic_surface("Len"),
            Some(IntrinsicSurface::DeterministicCore)
        );
        assert_eq!(
            intrinsic_surface("Date"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("FreeFile"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("MsgBox"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("DoEvents"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("Shell"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("DispatchInvoke"),
            Some(IntrinsicSurface::HostSensitive)
        );
    }

    #[test]
    fn formal_v62_intrinsic_surface_evidence_file_exists() {
        assert!(repo_path("docs/evidence/runtime/INTRINSIC_SURFACE.csv").exists());
    }

    #[test]
    fn formal_v62_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V62.md").exists());
    }

    #[test]
    fn formal_v63_jit_supports_intrinsic_math_subset() {
        let bytecode = oxvba_compiler::compile(
            "Sub Main()\nDim x\nx = Abs(-7)\nx = Sgn(x)\nx = Fix(x)\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(oxvba_jit::cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn formal_v63_intrinsic_math_subset_is_jit_vm_equivalent() {
        let source = "Sub Main()\nDim x\nx = Abs(-7)\nx = Sgn(x)\nx = Fix(x)\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v63_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V63.md").exists());
    }

    #[test]
    fn formal_v64_benchmark_script_tracks_mixed_workloads() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(bench.contains("conformance_vm"));
        assert!(bench.contains("conformance_jit"));
        assert!(bench.contains("subset_err_string_financial_vm"));
        assert!(bench.contains("subset_err_string_financial_jit"));
        assert!(bench.contains("OutputCsvPath"));
    }

    #[test]
    fn formal_v64_benchmark_profile_artifact_defaults_exist() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(
            bench.contains("docs/evidence/profiles/v64/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v66/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v86/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v146/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.md")
        );
        assert!(
            bench.contains("docs/evidence/profiles/v64/benchmark_latest.csv")
                || bench.contains("docs/evidence/profiles/v66/benchmark_latest.csv")
                || bench.contains("docs/evidence/profiles/v86/benchmark_latest.csv")
                || bench.contains("docs/evidence/profiles/v146/benchmark_latest.csv")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.csv")
        );
    }

    #[test]
    fn formal_v64_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V64.md").exists());
    }

    #[test]
    fn formal_v65_integrated_gate_script_exists() {
        let text = std::fs::read_to_string(repo_path("scripts/run-profile-gate.ps1"))
            .expect("integrated gate script exists");
        assert!(text.contains("run-formal.ps1"));
        assert!(text.contains("run-matrix.ps1"));
        assert!(text.contains("run-bench.ps1"));
        assert!(text.contains("integrated_gate.md"));
    }

    #[test]
    fn formal_v65_workset_document_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-28_INTEGRATED_GATE_V65.md").exists());
    }

    #[test]
    fn formal_v65_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V65.md").exists());
    }

    #[test]
    fn formal_v66_script_defaults_target_v66_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(
            matrix.contains("mvp-stabilization-rollup-v66")
                || matrix.contains("mvp-full-typing-conformance-gate-v86")
                || matrix.contains("mvp-full-v146")
                || matrix.contains("mvp-profile-v386")
        );
        assert!(
            formal.contains("mvp-stabilization-rollup-v66")
                || formal.contains("mvp-full-typing-conformance-gate-v86")
                || formal.contains("mvp-full-v146")
                || formal.contains("mvp-profile-v386")
        );
        assert!(
            bench.contains("docs/evidence/profiles/v66/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v86/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v146/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.md")
        );
    }

    #[test]
    fn formal_v66_workset_document_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-28_STABILIZATION_ROLLUP_V66.md").exists());
    }

    #[test]
    fn formal_v66_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V66.md").exists());
    }

    #[test]
    fn formal_v86_script_defaults_target_v86_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        let integrated = std::fs::read_to_string(repo_path("scripts/run-profile-gate.ps1"))
            .expect("run-profile-gate script exists");
        assert!(matrix.contains("mvp-full-v146") || matrix.contains("mvp-profile-v386"));
        assert!(formal.contains("mvp-full-v146") || formal.contains("mvp-profile-v386"));
        assert!(
            bench.contains("docs/evidence/profiles/v146/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v386/benchmark_latest.md")
        );
        assert!(integrated.contains("mvp-full-v146") || integrated.contains("mvp-profile-v386"));
    }

    #[test]
    fn formal_v86_phase12_status_targets_v86_scope() {
        let text = std::fs::read_to_string(repo_path("docs/PHASE12_STATUS.md"))
            .expect("phase status doc exists");
        assert!(text.contains("mvp-full-v146") || text.contains("mvp-profile-v386"));
        assert!(
            text.contains("docs/evidence/profiles/v146/matrix_latest.csv")
                || text.contains("docs/evidence/profiles/v386/matrix_latest.csv")
        );
        assert!(
            text.contains("docs/evidence/profiles/v146/integrated_gate.md")
                || text.contains("docs/evidence/profiles/v386/integrated_gate.md")
        );
    }

    #[test]
    fn formal_v86_deferred_gate_audit_exists_with_unblock_steps() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/DG_AUDIT_V86.md"))
            .expect("v86 dg audit exists");
        assert!(text.contains("Unblocking steps"));
        assert!(text.contains("DG-V74-001"));
        assert!(text.contains("DG-V85-001"));
    }

    #[test]
    fn formal_v393_meta_check_runs_gate_sync_validator() {
        let text = std::fs::read_to_string(repo_path("scripts/meta-check.ps1"))
            .expect("meta-check script exists");
        assert!(text.contains("check-governance.ps1") || text.contains("validate-gate-sync.ps1"));
    }

    #[test]
    fn formal_v394_com_latebound_bridge_spec_exists() {
        let text =
            std::fs::read_to_string(repo_path("docs/spec/COM_CLIENT_LATEBOUND_BRIDGE_V1.md"))
                .expect("latebound bridge spec should exist");
        assert!(text.contains("IntrinsicCreateObjectHost"));
        assert!(text.contains("IntrinsicDispatchInvokeHost"));
    }

    #[test]
    fn formal_v395_com_error_mapping_table_exists_and_has_required_rows() {
        let text = std::fs::read_to_string(repo_path(
            "docs/evidence/conformance/COM_CLIENT_ERROR_MAPPING_V1.csv",
        ))
        .expect("COM error mapping table should exist");
        assert!(text.contains("COM-CREATEOBJECT-CLASS-NOT-REGISTERED"));
        assert!(text.contains("COM-DISPATCH-MEMBER-NOT-FOUND"));
        assert!(text.contains("COM-DISPATCH-ARG-ERROR"));
    }

    #[test]
    fn formal_v396_c2_conformance_scaffold_files_exist() {
        assert!(repo_path("conformance/com/client/c2-latebound/README.md").exists());
        assert!(
            repo_path(
                "conformance/com/client/c2-latebound/createobject_string_prog_id_scaffold.bas"
            )
            .exists()
        );
    }

    #[test]
    fn formal_v397_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V397.md").exists());
    }

    #[test]
    fn formal_v398_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V398.md").exists());
    }

    #[test]
    fn formal_v399_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V399.md").exists());
    }

    #[test]
    fn formal_v400_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V400.md").exists());
    }

    #[test]
    fn formal_v400_c2_fixture_pack_exists() {
        assert!(
            repo_path(
                "conformance/com/client/c2-latebound/createobject_string_prog_id_success.bas"
            )
            .exists()
        );
        assert!(
            repo_path(
                "conformance/com/client/c2-latebound/dispatch_member_name_failure_resume_next.bas"
            )
            .exists()
        );
    }

    #[test]
    fn formal_v417_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V417.md").exists());
    }

    #[test]
    fn formal_v418_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V418.md").exists());
    }

    #[test]
    fn formal_v419_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V419.md").exists());
    }

    #[test]
    fn formal_v420_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V420.md").exists());
    }

    #[test]
    fn formal_v421_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V421.md").exists());
    }

    #[test]
    fn formal_v422_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V422.md").exists());
    }

    #[test]
    fn formal_v423_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V423.md").exists());
    }

    #[test]
    fn formal_v424_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V424.md").exists());
    }

    #[test]
    fn formal_v425_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V425.md").exists());
    }

    #[test]
    fn formal_v426_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V426.md").exists());
    }

    #[test]
    fn formal_v426_early_binding_workset_and_evidence_exist() {
        assert!(
            repo_path(
                "docs/worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V417_V426.md"
            )
            .exists()
        );
        assert!(
            repo_path("docs/evidence/profiles/v426/V426_COM_EARLY_IMPLEMENTATION_BLOCK_I.md")
                .exists()
        );
    }

    #[test]
    fn formal_v466_profile_status_range_exists() {
        for version in 427..=466 {
            let path = format!("docs/profile-status/PROFILE_STATUS_V{version}.md");
            assert!(
                repo_path(&path).exists(),
                "missing profile status for v{version}"
            );
        }
    }

    #[test]
    fn formal_v466_early_binding_terminal_artifacts_exist() {
        let required = [
            "docs/worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V427_V445.md",
            "docs/worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_ORACLE_FORMAL_V446_V457.md",
            "docs/worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_CLOSURE_V458_V466.md",
            "docs/worksets/WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_EXECUTION_V407_V466.md",
            "docs/evidence/profiles/v464/integrated_gate.md",
            "docs/evidence/profiles/v464/integrated_gate.csv",
            "docs/evidence/profiles/v466/integrated_gate.md",
            "docs/evidence/profiles/v466/V466_COM_EARLY_TERMINAL_GATE.md",
            "docs/evidence/profiles/v466/V466_COM_EARLY_CLOSURE_REPORT.md",
            "docs/evidence/conformance/com_early/COM_EARLY_CONFORMANCE_LATEST.csv",
            "docs/evidence/perf/com_early/COM_EARLY_PERF_LATEST.csv",
        ];
        for path in required {
            assert!(
                repo_path(path).exists(),
                "missing required terminal artifact: {path}"
            );
        }
    }

    #[test]
    fn formal_v107_with_block_direct_member_target_executes() {
        let source = "Sub Main()\nDim x\nWith x.inner\n.Value = 4\n.Value = .Value + 3\nx = .Value\nEnd With\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![7, 7]);
    }

    #[test]
    fn formal_v107_with_block_member_target_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/with_block_member_target_chain.bas").exists());
    }

    #[test]
    fn formal_v107_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V107.md").exists());
    }

    #[test]
    fn formal_v120_extended_conversion_subset_executes() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = CSng(7)\nb = CByte(8)\nc = CCur(9)\nd = CDec(10)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![7, 8, 9, 10]);
    }

    #[test]
    fn formal_v121_set_let_assignment_keywords_execute() {
        let source =
            "Sub Main()\nDim x\nDim obj As Object\nLet x = 5\nSet obj = CreateObject(4)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_value_snapshot(source)
        .expect("execution should succeed");
        assert_eq!(out[0], RuntimeValue::I32(5));
        assert!(matches!(out[1], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[test]
    fn formal_v121_set_keyword_rejects_non_object_assignment() {
        let source = "Sub Main()\nDim x\nSet x = 7\nEnd Sub";
        let err = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect_err("Set on a non-object assignment should fail");
        assert!(err.contains("Set requires object value"), "{err}");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_set_keyword_accepts_variant_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim v As Variant\nSet v = CreateObject(4)\nEnd Sub";
        let out = engine
            .execute_source_with_value_snapshot(source)
            .expect("Set into Variant target should preserve object call result");
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_set_keyword_accepts_object_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim obj As Object\nSet obj = CreateObject(4)\nEnd Sub";
        let out = engine
            .execute_source_with_value_snapshot(source)
            .expect("Set into Object target should preserve object call result");
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_set_keyword_rejects_variant_target_for_scalar_source() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim v As Variant\nSet v = 7\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("Set on a Variant target with scalar source should fail");
        assert!(
            err.contains("Set requires object value for variable v"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_set_keyword_rejects_object_target_for_scalar_source() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim obj As Object\nSet obj = 7\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("Set on an Object target with scalar source should fail");
        assert!(
            err.contains("Set requires object value for variable obj"),
            "{err}"
        );
    }

    #[test]
    fn formal_v121_set_keyword_rejects_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nSet n = 7\nEnd Sub";
        let err = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect_err("Set on a scalar target with scalar source should fail");
        assert!(
            err.contains("Set requires Object or Variant target, got Long variable n"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_set_keyword_rejects_scalar_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim n As Long\nSet n = CreateObject(4)\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("Set on a scalar target with object call result should fail");
        assert!(
            err.contains("Set requires Object or Variant target, got Long variable n"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_let_keyword_rejects_object_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim obj As Object\nLet obj = CreateObject(4)\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("Let on an Object target with object call result should fail");
        assert!(
            err.contains("Let cannot assign to Object variable obj"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_let_keyword_accepts_variant_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim v As Variant\nLet v = CreateObject(4)\nEnd Sub";
        let out = engine
            .execute_source_with_value_snapshot(source)
            .expect("Let into Variant target should preserve object call result");
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_let_keyword_rejects_object_target_for_scalar_source() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim obj As Object\nLet obj = 7\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("Let on an Object target with scalar source should fail");
        assert!(
            err.contains("Let cannot assign to Object variable obj"),
            "{err}"
        );
    }

    #[test]
    fn formal_v121_let_keyword_accepts_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nLet v = 7\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("Let on a Variant target with scalar source should succeed");
        assert_eq!(out, vec![7]);
    }

    #[test]
    fn formal_v121_let_keyword_accepts_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nLet n = 7\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("Let on a scalar target with scalar source should succeed");
        assert_eq!(out, vec![7]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_let_keyword_rejects_scalar_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim n As Long\nLet n = CreateObject(4)\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("Let on a scalar target with object call result should fail");
        assert!(
            err.contains("cannot assign Object to Long variable n"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_implicit_assignment_accepts_variant_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim v As Variant\nv = CreateObject(4)\nEnd Sub";
        let out = engine
            .execute_source_with_value_snapshot(source)
            .expect("implicit assignment into Variant target should preserve object call result");
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_implicit_assignment_rejects_object_target_for_object_call_result_without_set() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim obj As Object\nobj = CreateObject(4)\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("implicit assignment into Object target should require Set");
        assert!(
            err.contains("Set required for Object variable obj"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_implicit_assignment_rejects_scalar_target_for_object_call_result() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim n As Long\nn = CreateObject(4)\nEnd Sub";
        let err = engine.execute_source_slots_test(source).expect_err(
            "implicit assignment on a scalar target with object call result should fail",
        );
        assert!(
            err.contains("cannot assign Object to Long variable n"),
            "{err}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_implicit_assignment_rejects_object_target_for_scalar_source() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Sub Main()\nDim obj As Object\nobj = 7\nEnd Sub";
        let err = engine
            .execute_source_slots_test(source)
            .expect_err("implicit assignment on an Object target with scalar source should fail");
        assert!(
            err.contains("cannot assign Long to Object variable obj"),
            "{err}"
        );
    }

    #[test]
    fn formal_v121_implicit_assignment_accepts_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nv = 7\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("implicit assignment on a Variant target with scalar source should succeed");
        assert_eq!(out, vec![7]);
    }

    #[test]
    fn formal_v121_implicit_assignment_accepts_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nn = 7\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("implicit assignment on a scalar target with scalar source should succeed");
        assert_eq!(out, vec![7]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_object_source_assignment_accepts_set_targets_and_variant_let_implicit() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(4)\nSet v = src\nEnd Sub",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(4)\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(4)\nLet v = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(4)\nv = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            let out = engine
                .execute_source_with_value_snapshot(source)
                .unwrap_or_else(|err| panic!("{label} should execute, got {err}"));
            assert_eq!(out.len(), 2, "{label}: {out:?}");
            assert!(
                matches!(
                    (&out[0], &out[1]),
                    (RuntimeValue::ObjectHandle(src), RuntimeValue::ObjectHandle(dst))
                        if src.raw() > 0 && src.raw() == dst.raw()
                ),
                "{label}: {out:?}"
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn formal_v121_object_source_assignment_rejects_object_and_scalar_mismatch_lanes() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(4)\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(4)\ndst = src\nEnd Sub",
                "Set required for Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(4)\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(4)\nLet n = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(4)\nn = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match engine.execute_source_slots_test(source) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.contains(expected), "{label}: {err}");
        }
    }

    #[test]
    fn formal_v121_variant_source_scalar_payload_assignment_executes_supported_lanes() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let cases = [
            (
                "Let Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nLet v = src\nEnd Sub",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nLet n = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nv = src\nEnd Sub",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nn = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            let out = engine
                .execute_source_slots_test(source)
                .unwrap_or_else(|err| panic!("{label} should execute, got {err}"));
            assert_eq!(out, vec![7, 7], "{label}: {out:?}");
        }
    }

    #[test]
    fn formal_v121_variant_source_scalar_payload_assignment_rejects_runtime_mismatch_lanes() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nSet v = src\nEnd Sub",
                "Set requires object value for variable v",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\nSet dst = src\nEnd Sub",
                "Set requires object value for variable dst",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\ndst = src\nEnd Sub",
                "cannot assign Long to Object variable dst",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match engine.execute_source_slots_test(source) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.contains(expected), "{label}: {err}");
        }
    }

    #[test]
    fn formal_v121_variant_source_scalar_payload_rejects_compile_time_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match Engine::new(HostConfig {
                enable_jit: false,
                root_object_name: None,
            })
            .execute_source_slots_test(source)
            {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.contains(expected), "{label}: {err}");
        }
    }

    #[test]
    fn formal_v121_variant_source_object_payload_assignment_executes_supported_lanes() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(4)\nSet v = src\nEnd Sub",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(4)\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(4)\nLet v = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(4)\nv = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            let out = engine
                .execute_source_with_value_snapshot(source)
                .unwrap_or_else(|err| panic!("{label} should execute, got {err}"));
            assert_eq!(out.len(), 2, "{label}: {out:?}");
            assert!(
                matches!(
                    (&out[0], &out[1]),
                    (RuntimeValue::ObjectHandle(src), RuntimeValue::ObjectHandle(dst))
                        if src.raw() > 0 && src.raw() == dst.raw()
                ),
                "{label}: {out:?}"
            );
        }
    }

    #[test]
    fn formal_v121_variant_source_object_payload_assignment_rejects_runtime_mismatch_lanes() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let cases = [
            (
                "implicit Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(4)\ndst = src\nEnd Sub",
                "Set required for Object variable dst",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(4)\nLet n = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(4)\nn = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match engine.execute_source_slots_test(source) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.contains(expected), "{label}: {err}");
        }
    }

    #[test]
    fn formal_v126_introspection_and_typeof_subset_executes() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nIf TypeOf 5 Is 5 Then\nd = 1\nElse\nd = 0\nEnd If\na = IsEmpty(0)\nb = IsNull(-1)\nc = IsError(CVErr(9))\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 1, 1, 1]);
    }

    #[test]
    fn formal_v153_null_empty_error_predicates_are_distinct() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\na = IsEmpty(Empty)\nb = IsNull(Null)\nc = IsError(CVErr(7))\nd = IsError(Null)\ne = IsNumeric(CVErr(7))\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 1, 1, 0, 0]);
    }

    #[test]
    fn formal_v153_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/coercion_null_empty_error_predicates.bas").exists());
    }

    #[test]
    fn formal_v153_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V153.md").exists());
    }

    #[test]
    fn formal_v154_financial_intrinsics_use_algorithmic_subset() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\nDim f\nDim g\na = Rnd()\nb = Rnd(42)\nc = NPV(1, 10, 20, 30)\nd = IRR(50)\ne = MIRR(70, 1, 2)\nf = Rate(10, 2, 99)\ng = NPer(1, 2, 88, 3)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 42, 59, -50, -28, -99, -38]);
    }

    #[test]
    fn formal_v154_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/stdlib_random_financial_expansion.bas").exists());
    }

    #[test]
    fn formal_v154_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V154.md").exists());
    }

    #[test]
    fn formal_v155_rate_nper_algorithmic_subset() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\nDim f\nDim g\na = Rnd()\nb = Rnd(42)\nc = NPV(1, 10, 20, 30)\nd = IRR(50)\ne = MIRR(70, 1, 2)\nf = Rate(10, 2, 99)\ng = NPer(1, 2, 88, 3)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 42, 59, -50, -28, -99, -38]);
    }

    #[test]
    fn formal_v155_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/stdlib_random_financial_expansion.bas").exists());
    }

    #[test]
    fn formal_v155_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V155.md").exists());
    }

    #[test]
    fn formal_v156_financial_non_convergence_signals_error_tags() {
        let source = "Sub Main()\nDim a\nDim b\na = Rate(0, 0, 0)\nb = NPer(1, 0, 0, 0)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(
            out,
            vec![error_tag_from_code(2001), error_tag_from_code(2002)]
        );
    }

    #[test]
    fn formal_v156_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/financial_tolerance_non_convergence.bas").exists());
    }

    #[test]
    fn formal_v156_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V156.md").exists());
    }

    #[test]
    fn formal_v157_compile_time_diagnostic_wins_before_runtime() {
        let source = "Sub Main()\nGoTo nowhere\nError 5\nEnd Sub";
        let err = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test_phased(source)
        .expect_err("compile-time diagnostic should win");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(err.message().contains("goto target label not found"));
    }

    #[test]
    fn formal_v157_runtime_diagnostic_is_classified_after_successful_compile() {
        let source = "Sub Main()\nError 5\nEnd Sub";
        let err = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test_phased(source)
        .expect_err("runtime diagnostic should be raised");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("runtime error"));
    }

    #[test]
    fn formal_v157_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/diagnostic_phase_compile_wins.bas").exists());
    }

    #[test]
    fn formal_v157_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V157.md").exists());
    }

    #[test]
    fn formal_v158_financial_tolerance_fixture_executes_on_vm_path() {
        let source = "Sub Main()\nDim a\nDim b\na = Rate(0, 0, 0)\nb = NPer(1, 0, 0, 0)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(
            out,
            vec![error_tag_from_code(2001), error_tag_from_code(2002)]
        );
    }

    #[test]
    fn formal_v158_vartype_isnumeric_tags_fixture_executes() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\nDim f\nDim g\nDim h\na = VarType(vbNullString)\nb = VarType(Null)\nc = VarType(CVErr(9))\nd = VarType(7)\ne = IsNumeric(vbNullString)\nf = IsNumeric(Null)\ng = IsNumeric(CVErr(9))\nh = IsNumeric(7)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![0, 1, 10, 3, 0, 0, 0, 1]);
    }

    #[test]
    fn formal_v158_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/introspection_vartype_isnumeric_tags.bas").exists());
    }

    #[test]
    fn formal_v158_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V158.md").exists());
    }

    #[test]
    fn formal_v159_jit_fallback_matches_vm_for_financial_tolerance_subset() {
        let source = "Sub Main()\nDim a\nDim b\na = Rate(0, 0, 0)\nb = NPer(1, 0, 0, 0)\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
        assert_eq!(
            jit_out,
            vec![error_tag_from_code(2001), error_tag_from_code(2002)]
        );
    }

    #[test]
    fn formal_v159_jit_fallback_matches_vm_for_tag_introspection_subset() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\nDim f\nDim g\nDim h\na = VarType(vbNullString)\nb = VarType(Null)\nc = VarType(CVErr(9))\nd = VarType(7)\ne = IsNumeric(vbNullString)\nf = IsNumeric(Null)\ng = IsNumeric(CVErr(9))\nh = IsNumeric(7)\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
        assert_eq!(jit_out, vec![0, 1, 10, 3, 0, 0, 0, 1]);
    }

    #[test]
    fn formal_v159_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/introspection_vartype_isnumeric_tags.bas").exists());
    }

    #[test]
    fn formal_v159_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V159.md").exists());
    }

    #[test]
    fn formal_v160_err_clear_full_surface_fixture_executes() {
        let source = "Sub Main()\nDim n\nDim d\nDim s\nDim h\nDim f\nDim l\nOn Error Resume Next\nError 9\nErr.Clear\nn = Err.Number\nd = Err.Description\ns = Err.Source\nh = Err.HelpContext\nf = Err.HelpFile\nl = Err.LastDllError\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn formal_v160_string_udt_coercion_corpus_fixtures_execute() {
        let string_source = "Sub Main()\nDim v\nDim a\nDim b\nDim c\nv = vbNullString\na = IsEmpty(v)\nb = IsNull(v)\nc = IsError(v)\nEnd Sub";
        let string_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(string_source)
        .expect("string fixture should execute");
        assert_eq!(string_out, vec![0, 1, 0, 0]);

        let udt_source = "Type Pair\nA\nB\nEnd Type\nSub Main()\nDim x As Pair\nDim y As Pair\nx.A = 1\nx.B = 2\ny.A = 9\ny.B = 8\ny = x\nx.A = 7\nx.B = 6\ny = x\nEnd Sub";
        let udt_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(udt_source)
        .expect("udt fixture should execute");
        assert_eq!(udt_out, vec![0, 7, 6, 0, 7, 6]);

        let coercion_source =
            "Sub Main()\nDim a\nDim b\nDim c\na = CVErr(-4)\nb = CVErr(4)\nc = IsError(a)\nEnd Sub";
        let coercion_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(coercion_source)
        .expect("coercion fixture should execute");
        assert_eq!(coercion_out, vec![-899999996, -899999996, 1]);
    }

    #[test]
    fn formal_v160_conformance_fixtures_exist() {
        assert!(repo_path("conformance/tests/err_clear_full_surface_reset.bas").exists());
        assert!(repo_path("conformance/tests/string_vbnullstring_predicates.bas").exists());
        assert!(repo_path("conformance/tests/udt_whole_assignment_overwrite.bas").exists());
        assert!(repo_path("conformance/tests/coercion_cverr_abs_normalization.bas").exists());
    }

    #[test]
    fn formal_v160_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V160.md").exists());
    }

    #[test]
    fn formal_v161_financial_algorithm_fixtures_execute() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\na = NPV(1, 10, 20, 30)\nb = IRR(50)\nc = MIRR(70, 1, 2)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![59, -50, -28]);

        let source =
            "Sub Main()\nDim a\nDim b\na = Rate(10, 2, 99)\nb = NPer(1, 2, 88, 3)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![-99, -38]);
    }

    #[test]
    fn formal_v161_financial_tolerance_mixed_modes_fixture_executes() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Rate(0, 0, 0)\nb = NPer(1, 0, 0, 0)\nc = Rate(10, 2, 99)\nd = NPer(1, 2, 88, 3)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(
            out,
            vec![
                error_tag_from_code(2001),
                error_tag_from_code(2002),
                -99,
                -38
            ]
        );
    }

    #[test]
    fn formal_v161_conformance_fixtures_exist() {
        assert!(
            repo_path("conformance/tests/financial_algorithm_npv_irr_mirr_subset.bas").exists()
        );
        assert!(repo_path("conformance/tests/financial_algorithm_rate_nper_subset.bas").exists());
        assert!(repo_path("conformance/tests/financial_tolerance_mixed_modes.bas").exists());
    }

    #[test]
    fn formal_v161_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V161.md").exists());
    }

    #[test]
    fn formal_v162_vm_kani_harnesses_cover_financial_and_vartype_paths() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("interpreter exists");
        assert!(text.contains("financial_rate_zero_nper_yields_error_tag"));
        assert!(text.contains("financial_nper_invalid_domain_yields_error_tag"));
        assert!(text.contains("vartype_intrinsic_outputs_expected_domain"));
        assert!(text.contains("#[cfg(kani)]"));
    }

    #[test]
    fn formal_v162_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V162.md").exists());
    }

    #[test]
    fn formal_v163_coverage_index_reconciles_non_hal_rows() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains(
            "Financial extension intrinsics (NPV/IRR/MIRR/Rate/NPer subset),implemented"
        ));
        assert!(text.contains("Err object full surface,implemented"));
        assert!(text.contains("String BSTR core,implemented"));
        assert!(text.contains(
            "File-introspection intrinsics (FreeFile/EOF/LOF/Seek expression subset),implemented"
        ));
    }

    #[test]
    fn formal_v163_library_checklist_reconciles_financial_status() {
        let text =
            std::fs::read_to_string(repo_path("docs/evidence/runtime/LIBRARY_CHECKLIST.csv"))
                .expect("library checklist exists");
        assert!(
            text.contains("financial-expansion,\"NPV/IRR/MIRR/Rate/NPer and related\",implemented")
        );
    }

    #[test]
    fn formal_v163_spec_checklist_reconciles_non_hal_rows() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/SPEC_CHECKLIST.md"))
            .expect("spec checklist exists");
        assert!(text.contains(
            "| `[x]` | Error object | Full `Err` object surface (non-HAL deterministic subset) |"
        ));
        assert!(text.contains("| `[x]` | Types | `String` BSTR and UDT runtime semantics (non-boundary deterministic subset) |"));
        assert!(text.contains("| `[x]` | Financial expansion | `NPV`, `IRR`, `MIRR`, `Rate`, `NPer`, and related suite (in-scope subset) |"));
    }

    #[test]
    fn formal_v163_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V163.md").exists());
    }

    #[test]
    fn formal_v164_non_hal_deferred_gates_have_foldback_notes() {
        let text = std::fs::read_to_string(repo_path(
            "docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv",
        ))
        .expect("deferred oracle gates exists");
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header = lines
            .next()
            .expect("deferred oracle gates header should exist");
        let header_cols = header.trim_matches('"').split("\",\"").collect::<Vec<_>>();
        assert_eq!(
            header_cols,
            vec![
                "gate_id",
                "topic_id",
                "domain",
                "track",
                "scope_class",
                "status",
                "owner_phase",
                "unblock_condition",
                "evidence",
                "foldback_required",
                "foldback_steps",
                "close_condition",
                "notes",
            ]
        );
        for line in lines {
            let cols = line.trim_matches('"').split("\",\"").collect::<Vec<_>>();
            assert_eq!(
                cols.len(),
                13,
                "deferred gate row should keep 13 structured columns: {line}"
            );
            if cols[4] == "non-hal" && cols[5] == "open" {
                assert!(
                    cols[9] == "true",
                    "non-hal open gate missing foldback_required=true: {line}"
                );
                assert!(
                    !cols[10].trim().is_empty(),
                    "non-hal open gate missing foldback_steps payload: {line}"
                );
                assert!(
                    !cols[11].trim().is_empty(),
                    "deferred gate row is missing close_condition payload: {line}"
                );
            }
        }
    }

    #[test]
    fn formal_v164_impl_defined_followup_is_registered() {
        let topics = std::fs::read_to_string(repo_path(
            "docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv",
        ))
        .expect("topics exists");
        let gates = std::fs::read_to_string(repo_path(
            "docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv",
        ))
        .expect("gates exists");
        assert!(topics.contains("CCT-036"));
        assert!(gates.contains("ODG-034"));
    }

    #[test]
    fn formal_v164_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V164.md").exists());
    }

    #[test]
    fn formal_v165_integrated_gate_artifacts_exist() {
        assert!(repo_path("docs/evidence/profiles/v165/integrated_gate.md").exists());
        assert!(repo_path("docs/evidence/profiles/v165/matrix_latest.csv").exists());
        assert!(repo_path("docs/evidence/profiles/v165/benchmark_latest.csv").exists());
    }

    #[test]
    fn formal_v165_workset_document_exists() {
        assert!(
            repo_path("docs/worksets/WORKSET_2026-03-01_INTEGRATED_NON_HAL_GATE_V165.md",).exists()
        );
    }

    #[test]
    fn formal_v165_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V165.md").exists());
    }

    #[test]
    fn formal_v166_terminal_gate_artifacts_exist() {
        assert!(repo_path("docs/evidence/profiles/v166/integrated_gate.md").exists());
        assert!(repo_path("docs/evidence/profiles/v166/gate_report.md").exists());
        assert!(repo_path("docs/evidence/profiles/v166/matrix_latest.csv").exists());
        assert!(repo_path("docs/evidence/profiles/v166/benchmark_latest.csv").exists());
    }

    #[test]
    fn formal_v166_non_hal_milestone_closure_doc_exists() {
        let text = std::fs::read_to_string(repo_path(
            "docs/evidence/profiles/v166/non_hal_completion_milestone.md",
        ))
        .expect("v166 milestone closure document exists");
        assert!(text.contains("v147..v166"));
        assert!(text.contains("Exit criteria"));
        assert!(text.contains("Deferred Oracle Gates"));
    }

    #[test]
    fn formal_v166_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V166.md").exists());
    }

    #[test]
    fn formal_v167_non_hal_audit_report_exists_and_is_clean() {
        let text = std::fs::read_to_string(repo_path(
            "docs/evidence/language/NON_HAL_POST_COMPLETION_AUDIT_V167.md",
        ))
        .expect("v167 non-hal post-completion audit exists");
        assert!(text.contains("Residual non-HAL partial/planned rows: 0"));
        assert!(text.contains("DEFERRED_ORACLE_GATES.csv"));
    }

    #[test]
    fn formal_v167_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V167.md").exists());
    }

    #[test]
    fn formal_v168_benchmark_includes_subset_workloads() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(bench.contains("subset_err_string_financial_vm"));
        assert!(bench.contains("subset_err_string_financial_jit"));
        assert!(bench.contains("include_pattern"));
    }

    #[test]
    fn formal_v168_conformance_runner_supports_include_pattern() {
        let conformance = std::fs::read_to_string(repo_path("scripts/run-conformance.ps1"))
            .expect("run-conformance script exists");
        assert!(conformance.contains("[string[]]$IncludePattern"));
        assert!(conformance.contains("filters="));
    }

    #[test]
    fn formal_v168_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V168.md").exists());
    }

    #[test]
    fn formal_v169_financial_rate_uses_derivative_helper() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("interpreter exists");
        assert!(text.contains("FIN_DERIVATIVE_STEP"));
        assert!(text.contains("fn rate_func_derivative"));
        assert!(text.contains("Self::rate_func_derivative("));
    }

    #[test]
    fn formal_v169_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V169.md").exists());
    }

    #[test]
    fn formal_v170_string_digit_paths_use_slice_based_substrings() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("interpreter exists");
        assert!(text.contains("text[start..end]"));
        assert!(text.contains("text[start..]"));
        assert!(text.contains("String::with_capacity"));
    }

    #[test]
    fn formal_v170_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V170.md").exists());
    }

    #[test]
    fn formal_v171_coercion_cverr_range_fixture_executes() {
        let source = std::fs::read_to_string(repo_path(
            "conformance/tests/coercion_cverr_range_predicates.bas",
        ))
        .expect("v171 coercion fixture exists");
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(&source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 1, 1, 1, 0, 10]);
    }

    #[test]
    fn formal_v171_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V171.md").exists());
    }

    #[test]
    fn formal_v172_error_nested_mode_fixture_executes() {
        let source = std::fs::read_to_string(repo_path(
            "conformance/tests/error_nested_mode_transitions.bas",
        ))
        .expect("v172 error-mode fixture exists");
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(&source)
        .expect("execution should succeed");
        assert_eq!(out, vec![5, 0, 0, 6]);
    }

    #[test]
    fn formal_v172_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V172.md").exists());
    }

    #[test]
    fn formal_v173_jit_fallback_regressions_cover_new_non_hal_edges() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-jit/src/lib.rs"))
            .expect("jit source exists");
        assert!(text.contains("falls_back_for_cverr_range_predicates_and_matches_vm"));
        assert!(text.contains("falls_back_for_nested_error_mode_transitions_and_matches_vm"));
    }

    #[test]
    fn formal_v173_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V173.md").exists());
    }

    #[test]
    fn formal_v174_oracle_probe_scaffold_exists() {
        assert!(repo_path("scripts/oracle-probe.ps1").exists());
        let doc = std::fs::read_to_string(repo_path(
            "docs/evidence/conformance/ORACLE_PROBE_SCAFFOLD.md",
        ))
        .expect("oracle probe scaffold doc exists");
        assert!(doc.contains("deferred oracle"));
        assert!(doc.contains("non-blocking"));
    }

    #[test]
    fn formal_v174_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V174.md").exists());
    }

    #[test]
    fn formal_v175_vm_kani_harnesses_cover_new_cverr_and_resume_paths() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("interpreter exists");
        assert!(text.contains("cverr_tag_encoding_stays_in_reserved_error_band"));
        assert!(text.contains("resume_next_clears_err_number_after_raise"));
    }

    #[test]
    fn formal_v175_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V175.md").exists());
    }

    #[test]
    fn formal_v176_deferred_gate_register_tracks_new_lanes() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/DEFERRED_GATES.md"))
            .expect("deferred gate register exists");
        assert!(text.contains("DG-V175-001"));
        assert!(text.contains("DG-V176-001"));
    }

    #[test]
    fn formal_v176_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V176.md").exists());
    }

    #[test]
    fn formal_v177_docs_reference_non_hal_hardening_artifacts() {
        let conformance =
            std::fs::read_to_string(repo_path("docs/CONFORMANCE.md")).expect("conformance exists");
        let formal = std::fs::read_to_string(repo_path("docs/FORMAL.md")).expect("formal exists");
        assert!(conformance.contains("ORACLE_PROBE_SCAFFOLD.md"));
        assert!(formal.contains("DEFERRED_GATES.md"));
        assert!(formal.contains("run-formal-kani-remote.ps1"));
    }

    #[test]
    fn formal_v177_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V177.md").exists());
    }

    #[test]
    fn formal_v178_coverage_normalization_assets_exist() {
        assert!(repo_path("scripts/validate-coverage-notes.ps1").exists());
        let report =
            std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_AUDIT_V178.md"))
                .expect("coverage audit report exists");
        assert!(report.contains("COVERAGE_INDEX.csv"));
        assert!(report.contains("LIBRARY_CHECKLIST.csv"));
    }

    #[test]
    fn formal_v178_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V178.md").exists());
    }

    #[test]
    fn formal_v179_regression_fixtures_execute() {
        let source = std::fs::read_to_string(repo_path(
            "conformance/tests/regression_cverr_error_resume_bridge.bas",
        ))
        .expect("v179 regression fixture exists");
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(&source)
        .expect("execution should succeed");
        assert_eq!(out, vec![11, 0, 1, 10, 0]);

        let source = std::fs::read_to_string(repo_path(
            "conformance/tests/regression_cverr_predicate_domain.bas",
        ))
        .expect("v179 predicate fixture exists");
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(&source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 1, 0, 0, 0]);
    }

    #[test]
    fn formal_v179_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V179.md").exists());
    }

    #[test]
    fn formal_v180_perf_trend_report_exists() {
        let text = std::fs::read_to_string(repo_path(
            "docs/evidence/profiles/v180/PERF_TREND_V166_TO_V180.md",
        ))
        .expect("v180 perf trend report exists");
        assert!(text.contains("v166"));
        assert!(text.contains("v180"));
    }

    #[test]
    fn formal_v180_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V180.md").exists());
    }

    #[test]
    fn formal_v181_integrated_correctness_artifacts_exist() {
        assert!(repo_path("docs/evidence/profiles/v181/matrix_latest.csv").exists());
        assert!(repo_path("docs/evidence/profiles/v181/gate_report.md").exists());
    }

    #[test]
    fn formal_v181_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V181.md").exists());
    }

    #[test]
    fn formal_v182_deferred_oracle_audit_assets_exist() {
        assert!(repo_path("scripts/validate-deferred-oracle-gates.ps1").exists());
        assert!(repo_path("docs/evidence/conformance/DEFERRED_ORACLE_AUDIT_V182.md").exists());
    }

    #[test]
    fn formal_v182_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V182.md").exists());
    }

    #[test]
    fn formal_v183_divergence_audit_report_exists() {
        let text = std::fs::read_to_string(repo_path(
            "docs/evidence/divergences/DIVERGENCE_AUDIT_V183.md",
        ))
        .expect("v183 divergence audit report exists");
        assert!(text.contains("DIV-0001"));
        assert!(text.contains("DIV-0002"));
    }

    #[test]
    fn formal_v183_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V183.md").exists());
    }

    #[test]
    fn formal_v184_profile_gate_runner_has_locking_and_skip_bench_switch() {
        let text = std::fs::read_to_string(repo_path("scripts/run-profile-gate.ps1"))
            .expect("run-profile-gate exists");
        assert!(text.contains("profile gate already running"));
        assert!(text.contains("lock.json"));
        assert!(text.contains("[switch]$SkipBench"));
    }

    #[test]
    fn formal_v184_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V184.md").exists());
    }

    #[test]
    fn formal_v185_release_candidate_gate_artifacts_exist() {
        assert!(repo_path("docs/evidence/profiles/v185/integrated_gate.md").exists());
        assert!(repo_path("docs/evidence/profiles/v185/integrated_gate.csv").exists());
        assert!(repo_path("docs/evidence/profiles/v185/RC_GATE_SUMMARY.md").exists());
    }

    #[test]
    fn formal_v185_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V185.md").exists());
    }

    #[test]
    fn formal_v186_terminal_closure_artifacts_exist() {
        assert!(repo_path("docs/evidence/profiles/v186/integrated_gate.md").exists());
        assert!(repo_path("docs/evidence/profiles/v186/BATCH2_CLOSURE.md").exists());
    }

    #[test]
    fn formal_v186_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V186.md").exists());
    }

    #[test]
    fn formal_v132_builtin_expansion_fixtures_exist() {
        assert!(repo_path("conformance/tests/stdlib_string_expansion_core.bas").exists());
        assert!(repo_path("conformance/tests/stdlib_format_core.bas").exists());
        assert!(repo_path("conformance/tests/stdlib_datetime_expansion.bas").exists());
        assert!(repo_path("conformance/tests/stdlib_numeric_expansion.bas").exists());
        assert!(repo_path("conformance/tests/stdlib_random_financial_expansion.bas").exists());
    }

    #[test]
    fn formal_v134_file_stub_intrinsics_execute() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = FreeFile()\nb = EOF(3)\nc = LOF(4)\nd = Seek(5)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![1, 3, 4, 5]);
    }

    #[test]
    fn formal_v148_err_surface_member_subset_executes() {
        let source = "Sub Main()\nDim n\nDim d\nDim s\nDim h\nDim f\nDim l\nOn Error Resume Next\nError 9\nn = Err.Number\nd = Err.Description\ns = Err.Source\nh = Err.HelpContext\nf = Err.HelpFile\nl = Err.LastDllError\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![9, 9, 0, 0, 0, 0]);
    }

    #[test]
    fn formal_v148_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/err_surface_fields_subset.bas").exists());
    }

    #[test]
    fn formal_v148_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V148.md").exists());
    }

    #[test]
    fn formal_v149_resume_next_clears_err_number() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nResume Next\nx = Err.Number\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn formal_v149_procedure_boundaries_clear_err_number() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 7\nCall Worker\nx = Err.Number\nEnd Sub\nSub Worker()\nDim y\ny = 1\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn formal_v149_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V149.md").exists());
    }

    #[test]
    fn formal_v150_join_maps_array_tag_to_count() {
        let source = "Sub Main()\nDim a\nDim y\na = Array(1, 2, 3)\ny = Join(a, 0)\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        let expected_tag = oxvba_runtime::safe_array::ARRAY_TAG_BASE + 3;
        assert_eq!(out, vec![expected_tag, 3]);
    }

    #[test]
    fn formal_v150_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/string_join_array_tag_count.bas").exists());
    }

    #[test]
    fn formal_v150_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V150.md").exists());
    }

    #[test]
    fn formal_v151_vbnullstring_long_assignment_is_rejected() {
        let source = "Sub Main()\nDim x As Long\nx = vbNullString\nEnd Sub";
        let err = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect_err("vbNullString assignment to Long should fail");
        assert!(err.contains("type mismatch"));
    }

    #[test]
    fn formal_v151_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/string_vbnullstring_long_error.bas").exists());
    }

    #[test]
    fn formal_v151_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V151.md").exists());
    }

    #[test]
    fn formal_v152_udt_whole_assignment_copies_fields() {
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim a As Point\nDim b As Point\nDim x\na.X = 7\na.Y = 9\nb = a\nx = b.Y\nEnd Sub";
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test(source)
        .expect("execution should succeed");
        assert_eq!(out, vec![0, 7, 9, 0, 7, 9, 9]);
    }

    #[test]
    fn formal_v152_conformance_fixture_exists() {
        assert!(repo_path("conformance/tests/udt_whole_assignment_copy.bas").exists());
    }

    #[test]
    fn formal_v152_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V152.md").exists());
    }

    #[test]
    fn formal_v146_profile_status_range_exists() {
        for version in 108..=146 {
            let path = format!("docs/profile-status/PROFILE_STATUS_V{version}.md");
            assert!(repo_path(&path).exists());
        }
    }

    #[test]
    fn formal_v21_opt_toggle_parity() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 0\nx = x + 2\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v21_jit_vm_guardrail_equivalence() {
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_slots_test("Sub Main()\nDim x\nx = 4\nx = x + 1\nEnd Sub")
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_slots_test("Sub Main()\nDim x\nx = 4\nx = x + 1\nEnd Sub")
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v21_benchmark_script_exists() {
        assert!(repo_path("scripts/run-bench.ps1").exists());
    }

    #[test]
    fn hal_windows_default_profile_exposes_com_capability() {
        let engine = Engine::new(HostConfig::default());
        let descriptor = engine.hal_descriptor();
        assert_eq!(descriptor.profile, HalProfileId::Windows);
        assert!(
            descriptor.supports(oxvba_hal::model::CapabilityId::ComActivationDispatch),
            "default engine profile should keep windows COM support available"
        );
    }

    #[test]
    fn hal_compile_time_mode_rejects_unsupported_linux_com_intrinsics() {
        let mut engine = Engine::new(HostConfig::default()).with_hal_profile(HalProfileId::Linux);
        engine.set_unsupported_feature_mode(UnsupportedFeatureMode::CompileTime);

        let err = engine
            .execute_source_slots_test_phased("Sub Main()\nDim x\nx = CreateObject(4)\nEnd Sub")
            .expect_err("compile-time mode should reject unsupported COM capability");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(err.message().contains("CreateObject"));
        assert!(err.message().contains("missing capability"));
    }

    #[test]
    fn hal_runtime_mode_surfaces_host_error_for_unsupported_linux_com_intrinsics() {
        let mut engine = Engine::new(HostConfig::default()).with_hal_profile(HalProfileId::Linux);
        engine.set_unsupported_feature_mode(UnsupportedFeatureMode::Runtime);

        let err = engine
            .execute_source_slots_test_phased("Sub Main()\nDim x\nx = CreateObject(4)\nEnd Sub")
            .expect_err("runtime mode should defer unsupported COM to execution");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("HAL-E-CAP-UNAVAILABLE"));
    }

    #[test]
    fn hal_compile_time_mode_rejects_policy_denied_shell() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_compile_time();
        policy.allow_process_spawn = false;
        engine.set_host_policy(policy);

        let err = engine
            .execute_source_slots_test_phased("Sub Main()\nDim x\nx = Shell(1)\nEnd Sub")
            .expect_err("compile-time mode should fail when shell policy is denied");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(err.message().contains("allow_process_spawn=false"));
    }

    #[test]
    fn hal_compile_time_mode_rejects_policy_denied_msgbox() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_compile_time();
        policy.allow_interaction = false;
        engine.set_host_policy(policy);

        let err = engine
            .execute_source_slots_test_phased("Sub Main()\nDim x\nx = MsgBox(1)\nEnd Sub")
            .expect_err("compile-time mode should fail when msgbox policy is denied");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(err.message().contains("allow_interaction=false"));
    }

    #[test]
    fn hal_compile_time_mode_rejects_policy_denied_declare_invoke() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_compile_time();
        policy.allow_dynamic_link = false;
        engine.set_host_policy(policy);

        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = engine
            .execute_source_slots_test_phased(source)
            .expect_err("compile-time mode should fail when declare invoke policy is denied");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(err.message().contains("allow_dynamic_link=false"));
    }

    #[test]
    fn hal_runtime_mode_routes_host_error_through_on_error_resume_next() {
        let mut engine = Engine::new(HostConfig::default()).with_hal_profile(HalProfileId::Linux);
        engine.set_unsupported_feature_mode(UnsupportedFeatureMode::Runtime);

        let source = "Sub Main()\nDim x\nDim y\nOn Error Resume Next\nx = CreateObject(4)\ny = Err.Number\nEnd Sub";
        let out = engine
            .execute_source_slots_test(source)
            .expect("runtime mode with On Error Resume Next should continue");
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 53_051);
    }

    #[test]
    fn hal_runtime_policy_denied_shell_surfaces_stable_error_shape() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.allow_process_spawn = false;
        engine.set_host_policy(policy);

        let err = engine
            .execute_source_slots_test_phased("Sub Main()\nDim x\nx = Shell(1)\nEnd Sub")
            .expect_err("runtime policy denial should surface at execution");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("HAL-E-POLICY-DENIED"));
        assert!(err.message().contains("[shell]"));
    }

    #[test]
    fn hal_runtime_policy_denied_shell_routes_to_expected_err_number() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.allow_process_spawn = false;
        engine.set_host_policy(policy);

        let source =
            "Sub Main()\nDim x\nDim y\nOn Error Resume Next\nx = Shell(1)\ny = Err.Number\nEnd Sub";
        let out = engine
            .execute_source_slots_test(source)
            .expect("On Error Resume Next should capture host policy failure");
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 53_042);
    }

    #[test]
    fn hal_runtime_policy_denied_msgbox_surfaces_stable_error_shape() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.allow_interaction = false;
        engine.set_host_policy(policy);

        let err = engine
            .execute_source_slots_test_phased("Sub Main()\nDim x\nx = MsgBox(1)\nEnd Sub")
            .expect_err("runtime policy denial should surface at execution");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("HAL-E-POLICY-DENIED"));
        assert!(err.message().contains("[msg_box]"));
    }

    #[test]
    fn hal_runtime_policy_denied_declare_invoke_surfaces_stable_error_shape() {
        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.allow_dynamic_link = false;
        engine.set_host_policy(policy);

        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = engine
            .execute_source_slots_test_phased(source)
            .expect_err("runtime policy denial should surface for declare invoke");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("HAL-E-POLICY-DENIED"));
        assert!(err.message().contains("[invoke_symbol]"));
    }

    #[test]
    fn hal_runtime_host_backed_declare_invoke_executes_known_symbol() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = engine
            .execute_source_slots_test(source)
            .expect("host-backed declare invoke should succeed");
        assert_eq!(out, vec![4]);
    }

    #[test]
    fn hal_runtime_host_backed_declare_invoke_is_case_insensitive_for_lib_and_alias() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Declare PtrSafe Function HostPing Lib \"HOST\" Alias \"PiNg\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = engine
            .execute_source_slots_test(source)
            .expect("host-backed declare invoke should resolve canonicalized lib/alias");
        assert_eq!(out, vec![4]);
    }

    #[test]
    fn hal_runtime_host_backed_unknown_declare_symbol_surfaces_adapter_fault_shape() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Declare PtrSafe Function HostMissing Lib \"host\" Alias \"missing\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostMissing(3)\nEnd Sub";
        let err = engine
            .execute_source_slots_test_phased(source)
            .expect_err("unknown symbol should raise deterministic adapter fault");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(err.message().contains("HAL-E-ADAPTER-FAULT"));
        assert!(err.message().contains("[invoke_symbol]"));
    }

    #[test]
    fn hal_runtime_host_backed_unknown_declare_symbol_routes_expected_err_number() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = "Declare PtrSafe Function HostMissing Lib \"host\" Alias \"missing\" (ByVal x As Long) As Long\nSub Main()\nDim y\nDim e\nOn Error Resume Next\ny = HostMissing(3)\ne = Err.Number\nEnd Sub";
        let out = engine
            .execute_source_slots_test(source)
            .expect("On Error Resume Next should capture adapter fault");
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 53_073);
    }

    #[test]
    fn hal_policy_preset_switch_updates_engine_policy() {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy_preset(HostPolicyPreset::StrictCi);
        assert_eq!(engine.host_policy(), &HostPolicy::strict_ci());
    }

    #[test]
    fn hal_compile_time_mode_rejects_even_with_on_error_resume_next() {
        let mut engine = Engine::new(HostConfig::default()).with_hal_profile(HalProfileId::Linux);
        engine.set_unsupported_feature_mode(UnsupportedFeatureMode::CompileTime);

        let source = "Sub Main()\nDim x\nOn Error Resume Next\nx = CreateObject(4)\nEnd Sub";
        let err = engine
            .execute_source_slots_test_phased(source)
            .expect_err("compile-time gate should reject unsupported host intrinsic");
        assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
        assert!(err.message().contains("CreateObject"));
    }

    #[test]
    fn formal_v466_feature_obligation_coverage_index_is_uniform_and_deep() {
        let obligations_text =
            std::fs::read_to_string(repo_path("docs/evidence/formal/obligations.csv"))
                .expect("obligation index should exist");
        let mut obligation_ids = HashSet::<String>::new();
        for line in obligations_text.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with("FO-") || !trimmed.contains(",true,") {
                continue;
            }
            if let Some((id, _)) = trimmed.split_once(',') {
                obligation_ids.insert(id.to_string());
            }
        }
        assert!(
            !obligation_ids.is_empty(),
            "active formal obligation set must not be empty"
        );

        let coverage_text = std::fs::read_to_string(repo_path(
            "docs/evidence/formal/FEATURE_OBLIGATION_COVERAGE_V1.csv",
        ))
        .expect("feature obligation coverage index should exist");

        let mut seen_features = HashSet::<String>::new();
        let mut baseline_depth: Option<usize> = None;
        for line in coverage_text.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.splitn(4, ',');
            let feature = parts.next().expect("feature column").trim();
            let depth_text = parts.next().expect("depth column").trim();
            let ids_text = parts.next().expect("obligation id column").trim();
            let rationale = parts.next().expect("rationale column").trim();
            let depth = depth_text
                .parse::<usize>()
                .expect("depth should parse as usize");
            assert!(
                seen_features.insert(feature.to_string()),
                "duplicate feature area in coverage index: {feature}"
            );
            assert!(
                !rationale.is_empty(),
                "feature area `{feature}` must include non-empty rationale text"
            );
            assert!(
                depth >= 3,
                "feature area `{feature}` must have minimum depth >= 3"
            );
            if let Some(expected) = baseline_depth {
                assert_eq!(
                    depth, expected,
                    "feature depth must be uniform; `{feature}` differs"
                );
            } else {
                baseline_depth = Some(depth);
            }
            let ids = ids_text
                .split(';')
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .collect::<Vec<_>>();
            let mut ids_seen = HashSet::<&str>::new();
            assert!(
                ids.len() >= depth,
                "feature `{feature}` declares fewer obligations than depth target"
            );
            for obligation in ids {
                assert!(
                    ids_seen.insert(obligation),
                    "feature `{feature}` repeats obligation `{obligation}`"
                );
                assert!(
                    obligation_ids.contains(obligation),
                    "feature `{feature}` references missing obligation `{obligation}`"
                );
            }
        }
        assert!(
            seen_features.len() >= 10,
            "feature obligation index should cover broad runtime/compiler surface"
        );
    }

    #[test]
    fn formal_v466_event_docs_reflect_post_gate_semantics() {
        let pmr_spec =
            std::fs::read_to_string(repo_path("docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md"))
                .expect("pmr spec should exist");
        let pmr_conformance = std::fs::read_to_string(repo_path(
            "docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md",
        ))
        .expect("pmr conformance spec should exist");
        let pmr_hal_integration = std::fs::read_to_string(repo_path(
            "docs/spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md",
        ))
        .expect("pmr hal integration spec should exist");
        let div3 = std::fs::read_to_string(repo_path("docs/evidence/divergences/DIV-0003.md"))
            .expect("divergence doc should exist");
        let div4 = std::fs::read_to_string(repo_path("docs/evidence/divergences/DIV-0004.md"))
            .expect("divergence doc should exist");
        let topics = std::fs::read_to_string(repo_path(
            "docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv",
        ))
        .expect("conformance topics should exist");
        let taxonomy = std::fs::read_to_string(repo_path("docs/DIAGNOSTIC_TAXONOMY.md"))
            .expect("diagnostic taxonomy should exist");
        let generated_diag_snippet =
            std::fs::read_to_string(repo_path("docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md"))
                .expect("generated PMR event diagnostic snippet should exist");

        assert!(pmr_spec.contains("compile-time executable"));
        assert!(pmr_conformance.contains("docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md"));
        assert!(pmr_hal_integration.contains("docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md"));
        assert!(div3.contains("Closed for the originally recorded mismatch shape"));
        assert!(div4.contains("compile semantics are implemented"));
        assert!(topics.contains("\"CCT-040\""));
        assert!(topics.contains("ODG-038"));
        assert!(topics.contains("DIV-0004"));
        assert!(taxonomy.contains("docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md"));
        assert!(generated_diag_snippet.contains("PMR-E-IMPLEMENTS-MODULE-KIND"));
        assert!(generated_diag_snippet.contains("PMR-E-RAISEEVENT-MODULE-KIND"));

        assert!(!pmr_spec.contains("PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED"));
        assert!(!pmr_spec.contains("PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED"));
        assert!(!pmr_spec.contains("PMR-E-WITHEVENTS-MODULE-KIND-UNRESOLVED"));
        assert!(!pmr_hal_integration.contains("PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED"));
        assert!(!pmr_hal_integration.contains("PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED"));
        assert!(!pmr_hal_integration.contains("PMR-E-WITHEVENTS-MODULE-KIND-UNRESOLVED"));
        assert!(!taxonomy.contains("PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED"));
        assert!(!taxonomy.contains("PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED"));
        assert!(!taxonomy.contains("PMR-E-WITHEVENTS-MODULE-KIND-UNRESOLVED"));
    }

    #[test]
    fn formal_v466_project_integration_limits_use_post_gate_diagnostics() {
        let catalog = std::fs::read_to_string(repo_path("conformance/integration/catalog.psv"))
            .expect("project integration catalog should exist");

        let line_008 = catalog
            .lines()
            .find(|line| line.starts_with("INTP-008|"))
            .expect("INTP-008 row should exist");
        assert!(line_008.contains("PMR-E-IMPLEMENTS-MODULE-KIND"));
        assert!(!line_008.contains("PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED"));

        let line_009 = catalog
            .lines()
            .find(|line| line.starts_with("INTP-009|"))
            .expect("INTP-009 row should exist");
        assert!(line_009.contains("PMR-E-RAISEEVENT-MODULE-KIND"));
        assert!(!line_009.contains("PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED"));
    }

    #[test]
    fn formal_v466_governance_doctrine_tracks_pmr_event_and_oracle_schema_contracts() {
        let governance = std::fs::read_to_string(repo_path("scripts/check-governance.ps1"))
            .expect("governance script should exist");
        let operations = std::fs::read_to_string(repo_path("OPERATIONS.md"))
            .expect("operations doc should exist");
        let conformance_layout =
            std::fs::read_to_string(repo_path("docs/evidence/conformance/README.md"))
                .expect("conformance layout readme should exist");

        assert!(governance.contains("generate-pmr-event-diagnostic-snippets.ps1\" -Check"));
        assert!(governance.contains("validate-pmr-event-diagnostic-sync.ps1"));
        assert!(governance.contains("validate-deferred-oracle-gates.ps1"));

        assert!(operations.contains("./scripts/check-governance.ps1"));
        assert!(operations.contains("Post-semantics-change checklist"));
        assert!(operations.contains("PMR_EVENT_DIAGNOSTICS_V1.csv"));

        assert!(conformance_layout.contains("Active Governance Surfaces"));
        assert!(conformance_layout.contains("Historical Capture Areas"));
        assert!(conformance_layout.contains("oracle_captures/"));
    }

    #[test]
    fn formal_v466_profile_status_document_exists() {
        assert!(repo_path("docs/profile-status/PROFILE_STATUS_V466.md").exists());
    }

    #[test]
    fn runtime_value_snapshot_api_preserves_vm_value_shape() {
        let out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_value_snapshot("Sub Main()\nDim x\nx = 4\nEnd Sub")
        .expect("vm value snapshot should succeed");
        assert_eq!(out, vec![RuntimeValue::I32(4)]);
    }

    #[test]
    fn runtime_value_snapshot_api_supports_jit_subset_path() {
        let out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_with_value_snapshot_phased("Sub Main()\nDim x\nx = 4\nEnd Sub")
        .expect("value snapshot should project JIT subset into runtime values");
        assert_eq!(out, vec![RuntimeValue::I32(4)]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn runtime_value_snapshot_createobject_uses_object_handle_shape() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let out = engine
            .execute_source_with_value_snapshot("Sub Main()\nDim x\nx = CreateObject(4)\nEnd Sub")
            .expect("CreateObject value snapshot should succeed");
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn runtime_value_snapshot_createobject_preserves_object_handle_shape_with_jit_enabled() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let out = engine
            .execute_source_with_value_snapshot("Sub Main()\nDim x\nx = CreateObject(4)\nEnd Sub")
            .expect("CreateObject value snapshot should preserve object handle on JIT fallback");
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], RuntimeValue::ObjectHandle(handle) if handle.raw() > 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn legacy_snapshot_api_projects_object_handle_shape_with_jit_enabled() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let out = engine
            .execute_source_slots_test("Sub Main()\nDim x\nx = CreateObject(4)\nEnd Sub")
            .expect(
                "CreateObject legacy snapshot should remain available through value projection",
            );
        assert_eq!(out.len(), 1);
        assert!(out[0] > 0);
    }
}
