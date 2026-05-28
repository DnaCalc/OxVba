use std::path::PathBuf;

use oxvba_compiler::{
    ModuleKind, OxBundle, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
    compile_project, compile_with_runtime_metadata, module_unit_from_source,
};
use oxvba_runtime::Variant;
use oxvba_vm::{Vm, VmExecutionPackage, VmPackageIdentityEvidence, VmPackageOrigin};

#[derive(Debug)]
struct FixtureRow {
    id: String,
    file: String,
    expected_values: String,
    expected_procedures: Vec<String>,
    expected_descriptor_tokens: Vec<String>,
    expected_call_observation_tokens: Vec<String>,
    expected_call_descriptor_tokens: Vec<String>,
    expected_array_shape_tokens: Vec<String>,
    expected_udt_descriptor_tokens: Vec<String>,
    expected_object_descriptor_tokens: Vec<String>,
    expected_lifecycle_tokens: Vec<String>,
}

#[derive(Debug)]
struct DiagnosticFixtureRow {
    id: String,
    file: String,
    expected_phase: String,
    expected_error_tokens: Vec<String>,
    expected_vba_error: String,
    classification: String,
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
                11,
                "identity fixture manifest row should have 11 columns: {line}"
            );
            FixtureRow {
                id: columns[0].to_string(),
                file: columns[1].to_string(),
                expected_values: columns[2].to_string(),
                expected_procedures: columns[3].split('|').map(|name| name.to_string()).collect(),
                expected_descriptor_tokens: columns[4]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_call_observation_tokens: columns[5]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_call_descriptor_tokens: columns[6]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_array_shape_tokens: columns[7]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_udt_descriptor_tokens: columns[8]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_object_descriptor_tokens: columns[9]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_lifecycle_tokens: columns[10]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
            }
        })
        .collect()
}

