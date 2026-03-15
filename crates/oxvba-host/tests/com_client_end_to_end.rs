#[cfg(target_os = "windows")]
mod windows_com_e2e {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::{
        CurrencyValue, Decimal96, F64Value, ObjectHandle, RuntimeValue, bstr::BStr,
        safe_array::SafeArray,
    };

    fn run_windows_host_backed(source: &str, enable_jit: bool) -> Vec<RuntimeValue> {
        let mut engine = Engine::new(HostConfig {
            enable_jit,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_snapshot_phased(source)
            .expect("windows host-backed COM lane should execute")
    }

    fn run_windows_host_backed_error(source: &str, enable_jit: bool) -> String {
        let mut engine = Engine::new(HostConfig {
            enable_jit,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_snapshot_phased(source)
            .expect_err("windows host-backed COM lane should fail deterministically")
            .message()
            .to_string()
    }

    fn expect_object_handle(value: &RuntimeValue) -> ObjectHandle {
        match value {
            RuntimeValue::ObjectHandle(handle) => *handle,
            other => panic!("expected object handle, got {:?}", other),
        }
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

        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(
            out[1],
            RuntimeValue::I32(7),
            "controlled COM Count contract mismatch"
        );
        assert_eq!(
            out[2],
            RuntimeValue::Bool(true),
            "controlled COM Exists(42) contract mismatch"
        );
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
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(12),
            "SetValue should store direct value"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::I32(12),
            "Value getter should reflect SetValue result"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::I32(100_012),
            "SetValueRef should take deterministic putref route"
        );
        assert_eq!(
            vm[4],
            RuntimeValue::I32(100_012),
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
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(3_014),
            "SumPair should preserve both arguments"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::I32(205_009),
            "LookupPair should preserve both arguments"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::I32(307_011),
            "SetIndexedValue should route through multi-arg property put"
        );
        assert_eq!(
            vm[4],
            RuntimeValue::I32(307_011),
            "Value getter should reflect multi-arg property put result"
        );
        assert_eq!(
            vm[5],
            RuntimeValue::I32(408_013),
            "SetIndexedValueRef should route through multi-arg property putref"
        );
        assert_eq!(
            vm[6],
            RuntimeValue::I32(408_013),
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
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(307_011),
            "SetIndexedValue should accept fully named indexed property put arguments"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::I32(307_011),
            "Value getter should reflect named indexed property put result"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::I32(408_013),
            "SetIndexedValueRef should accept fully named indexed property putref arguments"
        );
        assert_eq!(
            vm[4],
            RuntimeValue::I32(408_013),
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
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::ErrorCode(17),
            "CVErr(17) should roundtrip as an error tag"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::Null,
            "Null should roundtrip as the stable null tag"
        );
    }

    #[test]
    fn natural_named_default_member_routes_when_identity_is_known() {
        let source = r#"
Sub Main()
Dim obj
Dim value
obj = CreateObject("OxVba.TestDispatch")
value = obj(value := 19)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on natural named default-member path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(19),
            "natural named default-member invoke should route to EchoVariant"
        );
    }

    #[test]
    fn dispatchinvoke_named_default_member_routes_when_identity_is_known() {
        let source = r#"
Sub Main()
Dim obj
Dim value
obj = CreateObject("OxVba.TestDispatch")
value = DispatchInvoke(obj, 0, value := 19)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on named default-member path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(19),
            "named default-member invoke should route to EchoVariant"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_integer_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim smallValue
Dim unsignedValue
Dim byteValue
Dim signedByteValue
Dim platformIntValue
Dim platformUIntValue
Dim hyperValue
Dim unsignedHyperValue
Dim longValue
Dim unsignedLongValue
obj = CreateObject("OxVba.TestDispatch")
smallValue = DispatchInvoke(obj, "ReturnSmallInt")
unsignedValue = DispatchInvoke(obj, "ReturnUnsignedWord")
byteValue = DispatchInvoke(obj, "ReturnByte")
signedByteValue = DispatchInvoke(obj, "ReturnSignedByte")
platformIntValue = DispatchInvoke(obj, "ReturnPlatformInt")
platformUIntValue = DispatchInvoke(obj, "ReturnPlatformUInt")
hyperValue = DispatchInvoke(obj, "ReturnHyper")
unsignedHyperValue = DispatchInvoke(obj, "ReturnUnsignedHyper")
longValue = DispatchInvoke(obj, "ReturnLong")
unsignedLongValue = DispatchInvoke(obj, "ReturnUnsignedLong")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on integer VARIANT result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(321),
            "VT_I2 result should coerce into the i32 token lane"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::I32(65_000),
            "VT_UI2 result should coerce into the i32 token lane"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::I32(255),
            "VT_UI1 result should coerce into the i32 token lane"
        );
        assert_eq!(
            vm[4],
            RuntimeValue::I32(-5),
            "VT_I1 result should coerce into the i32 token lane"
        );
        assert_eq!(
            vm[5],
            RuntimeValue::I32(-70_000),
            "VT_INT result should preserve the current i32 carrier lane"
        );
        assert_eq!(
            vm[6],
            RuntimeValue::I32(70_000),
            "VT_UINT result should preserve the current i32 carrier lane when the value fits"
        );
        assert_eq!(
            vm[7],
            RuntimeValue::I32(-70_000),
            "VT_I8 result should preserve the current i32 carrier lane when the value fits"
        );
        assert_eq!(
            vm[8],
            RuntimeValue::I32(70_000),
            "VT_UI8 result should preserve the current i32 carrier lane when the value fits"
        );
        assert_eq!(
            vm[9],
            RuntimeValue::I32(70_000),
            "VT_I4 result should preserve the current i32 carrier lane"
        );
        assert_eq!(
            vm[10],
            RuntimeValue::I32(70_000),
            "VT_UI4 result should preserve the current i32 carrier lane when the value fits"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_typed_safe_array_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim smallArray
Dim byteArray
Dim signedByteArray
Dim platformIntArray
Dim platformUIntArray
Dim hyperArray
Dim unsignedHyperArray
Dim longArray
Dim unsignedLongArray
Dim boolArray
Dim stringArray
obj = CreateObject("OxVba.TestDispatch")
smallArray = DispatchInvoke(obj, "ReturnSmallIntArray")
byteArray = DispatchInvoke(obj, "ReturnByteArray")
signedByteArray = DispatchInvoke(obj, "ReturnSignedByteArray")
platformIntArray = DispatchInvoke(obj, "ReturnPlatformIntArray")
platformUIntArray = DispatchInvoke(obj, "ReturnPlatformUIntArray")
hyperArray = DispatchInvoke(obj, "ReturnHyperArray")
unsignedHyperArray = DispatchInvoke(obj, "ReturnUnsignedHyperArray")
longArray = DispatchInvoke(obj, "ReturnLongArray")
unsignedLongArray = DispatchInvoke(obj, "ReturnUnsignedLongArray")
boolArray = DispatchInvoke(obj, "ReturnBoolArray")
stringArray = DispatchInvoke(obj, "ReturnStringArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on typed integer/bool/string SAFEARRAY path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(12),
                RuntimeValue::I32(-4),
                RuntimeValue::I32(321),
            ])),
            "VT_ARRAY|VT_I2 result should preserve scalar array elements"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(0),
                RuntimeValue::I32(12),
                RuntimeValue::I32(255),
            ])),
            "VT_ARRAY|VT_UI1 result should preserve byte array elements on the current i32 carrier lane"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(-5),
                RuntimeValue::I32(0),
                RuntimeValue::I32(120),
            ])),
            "VT_ARRAY|VT_I1 result should preserve signed byte array elements on the current i32 carrier lane"
        );
        assert_eq!(
            vm[4],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(-70_000),
                RuntimeValue::I32(0),
                RuntimeValue::I32(12),
            ])),
            "VT_ARRAY|VT_INT result should preserve platform int array elements"
        );
        assert_eq!(
            vm[5],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(12),
                RuntimeValue::I32(4_096),
                RuntimeValue::I32(70_000),
            ])),
            "VT_ARRAY|VT_UINT result should preserve platform uint array elements within the current i32 carrier lane"
        );
        assert_eq!(
            vm[6],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(-70_000),
                RuntimeValue::I32(0),
                RuntimeValue::I32(12),
            ])),
            "VT_ARRAY|VT_I8 result should preserve hyper array elements within the current i32 carrier lane"
        );
        assert_eq!(
            vm[7],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(12),
                RuntimeValue::I32(4_096),
                RuntimeValue::I32(70_000),
            ])),
            "VT_ARRAY|VT_UI8 result should preserve unsigned hyper array elements within the current i32 carrier lane"
        );
        assert_eq!(
            vm[8],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(12),
                RuntimeValue::I32(-4),
                RuntimeValue::I32(70_000),
            ])),
            "VT_ARRAY|VT_I4 result should preserve 32-bit signed array elements"
        );
        assert_eq!(
            vm[9],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(12),
                RuntimeValue::I32(4_096),
                RuntimeValue::I32(70_000),
            ])),
            "VT_ARRAY|VT_UI4 result should preserve 32-bit unsigned array elements within the current i32 carrier lane"
        );
        assert_eq!(
            vm[10],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(false),
                RuntimeValue::Bool(true),
            ])),
            "VT_ARRAY|VT_BOOL result should preserve boolean array elements"
        );
        assert_eq!(
            vm[11],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::String(BStr("Alpha".to_string())),
                RuntimeValue::String(BStr("Beta".to_string())),
            ])),
            "VT_ARRAY|VT_BSTR result should preserve string array elements"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_bool_and_string_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim boolValue
