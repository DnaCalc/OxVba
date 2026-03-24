//! oxvba-run: standalone launcher for compiled .oxb bundles.
//!
//! Usage:
//!   oxvba-run <bundle.oxb>              — execute a compiled bundle
//!   oxvba-run <bundle.oxb> --jit        — execute with JIT compilation

use std::{env, fs, process};

use oxvba_compiler::OxBundle;
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{HostPolicy, native_host_profile},
};
use oxvba_jit::JitEngine;
use oxvba_runtime::RuntimeValue;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: oxvba-run <bundle.oxb> [--jit]");
        process::exit(2);
    }

    let bundle_path = &args[0];
    let use_jit = args.iter().any(|a| a == "--jit");

    let bundle_data = fs::read(bundle_path).unwrap_or_else(|err| {
        eprintln!("oxvba-run: cannot read {bundle_path}: {err}");
        process::exit(1);
    });

    let bundle = OxBundle::deserialize_from_bytes(&bundle_data).unwrap_or_else(|err| {
        eprintln!("oxvba-run: invalid bundle: {err}");
        process::exit(1);
    });

    let host_services = HostBuilder::new()
        .profile(native_host_profile())
        .policy(HostPolicy::deterministic_runtime())
        .build();

    let result: Result<Vec<RuntimeValue>, String> = if use_jit {
        let jit = JitEngine;
        jit.execute_and_snapshot_with_host(&bundle.bytecode, host_services)
            .map_err(|e| e.to_string())
    } else {
        oxvba_vm::execute_and_snapshot_with_host(&bundle.bytecode, host_services)
    };

    match result {
        Ok(values) => {
            if !values.is_empty() {
                for (i, val) in values.iter().enumerate() {
                    eprintln!("  slot[{i}] = {val:?}");
                }
            }
        }
        Err(err) => {
            eprintln!("oxvba-run: execution failed: {err}");
            process::exit(1);
        }
    }
}
