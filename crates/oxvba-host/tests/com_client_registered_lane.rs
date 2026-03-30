#[cfg(target_os = "windows")]
mod windows_registered_com_lane {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::engine::DiagnosticPhase;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::{ObjectHandle, RuntimeValue, bstr::BStr};

    const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RegisteredProgIdFlavor {
        ScriptingDictionary,
        OxvbaTestDispatch,
        OxvbaTestEventServer,
        ExcelApplication,
        Other,
    }

    impl RegisteredProgIdFlavor {
        fn from_prog_id(prog_id: &str) -> Self {
            if prog_id.eq_ignore_ascii_case("Scripting.Dictionary") {
                return Self::ScriptingDictionary;
            }
            if prog_id.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
                return Self::OxvbaTestDispatch;
            }
            if prog_id.eq_ignore_ascii_case("OxVba.TestEventServer") {
                return Self::OxvbaTestEventServer;
            }
            if prog_id.eq_ignore_ascii_case("Excel.Application") {
                return Self::ExcelApplication;
            }
            Self::Other
        }

        fn expected_count_value(self) -> Option<i32> {
            match self {
                Self::ScriptingDictionary => Some(0),
                Self::OxvbaTestDispatch => Some(7),
                Self::OxvbaTestEventServer => None,
                Self::ExcelApplication => None,
                Self::Other => None,
            }
        }

        fn expected_exists_42_value(self) -> Option<i32> {
            match self {
                Self::ScriptingDictionary => Some(0),
                Self::OxvbaTestDispatch => Some(1),
                Self::OxvbaTestEventServer => None,
                Self::ExcelApplication => None,
                Self::Other => None,
            }
        }

