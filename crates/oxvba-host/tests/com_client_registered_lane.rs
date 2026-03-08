#[cfg(target_os = "windows")]
mod windows_registered_com_lane {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::engine::DiagnosticPhase;
    use oxvba_host::{Engine, HostConfig};

    const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RegisteredProgIdFlavor {
        ScriptingDictionary,
        OxvbaTestDispatch,
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
            Self::Other
        }

        fn expected_count_value(self) -> Option<i32> {
            match self {
                Self::ScriptingDictionary => Some(0),
                Self::OxvbaTestDispatch => Some(7),
                Self::Other => None,
            }
        }

        fn expected_exists_42_value(self) -> Option<i32> {
            match self {
                Self::ScriptingDictionary => Some(0),
                Self::OxvbaTestDispatch => Some(1),
                Self::Other => None,
            }
        }

        fn is_event_capable_for_registered_lane(self) -> bool {
            matches!(self, Self::ScriptingDictionary | Self::OxvbaTestDispatch)
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
        read_env_i32("OXVBA_REGISTERED_EVENT_TOKEN", 1)
    }

    fn registered_event_trigger_member() -> i32 {
        let default_member = match selected_registered_prog_id_flavor() {
            RegisteredProgIdFlavor::ScriptingDictionary => 2,
            _ => 3,
        };
        read_env_i32("OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER", default_member)
    }

    fn registered_event_trigger_arg() -> i32 {
        let default_arg = match selected_registered_prog_id_flavor() {
            RegisteredProgIdFlavor::ScriptingDictionary => 42,
            _ => 77,
        };
        read_env_i32("OXVBA_REGISTERED_EVENT_TRIGGER_ARG", default_arg)
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
        let selected_prog_id = selected_registered_prog_id();
        let flavor = RegisteredProgIdFlavor::from_prog_id(&selected_prog_id);
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
        if let Some(expected) = flavor.expected_count_value() {
            assert_eq!(
                out[1], expected,
                "Count result mismatch for ProgID `{selected_prog_id}`"
            );
        } else {
            eprintln!(
                "registered lane: no count-value expectation configured for ProgID `{selected_prog_id}`"
            );
        }
        if let Some(expected) = flavor.expected_exists_42_value() {
            assert_eq!(
                out[2], expected,
                "Exists(42) result mismatch for ProgID `{selected_prog_id}`"
            );
        } else {
            eprintln!(
                "registered lane: no Exists(42) expectation configured for ProgID `{selected_prog_id}`"
            );
        }
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
        engine.set_com_prog_id_override(4, selected_registered_prog_id());
        let out = engine
            .execute_source_with_snapshot_phased(
                r#"
Sub Main()
Dim obj
obj = CreateObject("Scripting.Dictionary")
End Sub
"#,
            )
            .expect("registered lane should create COM object");
        let object = out[0];
        assert!(
            object >= 20_001,
            "registered lane should allocate native COM handle, got {:?}",
            out
        );
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
            .unsubscribe_com_event_handler(99_901)
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
        engine.set_com_prog_id_override(4, &selected_prog_id);

        let out = engine
            .execute_source_with_snapshot_phased(
                r#"
Sub Main()
Dim obj
obj = CreateObject("Scripting.Dictionary")
End Sub
"#,
            )
            .expect("registered lane should create COM object");
        let object = out[0];
        assert!(
            object >= 20_001,
            "registered lane should allocate native COM handle, got {:?}",
            out
        );

        let event_token = registered_event_token();
        let trigger_member = registered_event_trigger_member();
        let trigger_arg = registered_event_trigger_arg();

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

        let callback = match engine.poll_com_event_callback() {
            Ok(Some(callback)) => callback,
            Ok(None) => {
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
            callback.args,
            vec![trigger_arg],
            "callback payload mismatch for `{selected_prog_id}`"
        );
        assert_eq!(
            callback.arg0, trigger_arg,
            "callback arg0 mismatch for `{selected_prog_id}`"
        );
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
