#[cfg(target_os = "windows")]
mod windows_pointer_helper_e2e {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::RuntimeValue;

    fn run_windows_host_backed(source: &str, enable_jit: bool) -> Vec<RuntimeValue> {
        let mut engine = Engine::new(HostConfig {
            enable_jit,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_value_snapshot(source)
            .expect("pointer helper probe should execute")
    }

    fn expect_i64(value: &RuntimeValue) -> i64 {
        match value {
            RuntimeValue::I64(value) => *value,
            other => panic!("expected i64 pointer-like value, got {other:?}"),
        }
    }

    #[test]
    fn strptr_supports_wide_native_call_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function wcslen Lib "ucrtbase" Alias "wcslen" (ByVal textPtr As LongPtr) As LongPtr

Sub Main()
    Dim pointerValue As LongPtr
    Dim charCount As LongPtr
    pointerValue = StrPtr(":memory:")
    charCount = wcslen(pointerValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert_eq!(snapshot.len(), 2);
            assert!(
                expect_i64(&snapshot[0]) != 0,
                "StrPtr should produce a non-zero pointer-like value for enable_jit={enable_jit}"
            );
            assert_eq!(
                snapshot[1],
                RuntimeValue::I64(8),
                "wcslen should observe the wide string length for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn varptr_supports_scalar_pointer_value_in_vm_and_jit() {
        let source = r#"

Sub Main()
    Dim value As Long
    Dim pointerValue As LongPtr
    value = 42
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert_eq!(snapshot.len(), 2);
            assert_eq!(snapshot[0], RuntimeValue::I32(42));
            assert!(
                expect_i64(&snapshot[1]) != 0,
                "VarPtr should produce a non-zero pointer-like value for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn objptr_is_stable_for_same_object_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim obj As Object
    Dim firstPtr As LongPtr
    Dim secondPtr As LongPtr
    Set obj = CreateObject("OxVba.TestDispatch")
    firstPtr = ObjPtr(obj)
    secondPtr = ObjPtr(obj)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert_eq!(snapshot.len(), 3);
            let first = expect_i64(&snapshot[1]);
            let second = expect_i64(&snapshot[2]);
            assert!(first != 0, "ObjPtr should be non-zero for a live object for enable_jit={enable_jit}");
            assert_eq!(
                first, second,
                "ObjPtr should be stable for the same live object identity for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn objptr_returns_zero_for_runtime_nothing_after_failed_createobject() {
        let source = r#"
Sub Main()
    Dim obj As Object
    Dim ptrValue As LongPtr
    On Error Resume Next
    Set obj = CreateObject("OxVba.DoesNotExist.Component")
    ptrValue = ObjPtr(obj)
End Sub
"#;

        let snapshot = run_windows_host_backed(source, false);
        assert_eq!(snapshot.len(), 2);
        assert_eq!(expect_i64(&snapshot[1]), 0);
    }
}
