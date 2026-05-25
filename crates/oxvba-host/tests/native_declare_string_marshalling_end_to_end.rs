#[cfg(target_os = "windows")]
mod windows_native_declare_string_e2e {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::Variant;

    fn run_windows_host_backed(source: &str, enable_jit: bool) -> Vec<Variant> {
        let mut engine = Engine::new(HostConfig { enable_jit });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_variant_snapshot(source)
            .expect("native declare probe should execute")
    }

    fn non_zero_i64(value: &Variant) -> Option<i64> {
        value.as_i64().filter(|value| *value != 0)
    }

    fn contains_string(snapshot: &[Variant], expected: &str) -> bool {
        snapshot.iter().any(|value| {
            value
                .as_bstr()
                .is_some_and(|text| text.as_str() == expected)
        })
    }

    #[test]
    fn loadlibrarya_byval_string_marshals_ansi_path_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function LoadLibrary Lib "kernel32" Alias "LoadLibraryA" (ByVal lpLibFileName As String) As LongPtr
Private Declare PtrSafe Function FreeLibrary Lib "kernel32" (ByVal hLibModule As LongPtr) As Long

Sub Main()
    Dim moduleHandle As LongPtr
    moduleHandle = LoadLibrary("kernel32.dll")
    If moduleHandle <> 0 Then
        Call FreeLibrary(moduleHandle)
    End If
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let handle = snapshot
                .iter()
                .find_map(non_zero_i64)
                .expect("snapshot should contain a non-zero module handle");
            assert_ne!(
                handle, 0,
                "LoadLibraryA should succeed for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn getmodulehandleexw_byref_longptr_writeback_marshals_native_out_pointer_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function GetModuleHandleExW Lib "kernel32" (ByVal dwFlags As Long, ByVal lpModuleName As LongPtr, ByRef phModule As LongPtr) As Long
Private Declare PtrSafe Function FreeLibrary Lib "kernel32" (ByVal hLibModule As LongPtr) As Long

Sub Main()
    Dim moduleHandle As LongPtr
    Dim ok As Long

    ok = GetModuleHandleExW(0, StrPtr("kernel32.dll"), moduleHandle)
    If ok <> 0 And moduleHandle <> 0 Then
        Call FreeLibrary(moduleHandle)
    End If
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            let handle = snapshot
                .iter()
                .find_map(non_zero_i64)
                .expect("snapshot should contain a non-zero module handle");
            assert_ne!(
                handle, 0,
                "GetModuleHandleExW should populate ByRef LongPtr output for enable_jit={enable_jit}"
            );
        }
    }

    #[test]
    fn multibytetowidechar_strptr_target_writes_back_string_slot_in_vm_and_jit() {
        let source = r#"
Private Const CP_UTF8 As Long = 65001
Private Declare PtrSafe Function MultiByteToWideChar Lib "kernel32" (ByVal CodePage As Long, ByVal dwFlags As Long, ByVal lpMultiByteStr As LongPtr, ByVal cbMultiByte As Long, ByVal lpWideCharStr As LongPtr, ByVal cchWideChar As Long) As Long

Sub Main()
    Dim utf8(0 To 5) As Byte
    Dim resultText As String
    Dim written As Long

    utf8(0) = 97
    utf8(1) = 108
    utf8(2) = 112
    utf8(3) = 104
    utf8(4) = 97
    utf8(5) = 0
    resultText = String(5, "*")
    written = MultiByteToWideChar(CP_UTF8, 0, VarPtr(utf8(0)), -1, StrPtr(resultText), 6)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                contains_string(&snapshot, "alpha"),
                "MultiByteToWideChar should write back through StrPtr target for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn strptr_target_writeback_is_driven_by_expression_shape_not_declared_name() {
        let source = r#"
Private Const CP_UTF8 As Long = 65001
Private Declare PtrSafe Function OxDecodeUtf8Buffer Lib "kernel32" Alias "MultiByteToWideChar" (ByVal CodePage As Long, ByVal dwFlags As Long, ByVal lpMultiByteStr As LongPtr, ByVal cbMultiByte As Long, ByVal lpWideCharStr As LongPtr, ByVal cchWideChar As Long) As Long

Sub Main()
    Dim utf8(0 To 5) As Byte
    Dim resultText As String
    Dim written As Long

    utf8(0) = 97
    utf8(1) = 108
    utf8(2) = 112
    utf8(3) = 104
    utf8(4) = 97
    utf8(5) = 0
    resultText = String(5, "*")
    written = OxDecodeUtf8Buffer(CP_UTF8, 0, VarPtr(utf8(0)), -1, StrPtr(resultText), 6)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                contains_string(&snapshot, "alpha"),
                "StrPtr writeback should not depend on the declared API name for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn sysreallocstring_varptr_string_target_writes_back_string_slot_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function SysReAllocString Lib "oleaut32" (ByVal pbstr As LongPtr, ByVal psz As LongPtr) As Long

Sub Main()
    Dim textValue As String
    Dim status As Long

    textValue = "*****"
    status = SysReAllocString(VarPtr(textValue), StrPtr("alpha"))
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                contains_string(&snapshot, "alpha"),
                "SysReAllocString should write back through VarPtr(String) for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn widechartomultibyte_varptr_buffer_target_writes_back_array_slot_in_vm_and_jit() {
        let source = r#"
Private Const CP_UTF8 As Long = 65001
Private Declare PtrSafe Function WideCharToMultiByte Lib "kernel32" (ByVal CodePage As Long, ByVal dwFlags As Long, ByVal lpWideCharStr As LongPtr, ByVal cchWideChar As Long, ByVal lpMultiByteStr As LongPtr, ByVal cbMultiByte As Long, ByVal lpDefaultChar As LongPtr, ByVal lpUsedDefaultChar As LongPtr) As Long
Private Declare PtrSafe Function lstrlenA Lib "kernel32" (ByVal lpString As LongPtr) As Long

Sub Main()
    Dim textValue As String
    Dim buf() As Byte
    Dim size As Long
    Dim actualLength As Long

    textValue = "@A"
    size = WideCharToMultiByte(CP_UTF8, 0, StrPtr(textValue), -1, 0, 0, 0, 0)
    ReDim buf(size)
    Call WideCharToMultiByte(CP_UTF8, 0, StrPtr(textValue), -1, VarPtr(buf(0)), size, 0, 0)
    actualLength = lstrlenA(VarPtr(buf(0)))
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.iter().any(|value| value.as_i32() == Some(2)),
                "WideCharToMultiByte should write a 2-byte C string into the array slot for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn varptr_buffer_writeback_is_driven_by_expression_shape_not_declared_name() {
        let source = r#"
Private Const CP_UTF8 As Long = 65001
Private Declare PtrSafe Function OxEncodeWideToUtf8 Lib "kernel32" Alias "WideCharToMultiByte" (ByVal CodePage As Long, ByVal dwFlags As Long, ByVal lpWideCharStr As LongPtr, ByVal cchWideChar As Long, ByVal lpMultiByteStr As LongPtr, ByVal cbMultiByte As Long, ByVal lpDefaultChar As LongPtr, ByVal lpUsedDefaultChar As LongPtr) As Long
Private Declare PtrSafe Function lstrlenA Lib "kernel32" (ByVal lpString As LongPtr) As Long

Sub Main()
    Dim textValue As String
    Dim buf() As Byte
    Dim size As Long
    Dim actualLength As Long

    textValue = "@A"
    size = OxEncodeWideToUtf8(CP_UTF8, 0, StrPtr(textValue), -1, 0, 0, 0, 0)
    ReDim buf(size)
    Call OxEncodeWideToUtf8(CP_UTF8, 0, StrPtr(textValue), -1, VarPtr(buf(0)), size, 0, 0)
    actualLength = lstrlenA(VarPtr(buf(0)))
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.iter().any(|value| value.as_i32() == Some(2)),
                "VarPtr buffer writeback should not depend on the declared API name for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn msvcrt_sqrt_round_trips_double_value_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function sqrt Lib "msvcrt" (ByVal x As Double) As Double

Sub Main()
    Dim resultValue As Double
    resultValue = sqrt(156.25)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.iter().any(|value| value.as_f64() == Some(12.5)),
                "sqrt should return 12.5 through the native Double lane for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn oleaut32_varcyfromi4_byref_currency_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function VarCyFromI4 Lib "oleaut32" (ByVal inputValue As Long, ByRef outValue As Currency) As Long

Sub Main()
    Dim status As Long
    Dim outValue As Currency

    status = VarCyFromI4(123, outValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| value.as_currency_scaled_i64() == Some(1_230_000)),
                "VarCyFromI4 should populate ByRef Currency output for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn oleaut32_varboolfromi4_byref_boolean_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function VarBoolFromI4 Lib "oleaut32" (ByVal inputValue As Long, ByRef outValue As Boolean) As Long

Sub Main()
    Dim outValue As Boolean
    Dim status As Long
    status = VarBoolFromI4(123, outValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.contains(&Variant::from_bool(true)),
                "VarBoolFromI4 should populate ByRef Boolean output for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn oleaut32_varr8fromi4_byref_double_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function VarR8FromI4 Lib "oleaut32" (ByVal inputValue As Long, ByRef outValue As Double) As Long

Sub Main()
    Dim outValue As Double
    Dim status As Long
    status = VarR8FromI4(123, outValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.iter().any(|value| value.as_f64() == Some(123.0)),
                "VarR8FromI4 should populate ByRef Double output for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn oleaut32_varr4fromi4_byref_single_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function VarR4FromI4 Lib "oleaut32" (ByVal inputValue As Long, ByRef outValue As Single) As Long

Sub Main()
    Dim outValue As Single
    Dim status As Long
    status = VarR4FromI4(123, outValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.iter().any(|value| value.as_f32() == Some(123.0)),
                "VarR4FromI4 should populate ByRef Single output for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn oleaut32_vari2fromi4_byref_integer_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function VarI2FromI4 Lib "oleaut32" (ByVal inputValue As Long, ByRef outValue As Integer) As Long

Sub Main()
    Dim outValue As Integer
    Dim status As Long
    status = VarI2FromI4(123, outValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot.contains(&Variant::from_i16(123)),
                "VarI2FromI4 should populate ByRef Integer output for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn getdiskfreespaceexw_byref_longlong_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function GetDiskFreeSpaceExW Lib "kernel32" (ByVal lpDirectoryName As LongPtr, ByRef lpFreeBytesAvailableToCaller As LongLong, ByRef lpTotalNumberOfBytes As LongLong, ByRef lpTotalNumberOfFreeBytes As LongLong) As Long

Sub Main()
    Dim freeBytes As LongLong
    Dim totalBytes As LongLong
    Dim totalFreeBytes As LongLong
    Dim status As Long
    status = GetDiskFreeSpaceExW(StrPtr("C:\"), freeBytes, totalBytes, totalFreeBytes)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| value.as_i64().is_some_and(|raw| raw > 0)),
                "GetDiskFreeSpaceExW should populate ByRef LongLong outputs for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }

    #[test]
    fn oleaut32_vardatefromstr_byref_date_writeback_round_trips_in_vm_and_jit() {
        let source = r#"
Private Declare PtrSafe Function VarDateFromStr Lib "oleaut32" (ByVal inputText As LongPtr, ByVal lcid As Long, ByVal flags As Long, ByRef outValue As Date) As Long

Sub Main()
    Dim status As Long
    Dim outValue As Date

    status = VarDateFromStr(StrPtr("January 1, 2000"), 1033, 0, outValue)
End Sub
"#;

        for enable_jit in [false, true] {
            let snapshot = run_windows_host_backed(source, enable_jit);
            assert!(
                snapshot
                    .iter()
                    .any(|value| value.as_date_f64() == Some(36526.0)),
                "VarDateFromStr should populate ByRef Date output for enable_jit={enable_jit}; snapshot={snapshot:?}"
            );
        }
    }
}
