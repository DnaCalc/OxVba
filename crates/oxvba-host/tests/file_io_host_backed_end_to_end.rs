#[cfg(target_os = "windows")]
mod windows_file_io_host_backed_end_to_end {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::{RuntimeValue, bstr::BStr};
    use std::fs;

    #[test]
    fn host_backed_file_print_line_input_roundtrip_returns_written_line() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("roundtrip.txt");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!(
            "Sub Main()\n\
             Dim a\n\
             Open \"{path_literal}\" For Output As #1\n\
             Print #1, \"world\"\n\
             Close #1\n\
             Open \"{path_literal}\" For Input As #2\n\
             Line Input #2, a\n\
             Close #2\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed file roundtrip should execute");
        println!("ODG032-OBSERVED[CCT-033-LINE-001]={}", render_observed(&out[0]));
        assert_eq!(out[0], RuntimeValue::String(BStr("world".to_string())));
    }

    fn render_observed(value: &RuntimeValue) -> String {
        match value {
            RuntimeValue::String(BStr(text)) => text.clone(),
            other => format!("{other:?}"),
        }
    }
}
