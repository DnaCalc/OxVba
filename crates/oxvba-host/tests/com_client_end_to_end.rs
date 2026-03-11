#[cfg(target_os = "windows")]
mod windows_com_e2e {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};

    fn run_windows_host_backed(source: &str, enable_jit: bool) -> Vec<i32> {
        let mut engine = Engine::new(HostConfig {
            enable_jit,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_snapshot_phased(source)
            .expect("windows host-backed COM lane should execute")
    }

    #[test]
    fn createobject_and_dispatchinvoke_use_controlled_native_com_server() {
        let out = run_windows_host_backed(
            r#"
Sub Main()
Dim obj
Dim countValue
Dim existsValue
obj = CreateObject("OxVba.TestDispatch")
countValue = DispatchInvoke(obj, "Count")
existsValue = DispatchInvoke(obj, "Exists", 42)
End Sub
"#,
            false,
        );

        assert!(
            out[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            out
        );
        assert_eq!(out[1], 7, "controlled COM Count contract mismatch");
        assert_eq!(out[2], 1, "controlled COM Exists(42) contract mismatch");
    }

    #[test]
    fn dispatchinvoke_property_put_and_putref_routes_are_deterministic() {
        let source = r#"
Sub Main()
Dim obj
Dim setValueResult
Dim valueAfterSet
Dim setValueRefResult
Dim valueAfterSetRef
obj = CreateObject("OxVba.TestDispatch")
setValueResult = DispatchInvoke(obj, "SetValue", 12)
valueAfterSet = DispatchInvoke(obj, "Value")
setValueRefResult = DispatchInvoke(obj, "SetValueRef", 12)
valueAfterSetRef = DispatchInvoke(obj, "Value")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on property put/putref path: vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            vm
        );
        assert_eq!(vm[1], 12, "SetValue should store direct value");
        assert_eq!(vm[2], 12, "Value getter should reflect SetValue result");
        assert_eq!(
            vm[3], 100_012,
            "SetValueRef should take deterministic putref route"
        );
        assert_eq!(
            vm[4], 100_012,
            "Value getter should reflect SetValueRef result"
        );
    }

    #[test]
    fn dispatchinvoke_multi_arg_method_and_indexed_property_routes_are_deterministic() {
        let source = r#"
Sub Main()
Dim obj
Dim sumPair
Dim lookupPair
Dim setIndexedValueResult
Dim valueAfterSetIndexed
Dim setIndexedValueRefResult
Dim valueAfterSetIndexedRef
obj = CreateObject("OxVba.TestDispatch")
sumPair = DispatchInvoke(obj, "SumPair", 3, 14)
lookupPair = DispatchInvoke(obj, "LookupPair", 5, 9)
setIndexedValueResult = DispatchInvoke(obj, "SetIndexedValue", 7, 11)
valueAfterSetIndexed = DispatchInvoke(obj, "Value")
setIndexedValueRefResult = DispatchInvoke(obj, "SetIndexedValueRef", 8, 13)
valueAfterSetIndexedRef = DispatchInvoke(obj, "Value")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on multi-arg COM path: vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            vm
        );
        assert_eq!(vm[1], 3_014, "SumPair should preserve both arguments");
        assert_eq!(vm[2], 205_009, "LookupPair should preserve both arguments");
        assert_eq!(
            vm[3], 307_011,
            "SetIndexedValue should route through multi-arg property put"
        );
        assert_eq!(
            vm[4], 307_011,
            "Value getter should reflect multi-arg property put result"
        );
        assert_eq!(
            vm[5], 408_013,
            "SetIndexedValueRef should route through multi-arg property putref"
        );
        assert_eq!(
            vm[6], 408_013,
            "Value getter should reflect multi-arg property putref result"
        );
    }

    #[test]
    fn dispatchinvoke_named_indexed_property_put_and_putref_routes_are_deterministic() {
        let source = r#"
Sub Main()
Dim obj
Dim setIndexedValueResult
Dim valueAfterSetIndexed
Dim setIndexedValueRefResult
Dim valueAfterSetIndexedRef
obj = CreateObject("OxVba.TestDispatch")
setIndexedValueResult = DispatchInvoke(obj, "SetIndexedValue", value := 11, lhs := 7)
valueAfterSetIndexed = DispatchInvoke(obj, "Value")
setIndexedValueRefResult = DispatchInvoke(obj, "SetIndexedValueRef", value := 13, lhs := 8)
valueAfterSetIndexedRef = DispatchInvoke(obj, "Value")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on named indexed property put path: vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            vm
        );
        assert_eq!(
            vm[1], 307_011,
            "SetIndexedValue should accept fully named indexed property put arguments"
        );
        assert_eq!(
            vm[2], 307_011,
            "Value getter should reflect named indexed property put result"
        );
        assert_eq!(
            vm[3], 408_013,
            "SetIndexedValueRef should accept fully named indexed property putref arguments"
        );
        assert_eq!(
            vm[4], 408_013,
            "Value getter should reflect named indexed property putref result"
        );
    }

    #[test]
    fn dispatchinvoke_variant_error_and_null_tokens_roundtrip_deterministically() {
        let source = r#"
Sub Main()
Dim obj
Dim errValue
Dim nullValue
obj = CreateObject("OxVba.TestDispatch")
errValue = DispatchInvoke(obj, "EchoVariant", CVErr(17))
nullValue = DispatchInvoke(obj, "EchoVariant", Null)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on VT_ERROR/VT_NULL roundtrip path: vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            vm
        );
        assert_eq!(
            vm[1], -899_999_983,
            "CVErr(17) should roundtrip as an error tag"
        );
        assert_eq!(vm[2], -1, "Null should roundtrip as the stable null tag");
    }

    #[test]
    fn dispatchinvoke_error_path_routes_through_on_error_resume_next() {
        let out = run_windows_host_backed(
            r#"
Sub Main()
Dim obj
Dim keepOk
Dim keepAfter
Dim errNo
On Error Resume Next
obj = CreateObject("OxVba.TestDispatch")
keepOk = DispatchInvoke(obj, "Exists", 42)
keepAfter = DispatchInvoke(obj, "Exists")
errNo = Err.Number
End Sub
"#,
            false,
        );

        assert!(
            out[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            out
        );
        assert_eq!(out[1], 1, "pre-error call should have succeeded");
        assert!(
            out[3] != 0,
            "missing-argument COM invoke should set Err.Number, got {:?}",
            out
        );
    }

    #[test]
    fn dispatchinvoke_exception_path_routes_through_on_error_resume_next() {
        let out = run_windows_host_backed(
            r#"
Sub Main()
Dim obj
Dim keepOk
Dim errNo
On Error Resume Next
obj = CreateObject("OxVba.TestDispatch")
keepOk = DispatchInvoke(obj, "Exists", 42)
Call DispatchInvoke(obj, "RaiseException")
errNo = Err.Number
End Sub
"#,
            false,
        );

        assert!(
            out[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            out
        );
        assert_eq!(out[1], 1, "pre-exception call should have succeeded");
        assert!(
            out[2] != 0,
            "exception COM invoke should set Err.Number, got {:?}",
            out
        );
    }

    #[test]
    fn repeated_dispatch_calls_stay_stable_under_pressure() {
        let mut source = String::from(
            "Sub Main()\nDim obj\nDim value\nobj = CreateObject(\"OxVba.TestDispatch\")\n",
        );
        for _ in 0..512 {
            source.push_str("value = DispatchInvoke(obj, \"Count\")\n");
            source.push_str("value = DispatchInvoke(obj, \"Exists\", 41)\n");
            source.push_str("value = DispatchInvoke(obj, \"Exists\", 42)\n");
        }
        source.push_str("End Sub\n");

        let out = run_windows_host_backed(&source, false);
        assert!(
            out[0] >= 20_001,
            "expected native COM object handle space, got {:?}",
            out
        );
        assert_eq!(
            out[1], 1,
            "final Exists(42) result should remain stable after repeated dispatch"
        );
    }

    #[test]
    fn createobject_dispatchinvoke_vm_jit_snapshots_match() {
        let source = r#"
Sub Main()
Dim obj
Dim countValue
Dim existsValue
obj = CreateObject("OxVba.TestDispatch")
countValue = DispatchInvoke(obj, "Count")
existsValue = DispatchInvoke(obj, "Exists", 42)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on controlled COM success path: vm={vm:?} jit={jit:?}"
        );
    }

    #[test]
    fn resume_next_com_failure_vm_jit_snapshots_match() {
        let source = r#"
Sub Main()
Dim obj
Dim keepOk
Dim keepAfter
Dim errNo
On Error Resume Next
obj = CreateObject("OxVba.TestDispatch")
keepOk = DispatchInvoke(obj, "Exists", 42)
keepAfter = DispatchInvoke(obj, "Exists")
errNo = Err.Number
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on COM failure/resume-next path: vm={vm:?} jit={jit:?}"
        );
    }
}
