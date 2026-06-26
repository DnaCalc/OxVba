//! Engine: the host orchestration entry point for the clean execution stack.
//!
//! `Engine` configures host services (HAL profile / policy / callbacks) and runs
//! VBA on the clean pipeline — `oxvba_bind` → `oxvba_bundle::linearize` →
//! `oxvba_vm2` — for a single source module (optionally carrying typelib/native/host
//! references, so an early-bound COM call reaches the resolver) or a `.basproj`
//! project closure. The
//! legacy compiler/VM execution path (and its COM-event / session / immediate-window
//! machinery) was removed with `oxvba-compiler`/`oxvba-vm`; see git history.

#[cfg(target_os = "windows")]
use std::ffi::c_void;
use std::sync::Arc;

use oxvba_com::PortableComProjection;
use oxvba_diagnostics::{Diagnostic as OxDiagnostic, DiagnosticPhase as OxDiagnosticPhase};
use oxvba_hal::{
    adapters::builder::HostBuilder,
    callbacks::HostCallbacks,
    model::{
        HalDescriptor, HalProfileId, HostPolicy, HostPolicyPreset, UnsupportedFeatureMode,
        native_host_profile,
    },
    traits::HostServices,
};
use oxvba_runtime::{ObjectRef, Variant};
use oxvba_symbol::{CatalogTypeLibResolver, TypeLibResolver};

use crate::runner::RuntimeProfileId;

const JIT_NOT_IMPLEMENTED_MESSAGE: &str =
    "JIT execution is not implemented; the clean stack runs on the oxvba_vm2 interpreter";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    CompileTime,
    Runtime,
}

/// The outcome of a vm3 run for the differential harness: a result snapshot, an
/// out-of-scope construct to skip, or a genuine failure. The skip/fail split lets the
/// harness gate vm3-vs-vm2 on the subset vm3 currently runs, growing automatically as
/// vm3 implements more (an `Unsupported` program becomes `Ran` once its construct lands).
#[derive(Debug, Clone)]
pub enum Vm3Snapshot {
    /// The run completed: the module globals followed by the entry `Main` frame's locals.
    Ran(Vec<Variant>),
    /// vm3 does not yet implement a construct the program uses (an elaboration- or
    /// execution-time `Unimplemented`) — SKIP it (out of vm3's current scope, not a
    /// divergence). The string names the construct.
    Unsupported(String),
    /// A genuine failure: a bind error, an elaboration `Malformed`, an uncaught run-time
    /// fault, or a vm3 `Malformed` defect.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseDiagnostic {
    phase: DiagnosticPhase,
    message: String,
    diagnostic: Box<OxDiagnostic>,
}

impl PhaseDiagnostic {
    pub(crate) fn from_diagnostic(diagnostic: OxDiagnostic) -> Self {
        let phase = if diagnostic.phase.is_runtime() {
            DiagnosticPhase::Runtime
        } else {
            DiagnosticPhase::CompileTime
        };
        let message = diagnostic.message.clone();
        Self {
            phase,
            message,
            diagnostic: Box::new(diagnostic),
        }
    }

