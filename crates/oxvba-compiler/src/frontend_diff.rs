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
    pub metadata_differences: Vec<String>,
    pub execution_trace_matches: bool,
    pub observable_output_matches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedBytecodeDrift {
    Bug,
    HarmlessDrift,
    IntentionalImprovement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedDiagnosticDrift {
    Bug,
    IntentionalImprovement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffClassificationInput {
    pub fixture_name: String,
    pub fixture_path: String,
    pub expected_bytecode_drift: Option<ExpectedBytecodeDrift>,
    pub expected_diagnostic_drift: Option<ExpectedDiagnosticDrift>,
    pub rationale: String,
    pub close_condition: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffClassificationKind {
    Equivalent,
    Bug,
    HarmlessDrift,
    IntentionalImprovement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffClassification {
    pub kind: DiffClassificationKind,
    pub fixture_name: String,
    pub fixture_path: String,
    pub summary: String,
    pub close_condition: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendCorpusClass {
    CompilerUnit,
    ConformanceCase,
    HostProject,
    ExcelOracle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendCorpusFixture {
    pub name: String,
    pub fixture_path: String,
    pub class: FrontendCorpusClass,
    pub source: Option<String>,
    pub expected_bytecode_drift: Option<ExpectedBytecodeDrift>,
    pub expected_diagnostic_drift: Option<ExpectedDiagnosticDrift>,
    pub rationale: String,
    pub close_condition: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendCorpusRowStatus {
    Ran,
    SkippedResidual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendCorpusRow {
    pub name: String,
    pub fixture_path: String,
    pub class: FrontendCorpusClass,
    pub status: FrontendCorpusRowStatus,
    pub skip_reason: Option<String>,
    pub classification: Option<DiffClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendCorpusReport {
    pub rows: Vec<FrontendCorpusRow>,
    pub ran_count: usize,
    pub skipped_count: usize,
    pub bug_count: usize,
    pub harmless_drift_count: usize,
    pub intentional_improvement_count: usize,
    pub equivalent_count: usize,
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

pub fn classify_frontend_diff(
    report: &FrontendDiffReport,
    input: DiffClassificationInput,
) -> DiffClassification {
    let mut reasons = Vec::new();
    if !report.diagnostics_match {
        reasons.push("diagnostics differ".to_string());
    }
    if !report.bytecode_matches {
        reasons.push("bytecode summary differs".to_string());
    }
    if !report.metadata_matches {
        if report.metadata_differences.is_empty() {
            reasons.push("metadata summary differs".to_string());
        } else {
            reasons.extend(
                report
                    .metadata_differences
                    .iter()
                    .map(|diff| format!("metadata summary differs: {diff}")),
            );
        }
    }
    if !report.execution_trace_matches {
        reasons.push("execution trace differs".to_string());
    }
    if !report.observable_output_matches {
        reasons.push("observable output differs".to_string());
    }

    let missing_policy =
        input.rationale.trim().is_empty() || input.close_condition.trim().is_empty();
    let kind = if reasons.is_empty() {
        DiffClassificationKind::Equivalent
    } else if missing_policy {
        DiffClassificationKind::Bug
    } else if !report.diagnostics_match {
        match (
            input.expected_diagnostic_drift,
            has_one_sided_bytecode_availability(report),
        ) {
            (Some(ExpectedDiagnosticDrift::IntentionalImprovement), true) => {
                DiffClassificationKind::IntentionalImprovement
            }
            (Some(ExpectedDiagnosticDrift::IntentionalImprovement), false)
            | (Some(ExpectedDiagnosticDrift::Bug), _)
            | (None, _) => DiffClassificationKind::Bug,
        }
    } else if !report.metadata_matches
        || !report.execution_trace_matches
        || !report.observable_output_matches
    {
        DiffClassificationKind::Bug
    } else if !report.bytecode_matches {
        match input.expected_bytecode_drift {
            Some(ExpectedBytecodeDrift::HarmlessDrift) => DiffClassificationKind::HarmlessDrift,
            Some(ExpectedBytecodeDrift::IntentionalImprovement) => {
                DiffClassificationKind::IntentionalImprovement
            }
            Some(ExpectedBytecodeDrift::Bug) | None => DiffClassificationKind::Bug,
        }
    } else {
        DiffClassificationKind::Bug
    };

    if missing_policy && !reasons.is_empty() {
        reasons.push("missing rationale or close condition".to_string());
    }

    DiffClassification {
        kind,
        fixture_name: input.fixture_name,
        fixture_path: input.fixture_path,
        summary: classification_summary(kind),
        close_condition: input.close_condition,
        reasons,
    }
}

pub fn run_frontend_diff_corpus(fixtures: &[FrontendCorpusFixture]) -> FrontendCorpusReport {
    let rows: Vec<_> = fixtures.iter().map(run_frontend_corpus_fixture).collect();
    let ran_count = rows
        .iter()
        .filter(|row| row.status == FrontendCorpusRowStatus::Ran)
        .count();
    let skipped_count = rows.len() - ran_count;
    let bug_count = count_classification(&rows, DiffClassificationKind::Bug);
    let harmless_drift_count = count_classification(&rows, DiffClassificationKind::HarmlessDrift);
    let intentional_improvement_count =
        count_classification(&rows, DiffClassificationKind::IntentionalImprovement);
    let equivalent_count = count_classification(&rows, DiffClassificationKind::Equivalent);
    FrontendCorpusReport {
        rows,
        ran_count,
        skipped_count,
        bug_count,
        harmless_drift_count,
        intentional_improvement_count,
        equivalent_count,
    }
}

pub fn observe_frontend(source: &str, path: FrontendPath) -> FrontendObservation {
    let compiled = match path {
        FrontendPath::Legacy => compile_with_runtime_metadata(source),
        FrontendPath::FrontendV2 => {
            syntax_bridge::compile_source_with_runtime_metadata_via_syntax_bridge(source).map_err(
                |err| CompileError::ResolveError(format!("frontend_v2 bridge error: {err}")),
            )
        }
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

fn run_frontend_corpus_fixture(fixture: &FrontendCorpusFixture) -> FrontendCorpusRow {
    let Some(source) = fixture.source.as_deref() else {
        return skipped_corpus_row(fixture, "fixture has no inline source for compiler harness");
    };
    if !matches!(
        fixture.class,
        FrontendCorpusClass::CompilerUnit | FrontendCorpusClass::ConformanceCase
    ) {
        return skipped_corpus_row(
            fixture,
            "fixture class requires VM, host project, or oracle runner",
        );
    }

    let diff = compare_legacy_to_frontend_v2(source);
    let classification = classify_frontend_diff(
        &diff,
        DiffClassificationInput {
            fixture_name: fixture.name.clone(),
            fixture_path: fixture.fixture_path.clone(),
            expected_bytecode_drift: fixture.expected_bytecode_drift,
            expected_diagnostic_drift: fixture.expected_diagnostic_drift,
            rationale: fixture.rationale.clone(),
            close_condition: fixture.close_condition.clone(),
        },
    );
    FrontendCorpusRow {
        name: fixture.name.clone(),
        fixture_path: fixture.fixture_path.clone(),
        class: fixture.class,
        status: FrontendCorpusRowStatus::Ran,
        skip_reason: None,
        classification: Some(classification),
    }
}

fn skipped_corpus_row(fixture: &FrontendCorpusFixture, reason: &str) -> FrontendCorpusRow {
    FrontendCorpusRow {
        name: fixture.name.clone(),
        fixture_path: fixture.fixture_path.clone(),
        class: fixture.class,
        status: FrontendCorpusRowStatus::SkippedResidual,
        skip_reason: Some(reason.to_string()),
        classification: None,
    }
}

fn count_classification(rows: &[FrontendCorpusRow], kind: DiffClassificationKind) -> usize {
    rows.iter()
        .filter(|row| {
            row.classification
                .as_ref()
                .is_some_and(|classification| classification.kind == kind)
        })
        .count()
}

fn make_report(left: FrontendObservation, right: FrontendObservation) -> FrontendDiffReport {
    let metadata_differences = diff_metadata_status(&left.metadata, &right.metadata);
    FrontendDiffReport {
        diagnostics_match: left.diagnostics == right.diagnostics,
        bytecode_matches: left.bytecode == right.bytecode,
        metadata_matches: left.metadata == right.metadata,
        metadata_differences,
        execution_trace_matches: left.execution_trace == right.execution_trace,
        observable_output_matches: left.observable_output == right.observable_output,
        left,
        right,
    }
}

fn diff_metadata_status(
    left: &MetadataSummaryStatus,
    right: &MetadataSummaryStatus,
) -> Vec<String> {
    match (left, right) {
        (MetadataSummaryStatus::Available(left), MetadataSummaryStatus::Available(right)) => {
            diff_metadata_summary(left, right)
        }
        (MetadataSummaryStatus::NotAvailable, MetadataSummaryStatus::NotAvailable) => Vec::new(),
        _ => vec!["availability".to_string()],
    }
}

fn diff_metadata_summary(left: &MetadataSummary, right: &MetadataSummary) -> Vec<String> {
    let mut diffs = Vec::new();
    push_diff(
        &mut diffs,
        "procedure_count",
        &left.procedure_count,
        &right.procedure_count,
    );
    for key in left.procedures.keys() {
        if !right.procedures.contains_key(key) {
            diffs.push(format!("procedures.{key}: missing on right"));
        }
    }
    for key in right.procedures.keys() {
        if !left.procedures.contains_key(key) {
            diffs.push(format!("procedures.{key}: missing on left"));
        }
    }
    for (key, left_proc) in &left.procedures {
        let Some(right_proc) = right.procedures.get(key) else {
            continue;
        };
        diff_procedure_metadata(&mut diffs, key, left_proc, right_proc);
    }
    diffs
}

fn diff_procedure_metadata(
    diffs: &mut Vec<String>,
    key: &str,
    left: &ProcedureMetadataSummary,
    right: &ProcedureMetadataSummary,
) {
    let prefix = format!("procedures.{key}");
    push_diff(
        diffs,
        &format!("{prefix}.module_name"),
        &left.module_name,
        &right.module_name,
    );
    push_diff(
        diffs,
        &format!("{prefix}.procedure_name"),
        &left.procedure_name,
        &right.procedure_name,
    );
    push_diff(
        diffs,
        &format!("{prefix}.entry_pc"),
        &left.entry_pc,
        &right.entry_pc,
    );
    push_diff(
        diffs,
        &format!("{prefix}.source_line_start"),
        &left.source_line_start,
        &right.source_line_start,
    );
    push_diff(
        diffs,
        &format!("{prefix}.source_line_end"),
        &left.source_line_end,
        &right.source_line_end,
    );
    push_diff(
        diffs,
        &format!("{prefix}.statement_line_numbers"),
        &left.statement_line_numbers,
        &right.statement_line_numbers,
    );
    push_diff(
        diffs,
        &format!("{prefix}.statement_entry_pcs"),
        &left.statement_entry_pcs,
        &right.statement_entry_pcs,
    );
    push_diff(diffs, &format!("{prefix}.slots"), &left.slots, &right.slots);
    push_diff(
        diffs,
        &format!("{prefix}.param_slots"),
        &left.param_slots,
        &right.param_slots,
    );
    push_diff(
        diffs,
        &format!("{prefix}.return_slot"),
        &left.return_slot,
        &right.return_slot,
    );
    push_diff(
        diffs,
        &format!("{prefix}.param_types"),
        &left.param_types,
        &right.param_types,
    );
    push_diff(
        diffs,
        &format!("{prefix}.return_type"),
        &left.return_type,
        &right.return_type,
    );
    push_diff(
        diffs,
        &format!("{prefix}.signature"),
        &left.signature,
        &right.signature,
    );
    push_diff(
        diffs,
        &format!("{prefix}.call_sites"),
        &left.call_sites,
        &right.call_sites,
    );
    push_diff(
        diffs,
        &format!("{prefix}.array_shapes"),
        &left.array_shapes,
        &right.array_shapes,
    );
    push_diff(
        diffs,
        &format!("{prefix}.udt_types"),
        &left.udt_types,
        &right.udt_types,
    );
    push_diff(
        diffs,
        &format!("{prefix}.object_types"),
        &left.object_types,
        &right.object_types,
    );
    push_diff(
        diffs,
        &format!("{prefix}.carrier_layouts"),
        &left.carrier_layouts,
        &right.carrier_layouts,
    );
    push_diff(
        diffs,
        &format!("{prefix}.value_states"),
        &left.value_states,
        &right.value_states,
    );
    push_diff(
        diffs,
        &format!("{prefix}.expression_semantics"),
        &left.expression_semantics,
        &right.expression_semantics,
    );
    push_diff(
        diffs,
        &format!("{prefix}.operator_semantics"),
        &left.operator_semantics,
        &right.operator_semantics,
    );
    push_diff(
        diffs,
        &format!("{prefix}.coercions"),
        &left.coercions,
        &right.coercions,
    );
    push_diff(
        diffs,
        &format!("{prefix}.name_bindings"),
        &left.name_bindings,
        &right.name_bindings,
    );
    push_diff(
        diffs,
        &format!("{prefix}.object_member_bindings"),
        &left.object_member_bindings,
        &right.object_member_bindings,
    );
}

fn push_diff<T: PartialEq>(diffs: &mut Vec<String>, path: &str, left: &T, right: &T) {
    if left != right {
        diffs.push(path.to_string());
    }
}

fn has_one_sided_bytecode_availability(report: &FrontendDiffReport) -> bool {
    matches!(
        (&report.left.bytecode, &report.right.bytecode),
        (
            BytecodeSummaryStatus::Available(_),
            BytecodeSummaryStatus::NotAvailable
        ) | (
            BytecodeSummaryStatus::NotAvailable,
            BytecodeSummaryStatus::Available(_)
        )
    )
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

fn classification_summary(kind: DiffClassificationKind) -> String {
    match kind {
        DiffClassificationKind::Equivalent => "no semantic or bytecode drift detected",
        DiffClassificationKind::Bug => {
            "diff must be fixed or explicitly reclassified with evidence"
        }
        DiffClassificationKind::HarmlessDrift => {
            "bytecode differs but diagnostics, metadata, execution trace, and output match"
        }
        DiffClassificationKind::IntentionalImprovement => {
            "bytecode differs because the new front-end intentionally improves documented behavior"
        }
    }
    .to_string()
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
    fn frontend_diff_v2_uses_bridge_compile_route_for_inline_statements() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let report = compare_legacy_to_frontend_v2(source);
        assert!(
            !report.left.diagnostics.is_empty(),
            "legacy path should still reject this inline sequence: {report:#?}"
        );
        assert!(
            report.right.diagnostics.is_empty(),
            "v2 bridge path should compile the CST-accepted inline sequence: {report:#?}"
        );
        assert!(matches!(
            report.left.bytecode,
            BytecodeSummaryStatus::NotAvailable
        ));
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

    #[test]
    fn diff_classifier_marks_identical_report_equivalent() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let report = compare_legacy_to_frontend_v2(source);
        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "supported_assignment".to_string(),
                fixture_path: "inline:frontend_diff".to_string(),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: None,
                rationale: String::new(),
                close_condition: String::new(),
            },
        );
        assert_eq!(classification.kind, DiffClassificationKind::Equivalent);
        assert!(classification.reasons.is_empty(), "{classification:#?}");
    }

    #[test]
    fn diff_classifier_requires_policy_for_bytecode_drift() {
        let report = bytecode_drift_report();
        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "synthetic_missing_policy".to_string(),
                fixture_path: "inline:frontend_diff".to_string(),
                expected_bytecode_drift: Some(ExpectedBytecodeDrift::HarmlessDrift),
                expected_diagnostic_drift: None,
                rationale: String::new(),
                close_condition: String::new(),
            },
        );
        assert_eq!(classification.kind, DiffClassificationKind::Bug);
        assert!(
            classification
                .reasons
                .contains(&"missing rationale or close condition".to_string()),
            "{classification:#?}"
        );
    }

    #[test]
    fn diff_classifier_classifies_documented_harmless_bytecode_drift() {
        let report = bytecode_drift_report();
        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "synthetic_slot_reuse_drift".to_string(),
                fixture_path:
                    "docs/evidence/frontend_rework/DIFF_CLASSIFIER_2026-06-01.md#fixture-1"
                        .to_string(),
                expected_bytecode_drift: Some(ExpectedBytecodeDrift::HarmlessDrift),
                expected_diagnostic_drift: None,
                rationale:
                    "alternate lowering reuses temporaries while preserving metadata and output"
                        .to_string(),
                close_condition:
                    "keep as harmless only while diagnostics, metadata, execution, and output match"
                        .to_string(),
            },
        );
        assert_eq!(classification.kind, DiffClassificationKind::HarmlessDrift);
        assert_eq!(
            classification.reasons,
            vec!["bytecode summary differs".to_string()]
        );
    }

    #[test]
    fn diff_classifier_classifies_documented_intentional_improvement() {
        let report = bytecode_drift_report();
        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "synthetic_legacy_divergence_fix".to_string(),
                fixture_path:
                    "docs/evidence/frontend_rework/DIFF_CLASSIFIER_2026-06-01.md#fixture-2"
                        .to_string(),
                expected_bytecode_drift: Some(ExpectedBytecodeDrift::IntentionalImprovement),
                expected_diagnostic_drift: None,
                rationale: "new lowering fixes a documented legacy divergence".to_string(),
                close_condition:
                    "requires fixture evidence linking the divergence and expected VBA behavior"
                        .to_string(),
            },
        );
        assert_eq!(
            classification.kind,
            DiffClassificationKind::IntentionalImprovement
        );
    }

    #[test]
    fn diff_classifier_requires_policy_for_diagnostic_improvement() {
        let report = inline_statement_improvement_report();
        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "inline_statement_without_policy".to_string(),
                fixture_path: "inline:frontend_diff::inline_statement_improvement_report"
                    .to_string(),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: None,
                rationale: "legacy rejects a v2 accepted inline statement sequence".to_string(),
                close_condition: "requires v2 compile success and follow-up execution evidence"
                    .to_string(),
            },
        );
        assert_eq!(classification.kind, DiffClassificationKind::Bug);
        assert!(
            classification
                .reasons
                .contains(&"diagnostics differ".to_string()),
            "{classification:#?}"
        );
    }

    #[test]
    fn diff_classifier_classifies_documented_diagnostic_improvement() {
        let report = inline_statement_improvement_report();
        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "inline_statement_separator_bridge_improvement".to_string(),
                fixture_path:
                    "docs/evidence/frontend_rework/DIFF_CLASSIFIER_2026-06-01.md#fixture-3"
                        .to_string(),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: Some(ExpectedDiagnosticDrift::IntentionalImprovement),
                rationale:
                    "v2 accepts a CST-valid inline statement sequence that legacy-default rejects"
                        .to_string(),
                close_condition:
                    "keep as improvement only while v2 compiles and FE-5.4 adds execution evidence"
                        .to_string(),
            },
        );
        assert_eq!(
            classification.kind,
            DiffClassificationKind::IntentionalImprovement
        );
        assert!(
            classification
                .reasons
                .contains(&"diagnostics differ".to_string()),
            "{classification:#?}"
        );
    }

    #[test]
    fn diff_classifier_rejects_diagnostic_policy_when_both_sides_compile() {
        let mut report =
            compare_legacy_to_frontend_v2("Sub Main()\n    Dim x As Long\n    x = 1\nEnd Sub\n");
        report
            .right
            .diagnostics
            .push("synthetic warning drift".to_string());
        report.diagnostics_match = false;

        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "diagnostic_drift_without_acceptance_improvement".to_string(),
                fixture_path: "inline:frontend_diff::synthetic_warning_drift".to_string(),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: Some(ExpectedDiagnosticDrift::IntentionalImprovement),
                rationale: "synthetic diagnostic drift while both sides compile".to_string(),
                close_condition:
                    "should not be classified as an acceptance improvement without one-sided compile availability"
                        .to_string(),
            },
        );
        assert_eq!(classification.kind, DiffClassificationKind::Bug);
    }

    #[test]
    fn frontend_corpus_runner_runs_source_backed_rows_and_skips_residuals() {
        let fixtures = frontend_rework_seed_corpus();
        let report = run_frontend_diff_corpus(&fixtures);

        assert_eq!(report.ran_count, 3, "{report:#?}");
        assert_eq!(report.skipped_count, 2, "{report:#?}");
        assert_eq!(report.equivalent_count, 1, "{report:#?}");
        assert_eq!(report.intentional_improvement_count, 1, "{report:#?}");
        assert_eq!(report.bug_count, 1, "{report:#?}");
        let bug_rows: Vec<_> = report
            .rows
            .iter()
            .filter(|row| {
                row.classification.as_ref().is_some_and(|classification| {
                    classification.kind == DiffClassificationKind::Bug
                })
            })
            .collect();
        assert_eq!(bug_rows.len(), 1, "{report:#?}");
        assert_eq!(
            bug_rows[0].name, "conformance_call_coercion_mixed_variant_to_long",
            "{report:#?}"
        );
        assert_eq!(
            report.rows[3].status,
            FrontendCorpusRowStatus::SkippedResidual
        );
        assert!(
            report.rows[3]
                .skip_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("requires VM")),
            "{report:#?}"
        );
        assert_eq!(
            report.rows[4].status,
            FrontendCorpusRowStatus::SkippedResidual
        );
    }

    #[test]
    fn frontend_diff_metadata_projection_exposes_stable_descriptor_fields() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let report = compare_legacy_to_frontend_v2(source);
        let MetadataSummaryStatus::Available(metadata) = &report.left.metadata else {
            panic!("expected metadata summary: {report:#?}");
        };
        let main = metadata
            .procedures
            .values()
            .find(|procedure| procedure.procedure_name.eq_ignore_ascii_case("main"))
            .expect("main procedure metadata");
        assert!(!main.signature.is_empty(), "{main:#?}");
        let _stable_projection_fields = (
            &main.call_sites,
            &main.array_shapes,
            &main.udt_types,
            &main.object_types,
            &main.carrier_layouts,
            &main.value_states,
            &main.expression_semantics,
            &main.operator_semantics,
            &main.coercions,
            &main.name_bindings,
            &main.object_member_bindings,
        );
    }

    #[test]
    fn frontend_diff_metadata_projection_reports_field_level_drift() {
        let mut report = compare_legacy_to_frontend_v2(
            "Function Main(ByVal seed As Long) As Long\nMain = seed\nEnd Function\n",
        );
        let MetadataSummaryStatus::Available(metadata) = &mut report.right.metadata else {
            panic!("expected right metadata summary: {report:#?}");
        };
        let main = metadata
            .procedures
            .get_mut("main")
            .expect("main procedure metadata");
        main.return_slot = None;
        report.metadata_matches = report.left.metadata == report.right.metadata;
        report.metadata_differences =
            diff_metadata_status(&report.left.metadata, &report.right.metadata);

        assert!(!report.metadata_matches);
        assert!(
            report
                .metadata_differences
                .iter()
                .any(|diff| diff == "procedures.main.return_slot"),
            "{report:#?}"
        );

        let classification = classify_frontend_diff(
            &report,
            DiffClassificationInput {
                fixture_name: "metadata_drift".to_string(),
                fixture_path: "inline".to_string(),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: None,
                rationale: "metadata drift should be diagnosed semantically".to_string(),
                close_condition: "field-level metadata diff is investigated".to_string(),
            },
        );
        assert!(classification.reasons.iter().any(|reason| {
            reason.contains("metadata summary differs: procedures.main.return_slot")
        }));
    }

    fn bytecode_drift_report() -> FrontendDiffReport {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let mut report = compare_legacy_to_frontend_v2(source);
        match &mut report.right.bytecode {
            BytecodeSummaryStatus::Available(summary) => {
                summary
                    .instructions
                    .push("SyntheticAlternateLowering".to_string());
                summary.instruction_count += 1;
            }
            BytecodeSummaryStatus::NotAvailable => panic!("expected bytecode summary"),
        }
        report.bytecode_matches = false;
        report
    }

    fn inline_statement_improvement_report() -> FrontendDiffReport {
        compare_legacy_to_frontend_v2(
            "Sub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n",
        )
    }

    fn frontend_rework_seed_corpus() -> Vec<FrontendCorpusFixture> {
        vec![
            FrontendCorpusFixture {
                name: "examples_basic_arithmetic".to_string(),
                fixture_path: "examples/basic/arithmetic.bas".to_string(),
                class: FrontendCorpusClass::CompilerUnit,
                source: Some(include_str!("../../../examples/basic/arithmetic.bas").to_string()),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: None,
                rationale: String::new(),
                close_condition: String::new(),
            },
            FrontendCorpusFixture {
                name: "conformance_call_coercion_mixed_variant_to_long".to_string(),
                fixture_path: "conformance/tests/call_coercion_mixed_variant_to_long.bas"
                    .to_string(),
                class: FrontendCorpusClass::ConformanceCase,
                source: Some(
                    include_str!(
                        "../../../conformance/tests/call_coercion_mixed_variant_to_long.bas"
                    )
                    .to_string(),
                ),
                expected_bytecode_drift: Some(ExpectedBytecodeDrift::Bug),
                expected_diagnostic_drift: None,
                rationale:
                    "HIR production now reaches same-module procedure call statements, but FE-8.5 still owns eliminating the remaining bytecode and metadata drift for this call/coercion fixture"
                        .to_string(),
                close_condition:
                    "reclassify when the call/coercion fixture routes through HIR production with equivalent behavior, call-site metadata, and accepted bytecode drift only where deliberately improved"
                        .to_string(),
            },
            FrontendCorpusFixture {
                name: "inline_statement_separator_bridge_improvement".to_string(),
                fixture_path:
                    "docs/evidence/frontend_rework/DIFF_CLASSIFIER_2026-06-01.md#fixture-3"
                        .to_string(),
                class: FrontendCorpusClass::CompilerUnit,
                source: Some(
                    "Sub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n".to_string(),
                ),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: Some(ExpectedDiagnosticDrift::IntentionalImprovement),
                rationale:
                    "v2 accepts a CST-valid inline statement sequence that legacy-default rejects"
                        .to_string(),
                close_condition:
                    "keep as improvement only while v2 compiles and FE-5.4 adds execution evidence"
                        .to_string(),
            },
            FrontendCorpusFixture {
                name: "integration_host_project_residual".to_string(),
                fixture_path: "conformance/integration/projects/INTP-001/main/Main.proc.bas"
                    .to_string(),
                class: FrontendCorpusClass::HostProject,
                source: Some(
                    include_str!(
                        "../../../conformance/integration/projects/INTP-001/main/Main.proc.bas"
                    )
                    .to_string(),
                ),
                expected_bytecode_drift: None,
                expected_diagnostic_drift: None,
                rationale: String::new(),
                close_condition: String::new(),
            },
            FrontendCorpusFixture {
                name: "excel_oracle_residual".to_string(),
                fixture_path:
                    "docs/evidence/frontend_rework/CORPUS_RUNNER_2026-06-01.md#excel-oracle-residual"
                        .to_string(),
                class: FrontendCorpusClass::ExcelOracle,
                source: None,
                expected_bytecode_drift: None,
                expected_diagnostic_drift: None,
                rationale: String::new(),
                close_condition: String::new(),
            },
        ]
    }
}
