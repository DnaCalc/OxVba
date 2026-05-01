#[cfg(target_os = "windows")]
mod windows_host_sensitive_oracle_lane {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig, compat::RuntimeValueCompatEngineExt};
    use oxvba_runtime::{RuntimeValue, bstr::BStr};
    use std::fs;

    fn run_host_backed_source(source: &str) -> Vec<RuntimeValue> {
        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_snapshot_phased(source)
            .expect("windows host-backed host-sensitive lane should execute")
    }

    fn emit_observed(case_id: &str, value: &RuntimeValue) {
        let rendered = match value {
            RuntimeValue::String(text) => text.as_str().to_string(),
            RuntimeValue::I32(pid) if *pid > 0 => "pid>0".to_string(),
            other => format!("{other:?}"),
        };
        println!("ODG033-OBSERVED[{case_id}]={rendered}");
    }

    #[test]
    #[ignore = "requires Windows host-backed oracle lane"]
    fn windows_host_backed_environ_string_returns_actual_value() {
        unsafe {
            std::env::set_var("OXVBA_ORACLE_ENV", "oracle-033-value");
        }
        let out =
            run_host_backed_source("Sub Main()\nDim x\nx = Environ(\"OXVBA_ORACLE_ENV\")\nEnd Sub");
        emit_observed("CCT-035-ENV-001", &out[0]);
        assert_eq!(out[0], RuntimeValue::String(BStr::from("oracle-033-value")));
    }

    #[test]
    #[ignore = "requires Windows host-backed oracle lane"]
    fn windows_host_backed_dir_existing_file_returns_filename() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("odg033-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("probe-file.txt");
        fs::write(&temp_file, "probe").expect("write temp file");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");
        let source = format!("Sub Main()\nDim x\nx = Dir(\"{path_literal}\")\nEnd Sub");
        let out = run_host_backed_source(&source);
        emit_observed("CCT-035-DIR-001", &out[0]);
        assert_eq!(out[0], RuntimeValue::String(BStr::from("probe-file.txt")));
    }

    #[test]
    #[ignore = "requires Windows host-backed oracle lane"]
    fn windows_host_backed_shell_returns_positive_process_identifier() {
        let out =
            run_host_backed_source("Sub Main()\nDim x\nx = Shell(\"cmd.exe /c exit 0\")\nEnd Sub");
        emit_observed("CCT-035-SHELL-001", &out[0]);
        let RuntimeValue::I32(pid) = out[0] else {
            panic!("expected Shell result to be an I32 pid, got {:?}", out[0]);
        };
        assert!(pid > 0, "Shell should return a positive process identifier");
    }
}
