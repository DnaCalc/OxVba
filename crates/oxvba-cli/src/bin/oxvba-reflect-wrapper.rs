use std::{env, path::PathBuf, process};

use oxvba_build::reflection_exe::ReflectionExeWrapper;
use oxvba_compiler::{OxBundle, compile_project};

fn main() {
    if let Err(err) = run() {
        eprintln!("oxvba-reflect-wrapper: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 || args[0] == "--help" || args[0] == "-h" {
        print_usage();
        return if args
            .first()
            .is_some_and(|arg| arg == "--help" || arg == "-h")
        {
            Ok(())
        } else {
            Err("missing project path or command".to_string())
        };
    }

    let project_path = PathBuf::from(args.remove(0));
    let command_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let loaded = oxvba_project::load_workspace_target(&project_path)
        .map_err(|err| format!("failed to load {}: {err}", project_path.display()))?;
    let compiled = compile_project(&loaded.manifest)
        .map_err(|err| format!("failed to compile {}: {err}", project_path.display()))?;
    let bundle = OxBundle::from_compiled_project(&compiled, &loaded.manifest.project_name);
    let bytes = bundle
        .serialize_to_bytes()
        .map_err(|err| format!("failed to serialize bundle: {err}"))?;
    let mut wrapper = ReflectionExeWrapper::from_bundle_bytes(&bytes).map_err(|err| {
        format!(
            "failed to prepare reflection wrapper: {}: {}",
            err.code, err.message
        )
    })?;
    let output = wrapper.run(&command_args);
    if !output.stdout.is_empty() {
        println!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprintln!("{}", output.stderr);
    }
    if output.status == 0 {
        Ok(())
    } else {
        Err(format!("command failed with status {}", output.status))
    }
}

fn print_usage() {
    eprintln!(
        "usage:\n  oxvba-reflect-wrapper <project.basproj|project-dir> list\n  oxvba-reflect-wrapper <project.basproj|project-dir> describe Module.Procedure\n  oxvba-reflect-wrapper <project.basproj|project-dir> call Module.Procedure [typed positional args...]\n\nExamples:\n  oxvba-reflect-wrapper examples/reflection_wrapper/engineering_math/EngineeringMath.basproj list\n  oxvba-reflect-wrapper examples/reflection_wrapper/engineering_math/EngineeringMath.basproj describe EngineeringMath.Hypotenuse\n  oxvba-reflect-wrapper examples/reflection_wrapper/engineering_math/EngineeringMath.basproj call EngineeringMath.AddLongs 20 22"
    );
}
