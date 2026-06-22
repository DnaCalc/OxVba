//! Wide VBA-semantics conformance for the **clean stack** (`oxvba-bind` →
//! `oxvba-bundle::linearize` → `oxvba-vm2`), re-pointed from the legacy
//! `oxvba-vm/tests/vm_feature_coverage.rs`. Same VBA snippets + assertions; only the
//! harness changed. Failures here = gaps in the clean stack vs the old compiler
//! (catalog them in POST_CLEANUP.md). Touches the core shapes the VM must run: the
//! scalar type matrix, strings/BSTR, arrays (fixed/dynamic), UDTs, control flow,
//! optional-arg defaults, indexed properties, and error handling.

use std::collections::BTreeMap;

use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_runtime::{Variant, bstr::BStr};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest,
};
use oxvba_symbol::provider::TypeLibResolver;

struct NullTypeLibs;
impl TypeLibResolver for NullTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        None
    }
}

/// Wrap a snippet as a one-module project (module name `Main`).
fn manifest(source: &str) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Conf".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: source.into(),
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

/// Bind + linearize + run on the clean stack; the user-visible snapshot is the
/// module globals followed by the entry (`Sub Main`) frame's locals (matching the
/// legacy `snapshot_variants` slot order).
fn run_result(source: &str) -> Result<Vec<Variant>, String> {
    let program = oxvba_bind::bind_program(&manifest(source), &NullTypeLibs)
        .map_err(|e| format!("bind error: {e:?}"))?;
    let bundle =
        oxvba_bundle::linearize(&program).map_err(|e| format!("linearize error: {e:?}"))?;
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host)
        .map_err(|e| format!("runtime error: {} {}", e.code, e.message))?;
    let entry = program
        .entry
        .ok_or_else(|| "no entry procedure".to_string())?;
    let main = program
        .procs
        .get(entry.0)
        .ok_or_else(|| "entry out of range".to_string())?;
    let count = bundle.global_count + main.locals.len();
    Ok((0..count)
        .map(|i| vm.slot(i).cloned().unwrap_or_else(Variant::empty))
        .collect())
}

/// Run a snippet expecting success and return the user-visible snapshot.
fn run(source: &str) -> Vec<Variant> {
    run_result(source).expect("snippet should execute")
}

#[test]
fn vba_collection_new_add_count_item() {
    // The built-in `Collection` is a class of the VBA library bundle: `New
    // Collection` mints it via the cross-bundle coclass path, and `.Add`/`.Count`/
    // `.Item` dispatch by name into native method bodies — no predeclared/Native
    // route. `Dim c As New Collection` auto-instantiates `c` at scope entry, so no
    // explicit `Set c = New Collection` is needed.
    let snap = run("Public n As Long\n\
         Public a As Long\n\
         Sub Main()\n\
             Dim c As New Collection\n\
             c.Add 10\n\
             c.Add 20\n\
             n = c.Count\n\
             a = c.Item(2)\n\
         End Sub\n");
    assert_eq!(snap[0].as_i32(), Some(2), "Count after two Adds: {snap:?}");
    assert_eq!(snap[1].as_i32(), Some(20), "Item(2): {snap:?}");
}

#[test]
fn vba_collection_default_member_indexing() {
    // `c(i)` on a `Collection` variable is the default member `c.Item(i)`, not an
    // array subscript: `x = c(2)` must read the second element.
    let snap = run("Public x As Long\n\
         Sub Main()\n\
             Dim c As New Collection\n\
             c.Add 10\n\
             c.Add 20\n\
             x = c(2)\n\
         End Sub\n");
    assert_eq!(snap[0].as_i32(), Some(20), "default-member c(2): {snap:?}");
}

#[test]
fn vartype_of_object_reports_vba_vbobject_discriminant() {
    let snap = run("Public vt As Long\n\
         Public tn As String\n\
         Sub Main()\n\
             Dim c As New Collection\n\
             vt = VBA.VarType(c)\n\
             tn = VBA.TypeName(c)\n\
         End Sub\n");
    assert_eq!(
        snap[0].as_i32(),
        Some(9),
        "VarType(Object) must be vbObject=9: {snap:?}"
    );
    assert!(
        snap.iter()
            .any(|v| v.as_bstr().map(|s| s.as_str()).as_deref() == Some("Collection")),
        "TypeName(Collection) should remain host/class aware: {snap:?}"
    );
}

#[test]
fn vba_collection_keys_position_and_for_each() {
    // String keys (Add item,key / Item(key) / Remove key), `after`-positioned
    // insertion (Add 20 after "a" ⇒ [10,20,30]), and `For Each` enumeration in
    // insertion order — all through the VBA-library Collection class.
    let snap = run("Public byKey As Long\n\
         Public total As Long\n\
         Public cnt As Long\n\
         Sub Main()\n\
             Dim c As Collection\n\
             Set c = New Collection\n\
             c.Add 10, \"a\"\n\
             c.Add 30, \"c\"\n\
             c.Add 20, \"b\", , \"a\"\n\
             byKey = c.Item(\"b\")\n\
             Dim v\n\
             For Each v In c\n\
                 total = total + v\n\
             Next\n\
             c.Remove \"a\"\n\
             cnt = c.Count\n\
         End Sub\n");
    assert_eq!(snap[0].as_i32(), Some(20), "Item(\"b\"): {snap:?}");
    assert_eq!(
        snap[1].as_i32(),
        Some(60),
        "For Each sum 10+20+30: {snap:?}"
    );
    assert_eq!(
        snap[2].as_i32(),
        Some(2),
        "Count after Remove \"a\": {snap:?}"
    );
}

#[test]
fn datevalue_month_name_and_cdate_numeric() {
    // `DateValue` parses the `d mmm yyyy` text form; `CDate` of a numeric is the date
    // serial directly. Both blocked SQLiteForExcel's TestBinding/TestDates.
    let snap = run("Public a As Long\nPublic b As Boolean\n\
         Sub Main()\n\
         a = DateValue(\"1 Jan 2000\") - DateSerial(2000, 1, 1)\n\
         b = (CDate(36526) = DateSerial(2000, 1, 1))\n\
         End Sub");
    assert_eq!(snap[0], Variant::from_i32(0)); // DateValue("1 Jan 2000") == DateSerial(2000,1,1)
    assert_eq!(snap[1], Variant::from_bool(true)); // CDate(36526) == that serial
}

#[test]
fn array_return_function_is_an_array_copy() {
    // `Function F() As Byte()` returns an array; the return type must be `Array(Byte)`
    // so `F = arr` (and the caller's `x = F()`) is a whole-array copy, not a scalar
    // coercion. This blocked SQLiteForExcel's `SQLite3ColumnBlob`.
    let snap = run("Public r As Long\n\
         Sub Main()\n\
         Dim x() As Byte\nx = MakeBytes()\nr = x(1)\n\
         End Sub\n\
         Function MakeBytes() As Byte()\n\
         Dim t(2) As Byte\nt(0) = 10\nt(1) = 20\nt(2) = 30\nMakeBytes = t\n\
         End Function");
    assert_eq!(snap[0], Variant::from_i32(20));
}

#[test]
fn not_not_array_is_an_allocation_probe() {
    // The VBA `(Not Not arr)` idiom probes whether a dynamic array is allocated:
    // 0 before `ReDim`, non-zero after. A single `Not <array>` used to fault
    // type 13 (the bitwise path called `int` on an array Variant).
    let snap = run("Public before As Long\nPublic after As Long\n\
         Sub Main()\n\
         Dim a() As Long\n\
         before = (Not Not a)\n\
         ReDim a(0 To 2)\n\
         after = (Not Not a)\n\
         End Sub");
    assert_eq!(
        snap[0].as_i32(),
        Some(0),
        "unallocated probes as 0: {snap:?}"
    );
    assert_ne!(
        snap[1].as_i32(),
        Some(0),
        "allocated probes as non-zero: {snap:?}"
    );
}

#[test]
fn pointer_helper_pins_are_freed_per_native_call_not_leaked() {
    // Each `StrPtr` pins a cloned cell in the process-global registry; the
    // consuming `Declare` call frees it afterwards. Under the deterministic policy
    // the native lane is denied, so the call errors and the error path frees the
    // pin (`On Error Resume Next` keeps the loop running). The registry must
    // therefore not grow with the iteration count instead of leaking one pin/iter.
    let before = oxvba_runtime::pointer_helpers::live_pin_count();
    run(
        "Declare PtrSafe Function FakeApi Lib \"fake\" (ByVal p As LongPtr) As Long\n\
         Sub Main()\n\
         On Error Resume Next\n\
         Dim s As String\ns = \"payload\"\n\
         Dim i As Long\n\
         For i = 1 To 3000\nFakeApi StrPtr(s)\nNext\n\
         End Sub",
    );
    let after = oxvba_runtime::pointer_helpers::live_pin_count();
    assert!(
        after.saturating_sub(before) < 200,
        "pointer-helper pins leaked across the loop: before={before} after={after}"
    );
}

#[test]
fn declare_longptr_pointer_params_preserve_64_bit_descriptor_types() {
    let source = "Private Declare PtrSafe Function MemMove Lib \"msvcrt\" Alias \"memmove\" (ByVal Destination As LongPtr, ByVal Source As LongPtr, ByVal Length As LongPtr) As LongPtr\n\
         Sub Main()\nEnd Sub\n";
    let program = oxvba_bind::bind_program(&manifest(source), &NullTypeLibs)
        .expect("bind MemMove declaration");
    let descriptor = program
        .external_calls
        .iter()
        .find(|call| call.declared_name.eq_ignore_ascii_case("MemMove"))
        .expect("MemMove descriptor");
    assert_eq!(
        descriptor.param_types,
        vec![
            oxvba_bundle::DeclareParamType::LongPtr,
            oxvba_bundle::DeclareParamType::LongPtr,
            oxvba_bundle::DeclareParamType::LongPtr,
        ]
    );
    assert_eq!(
        descriptor.return_type,
        Some(oxvba_bundle::DeclareParamType::LongPtr)
    );
}