fn diagnostic_fixture_rows() -> Vec<DiagnosticFixtureRow> {
    let manifest = std::fs::read_to_string(fixture_root().join("diagnostic_manifest.csv"))
        .expect("identity diagnostic fixture manifest should be readable");
    manifest
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let columns = line.split(',').collect::<Vec<_>>();
            assert_eq!(
                columns.len(),
                6,
                "identity diagnostic fixture manifest row should have 6 columns: {line}"
            );
            DiagnosticFixtureRow {
                id: columns[0].to_string(),
                file: columns[1].to_string(),
                expected_phase: columns[2].to_string(),
                expected_error_tokens: columns[3]
                    .split(';')
                    .filter(|token| !token.trim().is_empty())
                    .map(|token| token.trim().to_string())
                    .collect(),
                expected_vba_error: columns[4].to_string(),
                classification: columns[5].to_string(),
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

fn slot_descriptor_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .procedures
        .iter()
        .flat_map(|procedure| {
            procedure.slot_descriptors.iter().map(move |descriptor| {
                format!(
                    "{}:{}:{:?}:{:?}:{:?}:{:?}",
                    procedure.procedure_name.to_ascii_lowercase(),
                    descriptor
                        .name
                        .as_deref()
                        .unwrap_or("<unnamed>")
                        .to_ascii_lowercase(),
                    descriptor.role,
                    descriptor.declared_type,
                    descriptor.initial_state,
                    descriptor.carrier
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn slot_descriptor_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .procedures
        .iter()
        .map(|procedure| {
            format!(
                "{}={}",
                procedure.procedure_name.to_ascii_lowercase(),
                procedure.slot_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn signature_call_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .signature_call_evidence
        .iter()
        .flat_map(|call| {
            call.observations.iter().map(move |observation| {
                format!(
                    "{}:{}",
                    call.procedure_name.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn signature_call_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .signature_call_evidence
        .iter()
        .map(|call| {
            format!(
                "{}@{}={}",
                call.procedure_name.to_ascii_lowercase(),
                call.call_pc,
                call.signature_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn call_site_descriptor_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .call_site_evidence
        .iter()
        .flat_map(|call| {
            call.observations.iter().map(move |observation| {
                format!(
                    "{}->{}:{}",
                    call.caller_procedure_name.to_ascii_lowercase(),
                    call.target_name.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn call_site_descriptor_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .call_site_evidence
        .iter()
        .map(|call| {
            format!(
                "{}->{}@{}={}",
                call.caller_procedure_name.to_ascii_lowercase(),
                call.target_name.to_ascii_lowercase(),
                call.call_pc,
                call.call_site_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn array_shape_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .array_shape_evidence
        .iter()
        .flat_map(|array| {
            array.observations.iter().map(move |observation| {
                format!(
                    "{}:{}:{}",
                    array.procedure_name.to_ascii_lowercase(),
                    array.array_name.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn array_shape_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .array_shape_evidence
        .iter()
        .map(|array| {
            format!(
                "{}:{}={}",
                array.procedure_name.to_ascii_lowercase(),
                array.array_name.to_ascii_lowercase(),
                array.array_shape_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn udt_descriptor_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .udt_descriptor_evidence
        .iter()
        .flat_map(|udt| {
            udt.observations.iter().map(move |observation| {
                format!(
                    "{}:{}:{}",
                    udt.procedure_name.to_ascii_lowercase(),
                    udt.type_name.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn udt_descriptor_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .udt_descriptor_evidence
        .iter()
        .map(|udt| {
            format!(
                "{}:{}={}",
                udt.procedure_name.to_ascii_lowercase(),
                udt.type_name.to_ascii_lowercase(),
                udt.udt_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn object_descriptor_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .object_descriptor_evidence
        .iter()
        .flat_map(|object| {
            object.observations.iter().map(move |observation| {
                format!(
                    "{}:{}:{}",
                    object.procedure_name.to_ascii_lowercase(),
                    object.type_name.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn object_descriptor_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .object_descriptor_evidence
        .iter()
        .map(|object| {
            format!(
                "{}:{}={}",
                object.procedure_name.to_ascii_lowercase(),
                object.type_name.to_ascii_lowercase(),
                object.object_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn lifecycle_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .lifecycle_evidence
        .iter()
        .flat_map(|lifecycle| {
            lifecycle.observations.iter().map(move |observation| {
                format!(
                    "{}:{}:{}",
                    lifecycle.procedure_name.to_ascii_lowercase(),
                    lifecycle.cleanup_scope_id.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn lifecycle_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .lifecycle_evidence
        .iter()
        .map(|lifecycle| {
            format!(
                "{}:{}={}",
                lifecycle.procedure_name.to_ascii_lowercase(),
                lifecycle.cleanup_scope_id.to_ascii_lowercase(),
                lifecycle.lifecycle_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn error_descriptor_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .error_descriptor_evidence
        .iter()
        .flat_map(|error| {
            error.observations.iter().map(move |observation| {
                format!(
                    "{}:{}",
                    error.error_scope_id.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn error_descriptor_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .error_descriptor_evidence
        .iter()
        .map(|error| {
            format!(
                "{}={}",
                error.error_scope_id.to_ascii_lowercase(),
                error.error_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn deopt_snapshot_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .deopt_snapshot_evidence
        .iter()
        .flat_map(|deopt| {
            deopt.observations.iter().map(move |observation| {
                format!(
                    "{}:{}",
                    deopt.safepoint_id.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn deopt_snapshot_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .deopt_snapshot_evidence
        .iter()
        .map(|deopt| {
            format!(
                "{}={}",
                deopt.safepoint_id.to_ascii_lowercase(),
                deopt.deopt_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn host_policy_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .host_policy_evidence
        .iter()
        .flat_map(|policy| {
            policy.observations.iter().map(move |observation| {
                format!(
                    "{}:{}",
                    policy.host_policy_id.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn host_policy_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .host_policy_evidence
        .iter()
        .map(|policy| {
            format!(
                "{}={}",
                policy.host_policy_id.to_ascii_lowercase(),
                policy.host_policy_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

fn vm_consumption_observation_tokens(evidence: &VmPackageIdentityEvidence) -> Vec<String> {
    let mut tokens = evidence
        .vm_consumption_evidence
        .iter()
        .flat_map(|consumption| {
            consumption.observations.iter().map(move |observation| {
                format!(
                    "{}:{}",
                    consumption.consumption_id.to_ascii_lowercase(),
                    observation
                )
            })
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

fn vm_consumption_digest_tokens(evidence: &VmPackageIdentityEvidence) -> String {
    let mut tokens = evidence
        .vm_consumption_evidence
        .iter()
        .map(|consumption| {
            format!(
                "{}={}",
                consumption.consumption_id.to_ascii_lowercase(),
                consumption.consumption_descriptor_digest
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.join("|")
}

#[test]
fn vm_package_identity_seed_fixtures_emit_identity_values_and_slot_descriptors() {
    for row in fixture_rows() {
        let source = std::fs::read_to_string(fixture_root().join(&row.file))
            .unwrap_or_else(|err| panic!("failed to read identity fixture `{}`: {err}", row.file));
        let (bytecode, metadata) =
            compile_with_runtime_metadata(&source).expect("identity fixture should compile");
        let bundle = OxBundle::new(bytecode, metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let mut vm = Vm::default();
        vm.execute_package(&package)
            .expect("package VM execution should succeed");
        let package_snapshot = vm.snapshot_variants(package.bytecode.user_slot_count);
        let evidence = vm
            .package_identity_evidence()
            .expect("package identity evidence should be recorded");

        assert_eq!(snapshot_tokens(&package_snapshot), row.expected_values);
        assert_eq!(evidence.package_origin, VmPackageOrigin::OxBundle);
        assert!(evidence.package_digest.starts_with("fnv1a64:"));
        assert!(evidence.bytecode_digest.starts_with("fnv1a64:"));
        assert_eq!(evidence.slot_count, package.bytecode.slot_count);
        assert_eq!(evidence.user_slot_count, package.bytecode.user_slot_count);
        let descriptor_tokens = slot_descriptor_tokens(evidence);
        for expected_descriptor in &row.expected_descriptor_tokens {
            assert!(
                descriptor_tokens.contains(expected_descriptor),
                "{} descriptor evidence should include `{}`; got: {:?}",
                row.id,
                expected_descriptor,
                descriptor_tokens
            );
        }
        let signature_call_tokens = signature_call_observation_tokens(evidence);
        for expected_observation in &row.expected_call_observation_tokens {
            assert!(
                signature_call_tokens.contains(expected_observation),
                "{} call/signature evidence should include `{}`; got: {:?}",
                row.id,
                expected_observation,
                signature_call_tokens
            );
        }
        let call_site_tokens = call_site_descriptor_observation_tokens(evidence);
        for expected_call_descriptor in &row.expected_call_descriptor_tokens {
            assert!(
                call_site_tokens.contains(expected_call_descriptor),
                "{} call-site descriptor evidence should include `{}`; got: {:?}",
                row.id,
                expected_call_descriptor,
                call_site_tokens
            );
        }
        let array_shape_tokens = array_shape_observation_tokens(evidence);
        for expected_array_shape in &row.expected_array_shape_tokens {
            assert!(
                array_shape_tokens.contains(expected_array_shape),
                "{} array-shape evidence should include `{}`; got: {:?}",
                row.id,
                expected_array_shape,
                array_shape_tokens
            );
        }
        let udt_descriptor_tokens = udt_descriptor_observation_tokens(evidence);
        for expected_udt_descriptor in &row.expected_udt_descriptor_tokens {
            assert!(
                udt_descriptor_tokens.contains(expected_udt_descriptor),
                "{} UDT descriptor evidence should include `{}`; got: {:?}",
                row.id,
                expected_udt_descriptor,
                udt_descriptor_tokens
            );
        }
        let object_descriptor_tokens = object_descriptor_observation_tokens(evidence);
        for expected_object_descriptor in &row.expected_object_descriptor_tokens {
            assert!(
                object_descriptor_tokens.contains(expected_object_descriptor),
                "{} object descriptor evidence should include `{}`; got: {:?}",
                row.id,
                expected_object_descriptor,
                object_descriptor_tokens
            );
        }
        let lifecycle_tokens = lifecycle_observation_tokens(evidence);
        for expected_lifecycle in &row.expected_lifecycle_tokens {
            assert!(
                lifecycle_tokens.contains(expected_lifecycle),
                "{} lifecycle evidence should include `{}`; got: {:?}",
                row.id,
                expected_lifecycle,
                lifecycle_tokens
            );
        }
        let error_descriptor_tokens = error_descriptor_observation_tokens(evidence);
        let deopt_snapshot_tokens = deopt_snapshot_observation_tokens(evidence);
        let vm_consumption_tokens = vm_consumption_observation_tokens(evidence);
        assert!(
            vm_consumption_tokens
                .iter()
                .any(|token| token.contains("selection=VMR08-ERR-CLEAR-RESET-001")),
            "{} VM consumption evidence should include selected Err reset consumption; got: {:?}",
            row.id,
            vm_consumption_tokens
        );
        if row.id == "VMR04_CALL_ARGUMENT_BINDING" {
            assert!(
                error_descriptor_tokens
                    .iter()
                    .any(|token| token.contains("kind=call-frame-error-state")),
                "{} error descriptor evidence should record call-frame error state: {:?}",
                row.id,
                error_descriptor_tokens
            );
            assert!(
                deopt_snapshot_tokens
                    .iter()
                    .any(|token| token.contains("operation=call-procedure")),
                "{} deopt evidence should record call safepoints: {:?}",
                row.id,
                deopt_snapshot_tokens
            );
            for expected in [
                "selection=VMR06-CALL-BYVAL-COERCE-001",
                "status=supported-selected",
                "vm-path=Vm::apply_descriptor_driven_call_entry_bindings",
                "raw-bytecode-baseline=pre-vmr06-byval-copy-observes-source-carrier",
                "selection=CALL-OPTIONAL-MISSING-VARIANT",
                "status=unsupported-rejected",
                "selection=VMR08-CALL-FRAME-DEOPT-001",
            ] {
                assert!(
                    vm_consumption_tokens
                        .iter()
                        .any(|token| token.contains(expected)),
                    "{} VM consumption evidence should include `{expected}`; got: {:?}",
                    row.id,
                    vm_consumption_tokens
                );
            }
        }
        if row.id == "VMR05_ARRAY_SHAPE_BOUNDS" {
            for expected in [
                "selection=VMR06-ARRAY-STATIC-BOUNDS-001",
                "package-execution=descriptor-driven-static-array-bounds",
                "raw-bytecode-baseline=runtime-error-13-on-unallocated-fixed-array-base",
            ] {
                assert!(
                    vm_consumption_tokens
                        .iter()
                        .any(|token| token.contains(expected)),
                    "{} VM consumption evidence should include `{expected}`; got: {:?}",
                    row.id,
                    vm_consumption_tokens
                );
            }
        }
        if row.id == "VMR02_UDT_FIELD_SLOTS" || row.id == "VMR05_UDT_DESCRIPTOR_MEMBERS" {
            assert!(
                vm_consumption_tokens
                    .iter()
                    .any(|token| token.contains("selection=VMR06-UDT-OWNING-FIELD-CLEANUP-001")),
                "{} VM consumption evidence should record selected UDT cleanup consumption: {:?}",
                row.id,
                vm_consumption_tokens
            );
        }
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
            assert!(
                procedure.slot_descriptor_digest.starts_with("fnv1a64:"),
                "{} slot descriptor digest should be explicit for {}: {}",
                row.id,
                procedure.procedure_name,
                procedure.slot_descriptor_digest
            );
            assert!(
                procedure
                    .slot_descriptors
                    .iter()
                    .all(|descriptor| descriptor.name.is_some()),
                "{} descriptor evidence should preserve slot names for {}: {:?}",
                row.id,
                procedure.procedure_name,
                procedure.slot_descriptors
            );
        }
        for call in &evidence.signature_call_evidence {
            assert!(
                call.procedure_id.starts_with("proc:"),
                "{} signature/call evidence should retain procedure identity: {}",
                row.id,
                call.procedure_id
            );
            assert!(
                call.signature_descriptor_digest.starts_with("fnv1a64:")
                    || call.signature_descriptor_digest == "missing",
                "{} signature/call evidence should carry a descriptor digest or missing marker: {:?}",
                row.id,
                call
            );
            assert!(
                !call.observations.is_empty(),
                "{} signature/call evidence should classify each observed call: {:?}",
                row.id,
                call
            );
        }
        for call_site in &evidence.call_site_evidence {
            assert!(
                call_site.call_site_id.starts_with("callsite:"),
                "{} call-site descriptor evidence should retain call-site identity: {}",
                row.id,
                call_site.call_site_id
            );
            assert!(
                call_site
                    .call_site_descriptor_digest
                    .starts_with("fnv1a64:"),
                "{} call-site descriptor digest should be explicit: {:?}",
                row.id,
                call_site
            );
            assert!(
                !call_site.observations.is_empty(),
                "{} call-site descriptor evidence should classify each descriptor: {:?}",
                row.id,
                call_site
            );
        }
        for array_shape in &evidence.array_shape_evidence {
            assert!(
                array_shape
                    .array_shape_descriptor_digest
                    .starts_with("fnv1a64:"),
                "{} array-shape descriptor digest should be explicit: {:?}",
                row.id,
                array_shape
            );
            assert!(
                !array_shape.observations.is_empty(),
                "{} array-shape evidence should classify each descriptor: {:?}",
                row.id,
                array_shape
            );
        }
        for udt_descriptor in &evidence.udt_descriptor_evidence {
            assert!(
                udt_descriptor.udt_descriptor_id.starts_with("udt:"),
                "{} UDT descriptor id should be explicit: {:?}",
                row.id,
                udt_descriptor
            );
            assert!(
                udt_descriptor.udt_descriptor_digest.starts_with("fnv1a64:"),
                "{} UDT descriptor digest should be explicit: {:?}",
                row.id,
                udt_descriptor
            );
            assert!(
                !udt_descriptor.observations.is_empty(),
                "{} UDT descriptor evidence should classify each descriptor: {:?}",
                row.id,
                udt_descriptor
            );
        }
        for object_descriptor in &evidence.object_descriptor_evidence {
            assert!(
                object_descriptor.object_descriptor_id.contains(':'),
                "{} object descriptor id should be explicit: {:?}",
                row.id,
                object_descriptor
            );
            assert!(
                object_descriptor
                    .object_descriptor_digest
                    .starts_with("fnv1a64:"),
                "{} object descriptor digest should be explicit: {:?}",
                row.id,
                object_descriptor
            );
            assert!(
                !object_descriptor.observations.is_empty(),
                "{} object descriptor evidence should classify each descriptor: {:?}",
                row.id,
                object_descriptor
            );
        }
        for lifecycle in &evidence.lifecycle_evidence {
            assert!(
                lifecycle.cleanup_scope_id.starts_with("cleanup:"),
                "{} lifecycle evidence should retain cleanup scope identity: {:?}",
                row.id,
                lifecycle
            );
            assert!(
                lifecycle
                    .lifecycle_descriptor_digest
                    .starts_with("fnv1a64:"),
                "{} lifecycle descriptor digest should be explicit: {:?}",
                row.id,
                lifecycle
            );
            assert!(
                !lifecycle.observations.is_empty(),
                "{} lifecycle evidence should classify each cleanup descriptor: {:?}",
                row.id,
                lifecycle
            );
        }
        for error in &evidence.error_descriptor_evidence {
            assert!(
                error.error_scope_id.starts_with("error-routing:"),
                "{} error descriptor evidence should retain error scope identity: {:?}",
                row.id,
                error
            );
            assert!(
                error.error_descriptor_digest.starts_with("fnv1a64:"),
                "{} error descriptor digest should be explicit: {:?}",
                row.id,
                error
            );
            assert!(
                !error.observations.is_empty(),
                "{} error evidence should classify each descriptor: {:?}",
                row.id,
                error
            );
        }
        for deopt in &evidence.deopt_snapshot_evidence {
            assert!(
                deopt.safepoint_id.starts_with("deopt-snapshot:"),
                "{} deopt evidence should retain safepoint identity: {:?}",
                row.id,
                deopt
            );
            assert!(
                deopt.deopt_descriptor_digest.starts_with("fnv1a64:"),
                "{} deopt descriptor digest should be explicit: {:?}",
                row.id,
                deopt
            );
            assert!(
                !deopt.observations.is_empty(),
                "{} deopt evidence should classify each safepoint: {:?}",
                row.id,
                deopt
            );
        }
        for policy in &evidence.host_policy_evidence {
            assert!(
                policy.host_policy_id.starts_with("host-policy:"),
                "{} host-policy evidence should retain policy identity: {:?}",
                row.id,
                policy
            );
            assert!(
                policy.host_policy_descriptor_digest.starts_with("fnv1a64:"),
                "{} host-policy descriptor digest should be explicit: {:?}",
                row.id,
                policy
            );
            assert!(
                !policy.observations.is_empty(),
                "{} host-policy evidence should classify each requirement: {:?}",
                row.id,
                policy
            );
        }
        for consumption in &evidence.vm_consumption_evidence {
            assert!(
                consumption.consumption_id.starts_with("vm-consumption:"),
                "{} VM consumption evidence should retain consumption identity: {:?}",
                row.id,
                consumption
            );
            assert!(
                consumption
                    .consumption_descriptor_digest
                    .starts_with("fnv1a64:"),
                "{} VM consumption descriptor digest should be explicit: {:?}",
                row.id,
                consumption
            );
            assert!(
                !consumption.observations.is_empty()
                    && !consumption.descriptor_families.is_empty()
                    && !consumption.gap_classifications.is_empty(),
                "{} VM consumption evidence should classify each row: {:?}",
                row.id,
                consumption
            );
        }

        println!(
            "VM_PACKAGE_IDENTITY id={} values={} package_digest={} bytecode_digest={} slot_count={} user_slot_count={} procedures={} procedure_identities={} slot_descriptor_digests={} signature_call_digests={} call_site_descriptor_digests={} array_shape_digests={} udt_descriptor_digests={} object_descriptor_digests={} lifecycle_digests={} error_descriptor_digests={} deopt_snapshot_digests={} host_policy_digests={} vm_consumption_digests={} slot_descriptors={} signature_call_observations={} call_site_descriptor_observations={} array_shape_observations={} udt_descriptor_observations={} object_descriptor_observations={} lifecycle_observations={} error_descriptor_observations={} deopt_snapshot_observations={} host_policy_observations={} vm_consumption_observations={}",
            row.id,
            snapshot_tokens(&package_snapshot),
            evidence.package_digest,
            evidence.bytecode_digest,
            evidence.slot_count,
            evidence.user_slot_count,
            sorted_procedure_names(evidence).join("|"),
            procedure_identity_tokens(evidence),
            slot_descriptor_digest_tokens(evidence),
            signature_call_digest_tokens(evidence),
            call_site_descriptor_digest_tokens(evidence),
            array_shape_digest_tokens(evidence),
            udt_descriptor_digest_tokens(evidence),
            object_descriptor_digest_tokens(evidence),
            lifecycle_digest_tokens(evidence),
            error_descriptor_digest_tokens(evidence),
            deopt_snapshot_digest_tokens(evidence),
            host_policy_digest_tokens(evidence),
            vm_consumption_digest_tokens(evidence),
            descriptor_tokens.join("|"),
            signature_call_tokens.join("|"),
            call_site_tokens.join("|"),
            array_shape_tokens.join("|"),
            udt_descriptor_tokens.join("|"),
            object_descriptor_tokens.join("|"),
            lifecycle_tokens.join("|"),
            error_descriptor_tokens.join("|"),
            deopt_snapshot_tokens.join("|"),
            host_policy_observation_tokens(evidence).join("|"),
            vm_consumption_tokens.join("|")
        );
    }
}

#[test]
fn vm_project_object_descriptor_evidence_records_class_interface_and_com_withevents_routes() {
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As New ThingImpl\nEnd Sub",
    )
    .expect("main module should parse");
    let interface_module = module_unit_from_source(
        "IThing",
        ModuleKind::Class,
        "Attribute VB_Name = \"IThing\"\nPublic Sub Ping()\nEnd Sub",
    )
    .expect("interface module should parse");
    let impl_module = module_unit_from_source(
        "ThingImpl",
        ModuleKind::Class,
        "Attribute VB_Name = \"ThingImpl\"\nImplements IThing\nPublic Property Get Value()\nValue = 7\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPrivate Sub IThing_Ping()\nEnd Sub",
    )
    .expect("implementation module should parse");
    let sink_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        "Attribute VB_Name = \"Sink\"\nPrivate WithEvents src As OxVba.TestEventServer\nPublic Sub Attach(ByVal value As Object)\nSet src = value\nEnd Sub\nPrivate Sub src_OnValueChanged(ByVal value)\nCall MainModule.Main\nEnd Sub",
    )
    .expect("sink module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, interface_module, impl_module, sink_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
    };
    let compiled =
        compile_project(&manifest).expect("project object descriptor seed should compile");
    assert!(
        !compiled.project_dynamic_objects.is_empty(),
        "class/interface object route metadata should be present"
    );
    assert!(
        !compiled.project_com_withevents_routes.is_empty(),
        "imported COM WithEvents route metadata should be present"
    );

    let bundle = OxBundle::from_compiled_project_with_manifest(&compiled, &manifest);
    let package = VmExecutionPackage::from_bundle(&bundle);
    let package_route_evidence = package.identity_evidence();
    let package_route_tokens = object_descriptor_observation_tokens(&package_route_evidence);
    for expected in [
        "<package-routes>:thingimpl:activation=asnew-project-class",
        "<package-routes>:thingimpl:default-member=hasdefaultmember",
        "<package-routes>:thingimpl:default-member-target=value",
        "<package-routes>:thingimpl:interface:ithing:kind=implemented",
        "<package-routes>:thingimpl:object-identity-policy=stable-project-dynamic-object-handle",
        "<package-routes>:oxvba.testeventserver:event-binding=withevents",
        "<package-routes>:oxvba.testeventserver:imported-com-class=oxvba.testeventserver",
        "<package-routes>:oxvba.testeventserver:subscription-policy=set-assignment-updates-owner",
    ] {
        assert!(
            package_route_tokens.contains(&expected.to_string()),
            "bundle package route object evidence should include `{expected}`; got: {package_route_tokens:?}"
        );
    }

    let mut vm = Vm::default();
    vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
    vm.set_project_com_withevents_routes(compiled.project_com_withevents_routes.clone());
    vm.execute_package(&package)
        .expect("project package should execute");
    let evidence = vm
        .package_identity_evidence()
        .expect("project package evidence should be recorded");
    let tokens = object_descriptor_observation_tokens(evidence);
    for expected in [
        "<project-runtime>:thingimpl:descriptor-id=object-type:class:projecta:thingimpl",
        "<project-runtime>:thingimpl:kind=vbaclass",
        "<project-runtime>:thingimpl:activation=asnew-project-class",
        "<project-runtime>:thingimpl:default-member=hasdefaultmember",
        "<project-runtime>:thingimpl:default-member-target=value",
        "<project-runtime>:thingimpl:interface:ithing:kind=implemented",
        "<project-runtime>:oxvba.testeventserver:kind=witheventsobject",
        "<project-runtime>:oxvba.testeventserver:event-binding=withevents",
        "<project-runtime>:oxvba.testeventserver:event-source=oxvba.testeventserver",
        "<project-runtime>:oxvba.testeventserver:handler=pmr_projecta_sink_src_onvaluechanged",
    ] {
        assert!(
            tokens.contains(&expected.to_string()),
            "object descriptor evidence should include `{expected}`; got: {tokens:?}"
        );
    }
}

#[test]
fn vm_package_call_diagnostic_seed_fixtures_emit_current_compile_diagnostics() {
    for row in diagnostic_fixture_rows() {
        assert_eq!(
            row.expected_phase, "compile",
            "{} diagnostic fixture should currently be compile-phase evidence",
            row.id
        );
        let source =
            std::fs::read_to_string(fixture_root().join(&row.file)).unwrap_or_else(|err| {
                panic!(
                    "failed to read identity diagnostic fixture `{}`: {err}",
                    row.file
                )
            });
        let err = compile_with_runtime_metadata(&source)
            .expect_err("identity diagnostic fixture should fail compilation");
        let message = err.to_string();
        for expected in &row.expected_error_tokens {
            assert!(
                message.contains(expected),
                "{} diagnostic should include `{}`; got: {}",
                row.id,
                expected,
                message
            );
        }
        assert!(
            matches!(
                row.expected_vba_error.as_str(),
                "runtime-448" | "runtime-449" | "runtime-450"
            ),
            "{} diagnostic fixture should carry intended VBA error classification: {:?}",
            row.id,
            row
        );
        assert!(
            row.classification == "current-compiler-diagnostic",
            "{} diagnostic fixture should classify the current phase explicitly: {:?}",
            row.id,
            row
        );

        println!(
            "VM_PACKAGE_CALL_DIAGNOSTIC id={} phase={} intended_vba_error={} classification={} diagnostic={}",
            row.id, row.expected_phase, row.expected_vba_error, row.classification, message
        );
    }
}
