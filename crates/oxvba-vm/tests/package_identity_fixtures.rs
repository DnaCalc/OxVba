use std::path::PathBuf;

use oxvba_compiler::{OxBundle, compile_with_runtime_metadata};
use oxvba_runtime::Variant;
use oxvba_vm::{
    Vm, VmExecutionPackage, VmPackageIdentityEvidence, VmPackageOrigin,
    execute_and_snapshot_variants,
};

#[derive(Debug)]
struct FixtureRow {
    id: String,
    file: String,
    expected_values: String,
    expected_procedures: Vec<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_root() -> PathBuf {
    repo_root()
        .join("conformance")
        .join("vm_package")
        .join("identity_seed")
}

fn fixture_rows() -> Vec<FixtureRow> {
    let manifest = std::fs::read_to_string(fixture_root().join("manifest.csv"))
        .expect("identity fixture manifest should be readable");
    manifest
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let columns = line.split(',').collect::<Vec<_>>();
            assert_eq!(
                columns.len(),
                4,
                "identity fixture manifest row should have 4 columns: {line}"
            );
            FixtureRow {
                id: columns[0].to_string(),
                file: columns[1].to_string(),
                expected_values: columns[2].to_string(),
                expected_procedures: columns[3].split('|').map(|name| name.to_string()).collect(),
            }
        })
        .collect()
}

fn variant_token(value: &Variant) -> String {
    if *value == Variant::empty() {
        return "empty".to_string();
    }
    if let Some(value) = value.as_i32() {
        return format!("i32:{value}");
    }
    if let Some(value) = value.as_f64() {
        return format!("f64:{value}");
    }
    if let Some(value) = value.as_bool() {
        return format!("bool:{value}");
    }
    if let Some(value) = value.as_bstr() {
        return format!("string:{value}");
    }
    format!("{value:?}")
}

fn snapshot_tokens(values: &[Variant]) -> String {
    values
        .iter()
        .map(variant_token)
        .collect::<Vec<_>>()
        .join("|")
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

fn procedure_identity_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .procedures
        .iter()
        .map(|procedure| {
            format!(
                "{}@{}={}",
                procedure.procedure_name.to_ascii_lowercase(),
                procedure.entry_pc,
                procedure.procedure_id
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

#[test]
fn vmr01_identity_seed_fixtures_emit_identity_and_values() {
    for row in fixture_rows() {
        let source = std::fs::read_to_string(fixture_root().join(&row.file))
            .unwrap_or_else(|err| panic!("failed to read identity fixture `{}`: {err}", row.file));
        let (bytecode, metadata) =
            compile_with_runtime_metadata(&source).expect("identity fixture should compile");
        let expected_snapshot =
            execute_and_snapshot_variants(&bytecode).expect("raw VM snapshot should execute");
        let bundle = OxBundle::new(bytecode, metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let mut vm = Vm::default();
        vm.execute_package(&package)
            .expect("package VM execution should succeed");
        let package_snapshot = vm.snapshot_variants(package.bytecode.user_slot_count);
        let evidence = vm
            .package_identity_evidence()
            .expect("package identity evidence should be recorded");

        assert_eq!(
            package_snapshot, expected_snapshot,
            "{} package execution should not change the value snapshot",
            row.id
        );
        assert_eq!(snapshot_tokens(&package_snapshot), row.expected_values);
        assert_eq!(evidence.package_origin, VmPackageOrigin::OxBundle);
        assert!(evidence.package_digest.starts_with("fnv1a64:"));
        assert!(evidence.bytecode_digest.starts_with("fnv1a64:"));
        assert_eq!(evidence.slot_count, package.bytecode.slot_count);
        assert_eq!(evidence.user_slot_count, package.bytecode.user_slot_count);
        assert_eq!(sorted_procedure_names(evidence), {
            let mut expected = row
                .expected_procedures
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect::<Vec<_>>();
            expected.sort();
            expected
        });
        for procedure in &evidence.procedures {
            assert!(
                procedure.procedure_id.starts_with("proc:"),
                "{} procedure id should be explicit: {}",
                row.id,
                procedure.procedure_id
            );
            assert!(
                procedure.procedure_id.contains("@pc:"),
                "{} procedure id should include entry pc: {}",
                row.id,
                procedure.procedure_id
            );
        }

        println!(
            "VMR01_IDENTITY id={} values={} package_digest={} bytecode_digest={} slot_count={} user_slot_count={} procedures={} procedure_identities={}",
            row.id,
            snapshot_tokens(&package_snapshot),
            evidence.package_digest,
            evidence.bytecode_digest,
            evidence.slot_count,
            evidence.user_slot_count,
            sorted_procedure_names(evidence).join("|"),
            procedure_identity_tokens(evidence)
        );
    }
}