#[test]
fn longptr_arithmetic_widens_to_64_bit_not_long() {
    // `LongPtr` is 64-bit on the Win64 runtime target, so `p + p` for p just under
    // 2^31 must compute in 64 bits (4_294_967_294). If `LongPtr` ranked with `Long`,
    // the sum would coerce-store into a 32-bit Long temp and overflow (error 6).
    let snap = run("Public r As LongLong\n\
         Sub Main()\n\
         Dim p As LongPtr\np = 2147483647\nr = p + p\n\
         End Sub");
    assert_eq!(snap[0], Variant::from_i64(4_294_967_294));
}

#[test]
fn ubound_of_byref_array_param() {
    // `UBound`/`LBound` of a ByRef array parameter (the SQLiteForExcel
    // `SQLite3BindBlob(ByRef Value() As Byte)` shape) must read the array's bounds,
    // not coerce the array.
    let snap = run("Public r As Long\n\
         Sub Main()\n\
         Dim b(2) As Byte\nb(0) = 90\n\
         r = BlobLen(b)\n\
         End Sub\n\
         Function BlobLen(ByRef Value() As Byte) As Long\n\
         BlobLen = UBound(Value) - LBound(Value) + 1\n\
         End Function\n");
    assert_eq!(snap[0], Variant::from_i32(3));
}

#[test]
fn dynamic_array_whole_assignment_is_a_copy_not_a_scalar_coercion() {
    // `Dim dst() As Byte` is an array; `dst = src` copies the whole array. The
    // declarator must type `dst` as `Array(Byte)`, not scalar `Byte` — otherwise the
    // assignment scalar-coerces the array and fails ("ArrayVariant to …"). (This
    // blocked SQLiteForExcel's `StringToUtf8Bytes`.)
    let snap = run("Public r As Long\n\
         Sub Main()\n\
         Dim src(2) As Byte\nsrc(0) = 10\nsrc(1) = 20\nsrc(2) = 30\n\
         Dim dst() As Byte\ndst = src\n\
         r = dst(1)\n\
         End Sub");
    assert_eq!(snap[0], Variant::from_i32(20));
}

