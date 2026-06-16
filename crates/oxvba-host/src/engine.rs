//! Engine: the host orchestration entry point for the clean execution stack.
//!
//! `Engine` configures host services (HAL profile / policy / callbacks) and runs
//! VBA on the clean pipeline — `oxvba_bind` → `oxvba_bundle::linearize` →
//! `oxvba_vm2` — for a single source module (optionally carrying typelib/native/host
//! references, so an early-bound COM call reaches the resolver) or a `.basproj`
//! project closure. The
//! legacy compiler/VM execution path (and its COM-event / session / immediate-window
//! machinery) was removed with `oxvba-compiler`/`oxvba-vm`; see git history.

use std::sync::Arc;

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

use crate::runner::RuntimeProfileId;

const JIT_NOT_IMPLEMENTED_MESSAGE: &str =
    "JIT execution is not implemented; the clean stack runs on the oxvba_vm2 interpreter";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    CompileTime,
    Runtime,
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

pub struct Engine {
    config: HostConfig,
    runtime_profile: RuntimeProfileId,
    host_callbacks: Option<Arc<dyn HostCallbacks>>,
    host_services: Arc<dyn HostServices>,
}

pub struct ProjectRuntimeSession {
    vm: oxvba_vm2::Vm<'static>,
    entry_bundle: usize,
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
    pub fn new(config: HostConfig) -> Self {
        let runtime_profile = RuntimeProfileId::default_for_hal_profile(native_host_profile());
        let mut policy = HostPolicy::deterministic_runtime();
        policy.runtime_class = Some(runtime_profile.runtime_class());
        Self {
            config,
            runtime_profile,
            host_callbacks: None,
            host_services: build_host_services(
                runtime_profile.hal_profile(),
                runtime_profile.runtime_class(),
                policy,
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
            host_services,
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
        Ok(ProjectRuntimeSession { vm, entry_bundle })
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
        let typelibs = oxvba_symbol::CatalogTypeLibResolver;
        let programs = oxvba_bind::bind_projects(closure, &typelibs)
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
        let typelibs = oxvba_symbol::CatalogTypeLibResolver;
        let program = oxvba_bind::bind_program(manifest, &typelibs)
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
}