        fn is_event_capable_for_registered_lane(self) -> bool {
            matches!(
                self,
                Self::ScriptingDictionary
                    | Self::OxvbaTestDispatch
                    | Self::OxvbaTestEventServer
                    | Self::ExcelApplication
            )
        }
    }

    fn selected_registered_prog_id() -> String {
        std::env::var("OXVBA_REGISTERED_COM_PROGID")
            .unwrap_or_else(|_| "Scripting.Dictionary".to_string())
    }

    fn selected_registered_prog_id_flavor() -> RegisteredProgIdFlavor {
        RegisteredProgIdFlavor::from_prog_id(&selected_registered_prog_id())
    }

    fn read_env_i32(name: &str, default_value: i32) -> i32 {
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<i32>().ok())
            .unwrap_or(default_value)
    }

    fn read_env_usize(name: &str, default_value: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(default_value)
    }

    fn read_env_u64(name: &str, default_value: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(default_value)
    }

    fn registered_event_success_required() -> bool {
        std::env::var("OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS")
            .ok()
            .map(|value| {
                let normalized = value.trim().to_ascii_lowercase();
                normalized == "1" || normalized == "true" || normalized == "yes"
            })
            .unwrap_or(false)
    }

    fn registered_event_token() -> i32 {
        let default_token = match selected_registered_prog_id_flavor() {
            RegisteredProgIdFlavor::ExcelApplication => 10,
            _ => 1,
        };
        read_env_i32("OXVBA_REGISTERED_EVENT_TOKEN", default_token)
    }

    fn registered_event_trigger_member() -> i32 {
        let default_member = match selected_registered_prog_id_flavor() {
            RegisteredProgIdFlavor::ScriptingDictionary => 2,
            RegisteredProgIdFlavor::ExcelApplication => 10,
            _ => 3,
        };
        read_env_i32("OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER", default_member)
    }

    fn registered_event_trigger_arg() -> i32 {
        let default_arg = match selected_registered_prog_id_flavor() {
            RegisteredProgIdFlavor::ScriptingDictionary => 42,
            RegisteredProgIdFlavor::ExcelApplication => 0,
            _ => 77,
        };
        read_env_i32("OXVBA_REGISTERED_EVENT_TRIGGER_ARG", default_arg)
    }

    fn registered_event_expected_arg_count() -> usize {
        let default_count = match selected_registered_prog_id_flavor() {
            RegisteredProgIdFlavor::ExcelApplication => 0,
            _ => 1,
        };
        read_env_usize("OXVBA_REGISTERED_EVENT_EXPECTED_ARGC", default_count)
    }

    fn registered_event_poll_iterations() -> usize {
        read_env_usize("OXVBA_REGISTERED_EVENT_POLL_ITERATIONS", 40).max(1)
    }

    fn registered_event_poll_delay_ms() -> u64 {
        read_env_u64("OXVBA_REGISTERED_EVENT_POLL_DELAY_MS", 50)
    }

    fn selected_registered_createobject_line() -> String {
        format!("obj = CreateObject(\"{}\")", selected_registered_prog_id())
    }

    fn run_registered_lane_source(source: &str) -> Vec<RuntimeValue> {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_snapshot_phased(source)
            .expect("registered COM lane source should execute")
    }

    fn registered_lane_available() -> bool {
        std::panic::catch_unwind(|| {
            let _ = run_registered_lane_source(
                &format!(
                    "Sub Main()\nDim obj\n{}\nEnd Sub\n",
                    selected_registered_createobject_line()
                ),
            );
        })
        .is_ok()
    }

    fn expect_object_handle(value: &RuntimeValue) -> ObjectHandle {
        match value {
            RuntimeValue::ObjectHandle(handle) => *handle,
            other => panic!("expected object handle, got {:?}", other),
        }
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_createobject_dispatchinvoke_success_lane() {
        let selected_prog_id = selected_registered_prog_id();
        let flavor = RegisteredProgIdFlavor::from_prog_id(&selected_prog_id);
        let source = format!(
            r#"
Sub Main()
Dim obj
Dim countValue
Dim existsValue
{createobject_line}
countValue = DispatchInvoke(obj, "Count")
existsValue = DispatchInvoke(obj, "Exists", 42)
End Sub
"#,
            createobject_line = selected_registered_createobject_line(),
        );
        let out = run_registered_lane_source(&source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        if let Some(expected) = flavor.expected_count_value() {
            assert_eq!(
                out[1],
                RuntimeValue::I32(expected),
                "Count result mismatch for ProgID `{selected_prog_id}`"
            );
        } else {
            eprintln!(
                "registered lane: no count-value expectation configured for ProgID `{selected_prog_id}`"
            );
        }
        if let Some(expected) = flavor.expected_exists_42_value() {
            assert_eq!(
                out[2],
                RuntimeValue::I32(expected),
                "Exists(42) result mismatch for ProgID `{selected_prog_id}`"
            );
        } else {
            eprintln!(
                "registered lane: no Exists(42) expectation configured for ProgID `{selected_prog_id}`"
            );
        }
    }

    #[test]
    fn registered_dispatchinvoke_missing_arg_routes_to_err() {
        if !registered_lane_available() {
            eprintln!(
                "registered lane: selected ProgID `{}` is not available in this environment",
                selected_registered_prog_id()
            );
            return;
        }

        let source = format!(
            r#"
Sub Main()
Dim obj
Dim value
Dim errNo
On Error Resume Next
{createobject_line}
value = DispatchInvoke(obj, "Exists")
errNo = Err.Number
End Sub
"#,
            createobject_line = selected_registered_createobject_line(),
        );
        let out = run_registered_lane_source(&source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert!(
            !matches!(out[2], RuntimeValue::I32(0)),
            "missing argument should set Err.Number under resume-next, got {:?}",
            out
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_lane_repeated_invokes_are_stable() {
        let mut source = format!(
            "Sub Main()\nDim obj\nDim value\n{}\n",
            selected_registered_createobject_line()
        );
        for _ in 0..256 {
            source.push_str("value = DispatchInvoke(obj, \"Count\")\n");
            source.push_str("value = DispatchInvoke(obj, \"Exists\", 7)\n");
        }
        source.push_str("End Sub\n");

        let out = run_registered_lane_source(&source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(
            out[1],
            RuntimeValue::I32(0),
            "final Exists(7) should remain deterministic for empty dictionary"
        );
    }

    #[test]
    fn registered_testeventserver_scalar_sum_pair_supported_subset() {
        if selected_registered_prog_id_flavor() != RegisteredProgIdFlavor::OxvbaTestEventServer {
            eprintln!(
                "registered lane: this marshaling probe is specific to OxVba.TestEventServer"
            );
            return;
        }

        let source = r#"
Sub Main()
Dim obj
Dim sumValue
obj = CreateObject("OxVba.TestEventServer")
 sumValue = DispatchInvoke(obj, "SumPair", 3, 14)
End Sub
"#;

        let out = run_registered_lane_source(source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(out[1], RuntimeValue::I32(17), "SumPair result mismatch");
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-testeventserver-marshaling-oracle.ps1)"]
    fn registered_testeventserver_array_argument_supported_subset() {
        if selected_registered_prog_id_flavor() != RegisteredProgIdFlavor::OxvbaTestEventServer {
            eprintln!(
                "registered lane: this marshaling probe is specific to OxVba.TestEventServer"
            );
            return;
        }

        let source = r#"
Sub Main()
Dim obj
Dim shapeValue
obj = CreateObject("OxVba.TestEventServer")
Call CaptureShape(shapeValue, obj, 1, 2, 3)
End Sub

Sub CaptureShape(ByRef target, ByVal obj, ParamArray items() As Variant)
    target = DispatchInvoke(obj, "DescribeArrayShape", items)
End Sub
"#;

        let out = run_registered_lane_source(source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(
            out[1],
            RuntimeValue::String(BStr("rank=1;len=3;lb=0;ub=2;first=1".to_string())),
            "DescribeArrayShape result mismatch"
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-testeventserver-marshaling-oracle.ps1)"]
    fn registered_testeventserver_object_argument_supported_subset() {
        if selected_registered_prog_id_flavor() != RegisteredProgIdFlavor::OxvbaTestEventServer {
            eprintln!(
                "registered lane: this marshaling probe is specific to OxVba.TestEventServer"
            );
            return;
        }

        let source = r#"
Sub Main()
Dim obj
Dim selfValue
obj = CreateObject("OxVba.TestEventServer")
selfValue = DispatchInvoke(obj, "IsSelf", obj)
End Sub
"#;

        let out = run_registered_lane_source(source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(out[1], RuntimeValue::Bool(true), "IsSelf result mismatch");
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-testeventserver-marshaling-oracle.ps1)"]
    fn registered_testeventserver_scalar_array_return_supported_subset() {
        if selected_registered_prog_id_flavor() != RegisteredProgIdFlavor::OxvbaTestEventServer {
            eprintln!(
                "registered lane: this marshaling probe is specific to OxVba.TestEventServer"
            );
            return;
        }

        let source = r#"
Sub Main()
Dim obj
Dim returned
obj = CreateObject("OxVba.TestEventServer")
returned = DispatchInvoke(obj, "ReturnLongArray")
End Sub
"#;

        let out = run_registered_lane_source(source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        let RuntimeValue::ArrayIntent(array) = &out[1] else {
            panic!("expected array result, got {:?}", out[1]);
        };
        let elements = array
            .elements
            .as_ref()
            .expect("ReturnLongArray should preserve elements");
        assert_eq!(elements.len(), 3, "ReturnLongArray length mismatch");
        assert_eq!(
            elements[0],
            RuntimeValue::I32(4),
            "ReturnLongArray first element mismatch"
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-testeventserver-marshaling-oracle.ps1)"]
    fn registered_testeventserver_dispatch_array_return_supported_subset() {
        if selected_registered_prog_id_flavor() != RegisteredProgIdFlavor::OxvbaTestEventServer {
            eprintln!(
                "registered lane: this marshaling probe is specific to OxVba.TestEventServer"
            );
            return;
        }

        let source = r#"
Sub Main()
Dim obj
Dim returnedSelf
obj = CreateObject("OxVba.TestEventServer")
returnedSelf = DispatchInvoke(obj, "ReturnSelfArray")
End Sub
"#;

        let out = run_registered_lane_source(source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        let RuntimeValue::ArrayIntent(array) = &out[1] else {
            panic!("expected array result, got {:?}", out[1]);
        };
        let elements = array
            .elements
            .as_ref()
            .expect("ReturnSelfArray should preserve elements");
        assert_eq!(elements.len(), 1, "ReturnSelfArray length mismatch");
        let RuntimeValue::ObjectHandle(handle) = elements[0] else {
            panic!(
                "expected first ReturnSelfArray element to be an object handle, got {:?}",
                elements[0]
            );
        };
        assert!(
            handle.raw() >= 20_001,
            "ReturnSelfArray object handle mismatch"
        );
    }

    #[test]
    fn registered_member_not_found_routes_through_resume_next() {
        if !registered_lane_available() {
            eprintln!(
                "registered lane: selected ProgID `{}` is not available in this environment",
                selected_registered_prog_id()
            );
            return;
        }

        let source = format!(
            r#"
Sub Main()
Dim obj
Dim value
Dim errNo
On Error Resume Next
{createobject_line}
value = DispatchInvoke(obj, 777, 0)
errNo = Err.Number
End Sub
"#,
            createobject_line = selected_registered_createobject_line(),
        );
        let out = run_registered_lane_source(&source);
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert!(
            !matches!(out[2], RuntimeValue::I32(0)),
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
        let err = engine
            .execute_source_with_snapshot_phased(
                r#"
Sub Main()
Dim obj
obj = CreateObject("OxVba.DoesNotExist.Component")
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

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_event_subscribe_without_connection_point_has_stable_error_shape() {
        if selected_registered_prog_id_flavor().is_event_capable_for_registered_lane() {
            eprintln!(
                "registered lane: selected ProgID appears event-capable; skipping failure-shape assertion"
            );
            return;
        }

        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        let source = format!(
            "Sub Main()\nDim obj\n{}\nEnd Sub\n",
            selected_registered_createobject_line()
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("registered lane should create COM object");
        let object = expect_object_handle(&out[0]);
        let err = engine
            .subscribe_com_event_handler(object, 99, "Sink_OnChanged")
            .expect_err("object without event connection-point mapping should fail subscribe");

        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message()
                .contains("COM-E-EVENT-CONNECTIONPOINT-MISSING")
                || err.message().contains("COM-E-EVENT-PATH-UNSUPPORTED"),
            "expected deterministic COM event subscribe failure mapping, got {}",
            err.message()
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_event_unsubscribe_unknown_subscription_has_stable_error_shape() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());

        let err = engine
            .unsubscribe_com_event_handler(99_901.into())
            .expect_err("unsubscribe with unknown token should fail deterministically");

        assert_eq!(err.phase(), DiagnosticPhase::Runtime);
        assert!(
            err.message().contains("COM-E-EVENT-ADVISE-FAILED"),
            "expected deterministic unadvise failure mapping, got {}",
            err.message()
        );
    }

    #[test]
    #[ignore = "requires registered external COM server lane (run via scripts/run-com-registered.ps1)"]
    fn registered_event_callback_success_when_event_capable_server_is_configured() {
        let selected_prog_id = selected_registered_prog_id();
        let require_success = registered_event_success_required();
        if !selected_registered_prog_id_flavor().is_event_capable_for_registered_lane()
            && !require_success
        {
            eprintln!(
                "registered lane: event callback success not required for ProgID `{selected_prog_id}`"
            );
            return;
        }

        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        let source = format!(
            "Sub Main()\nDim obj\n{}\nEnd Sub\n",
            selected_registered_createobject_line()
        );

        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("registered lane should create COM object");
        let object = expect_object_handle(&out[0]);

        let event_token = registered_event_token();
        let trigger_member = registered_event_trigger_member();
        let trigger_arg = registered_event_trigger_arg();
        let expected_arg_count = registered_event_expected_arg_count();
        let poll_iterations = registered_event_poll_iterations();
        let poll_delay_ms = registered_event_poll_delay_ms();

        let subscription = match engine.subscribe_com_event_handler(
            object,
            event_token,
            "SinkA_OnChanged",
        ) {
            Ok(subscription) => subscription,
            Err(err) => {
                if require_success {
                    panic!(
                        "registered lane event subscription was required for `{selected_prog_id}` but failed: {}",
                        err.message()
                    );
                }
                eprintln!(
                    "registered lane: event subscription unavailable for `{selected_prog_id}`: {}",
                    err.message()
                );
                return;
            }
        };

        let trigger_source = format!(
            "Sub Main()\nDim value\nvalue = DispatchInvoke({object}, {trigger_member}, {trigger_arg})\nEnd Sub\n"
        );
        let trigger_result = engine.execute_source_with_snapshot_phased(&trigger_source);
        if let Err(err) = trigger_result {
            let _ = engine.unsubscribe_com_event_handler(subscription);
            if require_success {
                panic!(
                    "registered lane event trigger was required for `{selected_prog_id}` but failed: {}",
                    err.message()
                );
            }
            eprintln!(
                "registered lane: event trigger did not produce callback lane for `{selected_prog_id}`: {}",
                err.message()
            );
            return;
        }

        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut callback = None;
        for _ in 0..poll_iterations {
            match engine.poll_com_event_callback() {
                Ok(Some(next)) => {
                    callback = Some(next);
                    break;
                }
                Ok(None) => {
                    let burst_count = (poll_delay_ms / 5).max(1);
                    for _ in 0..burst_count {
                        let _ = engine.poll_com_event_callback();
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
                Err(err) => {
                    let _ = engine.unsubscribe_com_event_handler(subscription);
                    if require_success {
                        panic!(
                            "registered lane event callback poll was required for `{selected_prog_id}` but failed: {}",
                            err.message()
                        );
                    }
                    eprintln!(
                        "registered lane: callback polling unavailable for `{selected_prog_id}`: {}",
                        err.message()
                    );
                    return;
                }
            }
        }
        let Some(callback) = callback else {
            let _ = engine.unsubscribe_com_event_handler(subscription);
            if require_success {
                panic!(
                    "registered lane event callback was required for `{selected_prog_id}` but no callback was available"
                );
            }
            eprintln!(
                "registered lane: no callback observed for `{selected_prog_id}`; treating as optional in this run"
            );
            return;
        };

        assert_eq!(
            callback.subscription_token, subscription,
            "callback subscription token mismatch for `{selected_prog_id}`"
        );
        assert_eq!(
            callback.handler_symbol, "sinka_onchanged",
            "callback handler symbol mismatch for `{selected_prog_id}`"
        );
        assert_eq!(
            callback.args.len(),
            expected_arg_count,
            "callback arg count mismatch for `{selected_prog_id}`"
        );
        if expected_arg_count == 0 {
            assert!(
                callback.args.is_empty(),
                "callback payload should be empty for `{selected_prog_id}`"
            );
        } else {
            assert_eq!(
                callback.args[0],
                RuntimeValue::I32(trigger_arg),
                "callback first-arg mismatch for `{selected_prog_id}`"
            );
            if expected_arg_count == 1 {
                assert_eq!(
                    callback.args,
                    vec![RuntimeValue::I32(trigger_arg)],
                    "callback payload mismatch for `{selected_prog_id}`"
                );
            }
        }
        let removed = engine
            .unsubscribe_com_event_handler(subscription)
            .expect("registered lane callback subscription should unsubscribe");
        assert!(
            removed,
            "registered lane callback subscription should be removed from handler registry"
        );
        assert!(
            engine
                .poll_com_event_callback()
                .expect("post-unsubscribe callback poll should succeed")
                .is_none(),
            "post-unsubscribe callback queue should be empty"
        );
    }
}