#[test]
fn err_lastdllerror_binds_and_defaults_to_zero() {
    // `Err.LastDllError` binds as a `Long` member read; with no native `Declare` call
    // it reads 0. The captured-after-a-real-call value is exercised on Windows by the
    // native_declare suite (which makes an actual FFI call that sets the OS error).
    let snap = run("Sub Main()\nDim x As Long\nx = Err.LastDllError\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_i32(0)]);
}

#[test]
fn numeric_conversion_intrinsics_with_bankers_rounding() {
    // The `Cxxx` conversions coerce to the named type; integer targets use VBA
    // banker's rounding (half-to-even). Each variable holds its declared type.
    let snap = run("Sub Main()\n\
         Dim a As Double\nDim b As Long\nDim c As Integer\nDim d As Byte\n\
         Dim e As Boolean\nDim f As Single\nDim g As LongLong\nDim h As Currency\n\
         a = CDbl(\"3.5\")\n\
         b = CLng(2.5)\n\
         c = CInt(-3.5)\n\
         d = CByte(255.4)\n\
         e = CBool(5)\n\
         f = CSng(1.25)\n\
         g = CLngLng(2.5)\n\
         h = CCur(1.5)\n\
         End Sub");
    assert_eq!(
        snap,
        vec![
            Variant::from_f64(3.5),                    // CDbl parses the string
            Variant::from_i32(2),                      // CLng(2.5) half-to-even → 2
            Variant::from_i16(-4),                     // CInt(-3.5) half-to-even → -4
            Variant::from_u8(255),                     // CByte(255.4) → 255
            Variant::from_bool(true),                  // CBool(5) → True
            Variant::from_f32(1.25),                   // CSng
            Variant::from_i64(2),                      // CLngLng(2.5) half-to-even → 2
            Variant::from_currency_scaled_i64(15_000), // CCur(1.5) → 1.5 scaled
        ]
    );
}

#[test]
fn numeric_conversion_intrinsics_accept_vba_radix_strings() {
    let snap = run("Sub Main()\n\
         Dim a As Integer\nDim b As Long\nDim c As Long\nDim d As Double\nDim e As Boolean\n\
         a = CInt(\"&H20\")\n\
         b = CLng(\"&O17\")\n\
         c = CLng(\"+&H7F\")\n\
         d = CDbl(\"&H2A\")\n\
         e = CBool(\"&H1\")\n\
         End Sub");
    assert_eq!(
        snap,
        vec![
            Variant::from_i16(32),
            Variant::from_i32(15),
            Variant::from_i32(127),
            Variant::from_f64(42.0),
            Variant::from_bool(true),
        ]
    );
}

#[test]
fn numeric_conversion_intrinsics_reject_invalid_vba_radix_strings() {
    let err = run_result("Sub Main()\nDim x As Integer\nx = CInt(\"&HZZ\")\nEnd Sub")
        .expect_err("invalid hex string should be a runtime error");
    assert!(
        err.contains("expected a numeric value"),
        "unexpected diagnostic: {err}"
    );
}

#[test]
fn conversion_intrinsic_overflow_is_error_6() {
    let err = run_result("Sub Main()\nDim x As Integer\nx = CInt(100000)\nEnd Sub")
        .expect_err("CInt overflow should be a runtime error");
    assert!(
        err.contains("does not fit in Integer"),
        "unexpected diagnostic: {err}"
    );
}

#[test]
fn scalar_long_arithmetic() {
    let snap = run("Sub Main()\nDim x As Long\nx = 2\nx = x * 3 + 4\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_i32(10)]);
}

#[test]
fn scalar_longlong_const_carrier_executes() {
    let snap = run(
        "Const CTotal As LongLong = 5000000000\nSub Main()\nDim x As LongLong\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i64(5_000_000_000)]);
}

#[test]
fn scalar_typed_integer_const_expressions_execute() {
    let snap = run(
        "Const CByte As Byte = 1 + 2\nConst CInteger As Integer = 32767 - 1\nConst CLong As Long = 2 ^ 3 \\ 2 Mod 3 + 4\nSub Main()\nDim b As Byte\nDim i As Integer\nDim l As Long\nb = CByte\ni = CInteger\nl = CLong\nEnd Sub",
    );
    // Each variable holds its *declared* type (the clean stack coerces on store):
    // `Byte`, `Integer`, `Long` — not the legacy VM's uniform `Long` widening.
    assert_eq!(
        snap,
        vec![
            Variant::from_u8(3),
            Variant::from_i16(32_766),
            Variant::from_i32(5),
        ]
    );
}

#[test]
fn scalar_untyped_integer_const_expression_executes() {
    let snap = run(
        "Const CBase = 1 + 2\nConst COffset = -1 + 2\nConst CTotal = CBase + COffset\nSub Main()\nDim x\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i32(4)]);
}

#[test]
fn scalar_double_const_carrier_executes() {
    let snap = run(
        "Const CBase As Long = 1\nConst CTotal As Double = CBase + 0.5\nSub Main()\nDim x As Double\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_f64(1.5)]);
}

#[test]
fn scalar_single_const_carrier_executes() {
    let snap = run(
        "Const CBase As Double = 1.25\nConst CTotal As Single = CBase + 0.25\nSub Main()\nDim x As Single\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_f32(1.5)]);
}

#[test]
fn scalar_type_char_const_carriers_execute() {
    // VBA 7.1 type-declaration chars are `% & ! # @ $` (there is no LongLong char), so
    // the `LongLong` carrier uses an explicit `As LongLong` rather than the legacy
    // snippet's non-standard `^`.
    let snap = run(
        "Const CInteger% = 7\nConst CLong& = 8\nConst CLongLong As LongLong = 5000000000\nConst CTotal! = 1.5\nConst CDouble# = 2\nConst CAmount@ = 1.25\nConst CText$ = \"ok\"\nSub Main()\nDim i As Integer\nDim l As Long\nDim ll As LongLong\nDim x As Single\nDim d As Double\nDim amount As Currency\nDim s As String\ni = CInteger\nl = CLong\nll = CLongLong\nx = CTotal\nd = CDouble\namount = CAmount\ns = CText\nEnd Sub",
    );
    // Each carrier coerces to its declared type: `%`→Integer, `&`→Long, `As LongLong`,
    // `!`→Single, `#`→Double, `@`→Currency, `$`→String.
    assert_eq!(
        snap,
        vec![
            Variant::from_i16(7),
            Variant::from_i32(8),
            Variant::from_i64(5_000_000_000),
            Variant::from_f32(1.5),
            Variant::from_f64(2.0),
            Variant::from_currency_scaled_i64(12_500),
            Variant::from_string(BStr::from("ok")),
        ]
    );
}

#[test]
fn scalar_typed_const_exact_carriers_preserve_variant_values() {
    let snap = run(
        "Const CSingle As Single = 1.5!\nConst CAmount As Currency = 1.25@\nConst CStamp As Date = #2026-02-28#\nConst CText As String = CSingle & \"|\" & CAmount\nSub Main()\nDim singleValue As Variant\nDim amount As Variant\nDim stamp As Variant\nDim text As String\nsingleValue = CSingle\namount = CAmount\nstamp = CStamp\ntext = CText\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_f32(1.5),
            Variant::from_currency_scaled_i64(12_500),
            Variant::from_date_f64(46_081.0),
            Variant::from_string(BStr::from("1.5|1.25")),
        ]
    );
}

#[test]
fn scalar_string_typed_const_values_coerce_to_declared_carriers() {
    let snap = run(
        "Const CLong As Long = \"7\"\nConst CBool As Boolean = \"False\"\nConst CSingle As Single = \"1.5\"\nConst CDouble As Double = \"2.5\"\nConst CAmount As Currency = \"1.25\"\nConst CStamp As Date = \"2026-02-28\"\nSub Main()\nDim l As Long\nDim b As Boolean\nDim s As Single\nDim d As Double\nDim amount As Currency\nDim stamp As Date\nl = CLong\nb = CBool\ns = CSingle\nd = CDouble\namount = CAmount\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_i32(7),
            Variant::from_bool(false),
            Variant::from_f32(1.5),
            Variant::from_f64(2.5),
            Variant::from_currency_scaled_i64(12_500),
            Variant::from_date_f64(46_081.0),
        ]
    );
}

#[test]
fn scalar_deftype_defaults_affect_variables_params_and_returns() {
    let snap = run("DefSng F, P, S\n\
         Public outSample As Variant\n\
         Public outParam As Variant\n\
         Public outFunction As Variant\n\
         Sub Main()\n\
             Dim sample\n\
             sample = 7\n\
             outSample = sample\n\
             Take 9\n\
             outFunction = FormatIt()\n\
         End Sub\n\
         Sub Take(ByVal payload)\n\
             outParam = payload\n\
         End Sub\n\
         Function FormatIt()\n\
             FormatIt = 8\n\
         End Function\n");
    assert_eq!(
        snap[0].as_f32(),
        Some(7.0),
        "DefSng S should type the local sample as Single: {snap:?}"
    );
    assert_eq!(
        snap[1].as_f32(),
        Some(9.0),
        "DefSng P should type the parameter payload as Single: {snap:?}"
    );
    assert_eq!(
        snap[2].as_f32(),
        Some(8.0),
        "DefSng F should type the function return as Single: {snap:?}"
    );
}

#[test]
fn scalar_boolean_const_expression_executes() {
    let snap = run(
        "Const Prefix As String = \"re\"\nConst Enabled As Boolean = True\nConst CFlag As Boolean = Enabled = Not False And 2 > 1 And Prefix & \"ady\" = \"ready\"\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn scalar_boolean_xor_const_expression_executes() {
    let snap = run(
        "Const Enabled As Boolean = True\nConst CFlag As Boolean = Enabled Xor True\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(false)]);
}

#[test]
fn scalar_boolean_eqv_imp_const_expressions_execute() {
    let snap = run(
        "Const Enabled As Boolean = True\nConst CEqv As Boolean = Enabled Eqv False\nConst CImp As Boolean = Enabled Imp False\nSub Main()\nDim sameFlag As Boolean\nDim impliesFlag As Boolean\nsameFlag = CEqv\nimpliesFlag = CImp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![Variant::from_bool(false), Variant::from_bool(false)]
    );
}

#[test]
fn scalar_option_compare_text_boolean_const_expression_executes() {
    let snap = run(
        "Option Compare Text\nConst CFlag As Boolean = \"a\" = \"A\"\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn scalar_boolean_like_const_expression_executes() {
    let snap = run(
        "Option Compare Text\nConst Prefix As String = \"he\"\nConst CFlag As Boolean = Prefix & \"llo\" Like \"HELLO\"\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn filter_datepart_and_ismissing_intrinsics() {
    // Filter keeps the matching elements; DatePart("q") is the quarter;
    // IsMissing is True for an omitted optional Variant, False when supplied.
    let snap = run(
        "Public n As Long\nPublic q As Long\nPublic miss As Boolean\nPublic present As Boolean\n\
         Sub Main()\n\
         Dim f\nf = Filter(Array(\"apple\", \"banana\", \"grape\"), \"an\")\n\
         n = UBound(f) - LBound(f) + 1\n\
         q = DatePart(\"q\", DateSerial(2024, 8, 15))\n\
         miss = Probe()\n\
         present = Probe(5)\n\
         End Sub\n\
         Function Probe(Optional x) As Boolean\nProbe = IsMissing(x)\nEnd Function",
    );
    assert_eq!(
        snap[0],
        Variant::from_i32(1),
        "Filter(\"an\") keeps only banana"
    );
    assert_eq!(snap[1], Variant::from_i32(3), "August is Q3");
    assert_eq!(
        snap[2],
        Variant::from_bool(true),
        "omitted optional is Missing"
    );
    assert_eq!(
        snap[3],
        Variant::from_bool(false),
        "supplied optional is not Missing"
    );
}

#[test]
fn time_and_weekday_intrinsics() {
    // Hour/Minute/Second extract the time-of-day from a serial; WeekdayName
    // maps a 1-based weekday (1 = Sunday) to its name; LenB is 2× the UTF-16
    // code-unit length.
    let snap = run(
        "Public h As Long\nPublic m As Long\nPublic s As Long\nPublic w As String\nPublic b As Long\n\
         Sub Main()\n\
         h = Hour(TimeSerial(13, 45, 30))\n\
         m = Minute(TimeSerial(13, 45, 30))\n\
         s = Second(TimeSerial(13, 45, 30))\n\
         w = WeekdayName(1)\n\
         b = LenB(\"hi\")\n\
         End Sub",
    );
    assert_eq!(snap[0], Variant::from_i32(13));
    assert_eq!(snap[1], Variant::from_i32(45));
    assert_eq!(snap[2], Variant::from_i32(30));
    assert_eq!(snap[3], Variant::from_string(BStr::from("Sunday")));
    assert_eq!(snap[4], Variant::from_i32(4));
}

#[test]
fn lenb_reports_scalar_storage_widths() {
    let snap = run(
        "Public bByte As Long\nPublic bInteger As Long\nPublic bLong As Long\nPublic bLongPtr As Long\nPublic bSingle As Long\nPublic bDouble As Long\n\
         Sub Main()\n\
         Dim by As Byte\n\
         Dim i As Integer\n\
         Dim l As Long\n\
         Dim p As LongPtr\n\
         Dim s As Single\n\
         Dim d As Double\n\
         bByte = LenB(by)\n\
         bInteger = LenB(i)\n\
         bLong = LenB(l)\n\
         bLongPtr = LenB(p)\n\
         bSingle = LenB(s)\n\
         bDouble = LenB(d)\n\
         End Sub",
    );
    assert_eq!(snap[0], Variant::from_i32(1));
    assert_eq!(snap[1], Variant::from_i32(2));
    assert_eq!(snap[2], Variant::from_i32(4));
    assert_eq!(snap[3], Variant::from_i32(8));
    assert_eq!(snap[4], Variant::from_i32(4));
    assert_eq!(snap[5], Variant::from_i32(8));
}

#[test]
fn nested_scalar_udt_fields_are_recursively_materialized() {
    // A UDT field that is itself a UDT (`Outer.Item As Inner`) is recursively
    // default-initialized as a record, so `o.Item.N` is a live nested-record
    // field — not an Empty that faults with "record expected".
    let snap = run("Private Type Inner\nText As String\nN As Long\nEnd Type\n\
         Private Type Outer\nItem As Inner\nCount As Long\nEnd Type\n\
         Sub Main()\nDim o As Outer\no.Count = 5\no.Item.N = 7\nDim r As Long\nr = o.Item.N + o.Count\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i32(12)),
        "expected o.Item.N + o.Count = 12 in {snap:?}"
    );
}

#[test]
fn udt_array_elements_are_materialized_as_records() {
    // `ReDim arr(...)` of a UDT array seeds each element with a default record,
    // recursing into nested scalar-UDT subfields — so `Lines(i).Text` and the
    // nested `Lines(i).Rect.X` (the element's own scalar-UDT subfield is itself a
    // default record) read back instead of faulting type 13 ("record expected")
    // on an `Empty` element.
    let snap = run("Private Type RectF\nX As Long\nEnd Type\n\
         Private Type LineT\nText As String\nRect As RectF\nEnd Type\n\
         Public gText As String\nPublic gX As Long\n\
         Sub Main()\n\
         Dim lines() As LineT\n\
         ReDim lines(0 To 1)\n\
         lines(0).Text = \"hello\"\n\
         lines(1).Rect.X = 42\n\
         gText = lines(0).Text\n\
         gX = lines(1).Rect.X\n\
         End Sub");
    assert_eq!(
        snap[0].as_bstr().map(|b| b.as_str()),
        Some("hello".to_string()),
        "Lines(0).Text reads back: {snap:?}"
    );
    assert_eq!(
        snap[1].as_i32(),
        Some(42),
        "nested Lines(1).Rect.X reads back (recursive record seed): {snap:?}"
    );
}

#[test]
fn udt_array_whole_copy_is_independent() {
    // Assigning a whole UDT array is a value copy: mutating an element's record
    // field in the copy must not reach the source array's element.
    let snap = run("Private Type Cell\nV As Long\nEnd Type\n\
         Public srcV As Long\n\
         Sub Main()\n\
         Dim a() As Cell\nDim b() As Cell\n\
         ReDim a(0 To 0)\n\
         a(0).V = 1\n\
         b = a\n\
         b(0).V = 99\n\
         srcV = a(0).V\n\
         End Sub");
    assert_eq!(
        snap[0].as_i32(),
        Some(1),
        "source a(0).V stays 1 after b(0).V = 99: {snap:?}"
    );
}

#[test]
fn redim_preserve_keeps_existing_udt_element_records() {
    // `ReDim Preserve` of a UDT array keeps each existing element's record (its
    // populated field), default-seeding only the grown tail — so an earlier write
    // survives the resize and the new element is a usable record.
    let snap = run("Private Type Cell\nV As Long\nEnd Type\n\
         Public kept As Long\nPublic grown As Long\n\
         Sub Main()\n\
         Dim a() As Cell\n\
         ReDim a(0 To 0)\n\
         a(0).V = 7\n\
         ReDim Preserve a(0 To 1)\n\
         a(1).V = 9\n\
         kept = a(0).V\n\
         grown = a(1).V\n\
         End Sub");
    assert_eq!(snap[0].as_i32(), Some(7), "preserved a(0).V: {snap:?}");
    assert_eq!(snap[1].as_i32(), Some(9), "grown a(1).V: {snap:?}");
}

#[test]
fn single_line_function_body_executes() {
    // A whole `Function … : body : End Function` on one physical line (the
    // colon-separated single-line proc idiom, used heavily for trivial
    // accessors) parses and runs.
    let snap = run("Sub Main()\nDim r As Long\nr = Doubled(21)\nEnd Sub\n\
         Public Function Doubled(ByVal n As Long) As Long: Doubled = n * 2: End Function");
    assert!(
        snap.contains(&Variant::from_i32(42)),
        "expected single-line function result 42 in {snap:?}"
    );
}

#[test]
fn byval_call_argument_modifier_parses_and_runs() {
    // `ByVal expr` / `ByRef expr` as a call-site passing-mode override (the
    // CopyMemory/API idiom) parses and binds; here it matches the parameter's
    // own ByVal, so the call runs and returns the value unchanged.
    let snap = run(
        "Sub Main()\nDim r As Long\nDim x As Long\nx = 9\nr = Echo(ByVal x)\nEnd Sub\n\
         Public Function Echo(ByVal n As Long) As Long\nEcho = n\nEnd Function",
    );
    assert!(
        snap.contains(&Variant::from_i32(9)),
        "expected ByVal call-arg result 9 in {snap:?}"
    );
}

#[test]
fn replace_honors_start_count_and_compare() {
    // start drops the prefix before it; count limits replacements; compare=1
    // matches case-insensitively.
    let snap = run("Sub Main()\n\
         Dim a As String\nDim b As String\nDim c As String\n\
         a = Replace(\"abcabc\", \"b\", \"X\", 3)\n\
         b = Replace(\"abcabcabc\", \"b\", \"X\", 1, 2)\n\
         c = Replace(\"aXaXaX\", \"x\", \"_\", 1, 2, 1)\n\
         End Sub");
    assert!(
        snap.contains(&Variant::from_string(BStr::from("caXc"))),
        "start=3 drops the prefix: {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_string(BStr::from("aXcaXcabc"))),
        "count=2 limits replacements: {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_string(BStr::from("a_a_aX"))),
        "compare=1 matches case-insensitively, count=2: {snap:?}"
    );
}

#[test]
fn qualified_vba_replace_preserves_positional_arguments() {
    let snap = run("Sub Main()\n\
         Dim value As String\n\
         value = VBA.Replace(\"orders/{id}\", \"{id}\", \"A%20B\")\n\
         End Sub");
    assert!(
        snap.contains(&Variant::from_string(BStr::from("orders/A%20B"))),
        "qualified VBA.Replace should match bare Replace argument binding: {snap:?}"
    );
}

#[test]
fn strings_library_functions_route_through_vba_bundle() {
    // The `Strings` library functions are native-bodied procs of the synthetic
    // `VBA` bundle: the binder resolves each as `ExternMember { has_receiver:
    // false }` → `ExportToken::ModuleFunc` → `Op::CallExtern`, and the VM runs the
    // linked `NativeBody::Library` body through `oxvba-lib` (no frame). This proves
    // the routing change is behaviour-preserving across the representative shapes:
    // single-arg (Len/UCase), two-arg (Left), optional-trailing-arg (Mid without
    // length), and optional-mid/trailing args (InStr's start + Replace's find).
    let snap = run("Sub Main()\n\
         Dim a As String\nDim b As Long\nDim c As String\n\
         Dim d As String\nDim e As Long\nDim f As String\n\
         a = Left(\"hello\", 3)\n\
         b = InStr(\"abcabc\", \"b\")\n\
         c = UCase(\"ab\")\n\
         d = Replace(\"a-b-c\", \"-\", \"+\")\n\
         e = Len(\"hello\")\n\
         f = Mid(\"hello\", 2, 3)\n\
         End Sub");
    assert_eq!(
        snap.iter().filter_map(Variant::as_i32).collect::<Vec<_>>(),
        vec![2, 5],
        "InStr==2 and Len==5: {snap:?}"
    );
    for expected in ["hel", "AB", "a+b+c", "ell"] {
        assert!(
            snap.contains(&Variant::from_string(BStr::from(expected))),
            "expected {expected:?} among string results: {snap:?}"
        );
    }
}

#[test]
fn strings_mid_without_length_takes_remainder() {
    // `Mid(s, start)` with the optional length omitted must take the remainder —
    // the `ExternProc` route must not pad a trailing optional into an explicit
    // argument (which would change the result).
    let snap = run("Sub Main()\n\
         Dim a As String\n\
         a = Mid(\"hello\", 2)\n\
         End Sub");
    assert!(
        snap.contains(&Variant::from_string(BStr::from("ello"))),
        "Mid without length: {snap:?}"
    );
}

#[test]
fn math_datetime_conversion_functions_route_through_vba_bundle() {
    // Math/DateTime/Conversion/Financial functions are now native-bodied procs of
    // the synthetic `VBA` bundle (`ExternMember { has_receiver: false }` →
    // `ModuleFunc` → `Op::CallExtern` → `NativeBody::Library`), exactly like the
    // `Strings` slice. Behaviour must be identical to the old `Native` route across
    // the representative shapes: single-arg (Abs), banker's rounding with an omitted
    // optional digits arg (Round), base conversion (Hex), string→number coercion
    // (CLng), and a 3-arg constructor round-tripped through accessors
    // (DateSerial → Year/Month/Day).
    let snap = run("Sub Main()\n\
         Dim a As Long\nDim r As Double\nDim h As String\nDim n As Long\n\
         Dim d As Date\nDim y As Long\nDim mo As Long\nDim dy As Long\n\
         a = Abs(-5)\n\
         r = Round(2.5)\n\
         h = Hex(255)\n\
         n = CLng(\"42\")\n\
         d = DateSerial(2021, 7, 15)\n\
         y = Year(d)\n\
         mo = Month(d)\n\
         dy = Day(d)\n\
         End Sub");
    // Abs(-5)==5, CLng("42")==42, Year==2021, Month==7, Day==15.
    assert_eq!(
        snap.iter().filter_map(Variant::as_i32).collect::<Vec<_>>(),
        vec![5, 42, 2021, 7, 15],
        "integer-valued results: {snap:?}"
    );
    // Round(2.5)==2 (banker's rounding to even), as a Double.
    assert!(
        snap.iter().any(|v| v.as_f64() == Some(2.0)),
        "Round(2.5) must be 2.0 (banker's): {snap:?}"
    );
    // Hex(255)=="FF".
    assert!(
        snap.contains(&Variant::from_string(BStr::from("FF"))),
        "Hex(255) must be \"FF\": {snap:?}"
    );
}

#[test]
fn information_predicates_route_through_vba_bundle() {
    // The `Information` predicates (`IsArray`/`VarType`/`TypeName`/`IsNumeric`/…) are
    // now native-bodied procs of the synthetic `VBA` bundle (`ExternMember
    // { has_receiver: false }` → `ModuleFunc` → `Op::CallExtern` → `NativeBody::
    // Library`), like the `Strings`/`Math`/… slices — but their `Information`-module
    // siblings `IIf`/`Choose`/`Switch` stay eager special forms. Behaviour must be
    // identical to the old `Native` route.
    let snap = run("Sub Main()\n\
         Dim vt As Long\n\
         Dim tn As String\n\
         Dim isn As Boolean\n\
         Dim isa As Boolean\n\
         Dim arr(1 To 3) As Long\n\
         vt = VarType(5#)\n\
         tn = TypeName(\"hello\")\n\
         isn = IsNumeric(42)\n\
         isa = IsArray(arr)\n\
         End Sub");
    // VarType(5#) == vbDouble (5); TypeName("hello") == "String".
    assert!(
        snap.iter().any(|v| v.as_i32() == Some(5)),
        "VarType(5#) must be vbDouble (5): {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_string(BStr::from("String"))),
        "TypeName(\"hello\") must be \"String\": {snap:?}"
    );
    // IsNumeric(42) == True and IsArray(arr) == True.
    assert!(
        snap.iter()
            .filter(|v| **v == Variant::from_bool(true))
            .count()
            >= 2,
        "IsNumeric(42) and IsArray(arr) must both be True: {snap:?}"
    );
}

#[test]
fn is_numeric_recognises_numeric_strings() {
    // `IsNumeric` of a numeric-typed value worked already; the fix makes it also
    // recognise a STRING that represents a number ("42", "&HFF", "  -1.5e3  ") while
    // still rejecting non-numeric text ("12a", "abc", "").
    let snap = run("Public a As Boolean\n\
         Public b As Boolean\n\
         Public c As Boolean\n\
         Public d As Boolean\n\
         Public e As Boolean\n\
         Sub Main()\n\
             a = IsNumeric(\"42\")\n\
             b = IsNumeric(\"&HFF\")\n\
             c = IsNumeric(\"  -1.5e3  \")\n\
             d = IsNumeric(\"12a\")\n\
             e = IsNumeric(42)\n\
         End Sub");
    assert_eq!(
        snap[0],
        Variant::from_bool(true),
        "IsNumeric(\"42\"): {snap:?}"
    );
    assert_eq!(
        snap[1],
        Variant::from_bool(true),
        "IsNumeric(\"&HFF\"): {snap:?}"
    );
    assert_eq!(
        snap[2],
        Variant::from_bool(true),
        "IsNumeric(\"  -1.5e3  \"): {snap:?}"
    );
    assert_eq!(
        snap[3],
        Variant::from_bool(false),
        "IsNumeric(\"12a\"): {snap:?}"
    );
    assert_eq!(snap[4], Variant::from_bool(true), "IsNumeric(42): {snap:?}");
}

#[test]
fn chr_asc_ansi_distinct_from_chrw_ascw_wide() {
    // `Chr`/`Asc` are genuine ANSI (Windows-1252); `ChrW`/`AscW` are Unicode-wide.
    // They agree for ASCII but DIFFER in 128..255: Chr(150) is the CP-1252 en-dash
    // (U+2013) while ChrW(150) is the raw control char U+0096.
    let snap = run("Public ansiEq As Boolean\n\
         Public wideDiffers As Boolean\n\
         Public ascAnsi As Long\n\
         Public ascWide As Long\n\
         Public euro As Long\n\
         Sub Main()\n\
             ansiEq = (Chr(150) = ChrW(&H2013))\n\
             wideDiffers = (ChrW(150) <> Chr(150))\n\
             ascAnsi = Asc(ChrW(&H2013))\n\
             ascWide = AscW(ChrW(&H2013))\n\
             euro = Asc(ChrW(8364))\n\
         End Sub");
    assert_eq!(
        snap[0],
        Variant::from_bool(true),
        "Chr(150) == ChrW(0x2013) (CP-1252 en-dash): {snap:?}"
    );
    assert_eq!(
        snap[1],
        Variant::from_bool(true),
        "ChrW(150) differs from Chr(150): {snap:?}"
    );
    assert_eq!(snap[2].as_i32(), Some(150), "Asc(en-dash) == 150: {snap:?}");
    assert_eq!(
        snap[3].as_i32(),
        Some(8211),
        "AscW(en-dash) == 8211: {snap:?}"
    );
    assert_eq!(
        snap[4].as_i32(),
        Some(128),
        "Asc(€) == 128 (CP-1252): {snap:?}"
    );
}

#[test]
fn filesystem_functions_route_through_vba_bundle() {
    // The `FileIo` by-name FUNCTIONS (`FreeFile`/`CurDir`/`FileLen`/`GetAttr`/
    // `FileDateTime`/`EOF`/`LOF`/`Seek`/`Loc` — catalog `Ordinary`, non-empty names)
    // are native-bodied procs of the synthetic `VBA` bundle, exported under the
    // canonical `FileSystem` module: the binder resolves each as `ExternMember
    // { has_receiver: false }` → a `VBA`/`FileSystem` `ModuleFunc` import +
    // `Op::CallExtern` → `NativeBody::Library`, exactly like the `Interaction` host
    // functions. Their behaviour reaches the host filesystem (proved end-to-end by
    // `oxvba-host`'s `filesystem_statements.rs` against the real filesystem); the null
    // HAL adapter does not support filesystem I/O, so here we assert the *routing*:
    // the linearized entry bundle imports each callee from `VBA`/`FileSystem`, which
    // is exactly the cross-bundle link the bespoke `Native` route did not produce.
    //
    // The by-NAME `FileStatement` forms (`Kill`/`MkDir`/`RmDir`/`ChDir`/`ChDrive`/
    // `SetAttr`/`FileCopy` — not lexer keywords, so they resolve by name) route the
    // same way (P4). The name-LESS `FileStatement` forms (`Open`/`Print #`/`Put`/`Get`/
    // …) are parser-bound and covered by `file_statements_route_through_vba_bundle`.
    let program = oxvba_bind::bind_program(
        &manifest(
            "Sub Main()\n\
             Dim n As Long\nDim p As String\nDim l As Long\n\
             n = FreeFile\n\
             p = CurDir()\n\
             l = FileLen(\"x\")\n\
             MkDir \"d\"\n\
             FileCopy \"a\", \"b\"\n\
             End Sub",
        ),
        &NullTypeLibs,
    )
    .expect("bind should succeed");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize should succeed");

    for member in ["FreeFile", "CurDir", "FileLen", "MkDir", "FileCopy"] {
        assert!(
            bundle.imports.iter().any(|imp| {
                imp.unit.eq_ignore_ascii_case("VBA")
                    && matches!(
                        &imp.token,
                        oxvba_bundle::ExportToken::ModuleFunc { module, member: m, .. }
                            if module.eq_ignore_ascii_case("FileSystem")
                                && m.eq_ignore_ascii_case(member)
                    )
            }),
            "expected a VBA/FileSystem import for {member}: {:?}",
            bundle.imports
        );
    }
}

#[test]
fn file_statements_route_through_vba_bundle() {
    // The name-less, parser-bound funny-syntax file STATEMENTS (`Open`/`Close`/
    // `Print #`/`Write #`/`Input #`/`Line Input #`/`Put`/`Get`/`Seek #`/`Width #`/
    // `Name … As`/`Lock`/`Unlock`) keep their special parsing + arg-shaping in
    // `stmt.rs`, but now lower to a cross-bundle `ExternProc` call into the `VBA`
    // bundle's `FileSystem` module (the `library_statement_member` location — `Open`/
    // `Print`/`Put`/`Get`/… — or, for the `Seek #n, pos` statement, the by-name
    // `library_member(FileSeek)` member `Seek`) instead of `CoreCallee::Native(File*)`.
    // We assert the *routing*: the linearized entry bundle imports each statement's
    // member from `VBA`/`FileSystem` (the real filesystem behaviour is proved by
    // `oxvba-host`'s `filesystem_statements.rs` and the VM round-trip tests in
    // `bind_roundtrip.rs`).
    let program = oxvba_bind::bind_program(
        &manifest(
            "Sub Main()\n\
             Dim r As Long\n\
             Open \"f.dat\" For Random As #1 Len = 8\n\
             Print #1, \"hi\"\n\
             Put #1, 1, 222\n\
             Get #1, 1, r\n\
             Seek #1, 1\n\
             Width #1, 80\n\
             Close #1\n\
             Name \"a\" As \"b\"\n\
             End Sub",
        ),
        &NullTypeLibs,
    )
    .expect("bind should succeed");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize should succeed");

    // The internal member names the parser-bound statements import, plus `Seek`
    // (the statement form reuses the by-name function member).
    for member in [
        "Open", "Print", "Put", "Get", "Seek", "Width", "Close", "Name",
    ] {
        assert!(
            bundle.imports.iter().any(|imp| {
                imp.unit.eq_ignore_ascii_case("VBA")
                    && matches!(
                        &imp.token,
                        oxvba_bundle::ExportToken::ModuleFunc { module, member: m, .. }
                            if module.eq_ignore_ascii_case("FileSystem")
                                && m.eq_ignore_ascii_case(member)
                    )
            }),
            "expected a VBA/FileSystem import for statement member {member}: {:?}",
            bundle.imports
        );
    }
    // And no file statement remains on the bespoke `CoreCallee::Native` route.
    use oxvba_bundle::coreir::{CoreCallee, CoreStmt, CoreValue};
    use oxvba_bundle::native::NativeImplId;
    fn value_native(v: &CoreValue) -> Option<NativeImplId> {
        match v {
            CoreValue::Call {
                callee: CoreCallee::Native(n),
                ..
            } => Some(*n),
            CoreValue::Unary { expr, .. } => value_native(expr),
            _ => None,
        }
    }
    for proc in &program.procs {
        for stmt in &proc.body {
            let native = match stmt {
                CoreStmt::Eval(v) => value_native(v),
                CoreStmt::Assign { value, .. } => value_native(value),
                _ => None,
            };
            if let Some(id) = native {
                assert!(
                    id.library_member().is_none() && id.library_statement_member().is_none(),
                    "a migrated file id {id:?} must not stay on CoreCallee::Native",
                );
            }
        }
    }
}

#[test]
fn iif_stays_an_eager_special_form() {
    // `IIf` is NOT rerouted through the bundle: it stays a special form lowered
    // eagerly (both arms evaluated) to `CoreCallee::Native(IIf)`. It must still pick
    // the correct arm by its condition.
    let snap = run("Sub Main()\n\
         Dim a As Long\nDim b As Long\n\
         a = IIf(True, 10, 20)\n\
         b = IIf(False, 10, 20)\n\
         End Sub");
    assert!(
        snap.iter().any(|v| v.as_i32() == Some(10)),
        "IIf(True, 10, 20) must be 10: {snap:?}"
    );
    assert!(
        snap.iter().any(|v| v.as_i32() == Some(20)),
        "IIf(False, 10, 20) must be 20: {snap:?}"
    );
}

#[test]
fn like_charlist_ranges_negation_and_literal_bracket() {
    // `[charlist]` with `a-z` ranges, `!` negation, and a literal `]` first.
    let snap = run("Sub Main()\n\
         Dim a As Boolean\nDim b As Boolean\nDim c As Boolean\nDim d As Boolean\n\
         a = (\"f\" Like \"[a-z]\")\n\
         b = (\"F\" Like \"[!a-z]\")\n\
         c = (\"9\" Like \"[0-9a-f]\")\n\
         d = (\"]\" Like \"[]x]\")\n\
         End Sub");
    assert!(
        snap.iter()
            .filter(|v| **v == Variant::from_bool(true))
            .count()
            >= 4,
        "all four charlist matches should be true in {snap:?}"
    );
}

#[test]
fn scalar_string_const_expression_executes() {
    let snap = run(
        "Const Prefix As String = \"re\"\nConst CText As String = Prefix & \"ady\"\nSub Main()\nDim text As String\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("ready"))]);
}

#[test]
fn scalar_string_const_scalar_concat_expression_executes() {
    let snap = run(
        "Const Prefix As String = \"v\"\nConst CNumber As Long = 7\nConst CFlag As Boolean = True\nConst CText As String = Prefix & CNumber & CFlag\nSub Main()\nDim text As String\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("v7True"))]);
}

#[test]
fn scalar_string_store_coerces_numeric_values() {
    let snap = run("Public outLocal As Variant\n\
         Public outParam As Variant\n\
         Public outFunction As Variant\n\
         Sub Main()\n\
             Dim sample As String\n\
             sample = 7\n\
             outLocal = sample\n\
             Take 9\n\
             outFunction = Made()\n\
         End Sub\n\
         Sub Take(ByVal payload As String)\n\
             outParam = payload\n\
         End Sub\n\
         Function Made() As String\n\
             Made = 8\n\
         End Function\n");
    assert_eq!(snap[0], Variant::from_string(BStr::from("7")));
    assert_eq!(snap[1], Variant::from_string(BStr::from("9")));
    assert_eq!(snap[2], Variant::from_string(BStr::from("8")));
}

#[test]
fn fixed_length_string_store_coerces_numeric_values() {
    let snap = run("Public outLocal As Variant\n\
         Sub Main()\n\
             Dim sample As String * 3\n\
             sample = 7\n\
             outLocal = sample\n\
         End Sub\n");
    assert_eq!(snap[0], Variant::from_string(BStr::from("7  ")));
}

#[test]
fn fixed_length_string_store_rejects_null() {
    let err = run_result("Sub Main()\nDim sample As String * 3\nsample = Null\nEnd Sub")
        .expect_err("fixed-length String assignment from Null should be a runtime error");
    assert!(
        err.contains("runtime error: 13"),
        "expected type mismatch error 13, got: {err}"
    );
}

#[test]
fn scalar_deftype_string_defaults_affect_variables_params_and_returns() {
    let snap = run("DefStr M, P, R\n\
         Public outLocal As Variant\n\
         Public outParam As Variant\n\
         Public outFunction As Variant\n\
         Sub Main()\n\
             Dim message\n\
             message = 7\n\
             outLocal = message\n\
             Take 9\n\
             outFunction = ResultValue()\n\
         End Sub\n\
         Sub Take(ByVal payload)\n\
             outParam = payload\n\
         End Sub\n\
         Function ResultValue()\n\
             ResultValue = 8\n\
         End Function\n");
    assert_eq!(snap[0], Variant::from_string(BStr::from("7")));
    assert_eq!(snap[1], Variant::from_string(BStr::from("9")));
    assert_eq!(snap[2], Variant::from_string(BStr::from("8")));
}

#[test]
fn scalar_untyped_string_const_expression_executes() {
    let snap = run(
        "Const Prefix = \"re\"\nConst CText = Prefix & \"ady\"\nSub Main()\nDim text\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("ready"))]);
}

#[test]
fn scalar_untyped_string_const_scalar_concat_expression_executes() {
    let snap = run(
        "Const Prefix = \"v\"\nConst CNumber = 7\nConst CFlag = True\nConst CText = Prefix & CNumber & CFlag\nSub Main()\nDim text\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("v7True"))]);
}

#[test]
fn scalar_currency_date_const_carriers_execute() {
    let snap = run(
        "Const CAmount As Currency = 1.25@\nConst CStamp As Date = #2026-02-28#\nSub Main()\nDim amount As Currency\nDim stamp As Date\namount = CAmount\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_currency_scaled_i64(12_500),
            Variant::from_date_f64(46_081.0),
        ]
    );
}

#[test]
fn scalar_month_name_date_const_carrier_executes() {
    let snap = run(
        "Const CStamp As Date = #February 28, 2026#, CNext As Date = CStamp + 1\nSub Main()\nDim stamp As Date\nDim nextStamp As Date\nstamp = CStamp\nnextStamp = CNext\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_date_f64(46_081.0),
            Variant::from_date_f64(46_082.0),
        ]
    );
}

#[test]
fn scalar_numeric_month_day_date_const_carrier_executes() {
    let snap = run(
        "Const CStamp As Date = #2/28/2026#\nSub Main()\nDim stamp As Date\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_date_f64(46_081.0)]);
}

#[test]
fn scalar_ambiguous_numeric_date_literal_rejected() {
    let err = run_result("Sub Main()\nDim stamp As Date\nstamp = #2/3/2026#\nEnd Sub")
        .expect_err("ambiguous numeric Date literal should be rejected");
    assert!(
        err.contains("date literal `#2/3/2026#`"),
        "unexpected error: {err}"
    );
}

#[test]
fn scalar_string_date_store_coerces_deterministic_text() {
    let snap = run("Sub Main()\nDim stamp As Date\nstamp = \"2026-02-28\"\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_date_f64(46_081.0)]);
}

#[test]
fn scalar_ambiguous_string_date_store_rejected() {
    let err = run_result("Sub Main()\nDim stamp As Date\nstamp = \"2/3/2026\"\nEnd Sub")
        .expect_err("ambiguous numeric Date text should be rejected");
    assert!(err.contains("Type mismatch"), "unexpected error: {err}");
}

#[test]
fn scalar_untyped_date_const_carrier_executes() {
    let snap =
        run("Const CStamp = #2026-02-28#\nSub Main()\nDim stamp As Date\nstamp = CStamp\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_date_f64(46_081.0)]);
}

#[test]
fn scalar_currency_date_const_expression_carriers_execute() {
    let snap = run(
        "Const CAmount As Currency = 1.25@ * 2@ - 1.0@\nConst CStamp As Date = #2026-02-28# + 1\nSub Main()\nDim amount As Currency\nDim stamp As Date\namount = CAmount\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_currency_scaled_i64(15_000),
            Variant::from_date_f64(46_082.0),
        ]
    );
}

#[test]
fn scalar_double_arithmetic() {
    let snap = run("Sub Main()\nDim d As Double\nd = 1.5\nd = d * 2.0\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_f64(3.0)]);
}

#[test]
fn integer_division_and_mod() {
    // `\` integer division and `Mod`.
    let snap = run("Sub Main()\nDim a As Long\nDim b As Long\na = 17 \\ 5\nb = 17 Mod 5\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_i32(3), Variant::from_i32(2)]);
}

#[test]
fn boolean_and_or_in_condition() {
    // VM evaluation of And/Or through the supported branch-predicate path.
    // (Logical operators as assignment rvalues are a separate compiler-lowering
    // gap tracked outside this VM coverage suite.)
    let snap = run(
        "Sub Main()\nDim a As Boolean\nDim b As Boolean\nDim andRes As Long\nDim orRes As Long\na = True\nb = False\nIf a And b Then\nandRes = 1\nElse\nandRes = 0\nEnd If\nIf a Or b Then\norRes = 1\nElse\norRes = 0\nEnd If\nEnd Sub",
    );
    // a And b = False -> andRes = 0 ; a Or b = True -> orRes = 1
    assert!(
        snap.contains(&Variant::from_i32(0)) && snap.contains(&Variant::from_i32(1)),
        "expected andRes=0 and orRes=1 in {snap:?}"
    );
}

#[test]
fn string_concat_and_len() {
    let snap =
        run("Sub Main()\nDim s As String\nDim n As Long\ns = \"ab\" & \"cd\"\nn = Len(s)\nEnd Sub");
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("abcd")),
            Variant::from_i32(4)
        ]
    );
}

#[test]
fn optional_string_boolean_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim s As String\nDim b As Boolean\nCall Fill(s, b)\nEnd Sub\nSub Fill(ByRef target As String, ByRef flagTarget As Boolean, Optional ByVal text As String = \"ready\", Optional ByVal flag As Boolean = True)\ntarget = text\nflagTarget = flag\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("ready")),
            Variant::from_bool(true),
        ]
    );
}

#[test]
fn optional_string_boolean_module_constant_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const CText = \"ready\"\nConst CFlag = True\nSub Main()\nDim s As String\nDim b As Boolean\nCall Fill(s, b)\nEnd Sub\nSub Fill(ByRef target As String, ByRef flagTarget As Boolean, Optional ByVal text As String = CText, Optional ByVal flag As Boolean = CFlag)\ntarget = text\nflagTarget = flag\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("ready")),
            Variant::from_bool(true),
        ]
    );
}

#[test]
fn optional_string_concat_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Prefix = \"re\"\nSub Main()\nDim s As String\nCall Fill(s)\nEnd Sub\nSub Fill(ByRef target As String, Optional ByVal text As String = Prefix & \"ady\")\ntarget = text\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("ready"))]);
}

#[test]
fn optional_string_scalar_concat_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Prefix = \"v\"\nConst CNumber = 7\nConst CFlag = True\nSub Main()\nDim s As String\nCall Fill(s)\nEnd Sub\nSub Fill(ByRef target As String, Optional ByVal text As String = Prefix & CNumber & CFlag)\ntarget = text\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("v7True"))]);
}

#[test]
fn optional_boolean_expression_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Enabled = True\nSub Main()\nDim b As Boolean\nCall Fill(b)\nEnd Sub\nSub Fill(ByRef target As Boolean, Optional ByVal flag As Boolean = Enabled = Not False And 2 > 1)\ntarget = flag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn optional_boolean_like_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Prefix = \"he\"\nSub Main()\nDim b As Boolean\nCall Fill(b)\nEnd Sub\nSub Fill(ByRef target As Boolean, Optional ByVal flag As Boolean = Prefix & \"llo\" Like \"hello\")\ntarget = flag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn optional_enum_member_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Public Enum EncodingMode\nStrictUrlEncoding = 0\nFormUrlEncoding = 1\nEnd Enum\nSub Main()\nDim n As Long\nCall Fill(n)\nEnd Sub\nSub Fill(ByRef target As Long, Optional ByVal mode As EncodingMode = EncodingMode.FormUrlEncoding)\ntarget = mode\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i32(1)]);
}

#[test]
fn optional_typed_declared_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim s As String\nDim b As Boolean\nDim n As Long\nCall Fill(s, b, n)\nEnd Sub\nSub Fill(ByRef target As String, ByRef flagTarget As Boolean, ByRef numberTarget As Long, Optional ByVal text As String, Optional ByVal flag As Boolean, Optional ByVal value As Long)\ntarget = text\nflagTarget = flag\nnumberTarget = value\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("")),
            Variant::from_bool(false),
            Variant::from_i32(0),
        ]
    );
}