Dim stringValue
obj = CreateObject("OxVba.TestDispatch")
boolValue = DispatchInvoke(obj, "ReturnBool")
stringValue = DispatchInvoke(obj, "ReturnString")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on bool/string VARIANT result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::Bool(true),
            "VT_BOOL result should preserve the semantic bool carrier"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::String(BStr("Scalar BSTR".to_string())),
            "VT_BSTR result should preserve the semantic string carrier"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_empty_null_and_error_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim emptyValue
Dim nullValue
Dim errorValue
obj = CreateObject("OxVba.TestDispatch")
emptyValue = DispatchInvoke(obj, "ReturnEmpty")
nullValue = DispatchInvoke(obj, "ReturnNull")
errorValue = DispatchInvoke(obj, "ReturnError")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on empty/null/error VARIANT result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::Empty,
            "VT_EMPTY result should preserve the semantic empty carrier"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::Null,
            "VT_NULL result should preserve the semantic null carrier"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::ErrorCode(17),
            "VT_ERROR result should preserve the semantic error-code carrier"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_float_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim doubleValue
Dim singleValue
Dim dateValue
obj = CreateObject("OxVba.TestDispatch")
doubleValue = DispatchInvoke(obj, "ReturnDouble")
singleValue = DispatchInvoke(obj, "ReturnSingle")
dateValue = DispatchInvoke(obj, "ReturnDate")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on float VARIANT result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::F64(F64Value::from_f64(12.5)),
            "VT_R8 result should preserve the semantic f64 carrier"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::F64(F64Value::from_single_f64(12.5)),
            "VT_R4 result should preserve the semantic f64 carrier with Single subtype"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::F64(F64Value::from_date_f64(45200.25)),
            "VT_DATE result should preserve the automation date payload with Date subtype"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_typed_float_safe_array_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim doubleArray
