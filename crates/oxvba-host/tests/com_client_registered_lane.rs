#[cfg(target_os = "windows")]
mod windows_registered_com_lane {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::engine::DiagnosticPhase;
    use oxvba_host::{Engine, HostConfig};

    fn selected_registered_prog_id() -> String {
        std::env::var("OXVBA_REGISTERED_COM_PROGID")
            .unwrap_or_else(|_| "Scripting.Dictionary".to_string())
    }

    fn run_registered_lane_source(source: &str) -> Vec<i32> {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine.set_com_prog_id_override(4, selected_registered_prog_id());
        engine
            .execute_source_with_snapshot_phased(source)
            .expect("registered COM lane source should execute")
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_createobject_dispatchinvoke_success_lane() {
        let out = run_registered_lane_source(
            r#"
Sub Main()
Dim obj
Dim countValue
Dim existsValue
obj = CreateObject("Scripting.Dictionary")
countValue = DispatchInvoke(obj, "Count")
existsValue = DispatchInvoke(obj, "Exists", 42)
End Sub
"#,
        );
        assert!(
            out[0] >= 20_001,
            "registered lane should allocate native COM handle, got {:?}",
            out
        );
        assert_eq!(out[1], 0, "Scripting.Dictionary Count should be 0");
        assert_eq!(out[2], 0, "Scripting.Dictionary Exists(42) should be 0");
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_dispatchinvoke_missing_arg_routes_to_err() {
        let out = run_registered_lane_source(
            r#"
Sub Main()
Dim obj
Dim value
Dim errNo
On Error Resume Next
obj = CreateObject("Scripting.Dictionary")
value = DispatchInvoke(obj, "Exists")
errNo = Err.Number
End Sub
"#,
        );
        assert!(
            out[0] >= 20_001,
            "registered lane should allocate native COM handle, got {:?}",
            out
        );
        assert!(
            out[2] != 0,
            "missing argument should set Err.Number under resume-next, got {:?}",
            out
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_lane_repeated_invokes_are_stable() {
        let mut source = String::from(
            "Sub Main()\nDim obj\nDim value\nobj = CreateObject(\"Scripting.Dictionary\")\n",
        );
        for _ in 0..256 {
            source.push_str("value = DispatchInvoke(obj, \"Count\")\n");
            source.push_str("value = DispatchInvoke(obj, \"Exists\", 7)\n");
        }
        source.push_str("End Sub\n");

        let out = run_registered_lane_source(&source);
        assert!(
            out[0] >= 20_001,
            "registered lane should allocate native COM handle, got {:?}",
            out
        );
        assert_eq!(
            out[1], 0,
            "final Exists(7) should remain deterministic for empty dictionary"
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_member_not_found_routes_through_resume_next() {
        let out = run_registered_lane_source(
            r#"
Sub Main()
Dim obj
Dim value
Dim errNo
On Error Resume Next
obj = CreateObject("Scripting.Dictionary")
value = DispatchInvoke(obj, 777, 0)
errNo = Err.Number
End Sub
"#,
        );
        assert!(
            out[0] >= 20_001,
            "registered lane should allocate native COM handle, got {:?}",
            out
        );
        assert!(
            out[2] != 0,
            "member-not-found invoke should set Err.Number, got {:?}",
            out
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_class_not_registered_is_reported_with_stable_label() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine.set_com_prog_id_override(4, "OxVba.DoesNotExist.Component");
        let err = engine
            .execute_source_with_snapshot_phased(
                r#"
Sub Main()
Dim obj
obj = CreateObject("Scripting.Dictionary")
End Sub
"#,
            )
            .expect_err("missing registered class should error");
        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("com-createobject-class-not-registered")
                || err
                    .message()
                    .contains("com-createobject-invalid-class-string")
                || err.message().contains("0x80040154"),
            "expected class-not-registered mapping in error message, got {}",
            err.message()
        );
    }
}
