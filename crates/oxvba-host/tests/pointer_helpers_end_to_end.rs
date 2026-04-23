#[cfg(target_os = "windows")]
mod windows_pointer_helper_e2e {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::{Decimal96, RuntimeValue};
    use windows_sys::Win32::Foundation::SysStringLen;

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
    fn varptr_supports_byte_buffer_native_read_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function strlen Lib "ucrtbase" Alias "strlen" (ByVal textPtr As LongPtr) As LongPtr

Sub Main()
    Dim buf() As Byte
    Dim pointerValue As LongPtr
    Dim byteCount As LongPtr
    ReDim buf(0 To 2)
    buf(0) = 65
    buf(1) = 66
    buf(2) = 0
    pointerValue = VarPtr(buf(0))
    byteCount = strlen(pointerValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let pointer = snapshot
                .iter()
                .find_map(|value| match value {
                    RuntimeValue::I64(value) => Some(*value),
                    _ => None,
                })
                .expect("snapshot should contain a pointer-like value");
            assert!(
                pointer != 0,
                "VarPtr(buf(0)) should produce a non-zero pointer-like value for enable_jit={enable_jit}"
            );
            assert_eq!(
                snapshot
                    .iter()
                    .find(|value| matches!(value, RuntimeValue::I64(2)))
                    .cloned()
                    .unwrap_or(RuntimeValue::Empty),
                RuntimeValue::I64(2),
                "strlen should observe the zero-terminated byte buffer for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn varptr_supports_static_byte_buffer_native_read_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function strlen Lib "ucrtbase" Alias "strlen" (ByVal textPtr As LongPtr) As LongPtr

Sub Main()
    Dim buf(2) As Byte
    Dim pointerValue As LongPtr
    Dim byteCount As LongPtr
    buf(0) = 65
    buf(1) = 66
    buf(2) = 0
    pointerValue = VarPtr(buf(0))
    byteCount = strlen(pointerValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let pointer = snapshot
                .iter()
                .find_map(|value| match value {
                    RuntimeValue::I64(value) => Some(*value),
                    _ => None,
                })
                .expect("snapshot should contain a pointer-like value");
            assert!(
                pointer != 0,
                "VarPtr(buf(0)) should produce a non-zero pointer-like value for enable_jit={enable_jit}"
            );
            assert_eq!(
                snapshot
                    .iter()
                    .find(|value| matches!(value, RuntimeValue::I64(2)))
                    .cloned()
                    .unwrap_or(RuntimeValue::Empty),
                RuntimeValue::I64(2),
                "strlen should observe the zero-terminated static byte buffer for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn varptr_supports_byte_buffer_parameter_native_read_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function strlen Lib "ucrtbase" Alias "strlen" (ByVal textPtr As LongPtr) As LongPtr

Private Function BufferLen(ByRef value() As Byte) As LongPtr
    BufferLen = strlen(VarPtr(value(0)))
End Function

Sub Main()
    Dim buf(2) As Byte
    Dim byteCount As LongPtr
    buf(0) = 65
    buf(1) = 66
    buf(2) = 0
    byteCount = BufferLen(buf)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert_eq!(
                snapshot
                    .iter()
                    .find(|value| matches!(value, RuntimeValue::I64(2)))
                    .cloned()
                    .unwrap_or(RuntimeValue::Empty),
                RuntimeValue::I64(2),
                "strlen should observe the zero-terminated byte buffer through the array-parameter lane for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn fixed_byte_array_parameter_preserves_runtime_bounds_in_vm_and_jit() {
        let source = r#"
Private Sub MeasureBounds(ByRef value() As Byte, ByRef lowerValue As Long, ByRef upperValue As Long)
    lowerValue = LBound(value)
    upperValue = UBound(value)
End Sub

Sub Main()
    Dim buf(2) As Byte
    Dim lowerValue As Long
    Dim upperValue As Long
    MeasureBounds buf, lowerValue, upperValue
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 2)),
                "UBound on a fixed array passed to a regular array parameter should be 2 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn fixed_byte_array_parameter_preserves_lbound_and_span_in_vm_and_jit() {
        let source = r#"
Private Sub MeasureBounds(ByRef value() As Byte, ByRef lowerValue As Long, ByRef upperValue As Long, ByRef spanValue As Long)
    lowerValue = LBound(value)
    upperValue = UBound(value)
    spanValue = UBound(value) - LBound(value) + 1
End Sub

Sub Main()
    Dim buf(2) As Byte
    Dim lowerValue As Long
    Dim upperValue As Long
    Dim spanValue As Long
    MeasureBounds buf, lowerValue, upperValue, spanValue
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 0)),
                "LBound on a fixed array passed to a regular array parameter should be 0 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 2)),
                "UBound on a fixed array passed to a regular array parameter should be 2 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 3)),
                "Span on a fixed array passed to a regular array parameter should be 3 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn varptr_exposes_byte_buffer_contents_to_runtime_registry_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim buf() As Byte
    Dim pointerValue As LongPtr
    ReDim buf(0 To 2)
    buf(0) = 65
    buf(1) = 66
    buf(2) = 0
    pointerValue = VarPtr(buf(0))
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let pointer = snapshot
                .iter()
                .find_map(|value| match value {
                    RuntimeValue::I64(value) if *value != 0 => Some(*value),
                    _ => None,
                })
                .expect("snapshot should contain a non-zero pointer-like value");
            assert_ne!(pointer, 0);
            let raw = oxvba_runtime::pointer_helpers::lookup_pointer(pointer)
                .expect("pointer helper registry should contain VarPtr result")
                .cast::<u8>();
            assert!(!raw.is_null());
            let bytes = unsafe { std::slice::from_raw_parts(raw, 3) };
            assert_eq!(
                bytes,
                &[65, 66, 0],
                "VarPtr(buf(0)) should expose the byte buffer contents for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn dynamic_byte_array_direct_indexing_preserves_byte_values_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim buf() As Byte
    Dim x0 As Long
    Dim x1 As Long
    Dim x2 As Long
    ReDim buf(2)
    buf(0) = 90
    buf(1) = 91
    buf(2) = 92
    x0 = buf(0)
    x1 = buf(1)
    x2 = buf(2)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 90)),
                "direct dynamic-array index 0 should be 90 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 91)),
                "direct dynamic-array index 1 should be 91 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 92)),
                "direct dynamic-array index 2 should be 92 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn runtime_sized_byte_array_native_writeback_and_index_reads_work_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Sub RtlMoveMemory Lib "kernel32" (ByVal pDest As LongPtr, ByVal pSource As LongPtr, ByVal length As Long)

Sub Main()
    Dim src(2) As Byte
    Dim length As Long
    Dim dst() As Byte
    Dim x0 As Long
    Dim x1 As Long
    Dim x2 As Long
    Dim i As Long
    Dim sum As Long
    Dim signature As Long
    src(0) = 37
    src(1) = 41
    src(2) = 43
    length = 3
    ReDim dst(length - 1)
    RtlMoveMemory VarPtr(dst(0)), VarPtr(src(0)), length
    x0 = dst(0)
    x1 = dst(1)
    x2 = dst(2)
    For i = LBound(dst) To UBound(dst)
        sum = sum + dst(i)
    Next
    signature = x0 + (x1 * 1000) + (x2 * 1000000)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 121)),
                "runtime-sized byte-array indexed loop sum should be 121 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 43_041_037)),
                "runtime-sized byte-array constant index signature should be 43041037 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn dynamic_byte_array_function_return_assignment_preserves_byte_values_in_vm_and_jit() {
        let source = r#"
