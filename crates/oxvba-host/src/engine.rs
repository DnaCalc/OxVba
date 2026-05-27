use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use oxvba_com::{
    ComCallbackToken, ComMemberToken, ComObjectDescriptor, ComSubscriptionToken,
    DynamicEventPayload, DynamicObjectBridge,
};
use oxvba_compiler::{
    Bytecode, CompiledProject, Instruction, ProcedureRuntimeMetadata, ProjectDynamicMemberKind,
    ProjectDynamicMemberRoute, ProjectManifest, compile_project, compile_with_runtime_metadata,
};
use oxvba_hal::{
    HalComDynamicBridge,
    adapters::builder::HostBuilder,
    callbacks::HostCallbacks,
    model::{
        CapabilityId, HalDescriptor, HalProfileId, HostPolicy, HostPolicyPreset,
        UnsupportedFeatureMode, native_host_profile,
    },
    traits::HostServices,
};
#[cfg(feature = "jit")]
use oxvba_jit::JIT_NOT_IMPLEMENTED_MESSAGE;
use oxvba_runtime::{
    ObjectRef, RuntimeCallArgument, RuntimeCallContext, RuntimeCallFrame, RuntimeCallKind,
    RuntimeCallResult, RuntimeCallSelector, RuntimeCallSource, RuntimeInterfaceId, Variant,
};
use oxvba_vm::{Vm, VmExecutionPackage, VmPackageIdentityEvidence, VmPackageOrigin};

