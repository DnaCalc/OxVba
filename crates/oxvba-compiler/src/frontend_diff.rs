use std::collections::BTreeMap;

use crate::{
    Bytecode, CompileError, ProcedureRuntimeMetadata, compile_with_runtime_metadata, syntax_bridge,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendPath {
    Legacy,
    FrontendV2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeSummaryStatus {
    Available(BytecodeSummary),
    NotAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeSummary {
    pub instruction_count: usize,
    pub slot_count: usize,
    pub user_slot_count: usize,
    pub external_call_count: usize,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataSummaryStatus {
    Available(MetadataSummary),
    NotAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataSummary {
    pub procedure_count: usize,
    pub procedures: BTreeMap<String, ProcedureMetadataSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcedureMetadataSummary {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
    pub statement_line_numbers: Vec<usize>,
    pub statement_entry_pcs: Vec<usize>,
    pub slots: Vec<SlotMetadataSummary>,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
    pub param_types: Vec<String>,
    pub return_type: Option<String>,
    pub signature: String,
    pub call_sites: Vec<String>,
    pub array_shapes: Vec<String>,
    pub udt_types: Vec<String>,
    pub object_types: Vec<String>,
    pub carrier_layouts: Vec<String>,
    pub value_states: Vec<String>,
    pub expression_semantics: Vec<String>,
    pub operator_semantics: Vec<String>,
    pub coercions: Vec<String>,
    pub name_bindings: Vec<String>,
    pub object_member_bindings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotMetadataSummary {
    pub name: String,
    pub slot: usize,
    pub kind: String,
    pub declared_type: String,
    pub initial_state: String,
    pub carrier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeObservationStatus {
    NotRun(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendObservation {
    pub path: FrontendPath,
    pub diagnostics: Vec<String>,
    pub bytecode: BytecodeSummaryStatus,
    pub metadata: MetadataSummaryStatus,
    pub execution_trace: RuntimeObservationStatus,
    pub observable_output: RuntimeObservationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendDiffReport {
    pub left: FrontendObservation,
    pub right: FrontendObservation,
    pub diagnostics_match: bool,
    pub bytecode_matches: bool,
    pub metadata_matches: bool,
    pub execution_trace_matches: bool,
    pub observable_output_matches: bool,
}

pub fn compare_legacy_to_legacy(source: &str) -> FrontendDiffReport {
    make_report(
        observe_frontend(source, FrontendPath::Legacy),
        observe_frontend(source, FrontendPath::Legacy),
    )
}

pub fn compare_legacy_to_frontend_v2(source: &str) -> FrontendDiffReport {
    make_report(
        observe_frontend(source, FrontendPath::Legacy),
        observe_frontend(source, FrontendPath::FrontendV2),
    )
}

pub fn observe_frontend(source: &str, path: FrontendPath) -> FrontendObservation {
    let compiled = match path {
        FrontendPath::Legacy => compile_with_runtime_metadata(source),
        FrontendPath::FrontendV2 => syntax_bridge::validate_source_with_cst(source)
            .map_err(|err| CompileError::ResolveError(format!("frontend_v2 bridge error: {err}")))
            .and_then(|()| compile_with_runtime_metadata(source)),
    };

    match compiled {
        Ok((bytecode, metadata)) => FrontendObservation {
            path,
            diagnostics: Vec::new(),
            bytecode: BytecodeSummaryStatus::Available(summarize_bytecode(&bytecode)),
            metadata: MetadataSummaryStatus::Available(summarize_metadata(&metadata)),
            execution_trace: runtime_not_run(),
            observable_output: runtime_not_run(),
        },
        Err(err) => FrontendObservation {
            path,
            diagnostics: vec![err.to_string()],
            bytecode: BytecodeSummaryStatus::NotAvailable,
            metadata: MetadataSummaryStatus::NotAvailable,
            execution_trace: runtime_not_run(),
            observable_output: runtime_not_run(),
        },
    }
}

fn make_report(left: FrontendObservation, right: FrontendObservation) -> FrontendDiffReport {
    FrontendDiffReport {
        diagnostics_match: left.diagnostics == right.diagnostics,
        bytecode_matches: left.bytecode == right.bytecode,
        metadata_matches: left.metadata == right.metadata,
        execution_trace_matches: left.execution_trace == right.execution_trace,
        observable_output_matches: left.observable_output == right.observable_output,
        left,
        right,
    }
}

fn summarize_bytecode(bytecode: &Bytecode) -> BytecodeSummary {
    BytecodeSummary {
        instruction_count: bytecode.instructions.len(),
        slot_count: bytecode.slot_count,
        user_slot_count: bytecode.user_slot_count,
        external_call_count: bytecode.external_call_descriptors.len(),
        instructions: bytecode
            .instructions
            .iter()
            .map(|instruction| format!("{instruction:?}"))
            .collect(),
    }
}

fn summarize_metadata(metadata: &BTreeMap<String, ProcedureRuntimeMetadata>) -> MetadataSummary {
    MetadataSummary {
        procedure_count: metadata.len(),
        procedures: metadata
            .iter()
            .map(|(key, proc)| (key.clone(), summarize_procedure_metadata(proc)))
            .collect(),
    }
}

fn summarize_procedure_metadata(proc: &ProcedureRuntimeMetadata) -> ProcedureMetadataSummary {
    ProcedureMetadataSummary {
        module_name: proc.module_name.clone(),
        procedure_name: proc.procedure_name.clone(),
        entry_pc: proc.entry_pc,
        source_line_start: proc.source_line_start,
        source_line_end: proc.source_line_end,
        statement_line_numbers: proc.statement_line_numbers.clone(),
        statement_entry_pcs: proc.statement_entry_pcs.clone(),
        slots: proc
            .slots
            .iter()
            .map(|slot| SlotMetadataSummary {
                name: slot.name.clone(),
                slot: slot.slot,
                kind: format!("{:?}", slot.kind),
                declared_type: format!("{:?}", slot.declared_type),
                initial_state: format!("{:?}", slot.initial_state),
                carrier: format!("{:?}", slot.carrier),
            })
            .collect(),
        param_slots: proc.param_slots.clone(),
        return_slot: proc.return_slot,
        param_types: proc
            .param_types
            .iter()
            .map(|param_type| format!("{param_type:?}"))
            .collect(),
        return_type: proc
            .return_type
            .as_ref()
            .map(|return_type| format!("{return_type:?}")),
        signature: format!("{:?}", proc.signature),
        call_sites: debug_vec(&proc.call_sites),
        array_shapes: debug_vec(&proc.array_shapes),
        udt_types: debug_vec(&proc.udt_types),
        object_types: debug_vec(&proc.object_types),
        carrier_layouts: debug_vec(&proc.carrier_layouts),
        value_states: debug_vec(&proc.value_states),
        expression_semantics: debug_vec(&proc.expression_semantics),
        operator_semantics: debug_vec(&proc.operator_semantics),
        coercions: debug_vec(&proc.coercions),
        name_bindings: debug_vec(&proc.name_bindings),
        object_member_bindings: debug_vec(&proc.object_member_bindings),
    }
}

fn runtime_not_run() -> RuntimeObservationStatus {
    RuntimeObservationStatus::NotRun(
        "compiler-layer harness does not execute VM traces or host-visible output".to_string(),
    )
}

fn debug_vec<T: std::fmt::Debug>(items: &[T]) -> Vec<String> {
    items.iter().map(|item| format!("{item:?}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_diff_old_vs_old_is_stable() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let report = compare_legacy_to_legacy(source);
        assert!(report.diagnostics_match, "{report:#?}");
        assert!(report.bytecode_matches, "{report:#?}");
        assert!(report.metadata_matches, "{report:#?}");
        assert!(matches!(
            report.left.bytecode,
            BytecodeSummaryStatus::Available(_)
        ));
        assert!(matches!(
            report.left.metadata,
            MetadataSummaryStatus::Available(_)
        ));
    }

    #[test]
    fn frontend_diff_v2_smoke_matches_legacy_for_supported_assignment() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let report = compare_legacy_to_frontend_v2(source);
        assert!(report.diagnostics_match, "{report:#?}");
        assert!(report.bytecode_matches, "{report:#?}");
        assert!(report.metadata_matches, "{report:#?}");
        assert!(matches!(
            report.right.bytecode,
            BytecodeSummaryStatus::Available(_)
        ));
        assert!(matches!(
            report.right.metadata,
            MetadataSummaryStatus::Available(_)
        ));
    }

    #[test]
    fn frontend_diff_captures_v2_syntax_error() {
        let source = "Sub Main()\n    If Then\nEnd Sub\n";
        let report = compare_legacy_to_frontend_v2(source);
        assert!(
            !report.right.diagnostics.is_empty(),
            "expected v2 diagnostics in report: {report:#?}"
        );
        assert!(matches!(
            report.right.bytecode,
            BytecodeSummaryStatus::NotAvailable
        ));
        assert!(matches!(
            report.right.metadata,
            MetadataSummaryStatus::NotAvailable
        ));
    }
}