Private Function MakeBuf() As Byte()
    Dim buf() As Byte
    ReDim buf(2)
    buf(0) = 90
    buf(1) = 91
    buf(2) = 92
    MakeBuf = buf
End Function

Sub Main()
    Dim result() As Byte
    Dim x0 As Long
    Dim x1 As Long
    Dim x2 As Long
    result = MakeBuf()
    x0 = result(0)
    x1 = result(1)
    x2 = result(2)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 90)),
                "returned dynamic-array index 0 should be 90 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 91)),
                "returned dynamic-array index 1 should be 91 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            assert!(
                snapshot
                    .iter()
                    .any(|value| matches!(value, RuntimeValue::I32(raw) if *raw == 92)),
                "returned dynamic-array index 2 should be 92 for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn varptr_string_variable_exposes_bstr_container_cell_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim textValue As String
    Dim pointerValue As LongPtr
    textValue = "alpha"
    pointerValue = VarPtr(textValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let pointer = snapshot
                .iter()
                .find_map(|value| match value {
                    RuntimeValue::I64(value) if *value != 0 => Some(*value),
                    _ => None,
                })
                .expect("snapshot should contain a non-zero pointer-like value");
            let raw = oxvba_runtime::pointer_helpers::lookup_pointer(pointer)
                .expect("pointer helper registry should contain VarPtr(String) result")
                .cast::<usize>();
            assert!(!raw.is_null());
            let payload = unsafe { *raw as *mut u16 };
            assert!(!payload.is_null());
            assert_eq!(
                unsafe { SysStringLen(payload.cast()) },
                5,
                "VarPtr(String) should expose a BSTR cell for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn varptr_variant_variable_exposes_variant_container_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim value As Variant
    Dim pointerValue As LongPtr
    value = "alpha"
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let pointer = snapshot
                .iter()
                .find_map(|value| match value {
                    RuntimeValue::I64(value) if *value != 0 => Some(*value),
                    _ => None,
                })
                .expect("snapshot should contain a non-zero pointer-like value");
            assert_ne!(
                pointer, 0,
                "VarPtr(Variant) should return a non-zero actual slot pointer for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn varptr_variant_scalar_variable_exposes_scalar_variant_container_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim value As Variant
    Dim pointerValue As LongPtr
    value = 42
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            match snapshot.last() {
                Some(RuntimeValue::I64(value)) if *value != 0 => *value,
                other => panic!(
                    "snapshot should end with the non-zero VarPtr(Variant) result for enable_jit={enable_jit}, got {other:?}; snapshot={snapshot:?}"
                ),
            };
        }
    }

    #[test]
    fn varptr_variant_decimal_variable_exposes_decimal_variant_container_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim obj As Object
    Dim value As Variant
    Dim pointerValue As LongPtr
    Set obj = CreateObject("OxVba.TestDispatch")
    value = DispatchInvoke(obj, "ReturnDecimal")
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.contains(&RuntimeValue::Decimal(Decimal96::from_parts(
                    123_450, 0, 0, 3, true
                ))),
                "snapshot should preserve the Decimal Variant payload for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            match snapshot.last() {
                Some(RuntimeValue::I64(value)) if *value != 0 => *value,
                other => panic!(
                    "snapshot should end with the non-zero VarPtr(Variant) result for enable_jit={enable_jit}, got {other:?}; snapshot={snapshot:?}"
                ),
            };
        }
    }

    #[test]
    fn varptr_variant_wide_i64_variable_exposes_vt_i8_container_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim obj As Object
    Dim value As Variant
    Dim pointerValue As LongPtr
    Set obj = CreateObject("OxVba.TestDispatch")
    value = DispatchInvoke(obj, "ReturnWideHyper")
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.contains(&RuntimeValue::I64(5_000_000_000)),
                "snapshot should preserve the wide I64 Variant payload for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
            match snapshot.last() {
                Some(RuntimeValue::I64(value)) if *value != 0 => *value,
                other => panic!(
                    "snapshot should end with the non-zero VarPtr(Variant) result for enable_jit={enable_jit}, got {other:?}; snapshot={snapshot:?}"
                ),
            };
        }
    }

    #[test]
    fn varptr_variant_object_container_exposes_vt_unknown_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim value As Variant
    Dim pointerValue As LongPtr
    Set value = CreateObject("OxVba.TestDispatch")
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            match snapshot.last() {
                Some(RuntimeValue::I64(value)) if *value != 0 => *value,
                other => panic!(
                    "snapshot should end with the non-zero VarPtr(Variant) result for enable_jit={enable_jit}, got {other:?}; snapshot={snapshot:?}"
                ),
            };
        }
    }

    #[test]
    fn varptr_variant_array_container_exposes_variant_safearray_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim value As Variant
    Dim pointerValue As LongPtr
    value = Array(1, 2, 3)
    pointerValue = VarPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            match snapshot.last() {
                Some(RuntimeValue::I64(value)) if *value != 0 => *value,
                other => panic!(
                    "snapshot should end with the non-zero VarPtr(Variant) result for enable_jit={enable_jit}, got {other:?}; snapshot={snapshot:?}"
                ),
            };
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
            assert!(
                first != 0,
                "ObjPtr should be non-zero for a live object for enable_jit={enable_jit}"
            );
            assert_eq!(
                first, second,
                "ObjPtr should be stable for the same live object identity for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn objptr_accepts_object_valued_variant_in_vm_and_jit() {
        let source = r#"
Sub Main()
    Dim obj As Object
    Dim value As Variant
    Dim firstPtr As LongPtr
    Dim secondPtr As LongPtr
    Set obj = CreateObject("OxVba.TestDispatch")
    Set value = obj
    firstPtr = ObjPtr(obj)
    secondPtr = ObjPtr(value)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert_eq!(snapshot.len(), 4);
            let first = expect_i64(&snapshot[2]);
            let second = expect_i64(&snapshot[3]);
            assert!(first != 0);
            assert_eq!(
                first, second,
                "ObjPtr should accept an object-valued Variant for enable_jit={enable_jit}"
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
