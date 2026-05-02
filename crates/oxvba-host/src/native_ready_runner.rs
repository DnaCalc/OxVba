use std::time::Instant;

use oxvba_compiler::ProjectManifest;
use oxvba_runtime::Variant;

use crate::{Engine, HostConfig, PhaseDiagnostic};

pub const NATIVE_READY_RUNNER_SCHEMA_HEADER: &str = "run_id,timestamp_utc,host_os,target_arch,workload_id,workload_name,source_path,backend,artifact_kind,artifact_path,artifact_size_bytes,mode,iterations,warmup_iterations,mean_ms,min_ms,max_ms,exit_status,diagnostic_code,fallback_used,fallback_reason,result_kind,result_digest,claim_boundary";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReadyRunnerConfig {
    pub run_id_prefix: String,
    pub timestamp_utc: String,
    pub host_os: String,
    pub target_arch: String,
    pub workload_id: String,
    pub workload_name: String,
    pub source_path: String,
    pub mode: String,
    pub iterations: u32,
    pub warmup_iterations: u32,
    pub claim_boundary: String,
}

impl NativeReadyRunnerConfig {
    pub fn correctness(
        run_id_prefix: impl Into<String>,
        timestamp_utc: impl Into<String>,
        workload_id: impl Into<String>,
        workload_name: impl Into<String>,
        source_path: impl Into<String>,
    ) -> Self {
        Self {
            run_id_prefix: run_id_prefix.into(),
            timestamp_utc: timestamp_utc.into(),
            host_os: std::env::consts::OS.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
            workload_id: workload_id.into(),
            workload_name: workload_name.into(),
            source_path: source_path.into(),
            mode: "correctness".to_string(),
            iterations: 1,
            warmup_iterations: 0,
            claim_boundary: "Reference runner row produced by active Rust schema producer; not direct native PE/ELF evidence".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeReadyRunnerRow {
    pub run_id: String,
    pub timestamp_utc: String,
    pub host_os: String,
    pub target_arch: String,
    pub workload_id: String,
    pub workload_name: String,
    pub source_path: String,
    pub backend: String,
    pub artifact_kind: String,
    pub artifact_path: String,
    pub artifact_size_bytes: u64,
    pub mode: String,
    pub iterations: u32,
    pub warmup_iterations: u32,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub exit_status: i32,
    pub diagnostic_code: String,
    pub fallback_used: bool,
    pub fallback_reason: String,
    pub result_kind: String,
    pub result_digest: String,
    pub claim_boundary: String,
}

impl NativeReadyRunnerRow {
    pub fn to_csv_record(&self) -> String {
        [
            self.run_id.clone(),
            self.timestamp_utc.clone(),
            self.host_os.clone(),
            self.target_arch.clone(),
            self.workload_id.clone(),
            self.workload_name.clone(),
            self.source_path.clone(),
            self.backend.clone(),
            self.artifact_kind.clone(),
            self.artifact_path.clone(),
            self.artifact_size_bytes.to_string(),
            self.mode.clone(),
            self.iterations.to_string(),
            self.warmup_iterations.to_string(),
            format_ms(self.mean_ms),
            format_ms(self.min_ms),
            format_ms(self.max_ms),
            self.exit_status.to_string(),
            self.diagnostic_code.clone(),
            self.fallback_used.to_string(),
            self.fallback_reason.clone(),
            self.result_kind.clone(),
            self.result_digest.clone(),
            self.claim_boundary.clone(),
        ]
        .into_iter()
        .map(csv_escape)
        .collect::<Vec<_>>()
        .join(",")
    }
}

pub fn emit_native_ready_vm_jit_csv(
    manifest: &ProjectManifest,
    config: &NativeReadyRunnerConfig,
) -> Result<String, PhaseDiagnostic> {
    let rows = produce_native_ready_vm_jit_rows(manifest, config)?;
    let mut csv = String::new();
    csv.push_str(NATIVE_READY_RUNNER_SCHEMA_HEADER);
    csv.push('\n');
    for row in rows {
        csv.push_str(&row.to_csv_record());
        csv.push('\n');
    }
    Ok(csv)
}

pub fn produce_native_ready_vm_jit_rows(
    manifest: &ProjectManifest,
    config: &NativeReadyRunnerConfig,
) -> Result<Vec<NativeReadyRunnerRow>, PhaseDiagnostic> {
    let vm_run =
        execute_project_backend(manifest, false, config.iterations, config.warmup_iterations)?;
    let jit_run =
        execute_project_backend(manifest, true, config.iterations, config.warmup_iterations)?;

    Ok(vec![
        row_from_run(config, "vm", false, "not-applicable", vm_run),
        row_from_run(
            config,
            "jit",
            true,
            "project-visible-snapshot-vm-fallback",
            jit_run,
        ),
    ])
}

fn row_from_run(
    config: &NativeReadyRunnerConfig,
    backend: &str,
    fallback_used: bool,
    fallback_reason: &str,
    run: BackendRun,
) -> NativeReadyRunnerRow {
    let claim_boundary = if backend == "jit" && fallback_used {
        "JIT row includes active JIT preflight, then uses VM project-visible snapshot fallback; reference evidence only".to_string()
    } else {
        config.claim_boundary.clone()
    };
    NativeReadyRunnerRow {
        run_id: format!("{}-{backend}", config.run_id_prefix),
        timestamp_utc: config.timestamp_utc.clone(),
        host_os: config.host_os.clone(),
        target_arch: config.target_arch.clone(),
        workload_id: config.workload_id.clone(),
        workload_name: config.workload_name.clone(),
        source_path: config.source_path.clone(),
        backend: backend.to_string(),
        artifact_kind: "none".to_string(),
        artifact_path: String::new(),
        artifact_size_bytes: 0,
        mode: config.mode.clone(),
        iterations: config.iterations.max(1),
        warmup_iterations: config.warmup_iterations,
        mean_ms: run.mean_ms,
        min_ms: run.min_ms,
        max_ms: run.max_ms,
        exit_status: 0,
        diagnostic_code: String::new(),
        fallback_used,
        fallback_reason: fallback_reason.to_string(),
        result_kind: "variant-snapshot".to_string(),
        result_digest: digest_variant_snapshot(&run.snapshot),
        claim_boundary,
    }
}

#[derive(Debug, Clone)]
struct BackendRun {
    snapshot: Vec<Variant>,
    mean_ms: f64,
    min_ms: f64,
    max_ms: f64,
}

fn execute_project_backend(
    manifest: &ProjectManifest,
    enable_jit: bool,
    iterations: u32,
    warmup_iterations: u32,
) -> Result<BackendRun, PhaseDiagnostic> {
    let engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });

    for _ in 0..warmup_iterations {
        let _ = engine.execute_project_with_variant_snapshot_phased(manifest)?;
    }

    let iterations = iterations.max(1);
    let mut timings = Vec::with_capacity(iterations as usize);
    let mut last_snapshot = Vec::new();
    for _ in 0..iterations {
        let started = Instant::now();
        last_snapshot = engine.execute_project_with_variant_snapshot_phased(manifest)?;
        timings.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    let sum = timings.iter().copied().sum::<f64>();
    let mean_ms = sum / timings.len() as f64;
    let min_ms = timings.iter().copied().fold(f64::INFINITY, f64::min);
    let max_ms = timings.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    Ok(BackendRun {
        snapshot: last_snapshot,
        mean_ms,
        min_ms,
        max_ms,
    })
}

fn digest_variant_snapshot(snapshot: &[Variant]) -> String {
    let payload = format!("{snapshot:?}");
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in payload.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn format_ms(value: f64) -> String {
    format!("{value:.3}")
}

fn csv_escape(value: String) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}