#[test]
fn optional_single_default_is_bound_as_single_for_omitted_arg() {
    let snap = run(
        "Sub Main()\nDim value As Variant\nCall Fill(value)\nEnd Sub\nSub Fill(ByRef target As Variant, Optional ByVal value As Single = 1.5!)\ntarget = value\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_f32(1.5)]);
}

#[test]
fn optional_string_to_typed_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim n As Long\nDim b As Boolean\nDim stamp As Date\nDim amount As Currency\nCall Fill(n, b, stamp, amount)\nEnd Sub\nSub Fill(ByRef nTarget As Long, ByRef bTarget As Boolean, ByRef stampTarget As Date, ByRef amountTarget As Currency, Optional ByVal n As Long = \"7\", Optional ByVal b As Boolean = \"False\", Optional ByVal stamp As Date = \"2026-02-28\", Optional ByVal amount As Currency = \"1.25\")\nnTarget = n\nbTarget = b\nstampTarget = stamp\namountTarget = amount\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_i32(7),
            Variant::from_bool(false),
            Variant::from_date_f64(46_081.0),
            Variant::from_currency_scaled_i64(12_500),
        ]
    );
}

#[test]
fn optional_longlong_module_constant_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Big As LongLong = 5000000000\nSub Main()\nDim n As LongLong\nCall Fill(n)\nEnd Sub\nSub Fill(ByRef target As LongLong, Optional ByVal value As LongLong = Big + 7)\ntarget = value\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i64(5_000_000_007)]);
}

