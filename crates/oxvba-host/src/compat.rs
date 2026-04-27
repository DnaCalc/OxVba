//! Explicit compatibility adapters for legacy host observation surfaces.
//!
//! Host execution and debugger/immediate observation should prefer retained
//! `Variant` values. This module contains the deliberate projections needed by
//! older callers that still consume `RuntimeValue` snapshots or legacy slot
//! dumps.

use oxvba_compiler::{OxBundle, ProjectManifest};
use oxvba_runtime::{RuntimeValue, Variant};

use crate::{
    Engine, ImmediateSession, PhaseDiagnostic, ProjectRuntimeSession,
    engine::project_variants_to_legacy_slots,
};

pub fn project_session_snapshot_values(session: &ProjectRuntimeSession) -> Vec<RuntimeValue> {
    project_variants_to_runtime_values(session.snapshot_variants())
        .expect("project runtime session VARIANT snapshot should project")
}

pub fn project_session_snapshot_slots(session: &ProjectRuntimeSession) -> Vec<i32> {
    project_variants_to_legacy_slots(session.snapshot_variants())
}

pub fn project_session_read_slot(session: &ProjectRuntimeSession, slot: usize) -> RuntimeValue {
    session
        .read_variant_slot(slot)
        .to_runtime_value()
        .unwrap_or(RuntimeValue::Empty)
}

pub fn immediate_session_snapshot_values(session: &ImmediateSession<'_>) -> Vec<RuntimeValue> {
    project_session_snapshot_values(session.runtime())
}

pub fn execute_source_with_snapshot(
    engine: &Engine,
    source: &str,
) -> Result<Vec<RuntimeValue>, String> {
    execute_source_with_snapshot_phased(engine, source)
        .map_err(|diagnostic| diagnostic.message().to_string())
}

pub fn execute_source_with_snapshot_phased(
    engine: &Engine,
    source: &str,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    project_variants_to_runtime_values(engine.execute_source_with_variant_snapshot_phased(source)?)
}

pub fn execute_project_with_snapshot_phased(
    engine: &Engine,
    manifest: &ProjectManifest,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    project_variants_to_runtime_values(
        engine.execute_project_with_variant_snapshot_phased(manifest)?,
    )
}

pub fn execute_bundle_with_snapshot(
    engine: &Engine,
    bundle: &OxBundle,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    project_variants_to_runtime_values(engine.execute_bundle_with_variant_snapshot(bundle)?)
}

pub fn project_variants_to_runtime_values(
    values: Vec<Variant>,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    values
        .into_iter()
        .map(|value| value.to_runtime_value().map_err(PhaseDiagnostic::runtime))
        .collect()
}