Dim singleArray
Dim dateArray
obj = CreateObject("OxVba.TestDispatch")
doubleArray = DispatchInvoke(obj, "ReturnDoubleArray")
singleArray = DispatchInvoke(obj, "ReturnSingleArray")
dateArray = DispatchInvoke(obj, "ReturnDateArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on typed float SAFEARRAY path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::F64(F64Value::from_f64(12.5)),
                RuntimeValue::F64(F64Value::from_f64(-4.25)),
                RuntimeValue::F64(F64Value::from_f64(321.0)),
            ])),
            "VT_ARRAY|VT_R8 result should preserve float array elements on the semantic f64 carrier"
        );
        assert_eq!(
            vm[2],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::F64(F64Value::from_single_f64(12.5)),
                RuntimeValue::F64(F64Value::from_single_f64(-4.25)),
                RuntimeValue::F64(F64Value::from_single_f64(321.0)),
            ])),
            "VT_ARRAY|VT_R4 result should preserve float array elements with Single subtype"
        );
        assert_eq!(
            vm[3],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::F64(F64Value::from_date_f64(45200.25)),
                RuntimeValue::F64(F64Value::from_date_f64(12.5)),
                RuntimeValue::F64(F64Value::from_date_f64(-4.25)),
            ])),
            "VT_ARRAY|VT_DATE result should preserve automation date payloads with Date subtype"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_currency_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim currencyValue