    pub fn phase(&self) -> DiagnosticPhase {
        self.phase
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn diagnostic(&self) -> &OxDiagnostic {
        &self.diagnostic
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

fn jit_not_implemented_diagnostic() -> PhaseDiagnostic {
    PhaseDiagnostic::from_diagnostic(OxDiagnostic::error(
        "RUN-E-JIT-NOT-IMPLEMENTED",
        OxDiagnosticPhase::Runtime,
        JIT_NOT_IMPLEMENTED_MESSAGE,
    ))
}

fn linearize_diagnostic(err: oxvba_bundle::LinearizeError) -> OxDiagnostic {
    OxDiagnostic::error(
        "BUND-E-MALFORMED-CORE",
        OxDiagnosticPhase::Bundle,
        err.to_string(),
    )
    .with_help("This indicates the binder emitted invalid Core IR; reduce the source to a regression case.")
}

fn runtime_diagnostic(err: oxvba_vm2::VmError) -> PhaseDiagnostic {
    PhaseDiagnostic::from_diagnostic(err.to_diagnostic())
}

#[derive(Debug, Clone, Default)]
pub struct HostConfig {
    pub enable_jit: bool,
}

#[derive(Clone, Default)]
pub struct HostProfileProvider {
    typelib_resolver: Option<Arc<dyn TypeLibResolver>>,
    portable_com_projection: Option<Arc<PortableComProjection>>,
    host_policy: Option<HostPolicy>,
}

impl HostProfileProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_typelib_resolver(mut self, resolver: Arc<dyn TypeLibResolver>) -> Self {
        self.typelib_resolver = Some(resolver);
        self
    }

    pub fn with_portable_com_projection(mut self, projection: Arc<PortableComProjection>) -> Self {
        self.portable_com_projection = Some(projection);
        self
    }

    pub fn with_host_policy(mut self, policy: HostPolicy) -> Self {
        self.host_policy = Some(policy);
        self
    }
}

pub struct Engine {
    config: HostConfig,
    runtime_profile: RuntimeProfileId,
    host_callbacks: Option<Arc<dyn HostCallbacks>>,
    portable_com_projection: Option<Arc<PortableComProjection>>,
    typelib_resolver: Arc<dyn TypeLibResolver>,
    host_services: Arc<dyn HostServices>,
}

pub struct ProjectRuntimeSession {
    vm: oxvba_vm2::Vm<'static>,
    entry_bundle: usize,
    host_services: Arc<dyn HostServices>,
}

impl ProjectRuntimeSession {
    pub fn entry_bundle(&self) -> usize {
        self.entry_bundle
    }

    pub fn create_class_instance(
        &mut self,
        class_name: &str,
    ) -> Result<ObjectRef, PhaseDiagnostic> {
        self.vm
            .create_project_instance(self.entry_bundle, class_name)
            .map_err(runtime_diagnostic)
    }

    pub fn invoke_member_values(
        &mut self,
        object: ObjectRef,
        member_name: &str,
        kind_hint: Option<oxvba_bundle::ProjectMemberKind>,
        args: Vec<Variant>,
    ) -> Result<Variant, PhaseDiagnostic> {
        self.vm
            .invoke_project_member_values(object, member_name, kind_hint, args)
            .map_err(runtime_diagnostic)
    }

    pub fn set_project_event_sink<F>(&mut self, sink: F)
    where
        F: FnMut(ObjectRef, i32, Vec<Variant>) -> Result<(), String> + 'static,
    {
        self.vm.set_project_event_sink(sink);
    }

    pub fn clear_project_event_sink(&mut self) {
        self.vm.clear_project_event_sink();
    }

    /// Bind a retained native `IDispatch*` supplied by a host callback into the
    /// runtime COM object table, preserving the supplied type identity for
    /// early-bound member dispatch inside the VBA implementation.
    ///
    /// # Safety
    ///
    /// `dispatch` must be null or a valid `IDispatch*` carrying one retained
    /// reference owned by the caller. The HAL takes ownership of that reference.
    #[cfg(target_os = "windows")]
    pub unsafe fn bind_native_dispatch_object_value(
        &mut self,
        prog_id: &str,
        dispatch: *mut c_void,
    ) -> Result<Variant, PhaseDiagnostic> {
        // SAFETY: the caller provides the retained `IDispatch*` described by
        // this function's safety contract, and this method transfers it directly
        // to the host COM boundary that assumes that ownership.
        unsafe {
            self.host_services
                .com()
                .bind_native_dispatch_object_variant(prog_id, dispatch)
        }
        .map_err(|err| PhaseDiagnostic::from_diagnostic(err.to_diagnostic()))
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(HostConfig::default())
    }
}

fn build_host_services(
    profile: HalProfileId,
    runtime_class: oxvba_hal::model::HalRuntimeClass,
    policy: HostPolicy,
    callbacks: Option<Arc<dyn HostCallbacks>>,
    portable_com_projection: Option<Arc<PortableComProjection>>,
) -> Arc<dyn HostServices> {
    let mut builder = HostBuilder::new()
        .profile(profile)
        .runtime_class(runtime_class)
        .policy(policy);
    if let Some(callbacks) = callbacks {
        builder = builder.callbacks(callbacks);
    }
    if let Some(projection) = portable_com_projection {
        builder = builder.portable_objects(projection);
    }
    builder.build()
}

impl Engine {
    pub fn new(config: HostConfig) -> Self {
        let runtime_profile = RuntimeProfileId::default_for_hal_profile(native_host_profile());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.runtime_class = Some(runtime_profile.runtime_class());
        Self {
            config,
            runtime_profile,
            host_callbacks: None,
            portable_com_projection: None,
            typelib_resolver: Arc::new(CatalogTypeLibResolver),
            host_services: build_host_services(
                runtime_profile.hal_profile(),
                runtime_profile.runtime_class(),
                policy,
                None,
                None,
            ),
        }
    }

    /// Create an engine that replays from a recorded HAL journal.
    pub fn from_replay(config: HostConfig, journal: oxvba_hal::journal::HalJournal) -> Self {
        let policy = HostPolicy::deterministic_runtime();
        let host_services: Arc<dyn HostServices> = Arc::new(
            oxvba_hal::adapters::replay::ReplayHostServices::new(journal, policy),
        );
        Self {
            config,
            runtime_profile: RuntimeProfileId::default_for_hal_profile(HalProfileId::Null),
            host_callbacks: None,
            portable_com_projection: None,
            typelib_resolver: Arc::new(CatalogTypeLibResolver),
            host_services,
        }
    }

    pub fn set_hal_profile(&mut self, profile: HalProfileId) {
        let policy = self.host_services.policy().clone();
        self.runtime_profile = RuntimeProfileId::default_for_hal_profile(profile);
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services = build_host_services(
            profile,
            runtime_class,
            policy,
            self.host_callbacks.clone(),
            self.portable_com_projection.clone(),
        );
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
            self.portable_com_projection.clone(),
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

    pub fn set_host_policy(&mut self, policy: HostPolicy) {
        let profile = self.host_services.profile();
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services = build_host_services(
            profile,
            runtime_class,
            policy,
            self.host_callbacks.clone(),
            self.portable_com_projection.clone(),
        );
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
            self.portable_com_projection.clone(),
        );
    }

    pub fn with_host_callbacks(mut self, callbacks: Arc<dyn HostCallbacks>) -> Self {
        self.set_host_callbacks(Some(callbacks));
        self
    }

    pub fn set_portable_com_projection(&mut self, projection: Option<Arc<PortableComProjection>>) {
        self.portable_com_projection = projection;
        let policy = self.host_services.policy().clone();
        let runtime_class = policy
            .runtime_class
            .unwrap_or(self.runtime_profile.runtime_class());
        self.host_services = build_host_services(
            self.host_services.profile(),
            runtime_class,
            policy,
            self.host_callbacks.clone(),
            self.portable_com_projection.clone(),
        );
    }

    pub fn with_portable_com_projection(mut self, projection: Arc<PortableComProjection>) -> Self {
        self.set_portable_com_projection(Some(projection));
        self
    }

    pub fn set_typelib_resolver(&mut self, resolver: Arc<dyn TypeLibResolver>) {
        self.typelib_resolver = resolver;
    }

    pub fn with_typelib_resolver(mut self, resolver: Arc<dyn TypeLibResolver>) -> Self {
        self.set_typelib_resolver(resolver);
        self
    }

    pub fn set_host_profile_provider(&mut self, provider: HostProfileProvider) {
        if let Some(resolver) = provider.typelib_resolver {
            self.set_typelib_resolver(resolver);
        }
        if let Some(projection) = provider.portable_com_projection {
            self.set_portable_com_projection(Some(projection));
        }
        if let Some(policy) = provider.host_policy {
            self.set_host_policy(policy);
        }
    }

    pub fn with_host_profile_provider(mut self, provider: HostProfileProvider) -> Self {
        self.set_host_profile_provider(provider);
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

    /// Enable host-native runtime services for wrapper targets that are already
    /// executing inside the local host process boundary, such as an in-process
    /// COM server receiving native COM interface pointers from Office.
    pub fn enable_host_native_runtime(&mut self) {
        let mut policy = self.host_services.policy().clone();
        policy.deterministic_mode = false;
        policy.allow_com_activation = true;
        policy.allow_dynamic_link = true;
        policy.runtime_class = Some(self.runtime_profile.runtime_class());
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

    /// Prepare a package-backed runtime session without running a startup entry.
    /// Wrapper targets use this for activation-style hosts: the package is linked
    /// once, then class factories create project-class instances on demand.
    ///
    /// The current VM borrows bundles and host services. A loaded in-process COM
    /// server is process-lifetime, so this method intentionally promotes the
    /// package bundles and host-service Arc to `'static` for the session.
    pub fn prepare_bundle_package_session(
        &self,
        package: oxvba_bundle::BundlePackage,
    ) -> Result<ProjectRuntimeSession, PhaseDiagnostic> {
        if self.config.enable_jit {
            return Err(jit_not_implemented_diagnostic());
        }
        package.validate().map_err(|err| {
            PhaseDiagnostic::from_diagnostic(OxDiagnostic::error(
                "BUND-E-PACKAGE",
                OxDiagnosticPhase::Bundle,
                err.to_string(),
            ))
        })?;
        let entry_bundle = package.entry_bundle;
        let leaked_bundles: &'static [oxvba_bundle::Bundle] =
            Box::leak(package.bundles.into_boxed_slice());
        let bundle_refs: Vec<&'static oxvba_bundle::Bundle> = leaked_bundles.iter().collect();
        let leaked_host_services: &'static mut Arc<dyn HostServices> =
            Box::leak(Box::new(self.host_services.clone()));
        let host_services: &'static dyn HostServices = &**leaked_host_services;
        let vm = oxvba_vm2::Vm::link(&bundle_refs, host_services)
            .map_err(|err| PhaseDiagnostic::from_diagnostic(err.to_diagnostic()))?;
        Ok(ProjectRuntimeSession {
            vm,
            entry_bundle,
            host_services: self.host_services.clone(),
        })
    }

    /// Execute a **clean-path** project closure (the leaf-first, entry-last output of
    /// `oxvba_project::load_project_closure`): `oxvba_bind::bind_projects` (one bundle
    /// per project) → `linearize` each → `oxvba_vm2::Vm::link` (multi-bundle image,
    /// entry last) → run. The snapshot is the entry project's module-level globals.
    pub fn execute_project_closure_with_variant_snapshot(
        &self,
        closure: &[oxvba_symbol::manifest::SymbolProjectManifest],
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        if self.config.enable_jit {
            return Err(jit_not_implemented_diagnostic());
        }
        let programs = oxvba_bind::bind_projects(closure, &*self.typelib_resolver)
            .map_err(|e| PhaseDiagnostic::from_diagnostic(e.to_diagnostic()))?;
        let bundles: Vec<oxvba_bundle::Bundle> = programs
            .iter()
            .map(oxvba_bundle::linearize)
            .collect::<Result<_, _>>()
            .map_err(|e| PhaseDiagnostic::from_diagnostic(linearize_diagnostic(e)))?;
        let refs: Vec<&oxvba_bundle::Bundle> = bundles.iter().collect();
        let mut vm = oxvba_vm2::Vm::link(&refs, &*self.host_services)
            .map_err(|e| PhaseDiagnostic::from_diagnostic(e.to_diagnostic()))?;
        vm.run()
            .map_err(|e| PhaseDiagnostic::from_diagnostic(e.to_diagnostic()))?;
        // The entry project's globals are the result snapshot (entry bundle is last;
        // after `run` the cursor rests in it).
        let entry_globals = bundles.last().map(|b| b.global_count).unwrap_or(0);
        let values = (0..entry_globals)
            .map(|slot| vm.slot(slot).cloned().unwrap_or_else(Variant::empty))
            .collect();
        Ok(values)
    }

    /// Execute a single VBA **source** module on the clean path (`oxvba run <source>`):
    /// wrap it in a one-module project, run it, and snapshot the module-level globals
    /// **followed by the entry `Sub Main` frame's locals** (the script's variables —
    /// the meaningful result of a bare-source run, matching the bind-layer harness and
    /// what the legacy host returned). The closure entry point
    /// (`execute_project_closure_with_variant_snapshot`) snapshots globals only, since a
    /// multi-project closure has no single "the script's locals".
    pub fn execute_source_with_variant_snapshot_clean(
        &self,
        source: &str,
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        self.execute_source_with_references_and_snapshot(source, Vec::new())
    }

    /// Execute a single VBA **source** module that carries one or more project
    /// references (typelibs, native libraries, host-injected object models) on the
    /// clean path, snapshotting the same way as
    /// [`Self::execute_source_with_variant_snapshot_clean`] (module globals followed
    /// by the entry `Sub Main` frame's locals).
    ///
    /// This is the reference-carrying counterpart of the bare-source entry: passing
    /// a [`oxvba_symbol::manifest::ProjectReference::TypeLibrary`] threads a typelib
    /// through the resolver, so a typed receiver (`Dim x As Excel.Application`)
    /// resolves its members to dispids and the binder lowers them to early-bound COM
    /// dispatch (`EarlyCom{dispid}`) — the early-binding path the bare-source entry
    /// (with `references: Vec::new()`) can never reach.
    pub fn execute_source_with_references_and_snapshot(
        &self,
        source: &str,
        references: Vec<oxvba_symbol::manifest::ProjectReference>,
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        if self.config.enable_jit {
            return Err(jit_not_implemented_diagnostic());
        }
        use oxvba_symbol::manifest as sym;
        let manifest = sym::SymbolProjectManifest {
            project_name: "Main".to_string(),
            project_kind: sym::ProjectKind::Source,
            modules: vec![sym::ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: sym::ModuleKind::Procedural,
                attributes: sym::ModuleAttributes::named("Main"),
                source: source.to_string(),
            }],
            references,
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };
        self.execute_manifest_with_variant_snapshot(&manifest)
    }

    /// Execute a pre-built single-project **manifest** (one or more modules — e.g. a
    /// procedural `Main` plus a class module — and any references) on the clean path,
    /// snapshotting the module globals followed by the entry `Sub Main` frame's
    /// locals. This is the multi-module counterpart of
    /// [`Self::execute_source_with_references_and_snapshot`]; it is what a `WithEvents`
    /// sink test needs, since `WithEvents` is only valid in a class module.
    pub fn execute_manifest_with_variant_snapshot(
        &self,
        manifest: &oxvba_symbol::manifest::SymbolProjectManifest,
    ) -> Result<Vec<Variant>, PhaseDiagnostic> {
        if self.config.enable_jit {
            return Err(jit_not_implemented_diagnostic());
        }
        let program = oxvba_bind::bind_program(manifest, &*self.typelib_resolver)
            .map_err(|e| PhaseDiagnostic::from_diagnostic(e.to_diagnostic()))?;
        let bundle = oxvba_bundle::linearize(&program)
            .map_err(|e| PhaseDiagnostic::from_diagnostic(linearize_diagnostic(e)))?;
        let mut vm = oxvba_vm2::Vm::link(&[&bundle], &*self.host_services)
            .map_err(|e| PhaseDiagnostic::from_diagnostic(e.to_diagnostic()))?;
        vm.run()
            .map_err(|e| PhaseDiagnostic::from_diagnostic(e.to_diagnostic()))?;
        // Snapshot = module globals + the entry frame's locals (the script's variables).
        let local_count = program
            .entry
            .and_then(|entry| program.procs.get(entry.0))
            .map(|main| main.locals.len())
            .unwrap_or(0);
        let count = bundle.global_count + local_count;
        let values = (0..count)
            .map(|slot| vm.slot(slot).cloned().unwrap_or_else(Variant::empty))
            .collect();
        Ok(values)
    }

    /// Run a single VBA **source** module on the **vm3** path (`oxvba_bind` →
    /// `oxvba_oxir::elaborate` → `oxvba_vm3`), snapshotting the module globals followed
    /// by the entry `Sub Main` frame's locals — the *exact* index space
    /// [`Self::execute_source_with_variant_snapshot_clean`] (vm2) exposes, so the two are
    /// directly comparable in the differential harness. Distinguishes a vm3 *out-of-scope*
    /// construct (an elaboration- or execution-time `Unimplemented`) from a genuine
    /// failure, so the harness can skip an unimplemented program rather than score it as a
    /// divergence.
    pub fn execute_source_with_variant_snapshot_vm3(&self, source: &str) -> Vm3Snapshot {
        use oxvba_symbol::manifest as sym;
        let manifest = sym::SymbolProjectManifest {
            project_name: "Main".to_string(),
            project_kind: sym::ProjectKind::Source,
            modules: vec![sym::ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: sym::ModuleKind::Procedural,
                attributes: sym::ModuleAttributes::named("Main"),
                source: source.to_string(),
            }],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };
        self.execute_manifest_with_variant_snapshot_vm3(&manifest)
    }

    /// The manifest-level vm3 entry (see
    /// [`Self::execute_source_with_variant_snapshot_vm3`]).
    pub fn execute_manifest_with_variant_snapshot_vm3(
        &self,
        manifest: &oxvba_symbol::manifest::SymbolProjectManifest,
    ) -> Vm3Snapshot {
        let program = match oxvba_bind::bind_program(manifest, &*self.typelib_resolver) {
            Ok(p) => p,
            Err(e) => return Vm3Snapshot::Failed(format!("bind: {e:?}")),
        };
        let oxp = match oxvba_oxir::elaborate::elaborate(&program) {
            Ok(o) => o,
            Err(oxvba_oxir::elaborate::ElaborateError::Unimplemented { what }) => {
                return Vm3Snapshot::Unsupported(format!("elaborate: {what}"));
            }
            Err(e) => return Vm3Snapshot::Failed(format!("elaborate: {e}")),
        };
        let vm = match oxvba_vm3::Vm3::run(&oxp, &*self.host_services) {
            Ok(v) => v,
            Err(oxvba_vm3::Vm3Error::Unimplemented { what }) => {
                return Vm3Snapshot::Unsupported(format!("vm3: {what}"));
            }
            Err(e) => return Vm3Snapshot::Failed(format!("vm3: {e}")),
        };
        // The same snapshot shape vm2 exposes: module globals (vm3's global table is 1:1
        // with the Core IR globals, exactly as the linearized bundle's) followed by the
        // entry `Main` frame's locals.
        let local_count = program
            .entry
            .and_then(|entry| program.procs.get(entry.0))
            .map(|main| main.locals.len())
            .unwrap_or(0);
        let count = oxp.globals.len() + local_count;
        let values = (0..count)
            .map(|slot| vm.slot(slot).unwrap_or_else(Variant::empty))
            .collect();
        Vm3Snapshot::Ran(values)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticPhase, Engine, HostConfig};
    use oxvba_bundle::{BundlePackage, ProjectMemberKind};
    use oxvba_runtime::Variant;
    use oxvba_symbol::manifest::{
        ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest,
    };

    #[test]
    fn phase_diagnostic_exposes_stable_code() {
        let engine = Engine::new(HostConfig { enable_jit: true });
        let err = engine
            .execute_source_with_variant_snapshot_clean("Sub Main()\nEnd Sub\n")
            .expect_err("JIT path should return a diagnostic");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert_eq!(err.diagnostic().code.as_str(), "RUN-E-JIT-NOT-IMPLEMENTED");
        assert!(err.message().contains("JIT execution"));
    }

    #[test]
    fn package_session_can_create_class_and_invoke_member() {
        let mut attrs = ModuleAttributes::named("Calculator");
        attrs.vb_exposed = true;
        attrs.vb_creatable = true;
        let manifest = SymbolProjectManifest {
            project_name: "DemoServer".to_string(),
            project_kind: ProjectKind::Library,
            modules: vec![ModuleUnit {
                module_name: "Calculator".to_string(),
                module_kind: ModuleKind::Class,
                attributes: attrs,
                source: r#"
Public Function Add(ByVal a As Long, ByVal b As Long) As Long
    Add = a + b
End Function
"#
                .to_string(),
            }],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };
        let typelibs = oxvba_symbol::CatalogTypeLibResolver;
        let program = oxvba_bind::bind_program(&manifest, &typelibs).expect("bind");
        let bundle = oxvba_bundle::linearize(&program).expect("linearize");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_bundle_package_session(BundlePackage::single(bundle))
            .expect("prepare session");

        let object = session
            .create_class_instance("Calculator")
            .expect("create class instance");
        let result = session
            .invoke_member_values(
                object,
                "Add",
                Some(ProjectMemberKind::Method),
                vec![Variant::from_i32(2), Variant::from_i32(3)],
            )
            .expect("invoke Add");
        assert_eq!(result.as_i32(), Some(5));
    }

    #[test]
    fn package_session_project_event_sink_receives_raise_event() {
        let mut attrs = ModuleAttributes::named("Notifier");
        attrs.vb_exposed = true;
        attrs.vb_creatable = true;
        let manifest = SymbolProjectManifest {
            project_name: "DemoServer".to_string(),
            project_kind: ProjectKind::Library,
            modules: vec![ModuleUnit {
                module_name: "Notifier".to_string(),
                module_kind: ModuleKind::Class,
                attributes: attrs,
                source: r#"
Public Event Changed(ByVal value As Long)

Public Sub Fire(ByVal value As Long)
    RaiseEvent Changed(value)
End Sub
"#
                .to_string(),
            }],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };
        let typelibs = oxvba_symbol::CatalogTypeLibResolver;
        let program = oxvba_bind::bind_program(&manifest, &typelibs).expect("bind");
        let bundle = oxvba_bundle::linearize(&program).expect("linearize");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_bundle_package_session(BundlePackage::single(bundle))
            .expect("prepare session");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_sink = seen.clone();
        session.set_project_event_sink(move |source, event_id, args| {
            seen_sink.lock().expect("seen lock").push((
                source.raw(),
                event_id,
                args.first().and_then(Variant::as_i32),
            ));
            Ok(())
        });

        let object = session
            .create_class_instance("Notifier")
            .expect("create class instance");
        let raw = object.raw();
        session
            .invoke_member_values(
                object,
                "Fire",
                Some(ProjectMemberKind::Method),
                vec![Variant::from_i32(42)],
            )
            .expect("invoke Fire");

        assert_eq!(*seen.lock().expect("seen lock"), vec![(raw, 0, Some(42))]);
    }
}