use crate::{
    direct_host::{DirectHostIssue, DirectHostIssueKind},
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

fn runtime_call_kind_for_project_member(kind: ProjectDynamicMemberKind) -> RuntimeCallKind {
    match kind {
        ProjectDynamicMemberKind::Method | ProjectDynamicMemberKind::Function => {
            RuntimeCallKind::Method
        }
        ProjectDynamicMemberKind::PropertyGet => RuntimeCallKind::PropertyGet,
        ProjectDynamicMemberKind::PropertyLet => RuntimeCallKind::PropertyLet,
        ProjectDynamicMemberKind::PropertySet => RuntimeCallKind::PropertySet,
    }
}

impl PhaseDiagnostic {
    pub(crate) fn compile(message: impl Into<String>) -> Self {
        Self {
            phase: DiagnosticPhase::CompileTime,
            message: message.into(),
        }
    }

    pub(crate) fn runtime(message: impl Into<String>) -> Self {
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

    pub fn direct_host_issue_kind(&self) -> DirectHostIssueKind {
        match self.phase {
            DiagnosticPhase::CompileTime => DirectHostIssueKind::BuildFailed,
            DiagnosticPhase::Runtime => DirectHostIssueKind::RuntimeStartupFailed,
        }
    }

    pub fn direct_host_issue(&self) -> DirectHostIssue {
        DirectHostIssue::new(self.direct_host_issue_kind()).with_technical_detail(self.to_string())
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
}

pub struct Engine {
    config: HostConfig,
    event_dispatcher: Mutex<EventDispatcher>,
    com_subscription_handlers: Mutex<HashMap<ComSubscriptionToken, String>>,
    runtime_profile: RuntimeProfileId,
    host_callbacks: Option<Arc<dyn HostCallbacks>>,
    host_services: Arc<dyn HostServices>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventCallbackVariantDispatch {
    pub callback_token: ComCallbackToken,
    pub subscription_token: ComSubscriptionToken,
    pub object: ObjectRef,
    pub event: ComMemberToken,
    pub handler_symbol: String,
    pub args: Vec<Variant>,
}

pub struct ProjectRuntimeSession {
    compiled: CompiledProject,
    vm: Vm,
    package_origin: VmPackageOrigin,
}

impl ProjectRuntimeSession {
    pub fn project_reflection(&self) -> &oxvba_compiler::ProjectReflection {
        &self.compiled.project_reflection
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostVariantSnapshotWithPackageIdentity {
    pub values: Vec<Variant>,
    pub package_identity: VmPackageIdentityEvidence,
}

const STARTUP_ENTRY_SHIM_MODULE_PREFIX: &str = "__OxVbaStartupEntryShim";

fn entry_procedure_runtime_metadata(
    metadata: &std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Option<&ProcedureRuntimeMetadata> {
    metadata
        .values()
        .find(|metadata| {
            metadata.procedure_name.eq_ignore_ascii_case("main")
                && !is_startup_entry_shim_module_name(&metadata.module_name)
        })
        .or_else(|| {
            metadata.values().find(|metadata| {
                metadata.entry_pc == 0 && !is_startup_entry_shim_module_name(&metadata.module_name)
            })
        })
        .or_else(|| {
            metadata
                .values()
                .find(|metadata| !is_startup_entry_shim_module_name(&metadata.module_name))
        })
        .or_else(|| metadata.values().find(|metadata| metadata.entry_pc == 0))
}

fn project_visible_snapshot<T: Clone>(
    all_slots: &[T],
    metadata: &std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    fallback_count: usize,
) -> Vec<T> {
    if let Some(entry) = entry_procedure_runtime_metadata(metadata) {
        let mut visible = Vec::with_capacity(entry.slots.len());
        for slot in &entry.slots {
            if matches!(
                slot.kind,
                oxvba_compiler::ProcedureRuntimeSlotKind::ReturnValue
                    | oxvba_compiler::ProcedureRuntimeSlotKind::Temporary
            ) {
                continue;
            }
            if let Some(value) = all_slots.get(slot.slot).cloned() {
                visible.push(value);
            }
        }
        return visible;
    }
    all_slots[..fallback_count.min(all_slots.len())].to_vec()
}

fn recorded_package_identity(vm: &Vm) -> Result<VmPackageIdentityEvidence, PhaseDiagnostic> {
    vm.package_identity_evidence()
        .cloned()
        .ok_or_else(|| PhaseDiagnostic::runtime("VM package identity evidence was not recorded"))
}

impl ProjectRuntimeSession {
    /// Retained value-model snapshot for project-visible slots.
    pub fn snapshot_variants(&self) -> Vec<Variant> {
        let all_slots = self.vm.snapshot_variants(self.compiled.bytecode.slot_count);
        project_visible_snapshot(
            &all_slots,
            &self.compiled.procedure_runtime_metadata,
            self.compiled.bytecode.user_slot_count,
        )
    }

    pub fn compiled(&self) -> &CompiledProject {
        &self.compiled
    }

    pub fn package_origin(&self) -> VmPackageOrigin {
        self.package_origin
    }

    pub fn package_identity_evidence(&self) -> Option<&VmPackageIdentityEvidence> {
        self.vm.package_identity_evidence()
    }

    /// Retained value-model slot read.
    pub fn read_variant_slot(&self, slot: usize) -> Variant {
        let values = self.vm.snapshot_variants(slot + 1);
        values.into_iter().nth(slot).unwrap_or_else(Variant::empty)
    }

    pub fn procedure_metadata(
        &self,
    ) -> &std::collections::BTreeMap<String, ProcedureRuntimeMetadata> {
        &self.compiled.procedure_runtime_metadata
    }

    /// Low-level VM access for the external debug core.
    ///
    /// This is intentionally a narrow runtime-preparation seam for
    /// `oxvba-debug`; general hosts should prefer the higher-level host APIs.
    pub fn debug_vm(&self) -> &Vm {
        &self.vm
    }

    /// Mutable low-level VM access for the external debug core.
    ///
    /// This is intentionally a narrow runtime-preparation seam for
    /// `oxvba-debug`; general hosts should prefer the higher-level host APIs.
    pub fn debug_vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(HostConfig::default())
    }
}

fn is_startup_entry_shim_module_name(module_name: &str) -> bool {
    module_name
        .to_ascii_lowercase()
        .starts_with(&STARTUP_ENTRY_SHIM_MODULE_PREFIX.to_ascii_lowercase())
}

fn manifest_without_startup_entry_shims<'a>(
    manifest: &'a ProjectManifest,
) -> Cow<'a, ProjectManifest> {
    if !manifest
        .modules
        .iter()
        .any(|module| is_startup_entry_shim_module_name(&module.module_name))
    {
        return Cow::Borrowed(manifest);
    }

    let mut filtered = manifest.clone();
    filtered
        .modules
        .retain(|module| !is_startup_entry_shim_module_name(&module.module_name));
    Cow::Owned(filtered)
}

fn build_host_services(
    profile: HalProfileId,
    runtime_class: oxvba_hal::model::HalRuntimeClass,
    policy: HostPolicy,
    callbacks: Option<Arc<dyn HostCallbacks>>,
) -> Arc<dyn HostServices> {
    let mut builder = HostBuilder::new()
        .profile(profile)
        .runtime_class(runtime_class)
        .policy(policy);
    if let Some(callbacks) = callbacks {
        builder = builder.callbacks(callbacks);
    }
    builder.build()
}

impl Engine {
    fn invoke_session_package_procedure_with_variants(
        session: &mut ProjectRuntimeSession,
        entry_pc: usize,
        param_slots: &[usize],
        args: &[Variant],
    ) -> Result<(), PhaseDiagnostic> {
        let ProjectRuntimeSession {
            compiled,
            vm,
            package_origin,
        } = session;
        let package = VmExecutionPackage {
            bytecode: &compiled.bytecode,
            procedure_metadata: &compiled.procedure_runtime_metadata,
            package_origin: *package_origin,
            project_context: None,
            dynamic_object_routes: Some(&compiled.project_dynamic_objects),
            com_withevents_routes: Some(&compiled.project_com_withevents_routes),
        };
        vm.invoke_package_procedure_with_variants(&package, entry_pc, param_slots, args)
            .map_err(PhaseDiagnostic::runtime)
    }

    pub fn new(config: HostConfig) -> Self {
        let runtime_profile = RuntimeProfileId::default_for_hal_profile(native_host_profile());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.runtime_class = Some(runtime_profile.runtime_class());
        let host_callbacks = None;
        Self {
            config,
            event_dispatcher: Mutex::new(EventDispatcher::default()),
            com_subscription_handlers: Mutex::new(HashMap::new()),
            runtime_profile,
            host_callbacks: host_callbacks.clone(),
            host_services: build_host_services(
                runtime_profile.hal_profile(),
                runtime_profile.runtime_class(),
                policy,
                host_callbacks,
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
            build_host_services(profile, runtime_class, policy, self.host_callbacks.clone());
    }

    pub fn set_runtime_profile(&mut self, runtime_profile: RuntimeProfileId) {
        self.runtime_profile = runtime_profile;
        let mut policy = self.host_services.policy().clone();
        policy.runtime_class = Some(runtime_profile.runtime_class());
        self.host_services = build_host_services(
            runtime_profile.hal_profile(),
            runtime_profile.runtime_class(),
            policy,
            self.host_callbacks.clone(),
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

    /// Wrap the current host services in a recording layer.
    pub fn with_recording(mut self) -> Self {
        self.host_services = Arc::new(oxvba_hal::adapters::recording::RecordingHostServices::new(
            self.host_services.clone(),
        ));
        self
    }

    /// Create an engine that replays from a recorded journal.
    pub fn from_replay(config: HostConfig, journal: oxvba_hal::journal::HalJournal) -> Self {
        let policy = HostPolicy::deterministic_runtime();
        let host_services: Arc<dyn HostServices> = Arc::new(
            oxvba_hal::adapters::replay::ReplayHostServices::new(journal, policy),
        );
        let runtime_profile = RuntimeProfileId::default_for_hal_profile(HalProfileId::Null);
        Self {
            config,
            event_dispatcher: Mutex::new(EventDispatcher::default()),
            com_subscription_handlers: Mutex::new(HashMap::new()),
            runtime_profile,
            host_callbacks: None,
            host_services,
        }
    }

    pub fn set_host_policy(&mut self, policy: HostPolicy) {
        let profile = self.host_services.profile();
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services =
            build_host_services(profile, runtime_class, policy, self.host_callbacks.clone());
    }

    pub fn set_host_callbacks(&mut self, callbacks: Option<Arc<dyn HostCallbacks>>) {
        self.host_callbacks = callbacks;
        let policy = self.host_services.policy().clone();
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services = build_host_services(
            self.host_services.profile(),
            runtime_class,
            policy,
            self.host_callbacks.clone(),
        );
    }

    pub fn with_host_callbacks(mut self, callbacks: Arc<dyn HostCallbacks>) -> Self {
        self.set_host_callbacks(Some(callbacks));
        self
    }

    pub fn set_host_policy_preset(&mut self, preset: HostPolicyPreset) {
        self.set_host_policy(HostPolicy::for_preset(preset));
    }

    pub fn set_unsupported_feature_mode(&mut self, mode: UnsupportedFeatureMode) {
        let mut policy = self.host_services.policy().clone();
        policy.unsupported_feature_mode = mode;
        self.set_host_policy(policy);
    }

    pub fn host_policy(&self) -> &HostPolicy {
        self.host_services.policy()
    }

    pub fn host_services(&self) -> Arc<dyn HostServices> {
        self.host_services.clone()
    }

    pub fn runtime_profile(&self) -> RuntimeProfileId {
        self.runtime_profile
    }

    pub fn hal_descriptor(&self) -> HalDescriptor {
        self.host_services.descriptor()
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

    pub fn dispatch_host_event_variants_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        project_name: &str,
        module_name: &str,
        event_name: &str,
        source_instance: ObjectRef,
        args: &[Variant],
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
                let mut actual_args = vec![Variant::from_i32(source_instance.raw())];
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
            Self::invoke_session_package_procedure_with_variants(
                runtime,
                metadata.entry_pc,
                &metadata.param_slots,
                actual_args.as_slice(),
            )?;
        }
        Ok(true)
    }

    pub fn subscribe_com_event_handler(
        &self,
        object_token: ObjectRef,
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
            .unsubscribe_event_variant(subscription_token)
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
        object_token: ObjectRef,
    ) -> Result<Option<ComObjectDescriptor>, PhaseDiagnostic> {
        self.host_services
            .com()
            .describe_object(object_token)
            .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))
    }

    /// Bind an existing native `IDispatch` pointer into this engine's host COM
    /// state under a ProgID such as `Excel.Application`.
    ///
    /// # Safety
    ///
    /// `dispatch` must be null or a valid `IDispatch` pointer carrying one
    /// retained reference owned by the caller. On success or failure the host
    /// COM adapter takes ownership of that reference.
    pub unsafe fn bind_native_dispatch_object(
        &self,
        prog_id: &str,
        dispatch: *mut core::ffi::c_void,
    ) -> Result<ObjectRef, PhaseDiagnostic> {
        let value = unsafe {
            self.host_services
                .com()
                .bind_native_dispatch_object_variant(prog_id, dispatch)
        }
        .map_err(|err| PhaseDiagnostic::runtime(err.to_string()))?;
        value.as_object_ref().ok_or_else(|| {
            PhaseDiagnostic::runtime(format!(
                "COM-E-OBJECT-BINDING-MISSING: native dispatch binding for `{prog_id}` did not return an object"
            ))
        })
    }

    pub fn poll_com_event_callback_variants(
        &self,
    ) -> Result<Option<ComEventCallbackVariantDispatch>, PhaseDiagnostic> {
        let _ = self
            .host_services
            .events()
            .do_events_variant()
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

        Ok(Some(ComEventCallbackVariantDispatch {
            callback_token: callback.callback_token,
            subscription_token: callback.subscription_token,
            object: callback.object,
            event: callback.event,
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
        vm.set_project_com_withevents_routes(compiled.project_com_withevents_routes.clone());
        vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
        let package =
            VmExecutionPackage::new(&compiled.bytecode, &compiled.procedure_runtime_metadata);
        vm.execute_package(&package)
            .map_err(PhaseDiagnostic::runtime)?;
        Ok(ProjectRuntimeSession {
            compiled,
            vm,
            package_origin: VmPackageOrigin::InMemory,
        })
    }

    /// Compile a project manifest and prepare a runtime session.
    ///
    /// This callable-session path does not run the project entry point or any
    /// lowered procedure bodies up front; callers drive execution explicitly
    /// through `invoke_procedure`.
    pub fn compile_and_prepare_session(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<ProjectRuntimeSession, PhaseDiagnostic> {
        // Callable sessions should preserve module initialization state, but not
        // execute the synthetic startup shim that loaded Exe projects inject for
        // entry-point hosting.
        let session_manifest = manifest_without_startup_entry_shims(manifest);
        let compiled = compile_project(session_manifest.as_ref())
            .map_err(|e| PhaseDiagnostic::compile(e.to_string()))?;
        if let Ok(mut dispatcher) = self.event_dispatcher.lock() {
            dispatcher.apply_bindings(&compiled.event_dispatch_bindings);
        }
        self.preflight_host_sensitive_support(&compiled.bytecode)?;
        let mut vm = Vm::new(self.host_services.clone());
        let package =
            VmExecutionPackage::new(&compiled.bytecode, &compiled.procedure_runtime_metadata);
        vm.load_execution_package_metadata(&package);
        vm.set_project_com_withevents_routes(compiled.project_com_withevents_routes.clone());
        vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
        Ok(ProjectRuntimeSession {
            compiled,
            vm,
            package_origin: VmPackageOrigin::InMemory,
        })
    }

    /// Invoke a specific procedure by module and name with exact Variant args.
    pub fn invoke_procedure_with_variants(
        &self,
        session: &mut ProjectRuntimeSession,
        module: &str,
        procedure: &str,
        args: &[Variant],
    ) -> Result<Variant, PhaseDiagnostic> {
        // The procedure_runtime_metadata keys use the `pmr_{project}_{module}_{procedure}` format.
        // Try the full key first, then fall back to suffix matching.
        let suffix = format!(
            "_{}_{}",
            module.to_ascii_lowercase(),
            procedure.to_ascii_lowercase()
        );

        let metadata = session
            .compiled
            .procedure_runtime_metadata
            .iter()
            .find(|(k, _)| k.ends_with(&suffix))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                PhaseDiagnostic::runtime(format!("procedure not found: {module}.{procedure}"))
            })?;

        let expected = metadata.param_slots.len();
        if args.len() != expected {
            return Err(PhaseDiagnostic::runtime(format!(
                "arity mismatch for {module}.{procedure}: expected {expected} args, got {}",
                args.len()
            )));
        }

        Self::invoke_session_package_procedure_with_variants(
            session,
            metadata.entry_pc,
            &metadata.param_slots,
            args,
        )?;

        // Read return value from return_slot if present
        match metadata.return_slot {
            Some(slot) => Ok(session.read_variant_slot(slot)),
            None => Ok(Variant::empty()),
        }
    }

    /// Create a new instance of a class module in the runtime session.
    pub fn create_class_instance(
        &self,
        session: &mut ProjectRuntimeSession,
        class_name: &str,
    ) -> Result<ObjectRef, PhaseDiagnostic> {
        let lowered = class_name.to_ascii_lowercase();

        // Find the dynamic object route for this class
        let route = session
            .compiled
            .project_dynamic_objects
            .iter()
            .find(|r| r.module_name.to_ascii_lowercase() == lowered)
            .ok_or_else(|| PhaseDiagnostic::runtime(format!("class not found: {class_name}")))?;

        let handle = route.object_handle;
        let object = session
            .vm
            .project_dynamic_object_ref(handle)
            .ok_or_else(|| {
                PhaseDiagnostic::runtime(format!(
                    "object identity {} not found in project dynamic runtime state",
                    handle
                ))
            })?;

        // Try to invoke Class_Initialize if present
        let init_suffix = format!("_{}_class_initialize", lowered);
        if let Some(metadata) = session
            .compiled
            .procedure_runtime_metadata
            .iter()
            .find(|(k, _)| k.ends_with(&init_suffix))
            .map(|(_, v)| v.clone())
        {
            let init_args = if metadata.param_slots.is_empty() {
                Vec::new()
            } else {
                vec![Variant::from_object_ref(object.clone())]
            };
            Self::invoke_session_package_procedure_with_variants(
                session,
                metadata.entry_pc,
                &metadata.param_slots,
                &init_args,
            )?;
        }

        Ok(object)
    }

    /// Invoke a method on a class object instance with exact Variant args.
    pub fn invoke_member_on_object_with_variants(
        &self,
        session: &mut ProjectRuntimeSession,
        object: ObjectRef,
        member: &str,
        args: &[Variant],
    ) -> Result<Variant, PhaseDiagnostic> {
        self.invoke_member_on_object_with_kind(session, object, member, None, args)
    }

    pub fn invoke_member_on_object_with_kind(
        &self,
        session: &mut ProjectRuntimeSession,
        object: ObjectRef,
        member: &str,
        call_kind: Option<RuntimeCallKind>,
        args: &[Variant],
    ) -> Result<Variant, PhaseDiagnostic> {
        // Find the dynamic object route for this handle
        let route = session
            .compiled
            .project_dynamic_objects
            .iter()
            .find(|r| r.object_handle == object.raw())
            .ok_or_else(|| {
                PhaseDiagnostic::runtime(format!(
                    "object handle {} not found in project dynamic objects",
                    object
                ))
            })?;

        // Find the member by name (lowered_name is the full PMR key, not
        // the bare member name — match against member_name instead)
        let member_route = route
            .members
            .iter()
            .find(|m| {
                m.member_name.eq_ignore_ascii_case(member)
                    && call_kind
                        .map(|kind| runtime_call_kind_for_project_member(m.kind) == kind)
                        .unwrap_or(true)
            })
            .or_else(|| {
                if call_kind.is_some() {
                    route
                        .members
                        .iter()
                        .find(|m| m.member_name.eq_ignore_ascii_case(member))
                } else {
                    None
                }
            })
            .cloned()
            .ok_or_else(|| {
                PhaseDiagnostic::runtime(format!(
                    "member `{member}` not found on object {}",
                    route.module_name
                ))
            })?;

        let expected = member_route.visible_param_count;
        if args.len() != expected {
            return Err(PhaseDiagnostic::runtime(format!(
                "arity mismatch for {}.{}: expected {expected} args, got {}",
                route.module_name,
                member,
                args.len()
            )));
        }

        let frame = Self::build_project_member_call_frame(
            &object,
            route.module_name.as_str(),
            &member_route,
            args,
        );
        self.invoke_project_member_call_frame(session, object, &member_route, frame)
    }

    pub fn class_name_for_object<'a>(
        &self,
        session: &'a ProjectRuntimeSession,
        object: &ObjectRef,
    ) -> Option<&'a str> {
        let _ = self;
        session
            .compiled
            .project_dynamic_objects
            .iter()
            .find(|route| route.object_handle == object.raw())
            .map(|route| route.module_name.as_str())
    }

    fn build_project_member_call_frame(
        object: &ObjectRef,
        module_name: &str,
        member_route: &ProjectDynamicMemberRoute,
        args: &[Variant],
    ) -> RuntimeCallFrame {
        let selector = member_route
            .dispatch_id
            .or(member_route.known_dispatch_token)
            .map(|dispatch_id| RuntimeCallSelector::DispatchId {
                interface: object
                    .query_interface_projection(RuntimeInterfaceId::IDispatch)
                    .map(|projection| projection.interface_identity),
                dispatch_id,
            })
            .unwrap_or_else(|| RuntimeCallSelector::Name {
                receiver_type: Some(module_name.to_string()),
                member_name: member_route.member_name.clone(),
            });
        let mut frame = RuntimeCallFrame::new(
            selector,
            runtime_call_kind_for_project_member(member_route.kind),
        )
        .with_receiver(object.clone())
        .with_context(RuntimeCallContext::new(RuntimeCallSource::InternalProject));

        let property_put_index = match member_route.kind {
            ProjectDynamicMemberKind::PropertyLet | ProjectDynamicMemberKind::PropertySet => {
                args.len().checked_sub(1)
            }
            _ => None,
        };
        for (index, arg) in args.iter().cloned().enumerate() {
            let call_arg = RuntimeCallArgument::by_value(arg);
            if Some(index) == property_put_index {
                frame.set_property_put_arg(call_arg);
            } else {
                frame.push_positional_arg(call_arg);
            }
        }
        frame
    }

    fn invoke_project_member_call_frame(
        &self,
        session: &mut ProjectRuntimeSession,
        object: ObjectRef,
        member_route: &ProjectDynamicMemberRoute,
        frame: RuntimeCallFrame,
    ) -> Result<Variant, PhaseDiagnostic> {
        let mut visible_args = frame
            .positional_args
            .iter()
            .map(|arg| arg.value.clone())
            .collect::<Vec<_>>();
        if let Some(property_put_arg) = frame.property_put_arg {
            visible_args.push(property_put_arg.value);
        }
        // Class members have an implicit `Me` parameter in slot 0.
        // Prepend the canonical ObjectRef value for `Me`, then the caller-supplied args.
        let has_implicit_me = member_route.param_slots.len() > visible_args.len();
        let full_args: Vec<Variant> = if has_implicit_me {
            let mut v = vec![Variant::from_object_ref(object)];
            v.append(&mut visible_args);
            v
        } else {
            visible_args
        };

        Self::invoke_session_package_procedure_with_variants(
            session,
            member_route.entry_pc,
            &member_route.param_slots,
            &full_args,
        )?;

        match member_route.return_slot {
            Some(slot) => Ok(RuntimeCallResult::value(session.read_variant_slot(slot))
                .value
                .expect("call result should carry value")),
            None => Ok(RuntimeCallResult::empty()
                .value
                .unwrap_or_else(Variant::empty)),
        }
    }

    pub fn poll_and_dispatch_next_com_event_callback_variants(
        &self,
        runtime: &mut ProjectRuntimeSession,
    ) -> Result<bool, PhaseDiagnostic> {
        let Some(callback) = self.poll_com_event_callback_variants()? else {
            return Ok(false);
        };
        self.dispatch_com_event_callback_variants_into_runtime(runtime, &callback)?;
        Ok(true)
    }

    pub fn dispatch_com_event_callback_variants_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        callback: &ComEventCallbackVariantDispatch,
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
        Self::invoke_session_package_procedure_with_variants(
            runtime,
            metadata.entry_pc,
            &metadata.param_slots,
            callback.args.as_slice(),
        )
    }

    pub fn execute_source(&self, source: &str) -> Result<(), String> {
        let _ = self.execute_source_with_variant_snapshot(source)?;
        Ok(())
    }

    pub fn execute_source_with_variant_snapshot(
        &self,
        source: &str,
    ) -> Result<Vec<Variant>, String> {
        self.execute_source_with_variant_snapshot_phased(source)
            .map_err(|diagnostic| diagnostic.message().to_string())
    }

    pub fn execute_source_with_variant_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        Ok(self
            .execute_source_with_variant_snapshot_and_package_identity_phased(source)?
            .values)
    }

    pub fn execute_source_with_variant_snapshot_and_package_identity_phased(
        &self,
        source: &str,
    ) -> Result<HostVariantSnapshotWithPackageIdentity, PhaseDiagnostic> {
        let (bytecode, procedure_runtime_metadata) = compile_with_runtime_metadata(source)
            .map_err(|e| PhaseDiagnostic::compile(e.to_string()))?;
        self.preflight_host_sensitive_support(&bytecode)?;
        if self.config.enable_jit {
            #[cfg(feature = "jit")]
            {
                return Err(PhaseDiagnostic::runtime(JIT_NOT_IMPLEMENTED_MESSAGE));
            }
            #[cfg(not(feature = "jit"))]
            {
                return Err(PhaseDiagnostic::runtime(
                    "JIT execution requested but the `jit` feature is not enabled",
                ));
            }
        }

        let mut vm = Vm::new(self.host_services.clone());
        let package = VmExecutionPackage::new(&bytecode, &procedure_runtime_metadata);
        vm.execute_package(&package)
            .map_err(PhaseDiagnostic::runtime)?;
        let package_identity = recorded_package_identity(&vm)?;
        let all_slots = vm.snapshot_variants(bytecode.slot_count);
        let values = project_visible_snapshot(
            &all_slots,
            &procedure_runtime_metadata,
            bytecode.user_slot_count,
        );
        Ok(HostVariantSnapshotWithPackageIdentity {
            values,
            package_identity,
        })
    }

    pub fn execute_project_with_variant_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        Ok(self
            .execute_project_with_variant_snapshot_and_package_identity_phased(manifest)?
            .values)
    }

    pub fn execute_project_with_variant_snapshot_and_package_identity_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<HostVariantSnapshotWithPackageIdentity, PhaseDiagnostic> {
        let compiled =
            compile_project(manifest).map_err(|e| PhaseDiagnostic::compile(e.to_string()))?;
        if let Ok(mut dispatcher) = self.event_dispatcher.lock() {
            dispatcher.apply_bindings(&compiled.event_dispatch_bindings);
        }
        self.preflight_host_sensitive_support(&compiled.bytecode)?;
        if self.config.enable_jit {
            #[cfg(feature = "jit")]
            {
                return Err(PhaseDiagnostic::runtime(JIT_NOT_IMPLEMENTED_MESSAGE));
            }
            #[cfg(not(feature = "jit"))]
            {
                return Err(PhaseDiagnostic::runtime(
                    "JIT execution requested but the `jit` feature is not enabled",
                ));
            }
        }

        self.execute_compiled_project_with_variant_snapshot_vm(&compiled)
    }

    fn execute_compiled_project_with_variant_snapshot_vm(
        &self,
        compiled: &CompiledProject,
    ) -> Result<HostVariantSnapshotWithPackageIdentity, PhaseDiagnostic> {
        let mut vm = Vm::new(self.host_services.clone());
        vm.set_project_com_withevents_routes(compiled.project_com_withevents_routes.clone());
        vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
        let package =
            VmExecutionPackage::new(&compiled.bytecode, &compiled.procedure_runtime_metadata);
        vm.execute_package(&package)
            .map_err(PhaseDiagnostic::runtime)?;
        let package_identity = recorded_package_identity(&vm)?;
        let all_slots = vm.snapshot_variants(compiled.bytecode.slot_count);
        let values = project_visible_snapshot(
            &all_slots,
            &compiled.procedure_runtime_metadata,
            compiled.bytecode.user_slot_count,
        );
        Ok(HostVariantSnapshotWithPackageIdentity {
            values,
            package_identity,
        })
    }

    /// Prepare a runtime session from a deserialized OxBundle (no recompilation).
    ///
    /// Used by DLL shims that need to invoke individual procedures from the
    /// embedded bundle.
    pub fn compile_and_prepare_session_from_bundle(
        &self,
        bundle: &oxvba_compiler::OxBundle,
    ) -> Result<ProjectRuntimeSession, PhaseDiagnostic> {
        if let Some(ref bindings) = bundle.event_dispatch_bindings
            && let Ok(mut dispatcher) = self.event_dispatcher.lock()
        {
            dispatcher.apply_bindings(bindings);
        }
        self.preflight_host_sensitive_support(&bundle.bytecode)?;
        let project_reflection =
            bundle
                .project_reflection()
                .unwrap_or_else(|_| oxvba_compiler::ProjectReflection {
                    identity: oxvba_compiler::ProjectIdentity {
                        project_name: bundle
                            .manifest_snapshot
                            .as_ref()
                            .map(|snapshot| snapshot.project_name.clone())
                            .unwrap_or_default(),
                        project_id: String::new(),
                        source_fingerprint: String::new(),
                    },
                    modules: Vec::new(),
                    procedures: Vec::new(),
                    capabilities: Vec::new(),
                });
        let compiled = CompiledProject {
            bytecode: bundle.bytecode.clone(),
            procedure_runtime_metadata: bundle.procedure_metadata.clone(),
            source_maps: oxvba_compiler::CompilerSourceMap::default(),
            rewritten_source: String::new(),
            host_exports: bundle
                .export_inventory
                .as_ref()
                .map(|ei| ei.host_exports.clone())
                .unwrap_or_default(),
            reference_visible_exports: Vec::new(),
            event_dispatch_bindings: bundle.event_dispatch_bindings.clone().unwrap_or_default(),
            project_com_withevents_routes: bundle.com_withevents_routes.clone().unwrap_or_default(),
            project_dynamic_objects: bundle.dynamic_object_routes.clone().unwrap_or_default(),
            project_reflection,
        };
        let mut vm = Vm::new(self.host_services.clone());
        let package = VmExecutionPackage::from_bundle(bundle);
        vm.load_execution_package_metadata(&package);
        vm.set_project_com_withevents_routes(compiled.project_com_withevents_routes.clone());
        vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
        Ok(ProjectRuntimeSession {
            compiled,
            vm,
            package_origin: VmPackageOrigin::OxBundle,
        })
    }

    pub fn execute_bundle_with_variant_snapshot(
        &self,
        bundle: &oxvba_compiler::OxBundle,
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        Ok(self
            .execute_bundle_with_variant_snapshot_and_package_identity(bundle)?
            .values)
    }

    pub fn execute_bundle_with_variant_snapshot_and_package_identity(
        &self,
        bundle: &oxvba_compiler::OxBundle,
    ) -> Result<HostVariantSnapshotWithPackageIdentity, PhaseDiagnostic> {
        if let Some(ref bindings) = bundle.event_dispatch_bindings
            && let Ok(mut dispatcher) = self.event_dispatcher.lock()
        {
            dispatcher.apply_bindings(bindings);
        }
        self.preflight_host_sensitive_support(&bundle.bytecode)?;
        let mut vm = Vm::new(self.host_services.clone());
        if let Some(ref routes) = bundle.com_withevents_routes {
            vm.set_project_com_withevents_routes(routes.clone());
        }
        if let Some(ref routes) = bundle.dynamic_object_routes {
            vm.set_project_dynamic_objects(routes.clone());
        }
        let package = VmExecutionPackage::from_bundle(bundle);
        vm.execute_package(&package)
            .map_err(PhaseDiagnostic::runtime)?;
        let package_identity = recorded_package_identity(&vm)?;
        let values = vm.snapshot_variants(bundle.bytecode.user_slot_count);
        Ok(HostVariantSnapshotWithPackageIdentity {
            values,
            package_identity,
        })
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
) -> Result<ComEventCallbackVariantDispatch, PhaseDiagnostic> {
    Ok(ComEventCallbackVariantDispatch {
        callback_token: payload.callback.into(),
        subscription_token: payload.subscription.into(),
        object: payload.object,
        event: payload.event,
        handler_symbol: String::new(),
        args: payload
            .args
            .into_iter()
            .map(|value| value.variant().clone())
            .collect(),
    })
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
        Instruction::IntrinsicConsolePrintHost { .. } => Some(("Print", CapabilityId::ConsoleIo)),
        Instruction::IntrinsicConsoleInputHost { .. } => Some(("Input", CapabilityId::ConsoleIo)),
        Instruction::IntrinsicConsoleLineInputHost { .. } => {
            Some(("Line Input", CapabilityId::ConsoleIo))
        }
        Instruction::IntrinsicMsgBoxHost { .. } => Some(("MsgBox", CapabilityId::UiInteraction)),
        Instruction::IntrinsicInputBoxHost { .. } => {
            Some(("InputBox", CapabilityId::UiInteraction))
        }
        Instruction::IntrinsicDebugPrintHost { .. } => {
            Some(("Debug.Print", CapabilityId::DiagnosticsTelemetry))
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
    use oxvba_compiler::{
        DeclareParamType, ModuleKind, ProjectDynamicMemberKind, ProjectDynamicMemberRoute,
        ProjectKind, ProjectManifest, module_unit_from_source,
    };
    use oxvba_runtime::{ObjectRef, RuntimeCallKind, RuntimeCallSelector, Variant};
    use oxvba_vm::{VmPackageIdentityEvidence, VmPackageOrigin};

    use super::{Engine, HostConfig};

    fn manifest_from_source(source: &str) -> ProjectManifest {
        ProjectManifest {
            project_name: "HostIdentityEvidence".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("MainModule", ModuleKind::Procedural, source)
                    .expect("test module should parse"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        }
    }

    fn sorted_procedure_names(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
        let mut names = evidence
            .procedures
            .iter()
            .map(|procedure| procedure.procedure_name.to_ascii_lowercase())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn member_route(kind: ProjectDynamicMemberKind) -> ProjectDynamicMemberRoute {
        ProjectDynamicMemberRoute {
            member_name: "Value".to_string(),
            lowered_name: "project_widget_value".to_string(),
            known_dispatch_token: Some(0),
            dispatch_id: Some(0),
            member_flags: None,
            is_default_member: true,
            kind,
            visible_param_count: 1,
            params: Vec::new(),
            param_types: vec![DeclareParamType::Variant],
            return_type: Some(DeclareParamType::Variant),
            entry_pc: 10,
            param_slots: vec![0, 1],
            return_slot: Some(2),
        }
    }

    #[test]
    fn project_member_call_frame_separates_property_put_value() {
        let object = ObjectRef::from_compat_identity(88);
        let route = member_route(ProjectDynamicMemberKind::PropertyLet);
        let frame = Engine::build_project_member_call_frame(
            &object,
            "Widget",
            &route,
            &[Variant::from_i32(42)],
        );

        assert_eq!(frame.kind, RuntimeCallKind::PropertyLet);
        assert!(matches!(
            frame.selector,
            RuntimeCallSelector::DispatchId { dispatch_id: 0, .. }
        ));
        assert!(frame.positional_args.is_empty());
        assert_eq!(
            frame
                .property_put_arg
                .as_ref()
                .and_then(|arg| arg.value.as_i32()),
            Some(42)
        );
        assert_eq!(frame.receiver.as_ref().map(ObjectRef::raw), Some(88));
    }

    #[test]
    fn source_snapshot_path_records_package_identity_without_behavior_drift() {
        let source = r#"
Sub Main()
    Dim Result As Variant
    Result = Test(2.5, "kg")
End Sub

Function Test(dbl As Double, str As String) As Variant
    Test = CStr(dbl) & str
End Function
"#;
        let engine = Engine::new(HostConfig { enable_jit: false });

        let legacy_snapshot = engine
            .execute_source_with_variant_snapshot_phased(source)
            .expect("source snapshot should execute");
        let package_snapshot = engine
            .execute_source_with_variant_snapshot_and_package_identity_phased(source)
            .expect("package-backed source snapshot should execute");

        assert_eq!(package_snapshot.values, legacy_snapshot);
        assert!(
            package_snapshot
                .values
                .iter()
                .any(|value| value.as_bstr().is_some()),
            "source snapshot should still contain the computed string value: {:?}",
            package_snapshot.values
        );
        assert_eq!(
            package_snapshot.package_identity.package_origin,
            VmPackageOrigin::InMemory
        );
        assert!(
            package_snapshot
                .package_identity
                .package_digest
                .starts_with("fnv1a64:")
        );
        assert!(
            package_snapshot
                .package_identity
                .bytecode_digest
                .starts_with("fnv1a64:")
        );
        assert_eq!(
            sorted_procedure_names(&package_snapshot.package_identity),
            vec!["main".to_string(), "test".to_string()]
        );
        assert!(
            package_snapshot
                .package_identity
                .procedures
                .iter()
                .all(|procedure| procedure.procedure_id.contains("@pc:"))
        );
        let test_identity = package_snapshot
            .package_identity
            .procedures
            .iter()
            .find(|procedure| procedure.procedure_name.eq_ignore_ascii_case("Test"))
            .expect("Test descriptor evidence should be present");
        assert!(test_identity.slot_descriptor_digest.starts_with("fnv1a64:"));
        assert!(
            test_identity
                .slot_descriptors
                .iter()
                .any(|descriptor| descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("dbl"))
                    && descriptor.declared_type == oxvba_compiler::VbaTypeId::Double),
            "host package identity should expose parameter descriptor evidence"
        );
    }

    #[test]
    fn project_and_callable_session_paths_preserve_package_identity() {
        let source = r#"
Sub Main()
    Dim Observed As Long
    Observed = AddOne(41)
End Sub

Function AddOne(value As Long) As Long
    AddOne = value + 1
End Function
"#;
        let manifest = manifest_from_source(source);
        let engine = Engine::new(HostConfig { enable_jit: false });

        let legacy_project_snapshot = engine
            .execute_project_with_variant_snapshot_phased(&manifest)
            .expect("project snapshot should execute");
        let package_project_snapshot = engine
            .execute_project_with_variant_snapshot_and_package_identity_phased(&manifest)
            .expect("package-backed project snapshot should execute");

        assert_eq!(package_project_snapshot.values, legacy_project_snapshot);
        assert_eq!(
            package_project_snapshot.package_identity.package_origin,
            VmPackageOrigin::InMemory
        );
        assert_eq!(
            sorted_procedure_names(&package_project_snapshot.package_identity),
            vec!["addone".to_string(), "main".to_string()]
        );
        assert!(
            package_project_snapshot
                .package_identity
                .procedures
                .iter()
                .any(
                    |procedure| procedure.module_name.eq_ignore_ascii_case("MainModule")
                        && procedure.procedure_name.eq_ignore_ascii_case("AddOne")
                )
        );

        let mut session = engine
            .compile_and_prepare_session(&manifest)
            .expect("callable session should prepare");
        assert_eq!(session.package_origin(), VmPackageOrigin::InMemory);
        assert!(
            session.package_identity_evidence().is_none(),
            "preparation alone should not claim an executed package identity"
        );
        let result = engine
            .invoke_procedure_with_variants(
                &mut session,
                "MainModule",
                "AddOne",
                &[Variant::from_i32(41)],
            )
            .expect("callable procedure should execute");
        assert_eq!(result, Variant::from_i32(42));
        let session_identity = session
            .package_identity_evidence()
            .expect("callable invocation should record package identity");
        assert_eq!(session_identity.package_origin, VmPackageOrigin::InMemory);
        assert_eq!(
            sorted_procedure_names(session_identity),
            vec!["addone".to_string(), "main".to_string()]
        );
    }
}