obj = CreateObject("OxVba.TestDispatch")
currencyValue = DispatchInvoke(obj, "ReturnCurrency")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on currency VARIANT result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(125_000)),
            "VT_CY result should preserve the exact scaled currency carrier"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_typed_currency_safe_array_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim currencyArray
obj = CreateObject("OxVba.TestDispatch")
currencyArray = DispatchInvoke(obj, "ReturnCurrencyArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on typed currency SAFEARRAY path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::Currency(CurrencyValue::from_scaled_i64(125_000)),
                RuntimeValue::Currency(CurrencyValue::from_scaled_i64(-42_500)),
                RuntimeValue::Currency(CurrencyValue::from_scaled_i64(3_210_000)),
            ])),
            "VT_ARRAY|VT_CY result should preserve exact scaled currency elements"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_decimal_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim decimalValue
obj = CreateObject("OxVba.TestDispatch")
decimalValue = DispatchInvoke(obj, "ReturnDecimal")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on decimal VARIANT result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, true)),
            "VT_DECIMAL result should preserve the exact decimal carrier"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_typed_decimal_safe_array_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim decimalArray
obj = CreateObject("OxVba.TestDispatch")
decimalArray = DispatchInvoke(obj, "ReturnDecimalArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on typed decimal SAFEARRAY path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, false)),
                RuntimeValue::Decimal(Decimal96::from_parts(42_500, 0, 0, 4, true)),
                RuntimeValue::Decimal(Decimal96::from_parts(3_210_000, 0, 0, 4, false)),
            ])),
            "VT_ARRAY|VT_DECIMAL result should preserve exact decimal elements"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_object_variant_results() {
        let source = r#"
Sub Main()
Dim obj
Dim returnedDispatch
Dim returnedUnknown
Dim dispatchCount
Dim unknownCount
obj = CreateObject("OxVba.TestDispatch")
returnedDispatch = DispatchInvoke(obj, "ReturnSelfDispatch")
returnedUnknown = DispatchInvoke(obj, "ReturnSelfUnknown")
dispatchCount = DispatchInvoke(returnedDispatch, "Count")
unknownCount = DispatchInvoke(returnedUnknown, "Count")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on object-result COM path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert!(expect_object_handle(&vm[1]).raw() >= 20_001);
        assert!(expect_object_handle(&vm[2]).raw() >= 20_001);
        assert_eq!(
            vm[3],
            RuntimeValue::I32(7),
            "VT_DISPATCH result should rebind into an invokable object handle"
        );
        assert_eq!(
            vm[4],
            RuntimeValue::I32(7),
            "VT_UNKNOWN result exposing IDispatch should rebind into an invokable object handle"
        );
    }

    #[test]
    fn dispatchinvoke_classifies_scalar_variant_arguments_at_com_boundary() {
        let source = r#"
Sub Main()
Dim obj
Dim boolSeed
Dim stringSeed
Dim emptySeed
Dim nullSeed
Dim errorSeed
Dim boolVt
Dim stringVt
Dim emptyVt
Dim nullVt
Dim errorVt
obj = CreateObject("OxVba.TestDispatch")
boolSeed = DispatchInvoke(obj, "ReturnBool")
stringSeed = DispatchInvoke(obj, "ReturnString")
emptySeed = DispatchInvoke(obj, "ReturnEmpty")
nullSeed = DispatchInvoke(obj, "ReturnNull")
errorSeed = DispatchInvoke(obj, "ReturnError")
boolVt = DispatchInvoke(obj, "ClassifyVariantArg", boolSeed)
stringVt = DispatchInvoke(obj, "ClassifyVariantArg", stringSeed)
emptyVt = DispatchInvoke(obj, "ClassifyVariantArg", emptySeed)
nullVt = DispatchInvoke(obj, "ClassifyVariantArg", nullSeed)
errorVt = DispatchInvoke(obj, "ClassifyVariantArg", errorSeed)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on scalar-argument classifier path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[6],
            RuntimeValue::I32(11),
            "True should marshal as VT_BOOL"
        );
        assert_eq!(
            vm[7],
            RuntimeValue::I32(8),
            "string argument should marshal as VT_BSTR"
        );
        assert_eq!(
            vm[8],
            RuntimeValue::I32(0),
            "uninitialized Variant should marshal as VT_EMPTY"
        );
        assert_eq!(
            vm[9],
            RuntimeValue::I32(1),
            "Null should marshal as VT_NULL"
        );
        assert_eq!(
            vm[10],
            RuntimeValue::I32(10),
            "CVErr(...) should marshal as VT_ERROR"
        );
    }

    #[test]
    fn dispatchinvoke_classifies_float_currency_and_decimal_arguments_at_com_boundary() {
        let source = r#"
Sub Main()
Dim obj
Dim doubleSeed
Dim singleSeed
Dim dateSeed
Dim currencySeed
Dim decimalSeed
Dim doubleVt
Dim singleVt
Dim dateVt
Dim currencyVt
Dim decimalVt
obj = CreateObject("OxVba.TestDispatch")
doubleSeed = DispatchInvoke(obj, "ReturnDouble")
singleSeed = DispatchInvoke(obj, "ReturnSingle")
dateSeed = DispatchInvoke(obj, "ReturnDate")
currencySeed = DispatchInvoke(obj, "ReturnCurrency")
decimalSeed = DispatchInvoke(obj, "ReturnDecimal")
doubleVt = DispatchInvoke(obj, "ClassifyVariantArg", doubleSeed)
singleVt = DispatchInvoke(obj, "ClassifyVariantArg", singleSeed)
dateVt = DispatchInvoke(obj, "ClassifyVariantArg", dateSeed)
currencyVt = DispatchInvoke(obj, "ClassifyVariantArg", currencySeed)
decimalVt = DispatchInvoke(obj, "ClassifyVariantArg", decimalSeed)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on float/currency/decimal argument classifier path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[6],
            RuntimeValue::I32(5),
            "Double should marshal as VT_R8"
        );
        assert_eq!(
            vm[7],
            RuntimeValue::I32(4),
            "Single should now preserve VT_R4 on the outward COM boundary"
        );
        assert_eq!(
            vm[8],
            RuntimeValue::I32(7),
            "Date should now preserve VT_DATE on the outward COM boundary"
        );
        assert_eq!(
            vm[9],
            RuntimeValue::I32(6),
            "Currency should preserve VT_CY on the exact currency carrier"
        );
        assert_eq!(
            vm[10],
            RuntimeValue::I32(14),
            "Decimal should preserve VT_DECIMAL on the exact decimal carrier"
        );
    }

    #[test]
    fn dispatchinvoke_classifies_object_arguments_at_com_boundary() {
        let source = r#"
Sub Main()
Dim obj
Dim objectVt
obj = CreateObject("OxVba.TestDispatch")
objectVt = DispatchInvoke(obj, "ClassifyVariantArg", obj)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on object-argument classifier path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(9),
            "object argument should marshal as VT_DISPATCH"
        );
    }

    #[test]
    fn paramarray_forwarding_preserves_semantic_array_payloads_at_com_boundary() {
        let source = r#"
Sub Main()
Dim vt
Call Capture(vt, 5, 7, 9)
End Sub

Sub Capture(ByRef target, ParamArray items() As Variant)
    target = DispatchInvoke(CreateObject("OxVba.TestDispatch"), "ClassifyVariantArg", items)
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on ParamArray COM forwarding path: vm={vm:?} jit={jit:?}"
        );
        assert_eq!(
            vm[0],
            RuntimeValue::I32(8204),
            "ParamArray forwarding should marshal as VT_ARRAY | VT_VARIANT"
        );
    }

    #[test]
    fn dispatchinvoke_classifies_array_arguments_at_com_boundary() {
        let source = r#"
Sub Main()
Dim obj
Dim arrayVt
obj = CreateObject("OxVba.TestDispatch")
arrayVt = DispatchInvoke(obj, "ClassifyVariantArg", Array(1, 2, 3))
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on array-argument classifier path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(8204),
            "array argument should marshal as VT_ARRAY | VT_VARIANT"
        );
    }

    #[test]
    fn dispatchinvoke_classifies_object_elements_inside_variant_arrays_at_com_boundary() {
        let source = r#"
Sub Main()
Dim obj
Dim nestedVt
obj = CreateObject("OxVba.TestDispatch")
nestedVt = DispatchInvoke(obj, "ClassifyVariantArrayFirstElementArg", Array(obj))
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on object-array classifier path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        assert_eq!(
            vm[1],
            RuntimeValue::I32(9),
            "object element inside VT_ARRAY | VT_VARIANT should marshal as VT_DISPATCH"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_dispatch_arrays_with_nested_object_elements() {
        let source = r#"
Sub Main()
Dim obj
Dim returnedArray
obj = CreateObject("OxVba.TestDispatch")
returnedArray = DispatchInvoke(obj, "ReturnSelfDispatchArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on dispatch-array result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        let RuntimeValue::ArrayIntent(array) = &vm[1] else {
            panic!("expected SAFEARRAY result, got {:?}", vm[1]);
        };
        let elements = array
            .elements
            .as_ref()
            .expect("dispatch-array result should preserve owned elements");
        assert_eq!(elements.len(), 1, "dispatch-array result length mismatch");
        let RuntimeValue::ObjectHandle(handle) = elements[0] else {
            panic!(
                "expected first dispatch-array element to be an object handle, got {:?}",
                elements[0]
            );
        };
        assert!(
            handle.raw() >= 20_001,
            "dispatch-array result should preserve nested dispatch object handles"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_typed_dispatch_array_results() {
        let source = r#"
Sub Main()
Dim obj
Dim returnedArray
obj = CreateObject("OxVba.TestDispatch")
returnedArray = DispatchInvoke(obj, "ReturnSelfTypedDispatchArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on typed dispatch-array result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        let RuntimeValue::ArrayIntent(array) = &vm[1] else {
            panic!("expected SAFEARRAY result, got {:?}", vm[1]);
        };
        let elements = array
            .elements
            .as_ref()
            .expect("typed dispatch-array result should preserve owned elements");
        assert_eq!(
            elements.len(),
            1,
            "typed dispatch-array result length mismatch"
        );
        let RuntimeValue::ObjectHandle(handle) = elements[0] else {
            panic!(
                "expected first typed dispatch-array element to be an object handle, got {:?}",
                elements[0]
            );
        };
        assert!(
            handle.raw() >= 20_001,
            "typed dispatch-array result should preserve nested dispatch object handles"
        );
    }

    #[test]
    fn dispatchinvoke_accepts_typed_unknown_array_results() {
        let source = r#"
Sub Main()
Dim obj
Dim returnedArray
obj = CreateObject("OxVba.TestDispatch")
returnedArray = DispatchInvoke(obj, "ReturnSelfTypedUnknownArray")
End Sub
"#;

        let vm = run_windows_host_backed(source, false);
        let jit = run_windows_host_backed(source, true);
        assert_eq!(
            vm, jit,
            "VM/JIT snapshots diverged on typed unknown-array result path: vm={vm:?} jit={jit:?}"
        );
        assert!(expect_object_handle(&vm[0]).raw() >= 20_001);
        let RuntimeValue::ArrayIntent(array) = &vm[1] else {
            panic!("expected SAFEARRAY result, got {:?}", vm[1]);
        };
        let elements = array
            .elements
            .as_ref()
            .expect("typed unknown-array result should preserve owned elements");
        assert_eq!(
            elements.len(),
            1,
            "typed unknown-array result length mismatch"
        );
        let RuntimeValue::ObjectHandle(handle) = elements[0] else {
            panic!(
                "expected first typed unknown-array element to be an object handle, got {:?}",
                elements[0]
            );
        };
        assert!(
            handle.raw() >= 20_001,
            "typed unknown-array result should preserve nested unknown-exposed dispatch object handles"
        );
    }

    #[test]
    fn dispatchinvoke_multidim_smallint_array_results_fail_deterministically() {
        let source = r#"
Sub Main()
Dim obj
Dim returnedMatrix
obj = CreateObject("OxVba.TestDispatch")
returnedMatrix = DispatchInvoke(obj, "ReturnSmallIntMatrix")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("unsupported SAFEARRAY rank 2")
                && jit.contains("unsupported SAFEARRAY rank 2"),
            "expected unsupported-rank diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }

    #[test]
    fn dispatchinvoke_type_mismatch_arg_error_surfaces_deterministically() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "SumPair", Array(1), 42)
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("com-dispatch-arg-error;hresult=0x80020005;arg_err=1;")
                && jit.contains("com-dispatch-arg-error;hresult=0x80020005;arg_err=1;"),
            "expected stable type-mismatch arg_err surface across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("IDispatch::Invoke(method dispid=")
                && vm.contains("failed with HRESULT 0x80020005 (arg_err=1)")
                && jit.contains("IDispatch::Invoke(method dispid=")
                && jit.contains("failed with HRESULT 0x80020005 (arg_err=1)"),
            "expected raw invoke arg_err detail across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }

    #[test]
    fn dispatchinvoke_exception_details_surface_deterministically() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "RaiseException")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
                && jit.contains(
                    "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
                ),
            "expected stable exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("excep_source=\"OxVba.TestDispatch\"")
                && vm.contains("excep_description=\"controlled dispatch exception\"")
                && jit.contains("excep_source=\"OxVba.TestDispatch\"")
                && jit.contains("excep_description=\"controlled dispatch exception\""),
            "expected EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            !vm.contains("arg_err=") && !jit.contains("arg_err="),
            "exception path should not synthesize arg_err, got vm={vm:?} jit={jit:?}"
        );
    }
    #[test]
    fn dispatchinvoke_plain_unknown_results_fail_with_bounded_nondispatch_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnPlainUnknown")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002")
                && jit
                    .contains("IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002"),
            "expected bounded non-IDispatch VT_UNKNOWN diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && vm.contains(
                    "detail=\"IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002\""
                )
                && jit.contains("com-dispatch-fault-unspecified")
                && jit.contains(
                    "detail=\"IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002\""
                ),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }
    #[test]
    fn dispatchinvoke_plain_unknown_arrays_fail_with_bounded_nondispatch_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnPlainUnknownArray")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002")
                && jit
                    .contains("IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002"),
            "expected bounded non-IDispatch VT_UNKNOWN array diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && vm.contains(
                    "detail=\"IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002\""
                )
                && jit.contains("com-dispatch-fault-unspecified")
                && jit.contains(
                    "detail=\"IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002\""
                ),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }
    #[test]
    fn dispatchinvoke_wide_unsigned_long_results_fail_with_bounded_overflow_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnWideUnsignedLong")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("VT_UI4 value 4000000000 exceeds current i32 carrier lane")
                && jit.contains("VT_UI4 value 4000000000 exceeds current i32 carrier lane"),
            "expected bounded VT_UI4 overflow diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && jit.contains("com-dispatch-fault-unspecified"),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }

    #[test]
    fn dispatchinvoke_wide_unsigned_long_arrays_fail_with_bounded_overflow_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnWideUnsignedLongArray")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("VT_UI4 SAFEARRAY element 4000000000 exceeds current i32 carrier lane")
                && jit.contains(
                    "VT_UI4 SAFEARRAY element 4000000000 exceeds current i32 carrier lane"
                ),
            "expected bounded VT_UI4 SAFEARRAY overflow diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && jit.contains("com-dispatch-fault-unspecified"),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }

    #[test]
    fn dispatchinvoke_wide_platform_uint_results_fail_with_bounded_overflow_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnWidePlatformUInt")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("VT_UINT value 4000000000 exceeds current i32 carrier lane")
                && jit.contains("VT_UINT value 4000000000 exceeds current i32 carrier lane"),
            "expected bounded VT_UINT overflow diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && jit.contains("com-dispatch-fault-unspecified"),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
    }

    #[test]
    fn dispatchinvoke_wide_platform_uint_arrays_fail_with_bounded_overflow_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnWidePlatformUIntArray")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("VT_UINT SAFEARRAY element 4000000000 exceeds current i32 carrier lane")
                && jit.contains(
                    "VT_UINT SAFEARRAY element 4000000000 exceeds current i32 carrier lane"
                ),
            "expected bounded VT_UINT SAFEARRAY overflow diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && jit.contains("com-dispatch-fault-unspecified"),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
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

        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(
            out[1],
            RuntimeValue::Bool(true),
            "pre-error call should have succeeded"
        );
        assert!(
            !matches!(out[3], RuntimeValue::I32(0)),
            "missing-argument COM invoke should set Err.Number, got {:?}",
            out
        );
    }

    #[test]
    fn dispatchinvoke_byref_long_results_fail_with_bounded_diagnostic() {
        let source = r#"
Sub Main()
Dim obj
Dim failed
obj = CreateObject("OxVba.TestDispatch")
failed = DispatchInvoke(obj, "ReturnByRefLong")
End Sub
"#;

        let vm = run_windows_host_backed_error(source, false);
        let jit = run_windows_host_backed_error(source, true);
        assert!(
            vm.contains("runtime error: 53053") && jit.contains("runtime error: 53053"),
            "expected stable runtime fault code across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("unsupported VARIANT BYREF return type vt=16387")
                && jit.contains("unsupported VARIANT BYREF return type vt=16387"),
            "expected bounded VT_BYREF diagnostic across VM/JIT, got vm={vm:?} jit={jit:?}"
        );
        assert!(
            vm.contains("com-dispatch-fault-unspecified")
                && jit.contains("com-dispatch-fault-unspecified"),
            "expected bounded adapter fault prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
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

        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(
            out[1],
            RuntimeValue::Bool(true),
            "pre-exception call should have succeeded"
        );
        assert!(
            !matches!(out[2], RuntimeValue::I32(0)),
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
        assert!(expect_object_handle(&out[0]).raw() >= 20_001);
        assert_eq!(
            out[1],
            RuntimeValue::Bool(true),
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