#[test]
fn optional_date_currency_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim amount As Variant\nDim stamp As Variant\nDim literalStamp As Variant\nDim numericStamp As Variant\nDim blankAmount As Variant\nDim blankStamp As Variant\nCall Fill(amount, stamp, literalStamp, numericStamp, blankAmount, blankStamp)\nEnd Sub\nSub Fill(ByRef amountTarget As Variant, ByRef stampTarget As Variant, ByRef literalStampTarget As Variant, ByRef numericStampTarget As Variant, ByRef blankAmountTarget As Variant, ByRef blankStampTarget As Variant, Optional ByVal amount As Currency = 1.25@ * 2@ - 1.0@, Optional ByVal stamp As Date = (2.0 + 3.0) / 2.0, Optional ByVal literalStamp As Date = #2026-02-28#, Optional ByVal numericStamp As Date = #2/28/2026#, Optional ByVal blankAmount As Currency, Optional ByVal blankStamp As Date)\namountTarget = amount\nstampTarget = stamp\nliteralStampTarget = literalStamp\nnumericStampTarget = numericStamp\nblankAmountTarget = blankAmount\nblankStampTarget = blankStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_currency_scaled_i64(15_000),
            Variant::from_date_f64(2.5),
            Variant::from_date_f64(46_081.0),
            Variant::from_date_f64(46_081.0),
            Variant::from_currency_scaled_i64(0),
            Variant::from_date_f64(0.0),
        ]
    );
}

