use std::{
    error::Error,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use oxvba_hal::{
    adapters::builder::HostBuilder,
    conformance::run_conformance,
    model::{HalProfileId, HostPolicy, WasmRuntimeClass},
};

#[derive(Debug, Clone, Copy)]
enum Lane {
    Runtime,
    CompileTime,
    InteractiveDev,
}

impl Lane {
    const fn as_str(self) -> &'static str {
        match self {
            Lane::Runtime => "runtime",
            Lane::CompileTime => "compile-time",
            Lane::InteractiveDev => "interactive-dev",
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = parse_output_dir(std::env::args().skip(1))?;
    fs::create_dir_all(&output_dir)?;

    let ts = unix_timestamp();
    let markdown_path = output_dir.join(format!("HAL_CONFORMANCE_{ts}.md"));
    let jsonl_path = output_dir.join(format!("HAL_CONFORMANCE_{ts}.jsonl"));

    let profiles = [
        HalProfileId::Windows,
        HalProfileId::Linux,
        HalProfileId::MacOs,
        HalProfileId::Wasm,
        HalProfileId::Null,
    ];

    let mut md = String::new();
    let mut jsonl = String::new();
    md.push_str("# HAL Conformance Run\n\n");
    md.push_str(&format!("- Timestamp (unix): `{ts}`\n"));
    md.push_str("- Lanes: `runtime`, `compile-time`, `interactive-dev`\n");
    md.push_str("- Wasm runtime classes: `wasi`, `browser-sandbox`\n\n");
    md.push_str("| Profile | Runtime Class | Lane | Host-backed Eligible | Host-backed Active | Passed | Failures |\n");
    md.push_str("|---|---|---|---:|---:|---:|---:|\n");

    for profile in profiles {
        let wasm_runtime_classes: &[WasmRuntimeClass] = if profile == HalProfileId::Wasm {
            &[WasmRuntimeClass::Wasi, WasmRuntimeClass::BrowserSandbox]
        } else {
            &[WasmRuntimeClass::Wasi]
        };

        for wasm_runtime_class in wasm_runtime_classes {
            for lane in [Lane::Runtime, Lane::CompileTime, Lane::InteractiveDev] {
                let mut policy = match lane {
                    Lane::Runtime => HostPolicy::deterministic_runtime(),
                    Lane::CompileTime => HostPolicy::deterministic_compile_time(),
                    Lane::InteractiveDev => HostPolicy::interactive_dev(),
                };
                if profile == HalProfileId::Wasm {
                    policy = policy.with_wasm_runtime_class(*wasm_runtime_class);
                }
                let host = HostBuilder::new().profile(profile).policy(policy).build();
                let report = run_conformance(host.as_ref());
                let clause_coverage = report.clause_coverage_against_catalog();
                let clause_total = clause_coverage.len();
                let clause_pass = clause_coverage.values().filter(|passed| **passed).count();
                let clause_failed = clause_total.saturating_sub(clause_pass);

                md.push_str(&format!(
                    "| {:?} | {} | {} | {} | {} | {} | {} |\n",
                    profile,
                    report.descriptor.runtime_class,
                    lane.as_str(),
                    report.host_backed_eligible,
                    report.host_backed_active,
                    report.passed,
                    report.failures.len()
                ));
                md.push_str(&format!(
                    "  - Clauses: `{clause_pass}/{clause_total}` passed (`{clause_failed}` failed)\n"
                ));
                md.push_str(&format!(
                    "  - Governance notices: `{}`\n",
                    report.governance_notices.len()
                ));

                let failures = report
                    .failures
                    .iter()
                    .map(|f| format!("\"{}\"", json_escape(f)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let governance_notices = report
                    .governance_notices
                    .iter()
                    .map(|f| format!("\"{}\"", json_escape(f)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let failed_clauses = clause_coverage
                    .iter()
                    .filter_map(|(id, passed)| if *passed { None } else { Some(*id) })
                    .map(|id| format!("\"{id}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                let passed_probes = report
                    .probes
                    .iter()
                    .filter(|p| p.status == oxvba_hal::conformance::ProbeStatus::Passed)
                    .count();
                jsonl.push_str(&format!(
                    "{{\"profile\":\"{:?}\",\"runtime_class\":\"{}\",\"lane\":\"{}\",\"host_backed_eligible\":{},\"host_backed_active\":{},\"passed\":{},\"failure_count\":{},\"governance_notice_count\":{},\"probe_count\":{},\"probe_pass_count\":{},\"clause_count\":{},\"clause_pass_count\":{},\"failed_clauses\":[{}],\"governance_notices\":[{}],\"failures\":[{}]}}\n",
                    profile,
                    report.descriptor.runtime_class,
                    lane.as_str(),
                    report.host_backed_eligible,
                    report.host_backed_active,
                    report.passed,
                    report.failures.len(),
                    report.governance_notices.len(),
                    report.probes.len(),
                    passed_probes,
                    clause_total,
                    clause_pass,
                    failed_clauses,
                    governance_notices,
                    failures
                ));
            }
        }
    }

    fs::write(&markdown_path, md)?;
    fs::write(&jsonl_path, jsonl)?;

    println!("HAL conformance artifacts written:");
    println!("  {}", markdown_path.display());
    println!("  {}", jsonl_path.display());
    Ok(())
}

fn parse_output_dir(mut args: impl Iterator<Item = String>) -> Result<PathBuf, Box<dyn Error>> {
    let mut output_dir = PathBuf::from("docs/evidence/hal");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => {
                let Some(value) = args.next() else {
                    return Err("--output-dir requires a value".into());
                };
                output_dir = PathBuf::from(value);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    Ok(output_dir)
}

fn unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn print_help() {
    println!("Usage: cargo run -p oxvba-hal --bin hal-conformance -- [--output-dir PATH]");
    println!("Default output directory: docs/evidence/hal");
}