#[test]
fn indexed_property_get_executes_through_package_vm() {
    let snap = run(
        "Sub Main()\nDim value As Long\nvalue = Item(4)\nEnd Sub\nProperty Get Item(ByVal index As Long) As Long\nItem = index + 3\nEnd Property",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
}

#[test]
fn indexed_property_let_executes_through_package_vm() {
    let snap = run(
        "Sub Main()\nDim value As Long\nItem(value) = 3\nEnd Sub\nProperty Let Item(ByRef target As Long, ByVal newValue As Long)\ntarget = newValue + 4\nEnd Property",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
}

#[test]
fn named_indexed_property_let_executes_through_package_vm() {
    let snap = run(
        "Sub Main()\nDim value As Long\nItem(target := value) = 3\nEnd Sub\nProperty Let Item(ByRef target As Long, ByVal newValue As Long)\ntarget = newValue + 4\nEnd Property",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
}

#[test]
fn string_functions_left_mid_ucase() {
    let snap = run(
        "Sub Main()\nDim a As String\nDim b As String\na = Left$(\"hello\", 3)\nb = UCase$(Mid$(\"hello\", 2, 2))\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("hel")),
            Variant::from_string(BStr::from("EL")),
        ]
    );
}

#[test]
fn fixed_array_index_assign_read() {
    let snap = run(
        "Sub Main()\nDim a(1 To 3) As Long\nDim total As Long\na(1) = 10\na(2) = 20\na(3) = 30\ntotal = a(1) + a(2) + a(3)\nEnd Sub",
    );
    // total is the last user slot.
    assert_eq!(snap.last(), Some(&Variant::from_i32(60)));
}

#[test]
fn dynamic_array_redim_and_use() {
    let snap = run(
        "Sub Main()\nDim a() As Long\nDim v As Long\nReDim a(2)\na(0) = 7\na(2) = 5\nv = a(0) + a(2)\nEnd Sub",
    );
    assert_eq!(snap.last(), Some(&Variant::from_i32(12)));
}

#[test]
fn fixed_array_dim_in_loop_is_hoisted() {
    // VBA hoists declarations: a fixed-size array `Dim`'d inside a loop is allocated
    // once at proc entry, so element `a(1)` accumulates across iterations (3) rather
    // than resetting to 0 each pass.
    let snap = run(
        "Sub Main()\nDim total As Long\nDim i As Long\nFor i = 1 To 3\nDim a(1 To 2) As Long\na(1) = a(1) + 1\nNext i\ntotal = a(1)\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_i32(3)),
        "expected accumulated a(1)=3 in {snap:?}"
    );
}

#[test]
fn module_level_fixed_array_global_is_allocated() {
    // A module-level `Dim g(1 To 3)` global is allocated at program entry, before the
    // entry body runs.
    let snap = run(
        "Dim g(1 To 3) As Long\nSub Main()\ng(1) = 10\ng(2) = 20\nDim total As Long\ntotal = g(1) + g(2)\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_i32(30)),
        "expected g(1)+g(2)=30 in {snap:?}"
    );
}

#[test]
fn for_loop_accumulator() {
    let snap = run(
        "Sub Main()\nDim i As Long\nDim sum As Long\nsum = 0\nFor i = 1 To 5\nsum = sum + i\nNext i\nEnd Sub",
    );
    // sum = 15
    assert!(
        snap.contains(&Variant::from_i32(15)),
        "expected sum=15 in {snap:?}"
    );
}

#[test]
fn currency_cstr_keeps_sign_below_one() {
    // W1-runtime-001: the sign of a Currency in (-1, 0) lives only in the
    // integer part under truncating division and used to vanish.
    let snap = run("Sub Main()\nDim s As String\ns = CStr(CCur(-0.5))\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_string("-0.5")),
        "expected \"-0.5\" in {snap:?}"
    );
}

#[test]
fn static_local_persists_across_calls() {
    // A `Static` local is default-initialized once and persists, so the
    // accumulator reaches 3 over three calls instead of resetting to 1.
    let snap = run(
        "Sub Main()\nDim total As Long\nDim i As Long\nFor i = 1 To 3\ntotal = Accumulate()\nNext i\nEnd Sub\n\
         Function Accumulate() As Long\nStatic n As Long\nn = n + 1\nAccumulate = n\nEnd Function",
    );
    assert!(
        snap.contains(&Variant::from_i32(3)),
        "expected total=3 (persisted, not reset to 1) in {snap:?}"
    );
}

#[test]
fn static_proc_makes_all_locals_static() {
    // `Static Function` makes every local persist, even without its own
    // `Static` keyword.
    let snap = run(
        "Sub Main()\nDim r As Long\nDim i As Long\nFor i = 1 To 3\nr = Tick()\nNext i\nEnd Sub\n\
         Static Function Tick() As Long\nDim n As Long\nn = n + 1\nTick = n\nEnd Function",
    );
    assert!(
        snap.contains(&Variant::from_i32(3)),
        "expected r=3 (Static Function persists its locals) in {snap:?}"
    );
}

#[test]
fn static_array_persists_and_allocates_once() {
    // A `Static` fixed-size array allocates once at program entry and persists
    // across calls; it must not be re-allocated (reset) per call.
    let snap = run(
        "Sub Main()\nDim r As Long\nDim i As Long\nFor i = 1 To 4\nr = Push(i)\nNext i\nEnd Sub\n\
         Function Push(ByVal v As Long) As Long\nStatic a(1 To 3) As Long\nStatic count As Long\n\
         count = count + 1\na(((count - 1) Mod 3) + 1) = v\nPush = a(1) + a(2) + a(3)\nEnd Function",
    );
    // Calls push 1,2,3,4 into a 3-slot ring → a = [4,2,3], last sum = 9.
    assert!(
        snap.contains(&Variant::from_i32(9)),
        "expected the ring-buffer sum 9 (array persisted) in {snap:?}"
    );
}

#[test]
fn static_local_shadows_module_global() {
    // A proc's `Static` local shadows a same-named module global inside the
    // proc, while the module global keeps its own value.
    let snap = run(
        "Public n As Long\nSub Main()\nDim r As Long\nn = 100\nr = Bump()\nr = Bump()\nEnd Sub\n\
         Function Bump() As Long\nStatic n As Long\nn = n + 1\nBump = n\nEnd Function",
    );
    assert!(
        snap.contains(&Variant::from_i32(100)),
        "module global n must stay 100 in {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_i32(2)),
        "the static local must reach 2 over two calls in {snap:?}"
    );
}

#[test]
fn for_loop_negative_step_counts_down() {
    let snap = run(
        "Sub Main()\nDim i As Long\nDim sum As Long\nDim last As Long\nsum = 0\nFor i = 5 To 1 Step -1\nsum = sum + i\nlast = i\nNext i\nEnd Sub",
    );
    // 5+4+3+2+1 = 15; the last body iteration sees i = 1.
    assert!(
        snap.contains(&Variant::from_i32(15)),
        "expected sum=15 in {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_i32(1)),
        "expected last=1 in {snap:?}"
    );
}

#[test]
fn for_loop_negative_step_two_skips() {
    let snap = run(
        "Sub Main()\nDim i As Long\nDim c As Long\nc = 0\nFor i = 10 To 0 Step -2\nc = c + 1\nNext i\nEnd Sub",
    );
    // i = 10,8,6,4,2,0 -> 6 iterations.
    assert!(
        snap.contains(&Variant::from_i32(6)),
        "expected c=6 in {snap:?}"
    );
}

#[test]
fn for_loop_runtime_negative_step() {
    let snap = run(
        "Sub Main()\nDim i As Long\nDim s As Long\nDim c As Long\ns = -10\nc = 0\nFor i = 30 To 10 Step s\nc = c + 1\nNext i\nEnd Sub",
    );
    // The step is only known at run time: i = 30,20,10 -> 3 iterations.
    assert!(
        snap.contains(&Variant::from_i32(3)),
        "expected c=3 in {snap:?}"
    );
}

#[test]
fn for_loop_negative_step_empty_range() {
    let snap = run(
        "Sub Main()\nDim i As Long\nDim c As Long\nc = 9\nFor i = 1 To 5 Step -1\nc = c + 1\nNext i\nEnd Sub",
    );
    // Descending step with an ascending range runs zero iterations.
    assert!(
        snap.contains(&Variant::from_i32(9)),
        "expected c=9 in {snap:?}"
    );
}

#[test]
fn while_loop_countdown() {
    let snap = run(
        "Sub Main()\nDim n As Long\nDim steps As Long\nn = 3\nsteps = 0\nDo While n > 0\nn = n - 1\nsteps = steps + 1\nLoop\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_i32(3)),
        "expected steps=3 in {snap:?}"
    );
}

#[test]
fn if_elseif_else_branch() {
    let snap = run(
        "Sub Main()\nDim x As Long\nDim label As String\nx = 5\nIf x < 0 Then\nlabel = \"neg\"\nElseIf x = 0 Then\nlabel = \"zero\"\nElse\nlabel = \"pos\"\nEnd If\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_string(BStr::from("pos"))),
        "expected label=pos in {snap:?}"
    );
}

#[test]
fn udt_field_assign_and_read() {
    let snap = run("Type Point\nX As Long\nY As Long\nEnd Type\n\
         Sub Main()\nDim p As Point\nDim s As Long\np.X = 3\np.Y = 4\ns = p.X + p.Y\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i32(7)),
        "expected p.X+p.Y=7 in {snap:?}"
    );
}

#[test]
fn udt_whole_copy_independence() {
    // Copying a UDT must be by value: mutating the copy must not affect the source.
    let snap = run("Type Pair\nA As Long\nB As Long\nEnd Type\n\
         Sub Main()\nDim p As Pair\nDim q As Pair\nDim srcA As Long\np.A = 1\np.B = 2\nq = p\nq.A = 99\nsrcA = p.A\nEnd Sub");
    // p.A must remain 1 after q.A = 99.
    assert!(
        snap.contains(&Variant::from_i32(1)),
        "expected source p.A=1 preserved in {snap:?}"
    );
}

#[test]
fn udt_passed_byref_writes_through_byval_copies() {
    // A UDT generalizes through calls: a ByRef parameter writes a field back to the
    // caller's record; a ByVal parameter mutates an independent copy.
    let snap = run("Type P\nV As Long\nEnd Type\n\
         Sub Main()\nDim a As P\nDim afterRef As Long\nDim afterVal As Long\n\
         a.V = 1\nBumpRef a\nafterRef = a.V\nTouchVal a\nafterVal = a.V\nEnd Sub\n\
         Sub BumpRef(ByRef x As P)\nx.V = x.V + 1\nEnd Sub\n\
         Sub TouchVal(ByVal x As P)\nx.V = 99\nEnd Sub");
    // ByRef bumped a.V to 2; ByVal left it at 2 (its 99 stayed in the local copy).
    assert!(
        snap.contains(&Variant::from_i32(2)) && !snap.contains(&Variant::from_i32(99)),
        "expected a.V=2 after ByRef + unchanged by ByVal in {snap:?}"
    );
}

#[test]
fn on_error_resume_next_division_by_zero() {
    // Division by zero under On Error Resume Next: Err.Number set, execution continues.
    let snap = run(
        "Sub Main()\nDim r As Double\nDim afterErr As Long\nOn Error Resume Next\nr = 1 / 0\nafterErr = Err.Number\nEnd Sub",
    );
    // afterErr should be the division-by-zero error number (11).
    assert!(
        snap.contains(&Variant::from_i32(11)),
        "expected Err.Number=11 after div-by-zero in {snap:?}"
    );
}

#[test]
fn logical_operators_as_rvalues() {
    // And/Or/Not used as value-producing expressions (not just branch predicates).
    let snap = run(
        "Sub Main()\nDim a As Boolean\nDim b As Boolean\nDim andRes As Boolean\nDim orRes As Boolean\nDim notRes As Boolean\na = True\nb = False\nandRes = a And b\norRes = a Or b\nnotRes = Not a\nEnd Sub",
    );
    // a=True, b=False -> andRes=False, orRes=True, notRes=False
    assert_eq!(
        snap,
        vec![
            Variant::from_bool(true),
            Variant::from_bool(false),
            Variant::from_bool(false),
            Variant::from_bool(true),
            Variant::from_bool(false),
        ]
    );
}

#[test]
fn type_suffix_numeric_literals() {
    // VBA type-suffix literals: # Double, & Long.
    let snap = run("Sub Main()\nDim d As Double\nDim n As Long\nd = 2# * 1.5\nn = 100&\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_f64(3.0)),
        "expected 2# * 1.5 = 3.0 in {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_i32(100)),
        "expected 100& = 100 in {snap:?}"
    );
}

/// Asserts a snippet raises VBA run-time error 6 ("Overflow"), per the Excel oracle (bd-0d1y).
fn expect_overflow(source: &str) {
    let err = run_result(source).expect_err("expected VBA overflow error 6");
    assert!(
        err.contains("runtime error: 6"),
        "expected overflow error 6, got: {err}"
    );
}

#[test]
fn overflow_fixed_integer_assignment_raises_error_6() {
    // Overflow into a declared fixed-integer target is error 6 (Excel-oracle confirmed).
    expect_overflow("Sub Main()\nDim x As Long\nx = 2000000000\nx = x + 2000000000\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Integer\nx = 32767\nx = x + 1\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Integer\nx = -32768\nx = x - 1\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Byte\nx = 200\nx = x + 100\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Long\nx = 50000\nx = x * 50000\nEnd Sub");
}

#[test]
fn overflow_fixed_integer_expression_raises_error_6() {
    // Fixed-type arithmetic overflow errors at the operation even when the result flows into a
    // Variant (no widening), including intermediate overflow inside a larger expression.
    expect_overflow("Sub Main()\nDim ai As Integer\nDim r\nai = 32767\nr = ai + 1\nEnd Sub");
    expect_overflow("Sub Main()\nDim ai As Integer\nDim r\nai = -32768\nr = -ai\nEnd Sub");
    expect_overflow("Sub Main()\nDim al As Long\nDim r\nal = 2000000000\nr = al + al\nEnd Sub");
    expect_overflow(
        "Sub Main()\nDim al As Long\nDim r\nal = 2000000000\nr = (al + al) Mod 7\nEnd Sub",
    );
}

#[test]
fn overflow_variant_operands_widen_instead_of_erroring() {
    // Variant operands widen on overflow (Integer->Long->Double); no error.
    let snap = run("Sub Main()\nDim v\nDim r\nv = 2000000000\nr = v + v\nEnd Sub");
    assert!(
        snap.iter().any(|x| x.as_f64() == Some(4_000_000_000.0)),
        "expected widened Double 4e9 in {snap:?}"
    );
    let snap = run("Sub Main()\nDim v\nDim r\nv = 50000\nr = v * 50000\nEnd Sub");
    assert!(
        snap.iter().any(|x| x.as_f64() == Some(2_500_000_000.0)),
        "expected widened Double 2.5e9 in {snap:?}"
    );
}

#[test]
fn longlong_multiplication_is_exact() {
    // 64-bit integer arithmetic is computed in i64, not through f64 — so a product
    // beyond 2^53 keeps every bit (the Double-biased VM ops would have lost the low
    // digits). 1000000001^2 = 1000000002000000001 (< i64::MAX), exactly.
    let snap =
        run("Sub Main()\nDim a As LongLong\nDim b As LongLong\na = 1000000001\nb = a * a\nEnd Sub");
    assert_eq!(
        snap,
        vec![
            Variant::from_i64(1_000_000_001),
            Variant::from_i64(1_000_000_002_000_000_001)
        ]
    );
}

#[test]
fn longlong_multiplication_overflow_raises_error_6() {
    // A LongLong product that leaves i64's range is Overflow (error 6), not a silent
    // widen — fixed-typed 64-bit arithmetic is checked.
    let err = run_result("Sub Main()\nDim a As LongLong\na = 4000000000\na = a * a\nEnd Sub")
        .expect_err("expected LongLong overflow");
    assert!(
        err.contains("runtime error: 6"),
        "expected overflow error 6, got: {err}"
    );
}

#[test]
fn fixed_integer_arithmetic_in_range_does_not_error() {
    // In-range fixed-integer arithmetic is not flagged, and the result keeps the
    // promoted fixed type: `Integer + Integer` → Integer (the same typing that makes
    // out-of-range `Integer + Integer` overflow), not the legacy VM's widened Long.
    let snap = run("Sub Main()\nDim ai As Integer\nDim r\nai = 100\nr = ai + 1\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i16(101)),
        "expected Integer 101 in {snap:?}"
    );
    // Byte + Integer literal promotes to Integer (300 fits), so this does not overflow.
    let snap = run("Sub Main()\nDim ab As Byte\nDim r\nab = 200\nr = ab + 100\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i16(300)),
        "expected Integer 300 (Byte+Integer promotes, in range) in {snap:?}"
    );
}

#[test]
fn logical_operator_precedence_with_comparison() {
    // Comparison binds tighter than And/Or: `x > 0 And x < 10`.
    let snap = run(
        "Sub Main()\nDim x As Long\nDim inRange As Boolean\nx = 5\ninRange = x > 0 And x < 10\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_bool(true)),
        "expected inRange=True for x=5 in {snap:?}"
    );
}

// ── Pointer helpers (VarPtr / StrPtr / ObjPtr) ──
// These bind to `CoreValue::Ptr` and execute the `Op::Ptr*` ops, materializing a
// pinned pointer via the runtime pointer registry. The address is non-deterministic,
// so each test asserts a non-zero `LongPtr` (and that binding no longer rejects the
// `Structural(...)` route). The runtime layer's exact pinning is unit-tested in
// oxvba-runtime/src/pointer_helpers.rs.

/// The last snapshot slot (the most-recently declared local) as a non-zero pointer.
fn assert_nonzero_pointer(snap: &[Variant], context: &str) {
    let ptr = snap.last().and_then(Variant::as_i64);
    assert!(
        matches!(ptr, Some(p) if p != 0),
        "{context}: expected a non-zero LongPtr, got {:?} in {snap:?}",
        snap.last()
    );
}

#[test]
fn strptr_of_string_yields_nonzero_pointer() {
    let snap = run("Sub Main()\nDim s As String\nDim p\ns = \"abc\"\np = StrPtr(s)\nEnd Sub");
    assert_nonzero_pointer(&snap, "StrPtr(String)");
}

#[test]
fn varptr_of_scalar_yields_nonzero_pointer() {
    let snap = run("Sub Main()\nDim n As Long\nDim p\nn = 42\np = VarPtr(n)\nEnd Sub");
    assert_nonzero_pointer(&snap, "VarPtr(Long)");
}

#[test]
fn varptr_of_string_variable_yields_nonzero_pointer() {
    // A String variable routes through `PtrKind::VarString` (the BSTR cell).
    let snap = run("Sub Main()\nDim s As String\nDim p\ns = \"abc\"\np = VarPtr(s)\nEnd Sub");
    assert_nonzero_pointer(&snap, "VarPtr(String var)");
}

#[test]
fn varptr_of_variant_variable_yields_nonzero_pointer() {
    // A Variant variable routes through `PtrKind::VarVariant` (the VARIANT cell).
    let snap = run("Sub Main()\nDim v\nDim p\nv = 7\np = VarPtr(v)\nEnd Sub");
    assert_nonzero_pointer(&snap, "VarPtr(Variant var)");
}

#[test]
fn strptr_of_string_literal_yields_nonzero_pointer() {
    // `StrPtr` of an r-value literal (not an l-value) binds and pins a temporary.
    let snap = run("Sub Main()\nDim p\np = StrPtr(\"alpha\")\nEnd Sub");
    assert_nonzero_pointer(&snap, "StrPtr(literal)");
}

// ── Conditional compilation (#If) ──
// The predefined `Win64` constant is true on the 64-bit runtime, so the active
// branch compiles and runs; the inactive branch is stripped before parse.

#[test]
fn conditional_compilation_selects_win64_branch() {
    let snap =
        run("Sub Main()\nDim x As Long\n#If Win64 Then\nx = 64\n#Else\nx = 32\n#End If\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_i32(64)]);
}

#[test]
fn conditional_compilation_does_not_compile_inactive_branch() {
    // The inactive `#If Mac` branch references an unresolved name + bad arity; it
    // must be stripped (not compiled), so only `x = 7` runs.
    let snap = run(
        "Sub Main()\nDim x As Long\n#If Mac Then\nx = NoSuchFunction(1, 2, 3)\n#Else\nx = 7\n#End If\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
}
