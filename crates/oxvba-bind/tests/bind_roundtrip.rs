//! End-to-end: real VBA source → `oxvba_bind::bind_program` → `oxvba_oxir::elaborate`
//! → run on `oxvba-vm3`. This is the "tie the path together" proof — the whole
//! clean pipeline exercised from source text.

use std::collections::BTreeMap;

use oxvba_bind::bind_program;
use oxvba_bundle::DeclareParamType;
use oxvba_bundle::coreir::{CoreArg, CoreCallee, CoreProgram, CoreStmt, CoreValue};
use oxvba_bundle::native::NativeImplId;
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
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

fn manifest(source: &str) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Mod1".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Mod1"),
            source: source.into(),
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn manifest_modules(modules: &[(&str, ModuleKind, &str)]) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: modules
            .iter()
            .map(|(name, kind, source)| ModuleUnit {
                module_name: (*name).into(),
                module_kind: *kind,
                attributes: ModuleAttributes::named(*name),
                source: (*source).into(),
            })
            .collect(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn bind(source: &str) -> CoreProgram {
    bind_program(&manifest(source), &NullTypeLibs).expect("bind_program")
}

fn bind_error(source: &str) -> String {
    format!(
        "{:?}",
        bind_program(&manifest(source), &NullTypeLibs).expect_err("bind should fail")
    )
}

fn bind_error_display(source: &str) -> String {
    bind_program(&manifest(source), &NullTypeLibs)
        .expect_err("bind should fail")
        .to_string()
}

/// Bind + elaborate + run on vm3; read `Main`'s first local as a number.
fn run_main_local0(source: &str) -> Option<f64> {
    let program = bind(source);
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    let value = vm.slot(oxp.globals.len())?;
    value
        .as_f64()
        .or_else(|| value.as_f32().map(f64::from))
        .or_else(|| value.as_i16().map(f64::from))
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
        .or_else(|| value.as_currency_scaled_i64().map(|v| v as f64 / 10_000.0))
        .or_else(|| value.as_date_f64())
        .or_else(|| value.as_bool().map(|b| if b { -1.0 } else { 0.0 }))
}

/// Wrap a `Main` body that leaves its result in the first declared local.
fn main_sub(body: &str) -> String {
    format!("Sub Main()\n{body}End Sub\n")
}

// ── Arithmetic / coercion / operators ────────────────────────────────────────

#[test]
fn arithmetic_precedence() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = 1 + 2 * 3\n")),
        Some(7.0)
    );
}

fn run_main_local0_string(source: &str) -> Option<String> {
    let program = bind(source);
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    vm.slot(oxp.globals.len())?.as_bstr().map(|b| b.as_str())
}

#[test]
fn fixed_length_string_pads_on_assignment() {
    // `Dim s As String * 5` pads a shorter assignment with spaces.
    assert_eq!(
        run_main_local0_string("Sub Main()\n    Dim s As String * 5\n    s = \"ab\"\nEnd Sub\n"),
        Some("ab   ".to_string())
    );
}

#[test]
fn fixed_length_string_truncates_on_assignment() {
    assert_eq!(
        run_main_local0_string("Sub Main()\n    Dim s As String * 2\n    s = \"abcd\"\nEnd Sub\n"),
        Some("ab".to_string())
    );
}

#[test]
fn lset_rset_reject_non_string_targets_with_vba_messages() {
    let lset = bind_error_display("Sub Main()\n    Dim n As Long\n    LSet n = \"12\"\nEnd Sub\n");
    assert!(
        lset.contains("LSet allowed only on strings and user-defined types"),
        "expected VBA LSet target diagnostic, got {lset}"
    );
    let rset = bind_error_display("Sub Main()\n    Dim n As Long\n    RSet n = \"12\"\nEnd Sub\n");
    assert!(
        rset.contains("RSet allowed only on strings"),
        "expected VBA RSet target diagnostic, got {rset}"
    );
}

#[test]
fn lset_udt_record_copy_lowers_to_record_statement() {
    let program = bind(
        "Private Type A\n    X As String * 2\nEnd Type\n\
         Private Type B\n    X As String * 2\nEnd Type\n\
         Sub Main()\n    Dim a As A\n    Dim b As B\n    LSet a = b\nEnd Sub\n",
    );
    assert!(
        program.procs[0]
            .body
            .iter()
            .any(|stmt| matches!(stmt, CoreStmt::LSetRecord { .. })),
        "expected UDT LSet to lower as CoreStmt::LSetRecord, got {:?}",
        program.procs[0].body
    );
}

#[test]
fn lset_udt_record_copy_rejects_vba_type_mismatch_cases() {
    let non_record_rhs = bind_error_display(
        "Private Type A\n    X As String * 2\nEnd Type\n\
         Sub Main()\n    Dim a As A\n    LSet a = \"xy\"\nEnd Sub\n",
    );
    assert!(
        non_record_rhs.contains("Type mismatch"),
        "expected VBA Type mismatch, got {non_record_rhs}"
    );

    let owning_field = bind_error_display(
        "Private Type A\n    S As String\nEnd Type\n\
         Private Type B\n    S As String\nEnd Type\n\
         Sub Main()\n    Dim a As A\n    Dim b As B\n    LSet a = b\nEnd Sub\n",
    );
    assert!(
        owning_field.contains("Type mismatch"),
        "expected VBA Type mismatch, got {owning_field}"
    );
}

#[test]
fn date_literal_assigns_serial() {
    // 2020-01-01 is OLE automation serial 43831.
    assert_eq!(
        run_main_local0(&main_sub("    Dim d As Date\n    d = #1/1/2020#\n")),
        Some(43831.0)
    );
}

#[test]
fn iif_selects_branch() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = IIf(2 > 1, 10, 20)\n")),
        Some(10.0)
    );
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = IIf(1 > 2, 10, 20)\n")),
        Some(20.0)
    );
}

#[test]
fn choose_is_one_based() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = Choose(2, 100, 200, 300)\n")),
        Some(200.0)
    );
}

#[test]
fn switch_returns_first_true() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = Switch(False, 1, True, 42)\n")),
        Some(42.0)
    );
}

#[test]
fn const_references_const() {
    let body = "    Dim r As Long\n    Const A As Long = 1\n    Const B As Long = A + 1\n    Const C As Long = B * 2\n    r = C\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(4.0));
}

#[test]
fn const_forward_reference_resolves() {
    // C depends on B depends on A, all declared after the use site.
    let body = "    Dim r As Long\n    Const C As Long = B * 2\n    Const B As Long = A + 1\n    Const A As Long = 1\n    r = C\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(4.0));
}

#[test]
fn const_cycle_is_error() {
    let body = "    Const A As Long = B\n    Const B As Long = A\n    Dim r As Long\n    r = A\n";
    let src = main_sub(body);
    assert!(bind_program(&manifest(&src), &NullTypeLibs).is_err());
}

#[test]
fn module_qualified_call() {
    // `Mod1.SetIt` — a standard-module-qualified call (the form the project startup
    // shim emits as `Call Module.Proc()`). The module name is a namespace qualifier,
    // not a value; resolve the member as a qualified project call.
    let src = "Sub Main()\n    Dim r As Long\n    Call Mod1.SetIt\n    r = total\nEnd Sub\n\
               Public total As Long\n\
               Sub SetIt()\n    total = 7\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(7.0));
}

#[test]
fn module_qualified_const_and_variable() {
    // `Mod1.K` (a qualified Public Const) and `Mod1.gShared` (a qualified module
    // variable) resolve as a value / a place — not a call (regression for the
    // qualified-member path that previously routed everything through a call).
    let src = "Sub Main()\n    Dim r As Long\n    gShared = 5\n    r = Mod1.K + Mod1.gShared\nEnd Sub\n\
               Public Const K As Long = 10\n\
               Public gShared As Long\n";
    assert_eq!(run_main_local0(src), Some(15.0));
}

#[test]
fn module_qualified_const_initializer_folds() {
    // A local `Const` initializer is folded in the symbol layer before binding;
    // qualified module constants must therefore resolve there too, not only in
    // ordinary expression binding.
    let src = "Sub Main()\n    Dim r As Long\n    Const X As Long = Mod1.K + 1\n    r = X\nEnd Sub\n\
               Public Const K As Long = 10\n";
    assert_eq!(run_main_local0(src), Some(11.0));
}

#[test]
fn module_qualified_const_initializer_respects_local_shadowing() {
    let src = "Sub Main()\n    Const Mod1 As Long = 1\n    Const X As Long = Mod1.K + 1\n    Dim r As Long\n    r = X\nEnd Sub\n\
               Public Const K As Long = 10\n";
    assert!(
        bind_program(&manifest(src), &NullTypeLibs).is_err(),
        "a local const named like the module must shadow the module qualifier"
    );
}

#[test]
fn enum_qualified_member_binds_as_constant_value() {
    let src = "Public Enum WebFormat\n  PlainText = 0\n  Json = 1\nEnd Enum\n\
               Sub Main()\n    Dim r As Long\n    r = WebFormat.Json\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(1.0));
}

#[test]
fn public_enum_member_binds_unqualified_as_constant_value() {
    let src = "Public Enum WebFormat\n  PlainText = 0\n  Json = 1\nEnd Enum\n\
               Sub Main()\n    Dim r As Long\n    r = PlainText\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(0.0));
}

#[test]
fn keyword_token_can_be_parameter_name() {
    let src = "Sub UseName(Name As String)\n    Dim r As Long\n    r = Len(Name)\nEnd Sub\n\
               Sub Main()\n    Dim r As Long\n    UseName \"abc\"\n    r = 1\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(1.0));
}

#[test]
fn native_intrinsic_named_argument_reorders_to_parameter_slot() {
    assert_eq!(
        run_main_local0_string(&main_sub(
            "    Dim s As String\n    s = Replace(\"a?a?\", \"?\", \"\", Count:=1)\n"
        )),
        Some("aa?".to_string())
    );
}

#[test]
fn function_name_is_assignable_return_target_for_set() {
    let src = "Function MakeObject() As Object\n    Set MakeObject = Nothing\nEnd Function\n\
               Sub Main()\n    Dim r As Long\n    r = 1\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(1.0));
}

#[test]
fn indexed_function_result_binds_as_default_member_access() {
    let src = "Function Lookup() As Object\nEnd Function\n\
               Sub Main()\n    Dim value As Variant\n    value = Lookup()(\"MediaType\")\nEnd Sub\n";
    bind_program(&manifest(src), &NullTypeLibs).expect("bind indexed function result");
}

#[test]
fn mid_assignment_mutates_target_string() {
    assert_eq!(
        run_main_local0_string(&main_sub(
            "    Dim s As String\n    s = \"abcdef\"\n    Mid$(s, 3, 2) = \"XY\"\n"
        )),
        Some("abXYef".to_string())
    );
}

#[test]
fn mid_assignment_omitted_length_and_qualified_spelling_mutate_target_string() {
    assert_eq!(
        run_main_local0_string(&main_sub(
            "    Dim s As String\n    s = \"abcdef\"\n    VBA.Mid$(s, 4) = \"XYZ\"\n"
        )),
        Some("abcXYZ".to_string())
    );
}

#[test]
fn err_raise_accepts_foldable_error_number_expression() {
    let src = "Sub Main()\n    On Error Resume Next\n    Err.Raise 11099 + vbObjectError\n    Dim r As Long\n    r = Err.Number\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(-2147210405.0));
}

#[test]
fn err_raise_accepts_named_number_and_clear_resets_number() {
    let src = "Sub Main()\n    On Error Resume Next\n    Err.Raise Number:=12\n    Err.Clear\n    Dim r As Long\n    r = Err.Number\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(0.0));
}

#[test]
fn err_raise_accepts_dynamic_error_number_expression() {
    let src = "Sub Main()\n    On Error Resume Next\n    Dim n As Long\n    n = 7\n    Err.Raise n\n    Dim r As Long\n    r = Err.Number\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(7.0));
}

fn default_vba_help_file() -> &'static str {
    "C:\\Program Files\\Common Files\\Microsoft Shared\\VBA\\VBA7.1\\1033\\VbLR6.chm"
}

#[test]
fn err_help_fields_read_write_and_clear() {
    let src = "Sub Main()\n\
               Dim r As String\n\
               Err.HelpFile = \"help.chm\"\n\
               Err.HelpContext = 42\n\
               r = Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               Err.Clear\n\
               r = r & \";\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext) & \"|\" & CStr(Err.Number)\n\
               End Sub\n";
    assert_eq!(
        run_main_local0_string(src),
        Some("help.chm|42;|0|0".to_string())
    );
}

#[test]
fn err_help_defaults_for_error_statement() {
    let src = "Sub Main()\n\
               Dim r As String\n\
               On Error Resume Next\n\
               Error 9\n\
               r = Err.HelpFile & \"|\" & CStr(Err.HelpContext) & \"|\" & Err.Description\n\
               End Sub\n";
    assert_eq!(
        run_main_local0_string(src),
        Some(format!(
            "{}|1000009|Subscript out of range",
            default_vba_help_file()
        ))
    );
}

#[test]
fn err_raise_help_fields_explicit_and_named() {
    let src = "Sub Main()\n\
               Dim r As String\n\
               On Error Resume Next\n\
               Err.Raise 77, \"src\", \"desc\", \"help.chm\", 42\n\
               r = CStr(Err.Number) & \"|\" & Err.Description & \"|\" & Err.Source & \"|\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               Err.Clear\n\
               Err.Raise Number:=78, HelpContext:=43, HelpFile:=\"named.hlp\", Description:=\"desc2\", Source:=\"src2\"\n\
               r = r & \";\" & CStr(Err.Number) & \"|\" & Err.Description & \"|\" & Err.Source & \"|\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               End Sub\n";
    assert_eq!(
        run_main_local0_string(src),
        Some("77|desc|src|help.chm|42;78|desc2|src2|named.hlp|43".to_string())
    );
}

#[test]
fn err_raise_omitted_help_fields_inherit_when_err_state_is_inheritable() {
    let src = "Sub Main()\n\
               Dim r As String\n\
               On Error Resume Next\n\
               Err.Raise 5, \"prevsrc\", \"prevdesc\", \"prev.hlp\", 9\n\
               Err.Raise 79\n\
               r = CStr(Err.Number) & \"|\" & Err.Description & \"|\" & Err.Source & \"|\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               Err.Clear\n\
               Err.Description = \"prevdesc\"\n\
               Err.Source = \"prevsrc\"\n\
               Err.HelpFile = \"prev.hlp\"\n\
               Err.HelpContext = 9\n\
               Err.Raise 79\n\
               r = r & \";\" & CStr(Err.Number) & \"|\" & Err.Description & \"|\" & Err.Source & \"|\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               Err.Clear\n\
               Err.Raise 80\n\
               r = r & \";\" & CStr(Err.Number) & \"|\" & Err.Description & \"|\" & Err.Source & \"|\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               Err.Clear\n\
               Err.Raise 5, \"prevsrc\", \"prevdesc\", \"prev.hlp\", 9\n\
               Err.Raise 81, , , \"explicit.hlp\"\n\
               r = r & \";\" & CStr(Err.Number) & \"|\" & Err.Description & \"|\" & Err.Source & \"|\" & Err.HelpFile & \"|\" & CStr(Err.HelpContext)\n\
               End Sub\n";
    assert_eq!(
        run_main_local0_string(src),
        Some(
            format!(
                "79|prevdesc|prevsrc|prev.hlp|9;\
             79|prevdesc|prevsrc|prev.hlp|9;\
             80|Application-defined or object-defined error|Proj|{}|1000095;\
             81|prevdesc|prevsrc|explicit.hlp|9",
                default_vba_help_file()
            )
            .replace("\n             ", "")
        )
    );
}

#[test]
fn module_qualified_global_variable_is_read_and_written_as_place() {
    let manifest = manifest_modules(&[
        (
            "WebHelpers",
            ModuleKind::Procedural,
            "Public AsyncRequests As Long\n",
        ),
        (
            "Main",
            ModuleKind::Procedural,
            "Sub Main()\n    WebHelpers.AsyncRequests = 42\n    Dim r As Long\n    r = WebHelpers.AsyncRequests\nEnd Sub\n",
        ),
    ]);
    let program = bind_program(&manifest, &NullTypeLibs).expect("bind qualified global");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    assert_eq!(
        vm.slot(oxp.globals.len()).and_then(|v| v.as_i32()),
        Some(42)
    );
}

#[test]
fn module_qualified_object_global_can_receive_member_calls() {
    let manifest = manifest_modules(&[
        (
            "WebHelpers",
            ModuleKind::Procedural,
            "Public AsyncRequests As Collection\n",
        ),
        (
            "Main",
            ModuleKind::Procedural,
            "Sub Main()\n    Set WebHelpers.AsyncRequests = New Collection\n    WebHelpers.AsyncRequests.Add 10\n    Dim r As Long\n    r = WebHelpers.AsyncRequests.Count\nEnd Sub\n",
        ),
    ]);
    let program = bind_program(&manifest, &NullTypeLibs).expect("bind qualified object global");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    assert_eq!(vm.slot(oxp.globals.len()).and_then(|v| v.as_i32()), Some(1));
}

#[test]
fn vba_qualified_intrinsic_and_constant_resolve_through_library_namespace() {
    assert_eq!(
        run_main_local0(&main_sub(
            "    Dim r As Long\n    r = VBA.Len(\"abc\") + VBA.vbString\n"
        )),
        Some(11.0)
    );
}

#[test]
fn vba_module_qualified_intrinsic_requires_matching_module_owner() {
    assert_eq!(
        run_main_local0(&main_sub(
            "    Dim r As Long\n    r = VBA.Strings.Len(\"abc\")\n"
        )),
        Some(3.0)
    );
    let src = main_sub("    Dim r As Long\n    r = VBA.NotStrings.Len(\"abc\")\n");
    assert!(
        bind_program(&manifest(&src), &NullTypeLibs).is_err(),
        "VBA.<module>.<member> must not ignore a bogus middle qualifier"
    );
}

#[test]
fn vba_left_intrinsic_is_not_shadowed_by_unrelated_class_property() {
    let main = "Sub Main()\n    Dim r As Long\n    r = Len(Left(\"hello\", 2)) + Len(Left$(\"world\", 3))\nEnd Sub\n";
    let control = "Public Property Get Left() As Single\n    Left = 99\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("ControlLike", ModuleKind::Class, control),
        ]),
        Some(5.0)
    );
}

#[test]
fn left_dollar_intrinsic_is_not_shadowed_by_own_class_left_property() {
    // The same class that owns a `Left As Single` property (every MSForms control)
    // calls the bare `Left$` string intrinsic in one of its own methods. The `$`
    // type-declaration character makes `Left$` a distinct identifier from the
    // suffix-less `Left` property, so it must route to `VBA.Strings.Left` (here
    // `Len(Left$("hello", 2))` = 2), not to the implicit-`Me` property (whose
    // Single value would coerce to a 1-char string → 1, or fault).
    let main = "Sub Main()\n    Dim r As Long\n    Dim f As FormLike\n    \
                Set f = New FormLike\n    r = f.Lead()\nEnd Sub\n";
    let form = "Public Property Get Left() As Single\n    Left = 0\nEnd Property\n\n\
                Public Function Lead() As Long\n    Lead = Len(Left$(\"hello\", 2))\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "FormLike", form), Some(2.0));
}

#[test]
fn left_intrinsic_without_suffix_is_shadowed_by_own_class_left_property() {
    // Without a type-declaration character, an unqualified `Left` inside the class
    // that owns a `Left` property is the (implicit-`Me`) property — VBA scoping puts
    // the class member ahead of the library intrinsic. Reading it yields the
    // property's value (7), confirming the suffix is what distinguishes the two.
    let main = "Sub Main()\n    Dim r As Long\n    Dim f As FormLike\n    \
                Set f = New FormLike\n    r = f.Lead()\nEnd Sub\n";
    let form = "Public Property Get Left() As Long\n    Left = 7\nEnd Property\n\n\
                Public Function Lead() As Long\n    Lead = Left\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "FormLike", form), Some(7.0));
}

#[test]
fn local_value_named_vba_shadows_library_namespace_qualifier() {
    let src = main_sub("    Dim VBA As Long\n    Dim r As Long\n    r = VBA.Len(\"abc\")\n");
    assert!(
        bind_program(&manifest(&src), &NullTypeLibs).is_err(),
        "a local value named VBA must shadow the library namespace qualifier"
    );
}

#[test]
fn integer_division() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = 7 \\ 2\n")),
        Some(3.0)
    );
}

#[test]
fn xor_operator() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = 6 Xor 3\n")),
        Some(5.0)
    );
}

#[test]
fn currency_coercion_on_assignment() {
    // 2.5 assigned to a Currency local narrows via the new Currency coercion target.
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Currency\n    r = 2.5\n")),
        Some(2.5)
    );
}

// ── Control flow ─────────────────────────────────────────────────────────────

#[test]
fn if_elseif_else() {
    let body = "    Dim r As Long\n    r = 0\n    If 2 > 3 Then\n        r = 1\n    ElseIf 5 > 4 Then\n        r = 42\n    Else\n        r = 7\n    End If\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(42.0));
}

#[test]
fn for_with_exit_for() {
    let body = "    Dim total As Long\n    Dim i As Long\n    total = 0\n    For i = 1 To 10\n        If i > 5 Then Exit For\n        total = total + i\n    Next\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(15.0));
}

#[test]
fn for_each_over_array() {
    let body = "    Dim total As Long\n    Dim item\n    total = 0\n    For Each item In Array(10, 20, 30)\n        total = total + item\n    Next\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(60.0));
}

#[test]
fn do_while_loop() {
    let body = "    Dim n As Long\n    n = 0\n    Do While n < 10\n        n = n + 1\n    Loop\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(10.0));
}

#[test]
fn select_case_value_list_and_is() {
    let body = "    Dim r As Long\n    Dim x As Long\n    x = 2\n    Select Case x\n        Case 1\n            r = 10\n        Case 2, 3\n            r = 20\n        Case Is > 5\n            r = 30\n        Case Else\n            r = 99\n    End Select\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(20.0));
}

// ── Calls (project Sub ByRef + Function, native) ─────────────────────────────

#[test]
fn project_sub_by_ref_mutates_caller() {
    let src = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(105.0));
}

#[test]
fn project_function_returns_value() {
    let src = "Sub Main()\n    Dim r As Long\n    r = Add(2, 3)\nEnd Sub\n\nFunction Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn native_len() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = Len(\"hello\")\n")),
        Some(5.0)
    );
}

// ── Arrays / error-state ─────────────────────────────────────────────────────

#[test]
fn paramarray_sums_trailing_args() {
    let src = "Sub Main()\n    Dim r As Long\n    r = SumAll(10, 20, 30)\nEnd Sub\n\n\
               Function SumAll(ParamArray xs() As Variant) As Long\n\
               Dim i As Long\n    Dim t As Long\n    t = 0\n\
               For i = LBound(xs) To UBound(xs)\n        t = t + xs(i)\n    Next\n\
               SumAll = t\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(60.0));
}

#[test]
fn paramarray_empty_is_zero_length() {
    let src = "Sub Main()\n    Dim r As Long\n    r = CountAll()\nEnd Sub\n\n\
               Function CountAll(ParamArray xs() As Variant) As Long\n\
               CountAll = UBound(xs) - LBound(xs) + 1\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(0.0));
}

#[test]
fn paramarray_mixed_fixed_and_variadic() {
    let src = "Sub Main()\n    Dim r As Long\n    r = AddBase(100, 1, 2, 3)\nEnd Sub\n\n\
               Function AddBase(seed As Long, ParamArray xs() As Variant) As Long\n\
               Dim i As Long\n    Dim t As Long\n    t = seed\n\
               For i = LBound(xs) To UBound(xs)\n        t = t + xs(i)\n    Next\n\
               AddBase = t\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(106.0));
}

#[test]
fn lbound_ubound_invalid_dimension_raises_under_resume_next() {
    let src = "Sub Main()\n    Dim r As Long\n    Dim xs As Variant\n    Dim lower2 As Long\n    xs = Array(\"A\", \"B\", \"C\")\n    On Error Resume Next\n    lower2 = LBound(xs, 2)\n    If Err.Number <> 0 Then\n        Err.Clear\n        r = UBound(xs, 1) - LBound(xs, 1) + 1\n    Else\n        r = -100\n    End If\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(3.0));
}

#[test]
fn random_access_file_statements_bind_and_lower() {
    // Get/Put/Seek/Width/Lock/Unlock/Name previously errored in the binder; they
    // now route to native impls (Get lowers as an assignment of the read value).
    let src = "Sub Main()\n\
        Dim num As Integer\n    num = 1\n\
        Dim v As Long\n    v = 7\n\
        Put #num, 1, v\n\
        Get #num, 1, v\n\
        Seek #num, 1\n\
        Width #num, 80\n\
        Lock #num\n\
        Unlock #num\n\
        Name \"a\" As \"b\"\n\
    End Sub\n";
    let program = bind(src);
    assert!(oxvba_oxir::elaborate::elaborate(&program).is_ok());
}

#[test]
fn declare_byval_string_lvalue_binds_byref_for_ansi_writeback() {
    // VBA marshals a `Declare` String argument through a system-codepage ANSI
    // buffer and converts the (possibly callee-mutated) buffer back into the
    // variable after the call — even when the parameter is declared `ByVal` (the
    // pre-sized-buffer idiom). The call site must therefore bind a String-typed
    // l-value argument ByRef so the marshaled-back value reaches the variable,
    // while an r-value (here a literal) stays ByVal with no write-back target.
    let program = bind(
        "Private Declare PtrSafe Function lstrcpyA Lib \"kernel32\" (ByVal dst As String, ByVal src As String) As LongPtr\n\
         Sub Main()\n    Dim buffer As String\n    lstrcpyA buffer, \"alpha\"\nEnd Sub\n",
    );
    let entry = program.entry.expect("entry");
    let args = program.procs[entry.0]
        .body
        .iter()
        .find_map(|stmt| match stmt {
            CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::Declare { .. },
                args,
            }) => Some(args.clone()),
            _ => None,
        })
        .expect("the Declare call should lower in Main's body");
    assert!(
        matches!(args[0], CoreArg::ByRef(_)),
        "a String l-value to a ByVal String param must bind ByRef (ANSI write-back), got {:?}",
        args[0]
    );
    assert!(
        matches!(args[1], CoreArg::ByVal(_)),
        "a literal String argument must stay ByVal, got {:?}",
        args[1]
    );
}

#[test]
fn kill_statement_routes_to_vba_filesystem() {
    // `Kill pathname` is not a lexer keyword (unlike Open/Close/Print#/Name/…), so it
    // parses as an ordinary statement-call and resolves by name. Since P4 it routes
    // cross-bundle to the `VBA` unit's `FileSystem.Kill` member (an `ExternProc` call),
    // exactly like the by-name file functions — not the bespoke `Native` route.
    let program = bind("Sub Main()\n    Kill \"scratch.tmp\"\nEnd Sub\n");
    assert!(
        imports_vba_filesystem(&program, "Kill"),
        "`Kill` should import VBA/FileSystem.Kill: {:?}",
        program.imports
    );
}

/// True if the bound `program` imports a `VBA`/`FileSystem` `ModuleFunc` named
/// `member` (the cross-bundle link a `FileSystem` call lowers to).
fn imports_vba_filesystem(program: &oxvba_bundle::coreir::CoreProgram, member: &str) -> bool {
    program.imports.iter().any(|imp| {
        imp.unit.eq_ignore_ascii_case("VBA")
            && matches!(
                &imp.token,
                oxvba_bundle::ExportToken::ModuleFunc { module, member: m, .. }
                    if module.eq_ignore_ascii_case("FileSystem")
                        && m.eq_ignore_ascii_case(member)
            )
    })
}

#[test]
fn random_file_put_get_round_trips_through_vm() {
    // End-to-end: Open Random with Len=8, Put a Long at record 2, Get it back —
    // exercising the mode + Len plumbing through the standard host's in-memory file.
    let src = "Sub Main()\n\
        Dim r As Long\n    Dim v As Long\n    v = 222\n\
        Open \"rec.dat\" For Random As #1 Len = 8\n\
        Put #1, 2, v\n\
        Get #1, 2, r\n\
        Close #1\n\
    End Sub\n";
    let program = bind(src);
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = oxvba_hal::adapters::builder::HostBuilder::new()
        .profile(oxvba_hal::HalProfileId::Windows)
        .policy(oxvba_hal::HostPolicy {
            allow_filesystem_mutation: true,
            ..oxvba_hal::HostPolicy::default()
        })
        .build();
    let vm = oxvba_vm3::Vm3::run(&oxp, host.as_ref()).expect("run");
    assert_eq!(
        vm.slot(oxp.globals.len()).and_then(|v| v.as_i32()),
        Some(222)
    );
}

/// Run a single-module source on the standard (in-memory) host; read `Main`'s
/// first local as a string.
fn run_main_local0_string_std(src: &str) -> Option<String> {
    let program = bind(src);
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = oxvba_hal::adapters::builder::HostBuilder::new()
        .profile(oxvba_hal::HalProfileId::Windows)
        .policy(oxvba_hal::HostPolicy {
            allow_filesystem_mutation: true,
            ..oxvba_hal::HostPolicy::default()
        })
        .build();
    let vm = oxvba_vm3::Vm3::run(&oxp, host.as_ref()).expect("run");
    vm.slot(oxp.globals.len())?.as_bstr().map(|b| b.as_str())
}

#[test]
fn fixed_length_string_random_record_round_trips() {
    // String * 4 is written raw (no prefix, padded to 4) and read back as 4 chars.
    let src = "Sub Main()\n\
        Dim r As String * 4\n    Dim v As String * 4\n    v = \"ab\"\n\
        Open \"f.dat\" For Random As #1 Len = 4\n\
        Put #1, 1, v\n    Get #1, 1, r\n    Close #1\n\
    End Sub\n";
    assert_eq!(run_main_local0_string_std(src), Some("ab  ".to_string()));
}

#[test]
fn binary_variable_string_reads_current_length() {
    // Binary `Get` reads `Len(r)` bytes; r is pre-sized to 5 spaces, so it reads
    // the 5 bytes previously written.
    let src = "Sub Main()\n\
        Dim r As String\n\
        Open \"f.dat\" For Binary As #1\n\
        Put #1, 1, \"hello\"\n    r = \"     \"\n    Get #1, 1, r\n    Close #1\n\
    End Sub\n";
    assert_eq!(run_main_local0_string_std(src), Some("hello".to_string()));
}

/// Run a single-module source on the standard (in-memory) host; read `Main`'s first local as
/// a `Long`.
fn run_main_local0_i32_std(src: &str) -> Option<i32> {
    let program = bind(src);
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = oxvba_hal::adapters::builder::HostBuilder::new()
        .profile(oxvba_hal::HalProfileId::Windows)
        .policy(oxvba_hal::HostPolicy {
            allow_filesystem_mutation: true,
            ..oxvba_hal::HostPolicy::default()
        })
        .build();
    let vm = oxvba_vm3::Vm3::run(&oxp, host.as_ref()).expect("run");
    vm.slot(oxp.globals.len())?.as_i32()
}

#[test]
fn bare_close_closes_all_without_error() {
    // `Close` with no file number closes ALL open files — it must not raise Err 5. Reaching
    // `ok = 1` (the run did not raise) proves it.
    let src = "Sub Main()\n\
        Dim ok As Long\n\
        Open \"f.dat\" For Output As #1\n    Print #1, \"x\"\n    Close\n\
        ok = 1\n\
    End Sub\n";
    assert_eq!(run_main_local0_i32_std(src), Some(1));
}

#[test]
fn reset_closes_all_without_error() {
    // `Reset` parses as a `Close`-all and must likewise not raise.
    let src = "Sub Main()\n\
        Dim ok As Long\n\
        Open \"f.dat\" For Output As #1\n    Print #1, \"x\"\n    Reset\n\
        ok = 1\n\
    End Sub\n";
    assert_eq!(run_main_local0_i32_std(src), Some(1));
}

#[test]
fn seek_function_reads_position_without_resetting() {
    // The `Seek(filenumber)` FUNCTION returns the current position WITHOUT moving it (it used to
    // reset the position to 0). VBA's Seek is 1-based: after writing 5 bytes it reports the
    // next-write byte position 6 (live-Excel verified: 3 bytes → Seek = 4).
    let src = "Sub Main()\n\
        Dim p As Long\n\
        Open \"f.dat\" For Binary As #1\n    Put #1, 1, \"hello\"\n    p = Seek(1)\n    Close #1\n\
    End Sub\n";
    assert_eq!(run_main_local0_i32_std(src), Some(6));
}

/// The args of the (single) `Main`-body `ExternProc` call into `VBA`/`FileSystem.<member>`,
/// or `None` if there is no such call. Used to assert `Print`/`Write` pass every field.
fn extern_filesystem_call_args<'p>(
    program: &'p CoreProgram,
    member: &str,
) -> Option<&'p Vec<CoreArg>> {
    fn matches_member(program: &CoreProgram, import: usize, member: &str) -> bool {
        program.imports.get(import).is_some_and(|imp| {
            imp.unit.eq_ignore_ascii_case("VBA")
                && matches!(
                    &imp.token,
                    oxvba_bundle::ExportToken::ModuleFunc { module, member: m, .. }
                        if module.eq_ignore_ascii_case("FileSystem")
                            && m.eq_ignore_ascii_case(member)
                )
        })
    }
    program
        .procs
        .iter()
        .flat_map(|p| &p.body)
        .find_map(|s| match s {
            CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::ExternProc { import },
                args,
            }) if matches_member(program, *import, member) => Some(args),
            _ => None,
        })
}

/// Count the `Main`-body assignments whose value is an `ExternProc` call into
/// `VBA`/`FileSystem.<member>` — i.e. read-into-target statements like
/// `var = FileInput(handle, 1)` / `var = FileLineInput(handle)`.
fn extern_filesystem_assign_count(program: &CoreProgram, member: &str) -> usize {
    fn call_member(program: &CoreProgram, value: &CoreValue, member: &str) -> bool {
        fn unwrap_import(value: &CoreValue) -> Option<usize> {
            match value {
                CoreValue::Call {
                    callee: CoreCallee::ExternProc { import },
                    ..
                } => Some(*import),
                CoreValue::Coerce { value, .. } => unwrap_import(value),
                _ => None,
            }
        }
        unwrap_import(value).is_some_and(|import| {
            program.imports.get(import).is_some_and(|imp| {
                imp.unit.eq_ignore_ascii_case("VBA")
                    && matches!(
                        &imp.token,
                        oxvba_bundle::ExportToken::ModuleFunc { module, member: m, .. }
                            if module.eq_ignore_ascii_case("FileSystem")
                                && m.eq_ignore_ascii_case(member)
                    )
            })
        })
    }
    program
        .procs
        .iter()
        .flat_map(|p| &p.body)
        .filter(
            |s| matches!(s, CoreStmt::Assign { value, .. } if call_member(program, value, member)),
        )
        .count()
}

#[test]
fn print_hash_binds_every_field_with_a_separator_spec() {
    // `Print #` previously dropped all but the first field. The dedicated binder must now
    // emit `[handle, sep-spec, kind-spec, field0, field1, field2]` — a `,`/`;`-aware
    // separator spec plus EVERY field. `Print #1, a; b, c`: a→`;`, b→`,`, c→none(`n`)
    // ⇒ spec ";,n"; all are ordinary value fields (`vvv`).
    let program = bind(
        "Sub Main()\n    Dim a, b, c\n    Open \"x.txt\" For Output As #1\n    Print #1, a; b, c\nEnd Sub\n",
    );
    let args = extern_filesystem_call_args(&program, "Print").expect("a FileSystem.Print call");
    assert_eq!(
        args.len(),
        6,
        "handle + sep-spec + kind-spec + 3 fields: {args:?}"
    );
    match &args[1] {
        CoreArg::ByVal(CoreValue::Const(oxvba_bundle::coreir::CoreConst::Str(spec))) => {
            assert_eq!(spec, ";,n", "per-field separator spec");
        }
        other => panic!("args[1] should be the separator-spec string const, got {other:?}"),
    }
    match &args[2] {
        CoreArg::ByVal(CoreValue::Const(oxvba_bundle::coreir::CoreConst::Str(spec))) => {
            assert_eq!(spec, "vvv", "per-item value/control spec");
        }
        other => panic!("args[2] should be the item-kind spec string const, got {other:?}"),
    }
}

#[test]
fn print_hash_binds_spc_and_tab_as_print_clause_controls() {
    let program = bind(
        "Sub Main()\n    Open \"x.txt\" For Output As #1\n    Print #1, \"a\"; Spc(3); \"b\"; Tab(10); \"c\"; Tab; \"d\"\nEnd Sub\n",
    );
    let args = extern_filesystem_call_args(&program, "Print").expect("a FileSystem.Print call");
    assert_eq!(
        args.len(),
        10,
        "handle + sep-spec + kind-spec + 7 item values: {args:?}"
    );
    match &args[1] {
        CoreArg::ByVal(CoreValue::Const(oxvba_bundle::coreir::CoreConst::Str(spec))) => {
            assert_eq!(spec, ";;;;;;n", "separator spec should track every item");
        }
        other => panic!("args[1] should be the separator-spec string const, got {other:?}"),
    }
    match &args[2] {
        CoreArg::ByVal(CoreValue::Const(oxvba_bundle::coreir::CoreConst::Str(spec))) => {
            assert_eq!(spec, "vsvtvzv", "value/control spec");
        }
        other => panic!("args[2] should be the item-kind spec string const, got {other:?}"),
    }
}

#[test]
fn write_hash_binds_every_field() {
    // `Write #1, x, y` ⇒ `[handle, sep-spec, x, y]` — all fields reach the call.
    let program = bind(
        "Sub Main()\n    Dim x, y\n    Open \"x.txt\" For Output As #1\n    Write #1, x, y\nEnd Sub\n",
    );
    let args = extern_filesystem_call_args(&program, "Write").expect("a FileSystem.Write call");
    assert_eq!(args.len(), 4, "handle + sep-spec + 2 fields: {args:?}");
}

#[test]
fn input_hash_binds_one_writeback_assignment_per_target() {
    // `Input #` previously discarded its targets (no write-back). It must now bind ONE
    // `target = FileInput(handle, 1)` assignment per target.
    let program = bind(
        "Sub Main()\n    Dim a, b, c\n    Open \"x.txt\" For Input As #1\n    Input #1, a, b, c\nEnd Sub\n",
    );
    assert_eq!(
        extern_filesystem_assign_count(&program, "Input"),
        3,
        "one write-back assignment per Input # target"
    );
}

#[test]
fn line_input_hash_binds_a_writeback_assignment() {
    // `Line Input #1, s` must bind `s = FileLineInput(handle)` (previously discarded).
    let program = bind(
        "Sub Main()\n    Dim s As String\n    Open \"x.txt\" For Input As #1\n    Line Input #1, s\nEnd Sub\n",
    );
    assert_eq!(
        extern_filesystem_assign_count(&program, "LineInput"),
        1,
        "Line Input # must write the line back to its target"
    );
}

#[test]
fn addressof_binds_to_proc_ref() {
    let src = "Sub Main()\n    Dim p As Long\n    p = AddressOf Helper\nEnd Sub\n\nSub Helper()\nEnd Sub\n";
    let program = bind(src);
    let main = program
        .procs
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("Main"))
        .unwrap();
    // The proc-ref is stored into a `Long`, so the store coerces it to `Long`
    // (a no-op for an integer pointer); look through that `Coerce` wrapper.
    fn is_addressof(v: &CoreValue) -> bool {
        match v {
            CoreValue::AddressOf(_) => true,
            CoreValue::Coerce { value, .. } => is_addressof(value),
            _ => false,
        }
    }
    assert!(
        main.body
            .iter()
            .any(|s| matches!(s, CoreStmt::Assign { value, .. } if is_addressof(value)))
    );
}

#[test]
fn addressof_runs_as_integer() {
    // The proc reference materializes as an integer (round-trips through a slot).
    let src = "Sub Main()\n    Dim r As Long\n    r = AddressOf Helper\nEnd Sub\n\nSub Helper()\nEnd Sub\n";
    assert!(run_main_local0(src).is_some());
}

#[test]
fn addressof_unknown_is_error() {
    let src = "Sub Main()\n    Dim p As Long\n    p = AddressOf Nope\nEnd Sub\n";
    assert!(bind_program(&manifest(src), &NullTypeLibs).is_err());
}

#[test]
fn ubound_lbound_of_local_array() {
    let body = "    Dim r As Long\n    Dim v\n    ReDim v(2 To 9)\n    r = UBound(v) - LBound(v)\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(7.0));
}

#[test]
fn redim_array_set_get() {
    let body =
        "    Dim r As Long\n    Dim v1\n    ReDim v1(0 To 2)\n    v1(1) = 77\n    r = v1(1)\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(77.0));
}

#[test]
fn redim_undeclared_simple_name_declares_dynamic_variant_array() {
    let body = "    Dim r As Long\n    ReDim a(0 To 2)\n    a(1) = 77\n    r = a(1)\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(77.0));
}

#[test]
fn call_keyword_module_qualified_sub_with_attached_parens_runs() {
    let program = bind_program(
        &manifest_modules(&[
            (
                "Startup",
                ModuleKind::Procedural,
                "Public Sub Main()\nCall Program.Run()\nEnd Sub\n",
            ),
            (
                "Program",
                ModuleKind::Procedural,
                "Public result As Long\nPublic Sub Run()\nresult = 42\nEnd Sub\n",
            ),
        ]),
        &NullTypeLibs,
    )
    .expect("module-qualified Call statement should bind");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    assert_eq!(vm.slot(0).and_then(|value| value.as_i32()), Some(42));
}

#[test]
fn redim_preserve_does_not_declare_undeclared_name() {
    let err = bind_error_display("Option Explicit\nSub Main()\n    ReDim Preserve a(1)\nEnd Sub\n");
    assert_eq!(err, "Variable not defined");
}

#[test]
fn redim_scalar_declared_target_is_expected_array() {
    let err = bind_error_display(
        "Option Explicit\nSub Main()\n    Dim a As Long\n    ReDim a(1)\nEnd Sub\n",
    );
    assert_eq!(err, "Expected array");
}

#[test]
fn on_error_resume_next_err_number() {
    let body =
        "    Dim r As Long\n    On Error Resume Next\n    Err.Raise 11\n    r = Err.Number\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(11.0));
}

// ── Regression tests for review findings ────────────────────────────────────

#[test]
fn recursive_function_call() {
    // `Fact(n-1)` inside `Fact` must lower to a recursive call, not an index into
    // the return pseudo-variable.
    let src = "Sub Main()\n    Dim r As Long\n    r = Fact(5)\nEnd Sub\n\nFunction Fact(n As Long) As Long\n    If n <= 1 Then\n        Fact = 1\n    Else\n        Fact = n * Fact(n - 1)\n    End If\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(120.0));
}

#[test]
fn named_arguments_reordered() {
    // Named args must be reordered into the parameter positions, not passed in
    // call-site order.
    let src = "Sub Main()\n    Dim r As Long\n    r = Diff(b:=2, a:=10)\nEnd Sub\n\nFunction Diff(a As Long, b As Long) As Long\n    Diff = a - b\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(8.0));
}

#[test]
fn duplicate_named_argument_is_bind_error() {
    let src = "Sub Main()\n    Dim x As Long\n    x = 1\n    Call Fill(x, target := x)\nEnd Sub\n\nSub Fill(ByRef target As Long, ByVal value As Long)\n    target = value\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("duplicate argument for parameter target"),
        "unexpected error: {err}"
    );
}

#[test]
fn positional_after_named_argument_is_bind_error() {
    let src = "Sub Main()\n    Dim x As Long\n    x = 1\n    Call Fill(value := 9, x)\nEnd Sub\n\nSub Fill(ByRef target As Long, ByVal value As Long)\n    target = value\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("positional argument cannot follow named argument"),
        "unexpected error: {err}"
    );
}

#[test]
fn extra_argument_is_bind_error() {
    let src = "Sub Main()\n    TakeOne 1, 2\nEnd Sub\n\nSub TakeOne(ByVal n As Long)\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("WrongNumberOfArgumentsOrInvalidPropertyAssignment"),
        "unexpected error: {err}"
    );
}

#[test]
fn missing_required_argument_is_bind_error() {
    let src = "Sub Main()\n    TakeTwo 1\nEnd Sub\n\nSub TakeTwo(ByVal a As Long, ByVal b As Long)\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ArgumentNotOptional") && err.contains("b"),
        "unexpected error: {err}"
    );
}

#[test]
fn indexed_property_extra_argument_is_bind_error() {
    let src = "Private mV As Long\n\nSub Main()\n    Item(1, 2) = 3\nEnd Sub\n\nProperty Let Item(ByVal i As Long, ByVal v As Long)\n    mV = v\nEnd Property\n";
    let err = bind_error(src);
    assert!(
        err.contains("WrongNumberOfArgumentsOrInvalidPropertyAssignment"),
        "unexpected error: {err}"
    );
}

#[test]
fn indexed_property_missing_required_index_is_bind_error() {
    let src = "Private mV As Long\n\nSub Main()\n    Item = 3\nEnd Sub\n\nProperty Let Item(ByVal i As Long, ByVal v As Long)\n    mV = v\nEnd Property\n";
    let err = bind_error(src);
    assert!(
        err.contains("ArgumentNotOptional") && err.contains("i"),
        "unexpected error: {err}"
    );
}

#[test]
fn omitted_optional_argument_still_uses_default() {
    let src = "Sub Main()\n    Dim r As Long\n    r = AddOpt(5)\nEnd Sub\n\nFunction AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long\n    AddOpt = n + bonus\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(12.0));
}

#[test]
fn paramarray_still_accepts_extra_arguments() {
    let src = "Sub Main()\n    Dim r As Long\n    r = SumAll(1, 2, 3)\nEnd Sub\n\nFunction SumAll(ParamArray xs() As Variant) As Long\n    Dim i As Long\n    For i = LBound(xs) To UBound(xs)\n        SumAll = SumAll + CLng(xs(i))\n    Next i\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(6.0));
}

#[test]
fn duplicate_label_in_one_procedure_is_bind_error() {
    // Two `done:` labels in one `Sub` — a VBA compile error ("Duplicate declaration
    // in current scope"). The binder must reject it (vm2 used to run it leniently).
    let src = "Sub Main()\n    Dim x As Long\ndone:\n    x = 1\ndone:\n    x = 2\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("DuplicateLabel") && err.contains("done"),
        "unexpected error: {err}"
    );
}

#[test]
fn same_label_name_in_different_procedures_binds() {
    // Label scope is per-procedure, so the same name in two procedures is legal.
    let src = "Sub A()\ndone:\nEnd Sub\n\nSub B()\ndone:\nEnd Sub\n";
    bind(src);
}

#[test]
fn label_referenced_many_times_but_defined_once_binds() {
    // A label may be *referenced* any number of times (here by `On Error GoTo` and
    // `GoTo`); only a second *definition* is an error. Guards against counting a
    // reference as a definition.
    let src = "Sub Main()\n    Dim x As Long\n    On Error GoTo handler\n    GoTo handler\nhandler:\n    x = 1\nEnd Sub\n";
    bind(src);
}

#[test]
fn colonless_line_number_labels_bind_and_branch() {
    let src = "Sub Main()\n\
               Dim x As Long\n\
               GoTo 200\n\
               100 x = 1\n\
               200 x = 5\n\
               End Sub\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn erl_initial_and_numeric_line_without_error_stay_zero() {
    let initial = "Sub Main()\n    Dim r As String\n    r = CStr(Erl) & \":\" & CStr(VarType(Erl))\nEnd Sub\n";
    assert_eq!(run_main_local0_string(initial), Some("0:3".to_string()));

    let no_error = "Sub Main()\n\
                    Dim r As String\n\
                    Dim x As Long\n\
10                  x = 1\n\
                    r = CStr(Erl) & \":\" & CStr(VarType(Erl)) & \":\" & CStr(x)\n\
                    End Sub\n";
    assert_eq!(run_main_local0_string(no_error), Some("0:3:1".to_string()));
}

#[test]
fn erl_records_caught_error_line_in_current_activation() {
    let resume_next = "Sub Main()\n\
                       Dim r As String\n\
                       Dim x As Long\n\
                       On Error Resume Next\n\
10                     x = 1 / 0\n\
                       r = CStr(Err.Number) & \":\" & CStr(Erl)\n\
                       End Sub\n";
    assert_eq!(
        run_main_local0_string(resume_next),
        Some("11:10".to_string())
    );

    let handler = "Sub Main()\n\
                   Dim r As String\n\
                   On Error GoTo EH\n\
10                 Err.Raise 5\n\
                   r = \"miss\"\n\
                   Exit Sub\n\
EH:\n\
                   r = CStr(Err.Number) & \":\" & CStr(Erl)\n\
                   End Sub\n";
    assert_eq!(run_main_local0_string(handler), Some("5:10".to_string()));
}

#[test]
fn erl_uses_prior_numeric_label_for_unnumbered_faults() {
    let src = "Sub Main()\n\
               Dim r As String\n\
               Dim x As Long\n\
               On Error GoTo EH\n\
10             x = 1\n\
               x = 1 / 0\n\
               r = \"miss\"\n\
               Exit Sub\n\
EH:\n\
               r = CStr(Err.Number) & \":\" & CStr(Erl)\n\
               End Sub\n";
    assert_eq!(run_main_local0_string(src), Some("11:10".to_string()));
}

#[test]
fn caller_handler_reports_call_site_line_for_callee_fault() {
    let src = "Sub Main()\n\
               Dim r As String\n\
               On Error GoTo EH\n\
               Boom\n\
               r = \"miss\"\n\
               Exit Sub\n\
EH:\n\
               r = CStr(Err.Number) & \":\" & CStr(Erl)\n\
               End Sub\n\
\n\
               Private Sub Boom()\n\
20             Err.Raise 7\n\
               End Sub\n";
    assert_eq!(run_main_local0_string(src), Some("7:0".to_string()));
}

#[test]
fn on_error_undefined_label_is_bind_error() {
    let src = "Sub Main()\n    On Error GoTo MissingHandler\nEnd Sub\n";
    let err = bind_error_display(src);
    assert!(err.contains("Label not defined"), "unexpected error: {err}");
}

fn computed_goto_result(selector: &str) -> Option<String> {
    let src = format!(
        "Sub Main()\n\
         Dim r As String\n\
         Dim n As Variant\n\
         On Error GoTo EH\n\
         n = {selector}\n\
         On n GoTo L1, L2\n\
         r = \"fallthrough:\" & CStr(Err.Number)\n\
         Exit Sub\n\
         L1:\n\
         r = \"L1:\" & CStr(Err.Number)\n\
         Exit Sub\n\
         L2:\n\
         r = \"L2:\" & CStr(Err.Number)\n\
         Exit Sub\n\
         EH:\n\
         r = \"err:\" & CStr(Err.Number)\n\
         End Sub\n"
    );
    run_main_local0_string(&src)
}

fn computed_gosub_result(selector: &str) -> Option<String> {
    let src = format!(
        "Sub Main()\n\
         Dim r As String\n\
         Dim n As Variant\n\
         On Error GoTo EH\n\
         r = \"before\"\n\
         n = {selector}\n\
         On n GoSub S1, S2\n\
         r = r & \":after:\" & CStr(Err.Number)\n\
         Exit Sub\n\
         S1:\n\
         r = r & \":S1\"\n\
         Return\n\
         S2:\n\
         r = r & \":S2\"\n\
         Return\n\
         EH:\n\
         r = \"err:\" & CStr(Err.Number) & \":\" & r\n\
         End Sub\n"
    );
    run_main_local0_string(&src)
}

#[test]
fn computed_goto_selector_matches_vba_boundaries() {
    assert_eq!(computed_goto_result("1"), Some("L1:0".to_string()));
    assert_eq!(computed_goto_result("0"), Some("fallthrough:0".to_string()));
    assert_eq!(computed_goto_result("3"), Some("fallthrough:0".to_string()));
    assert_eq!(computed_goto_result("-1"), Some("err:5".to_string()));
    assert_eq!(computed_goto_result("1.5"), Some("L2:0".to_string()));
    assert_eq!(computed_goto_result("2.5"), Some("L2:0".to_string()));
    assert_eq!(computed_goto_result("\"x\""), Some("err:13".to_string()));
    assert_eq!(computed_goto_result("Null"), Some("err:94".to_string()));
}

#[test]
fn computed_gosub_selector_matches_vba_boundaries() {
    assert_eq!(
        computed_gosub_result("2"),
        Some("before:S2:after:0".to_string())
    );
    assert_eq!(
        computed_gosub_result("0"),
        Some("before:after:0".to_string())
    );
    assert_eq!(
        computed_gosub_result("3"),
        Some("before:after:0".to_string())
    );
    assert_eq!(
        computed_gosub_result("-1"),
        Some("err:5:before".to_string())
    );
    assert_eq!(
        computed_gosub_result("1.5"),
        Some("before:S2:after:0".to_string())
    );
}

#[test]
fn multivariable_next_closes_nested_for_loops() {
    let src = "Sub Main()\n\
               Dim total As Long\n\
               Dim i As Long\n\
               Dim j As Long\n\
               For i = 1 To 2\n\
               For j = 1 To 3\n\
                   total = total + 1\n\
               Next j, i\n\
               End Sub\n";
    assert_eq!(run_main_local0(src), Some(6.0));
}

#[test]
fn sub_used_as_expression_is_bind_error() {
    let src = "Sub Main()\nDim x\nx = DoIt()\nEnd Sub\nSub DoIt()\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ExpectedFunctionOrVariable") || err.contains("Expected Function or variable"),
        "unexpected error: {err}"
    );
}

#[test]
fn sub_called_as_statement_still_binds() {
    let src = "Sub Main()\nDim x As Long\nDoIt x\nCall DoIt(x)\nEnd Sub\nSub DoIt(ByRef x As Long)\nx = x + 1\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(2.0));
}

#[test]
fn exit_do_inside_while_wend_is_bind_error() {
    let src = "Sub Main()\nWhile True\nExit Do\nWend\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("Exit Do outside Do loop"),
        "unexpected error: {err}"
    );
}

#[test]
fn exit_do_inside_do_loop_still_binds() {
    let src = "Sub Main()\nDim x As Long\nDo\nx = x + 1\nExit Do\nLoop\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(1.0));
}

#[test]
fn exit_do_inside_while_nested_in_do_is_bind_error() {
    let src = "Sub Main()\nDo\nWhile True\nExit Do\nWend\nLoop\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("Exit Do outside Do loop"),
        "unexpected error: {err}"
    );
}

#[test]
fn keyword_named_proc_does_not_desync_frames() {
    // `Function Name()` has a keyword name (no plain Ident), but the scanner still
    // gives it a scope. The binder must agree on the proc set so `Compute`'s body
    // binds against its OWN frame (its local), not the keyword proc's frame.
    let src = "Sub Main()\n    Dim r As Long\n    r = Compute()\nEnd Sub\n\nFunction Name(p As Long) As String\n    Name = \"x\"\nEnd Function\n\nFunction Compute() As Long\n    Dim total As Long\n    total = 42\n    Compute = total\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(42.0));
}

#[test]
fn parenthesized_argument_forces_by_val() {
    // A parenthesized statement-call argument `Inc (r)` is forced ByVal → caller unchanged
    // (without the parens, `Inc r` would mutate r to 105 via ByRef).
    let src = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc (r)\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn call_site_byval_forces_by_val_over_byref_param() {
    // A call-site `ByVal r` overrides the callee's declared `ByRef`, so the
    // mutation must NOT reach the caller's `r`. (The parser accepted the
    // `ByVal`/`ByRef` modifier but dropped it, so this used to mutate r to 105.)
    let src = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc ByVal r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn call_site_byref_keeps_write_back() {
    // The control: `Inc r` (no modifier) and an explicit `Inc ByRef r` both
    // write back through the ByRef param.
    let bare = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    let explicit = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc ByRef r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(bare), Some(105.0));
    assert_eq!(run_main_local0(explicit), Some(105.0));
}

#[test]
fn byref_type_mismatch_is_bind_error() {
    let src = "Sub Main()\n    Dim x As Integer\n    TakeLong x\nEnd Sub\n\nSub TakeLong(ByRef n As Long)\n    n = 7\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ByRefTypeMismatch") && err.contains("Long") && err.contains("Integer"),
        "unexpected error: {err}"
    );
}

#[test]
fn byref_variant_to_scalar_is_bind_error() {
    let src = "Sub Main()\n    Dim x As Variant\n    x = 3\n    TakeLong x\nEnd Sub\n\nSub TakeLong(ByRef n As Long)\n    n = 7\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ByRefTypeMismatch") && err.contains("Long") && err.contains("Variant"),
        "unexpected error: {err}"
    );
}

#[test]
fn byref_variant_requires_variant_lvalue() {
    let src = "Sub Main()\n    Dim x As Long\n    Capture x\nEnd Sub\n\nSub Capture(ByRef target As Variant)\n    target = 7\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ByRefTypeMismatch") && err.contains("Variant") && err.contains("Long"),
        "unexpected error: {err}"
    );
}

#[test]
fn byref_variant_lvalue_still_aliases() {
    let src = "Sub Main()\n    Dim r As Variant\n    r = 5\n    Capture r\nEnd Sub\n\nSub Capture(ByRef target As Variant)\n    target = 7\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(7.0));
}

#[test]
fn byref_variant_accepts_array_lvalue() {
    let src = "Sub Main()\n    Dim bytes() As Byte\n    ReDim bytes(0 To 1)\n    Capture bytes\nEnd Sub\n\nSub Capture(ByRef target As Variant)\nEnd Sub\n";
    bind(src);
}

#[test]
fn parenthesized_byref_type_mismatch_uses_byval_temporary() {
    let src = "Sub Main()\n    Dim r As Long\n    Dim x As Integer\n    x = 5\n    TakeLong (x)\n    r = x\nEnd Sub\n\nSub TakeLong(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn hex_literal() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = &H1F\n")),
        Some(31.0)
    );
}

// ── Long tail ────────────────────────────────────────────────────────────────

#[test]
fn module_and_proc_consts_fold() {
    // A module-level Const and a proc-level Const both substitute their values.
    let src = "Const FACTOR As Long = 10\n\n\
               Sub Main()\n    Dim r As Long\n    Const ADD = 5\n    r = FACTOR * 2 + ADD\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(25.0));
}

#[test]
fn option_compare_text_is_case_insensitive() {
    // Under `Option Compare Text`, `"A" = "a"` is True (-1); Binary would give 0.
    let src = "Option Compare Text\n\nSub Main()\n    Dim r As Boolean\n    r = (\"A\" = \"a\")\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(-1.0));
}

// ── Objects: classes, New, Me, fields, methods, properties, Set ──────────────

/// A two-module project: a procedural `Main` + one class module. The class
/// module is named so it sorts after `Main`.
fn class_manifest(main_src: &str, class_name: &str, class_src: &str) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![
            ModuleUnit {
                module_name: "Main".into(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes::named("Main"),
                source: main_src.into(),
            },
            ModuleUnit {
                module_name: class_name.into(),
                module_kind: ModuleKind::Class,
                attributes: ModuleAttributes::named(class_name),
                source: class_src.into(),
            },
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

/// Bind + elaborate + run a class project on vm3; read `Main`'s first local as a number.
fn run_class_main_local0(main_src: &str, class_name: &str, class_src: &str) -> Option<f64> {
    let program = bind_program(
        &class_manifest(main_src, class_name, class_src),
        &NullTypeLibs,
    )
    .expect("bind_program");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    let value = vm.slot(oxp.globals.len())?;
    value
        .as_f64()
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
}

#[test]
fn new_initialize_field_and_method() {
    // New runs Class_Initialize (sets the instance field); a method reads it.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    r = w.GetValue()\nEnd Sub\n";
    let widget = "Private mValue As Long\n\n\
                  Private Sub Class_Initialize()\n    mValue = 42\nEnd Sub\n\n\
                  Public Function GetValue() As Long\n    GetValue = mValue\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(42.0));
}

#[test]
fn property_get_let_roundtrip() {
    // `w.Value = 10` routes to Property Let; `r = w.Value` to Property Get.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w.Value = 10\n    r = w.Value\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value() As Long\n    Value = mV\nEnd Property\n\n\
                  Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(10.0));
}

#[test]
fn standard_module_property_let_roundtrip() {
    let src = "Private mV As Long\n\n\
               Public Property Get Value() As Long\n    Value = mV\nEnd Property\n\n\
               Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n\n\
               Public Sub Main()\n    Dim r As Long\n    Value = 17\n    r = Value\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(17.0));
}

#[test]
fn write_only_standard_module_property_let_roundtrip() {
    let src = "Private mV As Long\n\n\
               Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n\n\
               Public Function GetValue() As Long\n    GetValue = mV\nEnd Function\n\n\
               Public Sub Main()\n    Dim r As Long\n    Value = 17\n    r = GetValue()\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(17.0));
}

#[test]
fn standard_module_property_let_updates_global_udt_field() {
    let src = "Private Type State\n    Value As Single\nEnd Type\n\
               Private s As State\n\n\
               Public Property Get Value() As Single\n    Value = s.Value\nEnd Property\n\n\
               Public Property Let Value(ByVal v As Single)\n    s.Value = v\nEnd Property\n\n\
               Public Sub Main()\n    Dim r As Single\n    Value = 0.25!\n    r = Value\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(0.25));
}

#[test]
fn module_udt_fixed_array_field_round_trips() {
    let src = "Private Type State\n    Buses(0 To 1) As Single\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    Dim r As Single\n    s.Buses(1) = 0.5!\n    r = s.Buses(1)\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(0.5));
}

#[test]
fn module_udt_fixed_array_field_accepts_const_bounds() {
    let src = "Private Const LastBus As Long = 1\n\
               Private Type State\n    Buses(0 To LastBus) As Single\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    Dim r As Single\n    s.Buses(1) = 0.75!\n    r = s.Buses(1)\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(0.75));
}

#[test]
fn module_udt_fixed_array_field_of_udt_elements_round_trips() {
    let src = "Private Type Buffer\n    Value As Long\nEnd Type\n\
               Private Type State\n    Buffers(0 To 1) As Buffer\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    Dim r As Long\n    s.Buffers(1).Value = 42\n    r = s.Buffers(1).Value\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(42.0));
}

#[test]
fn module_udt_scalar_field_index_read_is_expected_array() {
    let src = "Private Type State\n    Value As Long\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    Dim r As Long\n    r = s.Value(0)\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ExpectedArray"),
        "scalar UDT field indexing must bind as VBA compile error `Expected array`, got {err}"
    );
}

#[test]
fn module_udt_scalar_field_index_write_is_expected_array() {
    let src = "Private Type State\n    Value As Long\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    s.Value(0) = 7\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ExpectedArray"),
        "scalar UDT field index assignment must bind as VBA compile error `Expected array`, got {err}"
    );
}

#[test]
fn module_udt_scalar_field_redim_is_expected_array() {
    let src = "Private Type State\n    Value As Long\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    ReDim s.Value(0)\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ExpectedArray"),
        "scalar UDT field ReDim must bind as VBA compile error `Expected array`, got {err}"
    );
}

#[test]
fn module_udt_scalar_field_index_error_precedes_index_binding() {
    let src = "Private Type State\n    Value As Long\nEnd Type\n\
               Private s As State\n\n\
               Public Sub Main()\n    Dim r As Long\n    r = s.Value(MissingName)\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("ExpectedArray"),
        "scalar UDT field shape should raise `Expected array` before binding index expressions, got {err}"
    );
}

#[test]
fn local_variable_shadows_standard_module_property_let() {
    let src = "Private mV As Long\n\n\
               Public Property Get Value() As Long\n    Value = mV\nEnd Property\n\n\
               Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n\n\
               Public Sub Main()\n    Dim Value As Long\n    Dim r As Long\n    Value = 17\n    r = Value\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(17.0));
}

#[test]
fn indexed_property_get_let_roundtrip() {
    // `w.Value(3) = 10` is an indexed Property Let, not an array-element write
    // through a synthesized helper. The setter receives index args followed by
    // the assigned value, and the getter remains an indexed Property Get.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w.Value(3) = 10\n    r = w.Value(2)\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value(ByVal i As Long) As Long\n    Value = mV + i\nEnd Property\n\n\
                  Public Property Let Value(ByVal i As Long, ByVal v As Long)\n    mV = v + i\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(15.0));
}

#[test]
fn named_indexed_property_let_roundtrip() {
    // Named index arguments on an indexed Property Let are reordered by the
    // accessor signature before the assigned value is placed in the trailing slot.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w.Value(i := 3) = 10\n    r = w.Value(2)\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value(ByVal i As Long) As Long\n    Value = mV + i\nEnd Property\n\n\
                  Public Property Let Value(ByVal i As Long, ByVal v As Long)\n    mV = v + i\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(15.0));
}

#[test]
fn assigning_to_get_only_property_is_bind_error() {
    let main =
        "Sub Main()\n    Dim w As Widget\n    Set w = New Widget\n    w.Value = 10\nEnd Sub\n";
    let widget = "Public Property Get Value() As Long\n    Value = 1\nEnd Property\n";
    assert!(
        bind_program(&class_manifest(main, "Widget", widget), &NullTypeLibs).is_err(),
        "a get-only project property must not lower to a synthetic PropertyLet call"
    );
}

#[test]
fn assigning_to_indexed_get_only_property_is_bind_error() {
    let main =
        "Sub Main()\n    Dim w As Widget\n    Set w = New Widget\n    w.Value(3) = 10\nEnd Sub\n";
    let widget =
        "Public Property Get Value(ByVal i As Long) As Long\n    Value = i\nEnd Property\n";
    assert!(
        bind_program(&class_manifest(main, "Widget", widget), &NullTypeLibs).is_err(),
        "an indexed get-only project property must not lower to a synthetic PropertyLet call"
    );
}

#[test]
fn indexed_property_set_roundtrip() {
    // `Set b.Item(3) = t` must select the indexed Property Set accessor, carrying
    // the index argument before the object value argument.
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Dim t As Thing\n    Dim got As Thing\n    Set b = New Box\n    Set t = New Thing\n    Set b.Item(3) = t\n    Set got = b.Item(2)\n    r = got.GetVal()\nEnd Sub\n";
    let box_cls = "Private stored As Thing\n\n\
                   Public Property Get Item(ByVal i As Long) As Thing\n    Set Item = stored\nEnd Property\n\n\
                   Public Property Set Item(ByVal i As Long, ByVal v As Thing)\n    Set stored = v\nEnd Property\n";
    let thing = "Public Function GetVal() As Long\n    GetVal = 23\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, box_cls),
            ("Thing", ModuleKind::Class, thing),
        ]),
        Some(23.0)
    );
}

#[test]
fn project_default_member_get_let_roundtrip() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w(3) = 10\n    r = w(2)\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value(ByVal i As Long) As Long\n    Value = mV + i\nEnd Property\nAttribute Value.VB_UserMemId = 0\n\n\
                  Public Property Let Value(ByVal i As Long, ByVal v As Long)\n    mV = v + i\nEnd Property\nAttribute Value.VB_UserMemId = 0\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(15.0));
}

#[test]
fn project_newenum_attribute_marks_enumerator_member() {
    let main = "Sub Main()\nEnd Sub\n";
    let widget = "Public Property Get NewEnum() As IUnknown\nEnd Property\n\
                  Attribute NewEnum.VB_UserMemId = -4\n\
                  Attribute NewEnum.VB_MemberFlags = \"40\"\n";
    let program =
        bind_program(&class_manifest(main, "Widget", widget), &NullTypeLibs).expect("bind");
    let class = program
        .classes
        .iter()
        .find(|class| class.name == "Widget")
        .expect("Widget class");
    assert!(
        class.methods.iter().any(|method| {
            method.name == "NewEnum"
                && method.kind == oxvba_bundle::ProjectMemberKind::PropertyGet
                && method.is_enumerator_member
        }),
        "VB_UserMemId = -4 should mark the project-class enumerator member: {:?}",
        class.methods
    );
}

#[test]
fn project_newenum_attribute_requires_exact_minus_four_memid() {
    let main = "Sub Main()\nEnd Sub\n";
    let widget = "Public Property Get NewEnum() As IUnknown\nEnd Property\n\
                  Attribute NewEnum.VB_UserMemId = -40\n";
    let program =
        bind_program(&class_manifest(main, "Widget", widget), &NullTypeLibs).expect("bind");
    let class = program
        .classes
        .iter()
        .find(|class| class.name == "Widget")
        .expect("Widget class");
    assert!(
        class
            .methods
            .iter()
            .all(|method| !method.is_enumerator_member),
        "only exact VB_UserMemId = -4 should mark NewEnum: {:?}",
        class.methods
    );
}

#[test]
fn project_default_member_set_roundtrip() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Dim t As Thing\n    Dim got As Thing\n    Set b = New Box\n    Set t = New Thing\n    Set b(3) = t\n    Set got = b(2)\n    r = got.GetVal()\nEnd Sub\n";
    let box_cls = "Private stored As Thing\n\n\
                   Public Property Get Item(ByVal i As Long) As Thing\n    Set Item = stored\nEnd Property\nAttribute Item.VB_UserMemId = 0\n\n\
                   Public Property Set Item(ByVal i As Long, ByVal v As Thing)\n    Set stored = v\nEnd Property\nAttribute Item.VB_UserMemId = 0\n";
    let thing = "Public Function GetVal() As Long\n    GetVal = 23\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, box_cls),
            ("Thing", ModuleKind::Class, thing),
        ]),
        Some(23.0)
    );
}

#[test]
fn project_default_member_bare_get_let_roundtrip() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w = 10\n    r = w\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value() As Long\n    Value = mV\nEnd Property\nAttribute Value.VB_UserMemId = 0\n\n\
                  Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\nAttribute Value.VB_UserMemId = 0\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(10.0));
}

#[test]
fn set_assignment_keeps_defaulted_object_reference() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Dim w2 As Widget\n    Set w = New Widget\n    w = 10\n    Set w2 = w\n    w2 = 12\n    r = w\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value() As Long\n    Value = mV\nEnd Property\nAttribute Value.VB_UserMemId = 0\n\n\
                  Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\nAttribute Value.VB_UserMemId = 0\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(12.0));
}

#[test]
fn project_property_let_rhs_uses_default_member_value() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim src As Widget\n    Dim dst As Widget\n    Set src = New Widget\n    Set dst = New Widget\n    src = 7\n    dst = src\n    r = dst\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value() As Long\n    Value = mV\nEnd Property\nAttribute Value.VB_UserMemId = 0\n\n\
                  Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\nAttribute Value.VB_UserMemId = 0\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(7.0));
}

#[test]
fn let_assignment_to_object_without_default_member_is_runtime_error() {
    let main = "Sub Main()\n    Dim w As Widget\n    Set w = New Widget\n    w = 10\nEnd Sub\n";
    let widget = "Public Function GetValue() As Long\n    GetValue = 1\nEnd Function\n";
    let program =
        bind_program(&class_manifest(main, "Widget", widget), &NullTypeLibs).expect("bind_program");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let err = match oxvba_vm3::Vm3::run(&oxp, &host) {
        Ok(_) => panic!("Let into an object slot must fail"),
        Err(oxvba_vm3::Vm3Error::Fault(fault)) => fault,
        Err(other) => panic!("expected a VBA fault, got {other:?}"),
    };
    assert_eq!(err.code, 424);
}

#[test]
fn is_operator_rejects_statically_scalar_operands() {
    let err = bind_error_display(
        "Sub Main()\n    Dim a As Long\n    Dim b As Long\n    Dim r As Boolean\n    r = (a Is b)\nEnd Sub\n",
    );
    assert!(err.contains("Type mismatch"), "unexpected error: {err}");
}

#[test]
fn is_operator_variant_scalar_operands_raise_object_required() {
    for (label, src) in [
        (
            "Variant scalar Is Variant scalar",
            "Sub Main()\n    Dim a As Variant\n    Dim b As Variant\n    Dim r As Boolean\n    a = 1\n    b = 2\n    r = (a Is b)\nEnd Sub\n",
        ),
        (
            "Object Is Variant scalar",
            "Sub Main()\n    Dim o As Object\n    Dim v As Variant\n    Dim r As Boolean\n    v = 1\n    r = (o Is v)\nEnd Sub\n",
        ),
    ] {
        let program = bind_program(&manifest(src), &NullTypeLibs).expect("bind_program");
        let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
        let host = NullHostServices::new(HostPolicy::deterministic_runtime());
        let err = match oxvba_vm3::Vm3::run(&oxp, &host) {
            Ok(_) => panic!("{label} must fail"),
            Err(oxvba_vm3::Vm3Error::Fault(fault)) => fault,
            Err(other) => panic!("expected a VBA fault for {label}, got {other:?}"),
        };
        assert_eq!(err.code, 424, "{label}");
    }
}

#[test]
fn is_operator_keeps_object_and_nothing_identity() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As Widget\n    Dim b As Widget\n    Dim c As Widget\n    Dim z As Widget\n    Set a = New Widget\n    Set b = a\n    Set c = New Widget\n    If a Is b Then r = r + 1\n    If Not (a Is c) Then r = r + 10\n    If Not (a Is Nothing) Then r = r + 100\n    Set a = Nothing\n    If a Is Nothing Then r = r + 1000\n    If z Is Nothing Then r = r + 10000\nEnd Sub\n";
    assert_eq!(
        run_class_main_local0(main, "Widget", "' empty class\n"),
        Some(11111.0)
    );
}

#[test]
fn set_assigning_to_property_without_set_accessor_is_bind_error() {
    let main = "Sub Main()\n    Dim b As Box\n    Dim t As Thing\n    Set b = New Box\n    Set t = New Thing\n    Set b.Item(1) = t\nEnd Sub\n";
    let box_cls = "Public Property Get Item(ByVal i As Long) As Thing\n    Set Item = Nothing\nEnd Property\n";
    let thing = "Public Function GetVal() As Long\n    GetVal = 1\nEnd Function\n";
    assert!(
        bind_program(
            &multi_manifest(&[
                ("Main", ModuleKind::Procedural, main),
                ("Box", ModuleKind::Class, box_cls),
                ("Thing", ModuleKind::Class, thing),
            ]),
            &NullTypeLibs,
        )
        .is_err(),
        "an object property without Property Set must not lower to a synthetic setter"
    );
}

#[test]
fn method_mutates_instance_field_across_calls() {
    // Two `c.Inc` statement-calls mutate the same instance's field; Total() reads it.
    let main = "Sub Main()\n    Dim r As Long\n    Dim c As Counter\n    Set c = New Counter\n    c.Inc\n    c.Inc\n    r = c.Total()\nEnd Sub\n";
    let counter = "Private n As Long\n\n\
                   Public Sub Inc()\n    n = n + 1\nEnd Sub\n\n\
                   Public Function Total() As Long\n    Total = n\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Counter", counter), Some(2.0));
}

#[test]
fn project_method_byref_mutates_caller() {
    // o.Inc r passes r ByRef to a method; the method's write propagates back.
    // (The headline fix: method dispatch used to force ByVal.)
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As C\n    r = 5\n    Set o = New C\n    o.Inc r\nEnd Sub\n";
    let class_c = "Public Sub Inc(ByRef x As Long)\n    x = x + 100\nEnd Sub\n";
    assert_eq!(run_class_main_local0(main, "C", class_c), Some(105.0));
}

#[test]
fn byref_object_reassign_through_proc() {
    // `Set p = New Thing` through a ByRef object param rewrites the caller's
    // variable (release-old/retain-new), so `o` is a live Thing afterwards.
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As Thing\n    MakeThing o\n    r = o.GetVal()\nEnd Sub\n\n\
                Sub MakeThing(ByRef p As Thing)\n    Set p = New Thing\nEnd Sub\n";
    let thing = "Private mVal As Long\n\n\
                 Private Sub Class_Initialize()\n    mVal = 7\nEnd Sub\n\n\
                 Public Function GetVal() As Long\n    GetVal = mVal\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Thing", thing), Some(7.0));
}

#[test]
fn redim_member_array_then_use() {
    // ReDim a class instance's array field through a dotted target, then write and
    // read an element of it (the resized array must be stored back into the field).
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Set b = New Box\n    ReDim b.arr(5)\n    b.arr(2) = 7\n    r = b.arr(2)\nEnd Sub\n";
    let box_cls = "Public arr() As Long\n";
    assert_eq!(run_class_main_local0(main, "Box", box_cls), Some(7.0));
}

#[test]
fn callbyname_invokes_method() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As Calc\n    Set o = New Calc\n    r = CallByName(o, \"Add\", vbMethod, 2, 3)\nEnd Sub\n";
    let calc = "Public Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Calc", calc), Some(5.0));
}

#[test]
fn callbyname_property_let_then_get() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim ignore\n    Dim o As Box\n    Set o = New Box\n    ignore = CallByName(o, \"Value\", vbLet, 42)\n    r = CallByName(o, \"Value\", vbGet)\nEnd Sub\n";
    let box_cls = "Private mV As Long\n\n\
                   Public Property Get Value() As Long\n    Value = mV\nEnd Property\n\n\
                   Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Box", box_cls), Some(42.0));
}

#[test]
fn callbyname_unknown_member_errors() {
    let main = "Sub Main()\n    Dim r\n    Dim o As Calc\n    Set o = New Calc\n    r = CallByName(o, \"Nope\", vbMethod)\nEnd Sub\n";
    let calc = "Public Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
    let program = bind_program(&class_manifest(main, "Calc", calc), &NullTypeLibs).expect("bind");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    assert!(oxvba_vm3::Vm3::run(&oxp, &host).is_err());
}

// ── Implements: interface dispatch + TypeOf + strict Set ─────────────────────

const IANIMAL: &str = "Public Function Speak() As Long\nEnd Function\n";
const CDOG: &str = "Implements IAnimal\n\nPrivate Function IAnimal_Speak() As Long\n    IAnimal_Speak = 42\nEnd Function\n";

#[test]
fn implements_dispatch_through_interface_var() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As IAnimal\n    Set a = New CDog\n    r = a.Speak()\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CDog", ModuleKind::Class, CDOG),
        ]),
        Some(42.0)
    );
}

#[test]
fn implements_typeof_true_for_implementer() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As IAnimal\n    Set a = New CDog\n    If TypeOf a Is IAnimal Then\n        r = 1\n    Else\n        r = 0\n    End If\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CDog", ModuleKind::Class, CDOG),
        ]),
        Some(1.0)
    );
}

#[test]
fn as_new_auto_instantiates_a_user_class() {
    // `Dim t As New Thing` auto-instantiates `t` at scope entry — not Collection-
    // specific: any project class works. Without the fix `t` was `Nothing` (and was
    // mis-typed `Object("new")` by the parser-absorbs-`New` bug), so `t.GetVal()`
    // would fault.
    let main =
        "Sub Main()\n    Dim r As Long\n    Dim t As New Thing\n    r = t.GetVal()\nEnd Sub\n";
    let thing = "Public Function GetVal() As Long\n    GetVal = 7\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Thing", ModuleKind::Class, thing),
        ]),
        Some(7.0)
    );
}

#[test]
fn class_initialize_can_set_collection_field_and_read_count() {
    let main =
        "Sub Main()\n    Dim r As Long\n    Dim t As New Thing\n    r = t.ItemCount\nEnd Sub\n";
    let thing = "Private Items As VBA.Collection\n\n\
                 Private Sub Class_Initialize()\n    Set Items = New VBA.Collection\nEnd Sub\n\n\
                 Public Property Get ItemCount() As Long\n    ItemCount = Items.Count\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Thing", ModuleKind::Class, thing),
        ]),
        Some(0.0)
    );
}

#[test]
fn variant_property_set_preserves_collection_object_for_readback() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim h As New Holder\n    Dim c As New VBA.Collection\n    c.Add \"A\"\n    c.Add \"B\"\n    Set h.Body = c\n    r = h.Body.Count\nEnd Sub\n";
    let holder = "Private pBody As Variant\n\n\
                  Public Property Get Body() As Variant\n    Set Body = pBody\nEnd Property\n\n\
                  Public Property Set Body(Value As Variant)\n    Set pBody = Value\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Holder", ModuleKind::Class, holder),
        ]),
        Some(2.0)
    );
}

#[test]
fn variant_collection_typename_and_foreach_work_after_property_readback() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim h As New Holder\n    Dim c As New VBA.Collection\n    c.Add \"A\"\n    c.Add \"B\"\n    Set h.Body = c\n    r = CountItems(h.Body)\nEnd Sub\n\n\
                Public Function CountItems(Value As Variant) As Long\n    Dim item As Variant\n    If VBA.TypeName(Value) <> \"Collection\" Then\n        CountItems = -100\n        Exit Function\n    End If\n    For Each item In Value\n        CountItems = CountItems + 1\n    Next item\nEnd Function\n";
    let holder = "Private pBody As Variant\n\n\
                  Public Property Get Body() As Variant\n    Set Body = pBody\nEnd Property\n\n\
                  Public Property Set Body(Value As Variant)\n    Set pBody = Value\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Holder", ModuleKind::Class, holder),
        ]),
        Some(2.0)
    );
}

#[test]
fn with_receiver_function_result_is_evaluated_once_for_leading_dot_calls() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim s As New Suite\n    With s.It()\n        .Expect(\"actual\").ToEqual \"expected\"\n    End With\n    r = s.Created\nEnd Sub\n";
    let suite = "Public Created As Long\n\n\
                 Public Function It() As Spec\n    Created = Created + 1\n    Set It = New Spec\nEnd Function\n";
    let spec = "Public Expectations As VBA.Collection\n\n\
                Private Sub Class_Initialize()\n    Set Expectations = New VBA.Collection\nEnd Sub\n\n\
                Public Function Expect(Optional Actual As Variant) As Expectation\n    Dim e As New Expectation\n    If VBA.VarType(Actual) = VBA.vbObject Then\n        Set e.Actual = Actual\n    Else\n        e.Actual = Actual\n    End If\n    Expectations.Add e\n    Set Expect = e\nEnd Function\n";
    let expectation = "Public Actual As Variant\nPublic Expected As Variant\nPublic Passed As Boolean\n\n\
                       Public Sub ToEqual(Expected As Variant)\n    Check IsEqual(Me.Actual, Expected), Expected:=Expected\nEnd Sub\n\n\
                       Private Function IsEqual(Actual As Variant, Expected As Variant) As Variant\n    If VBA.IsObject(Actual) Or VBA.IsObject(Expected) Then\n        IsEqual = False\n    Else\n        IsEqual = Actual = Expected\n    End If\nEnd Function\n\n\
                       Private Sub Check(Result As Variant, Optional Expected As Variant)\n    If Not VBA.IsMissing(Expected) Then\n        If VBA.IsObject(Expected) Then\n            Set Me.Expected = Expected\n        Else\n            Me.Expected = Expected\n        End If\n    End If\n    Passed = Result\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Suite", ModuleKind::Class, suite),
            ("Spec", ModuleKind::Class, spec),
            ("Expectation", ModuleKind::Class, expectation),
        ]),
        Some(1.0)
    );
}

#[test]
fn isobject_false_for_scalar_variant_named_optional_argument() {
    let source = "Sub Main()\n    Dim r As Long\n    r = Outer(\"text\")\nEnd Sub\n\n\
                  Function Outer(Expected As Variant) As Long\n    Outer = Inner(Expected:=Expected)\nEnd Function\n\n\
                  Function Inner(Optional Expected As Variant) As Long\n    If VBA.IsObject(Expected) Then\n        Inner = 1\n    Else\n        Inner = 0\n    End If\nEnd Function\n";
    assert_eq!(run_main_local0(source), Some(0.0));
}

#[test]
fn isobject_guarded_set_branch_not_taken_for_scalar_named_optional_method_arg() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim e As New Expectation\n    e.ToEqual \"expected\"\n    r = e.Branch\nEnd Sub\n";
    let expectation = "Public Branch As Long\nPublic Expected As Variant\n\n\
                       Public Sub ToEqual(Expected As Variant)\n    Check Expected:=Expected\nEnd Sub\n\n\
                       Private Sub Check(Optional Expected As Variant)\n    If VBA.IsObject(Expected) Then\n        Branch = 1\n        Set Me.Expected = Expected\n    Else\n        Branch = 2\n        Me.Expected = Expected\n    End If\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Expectation", ModuleKind::Class, expectation),
        ]),
        Some(2.0)
    );
}

#[test]
fn method_argument_string_is_not_object_in_class_method() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim e As New Expectation\n    e.ToEqual \"expected\"\n    r = e.Branch\nEnd Sub\n";
    let expectation = "Public Branch As Long\n\n\
                       Public Sub ToEqual(Expected As Variant)\n    If VBA.IsObject(Expected) Then\n        Branch = 1\n    Else\n        Branch = 2\n    End If\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Expectation", ModuleKind::Class, expectation),
        ]),
        Some(2.0)
    );
}

#[test]
fn named_argument_to_private_class_method_binds_caller_parameter_value() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim e As New Expectation\n    e.ToEqual \"expected\"\n    r = e.Branch\nEnd Sub\n";
    let expectation = "Public Branch As Long\n\n\
                       Public Sub ToEqual(Expected As Variant)\n    Check Expected:=Expected\nEnd Sub\n\n\
                       Private Sub Check(Optional Expected As Variant)\n    If VBA.IsMissing(Expected) Then\n        Branch = 1\n    ElseIf Expected = \"expected\" Then\n        Branch = 2\n    Else\n        Branch = 3\n    End If\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Expectation", ModuleKind::Class, expectation),
        ]),
        Some(2.0)
    );
}

#[test]
fn statement_form_named_argument_binds_in_standard_module() {
    let source = "Sub Main()\n    Dim r As Long\n    ToEqual \"expected\"\n    r = g\nEnd Sub\n\n\
                  Public g As Long\n\n\
                  Public Sub ToEqual(Expected As Variant)\n    Check Expected:=Expected\nEnd Sub\n\n\
                  Private Sub Check(Optional Expected As Variant)\n    If VBA.IsMissing(Expected) Then\n        g = 1\n    ElseIf Expected = \"expected\" Then\n        g = 2\n    Else\n        g = 3\n    End If\nEnd Sub\n";
    assert_eq!(run_main_local0(source), Some(2.0));
}

#[test]
fn statement_form_call_accepts_negative_first_argument() {
    let source = "Sub Main()\n    Dim r As Long\n    Capture -1, 2\n    r = g\nEnd Sub\n\n\
                  Public g As Long\n\n\
                  Private Sub Capture(ByVal first As Long, ByVal second As Long)\n    g = first + second\nEnd Sub\n";
    assert_eq!(run_main_local0(source), Some(1.0));
}

#[test]
fn implements_typeof_false_for_non_implementer() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As Object\n    Set o = New CRock\n    If TypeOf o Is IAnimal Then\n        r = 1\n    Else\n        r = 0\n    End If\nEnd Sub\n";
    let crock = "Public Function Foo() As Long\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CRock", ModuleKind::Class, crock),
        ]),
        Some(0.0)
    );
}

#[test]
fn implements_set_type_mismatch_errors() {
    // Set into an IAnimal-typed var with a class that does not implement it → error 13.
    let main = "Sub Main()\n    Dim a As IAnimal\n    Set a = New CRock\nEnd Sub\n";
    let crock = "Public Function Foo() As Long\nEnd Function\n";
    let program = bind_program(
        &multi_manifest(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CRock", ModuleKind::Class, crock),
        ]),
        &NullTypeLibs,
    )
    .expect("bind");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    match oxvba_vm3::Vm3::run(&oxp, &host) {
        Ok(_) => panic!("Set of a non-implementing class into an interface var must fail"),
        Err(oxvba_vm3::Vm3Error::Fault(fault)) => assert_eq!(
            fault.code, 13,
            "interface type mismatch must be VBA error 13 (Type mismatch)"
        ),
        Err(other) => panic!("expected a VBA type-mismatch fault, got {other:?}"),
    }
}

#[test]
fn implements_missing_member_is_bind_error() {
    // CDog declares Implements IAnimal but omits IAnimal_Speak → bind error.
    let main = "Sub Main()\nEnd Sub\n";
    let cdog = "Implements IAnimal\n";
    assert!(
        bind_program(
            &multi_manifest(&[
                ("Main", ModuleKind::Procedural, main),
                ("IAnimal", ModuleKind::Class, IANIMAL),
                ("CDog", ModuleKind::Class, cdog),
            ]),
            &NullTypeLibs,
        )
        .is_err()
    );
}

#[test]
fn implements_property_through_interface_var() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim s As IShape\n    Set s = New CBox\n    s.Size = 10\n    r = s.Size\nEnd Sub\n";
    let ishape = "Public Property Get Size() As Long\nEnd Property\n\n\
                  Public Property Let Size(ByVal v As Long)\nEnd Property\n";
    let cbox = "Implements IShape\n\nPrivate mS As Long\n\n\
                Private Property Get IShape_Size() As Long\n    IShape_Size = mS\nEnd Property\n\n\
                Private Property Let IShape_Size(ByVal v As Long)\n    mS = v\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IShape", ModuleKind::Class, ishape),
            ("CBox", ModuleKind::Class, cbox),
        ]),
        Some(10.0)
    );
}

#[test]
fn implements_indexed_property_through_interface_var() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim s As IShape\n    Set s = New CBox\n    s.Size(3) = 10\n    r = s.Size(2)\nEnd Sub\n";
    let ishape = "Public Property Get Size(ByVal i As Long) As Long\nEnd Property\n\n\
                  Public Property Let Size(ByVal i As Long, ByVal v As Long)\nEnd Property\n";
    let cbox = "Implements IShape\n\nPrivate mS As Long\n\n\
                Private Property Get IShape_Size(ByVal i As Long) As Long\n    IShape_Size = mS + i\nEnd Property\n\n\
                Private Property Let IShape_Size(ByVal i As Long, ByVal v As Long)\n    mS = v + i\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IShape", ModuleKind::Class, ishape),
            ("CBox", ModuleKind::Class, cbox),
        ]),
        Some(15.0)
    );
}

// ── New of a COM coclass (lowers to CreateObject activation) ─────────────────

struct WidgetTypeLibs;
impl TypeLibResolver for WidgetTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        Some(oxvba_com::TypeLibMetadataBlob {
            identity: oxvba_com::TypeLibResolvedIdentity {
                reference_name: "Widget".into(),
                requested_coclass: None,
                importlib: "widget".into(),
                libid: None,
                major_version: 1,
                minor_version: 0,
                lcid: None,
                cache_key: "widget".into(),
            },
            activation_prog_id: Some("Widget.Thing".into()),
            member_name_to_token: Vec::new(),
            members: Vec::new(),
            events: Vec::new(),
            coclass_names: Vec::new(),
        })
    }
}

#[test]
fn new_com_coclass_lowers_to_create_object() {
    let main = "Sub Main()\n    Dim x As Widget\n    Set x = New Widget\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "Widget".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: None,
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &WidgetTypeLibs).expect("bind_program");
    // `New Widget` resolves the coclass to its ProgID and lowers to CreateObject.
    assert!(format!("{program:?}").contains("CreateObject"));
}

#[test]
fn getobject_call_lowers_to_native_getobject() {
    // `GetObject` is a SpecialForm intrinsic that keeps the bespoke `Native` route (like
    // `CreateObject`); each of its three call shapes must reach `NativeImplId::GetObject`,
    // including the leading-omitted `GetObject(, class)` (the running-instance mode).
    for src in [
        "Sub Main()\n    Dim x As Object\n    Set x = GetObject(\"c:\\book.xlsx\")\nEnd Sub\n",
        "Sub Main()\n    Dim x As Object\n    Set x = GetObject(, \"Excel.Application\")\nEnd Sub\n",
        "Sub Main()\n    Dim x As Object\n    Set x = GetObject(\"\", \"Scripting.Dictionary\")\nEnd Sub\n",
    ] {
        let program = bind(src);
        assert!(
            format!("{program:?}").contains("GetObject"),
            "expected a Native(GetObject) call for:\n{src}"
        );
    }
}

#[test]
fn getobject_omitted_pathname_passes_an_empty_first_arg() {
    // `GetObject(, "Excel.Application")` — the omitted pathname must reach the native call as
    // an Omitted arg (the VM materializes it as `Empty`), so the HAL can tell the
    // running-instance mode from a present `""` (new-instance). Assert the first arg is
    // Omitted and the second is the class string.
    let program = bind(
        "Sub Main()\n    Dim x As Object\n    Set x = GetObject(, \"Excel.Application\")\nEnd Sub\n",
    );
    fn native_getobject_args(program: &CoreProgram) -> Option<&Vec<CoreArg>> {
        fn find(value: &CoreValue) -> Option<&Vec<CoreArg>> {
            match value {
                CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::GetObject),
                    args,
                } => Some(args),
                CoreValue::Coerce { value, .. } => find(value),
                _ => None,
            }
        }
        program
            .procs
            .iter()
            .flat_map(|p| &p.body)
            .find_map(|s| match s {
                CoreStmt::Assign { value, .. } => find(value),
                CoreStmt::Eval(value) => find(value),
                _ => None,
            })
    }
    let args = native_getobject_args(&program).expect("a Native(GetObject) call");
    assert_eq!(args.len(), 2, "pathname + class: {args:?}");
    assert!(
        matches!(args[0], CoreArg::Omitted),
        "omitted pathname must stay Omitted: {:?}",
        args[0]
    );
}

// ── Events: WithEvents + RaiseEvent routing ──────────────────────────────────

fn multi_manifest(modules: &[(&str, ModuleKind, &str)]) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: modules
            .iter()
            .map(|(name, kind, src)| ModuleUnit {
                module_name: (*name).into(),
                module_kind: *kind,
                attributes: ModuleAttributes::named(*name),
                source: (*src).into(),
            })
            .collect(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn run_multi_main_local0(modules: &[(&str, ModuleKind, &str)]) -> Option<f64> {
    let program = bind_program(&multi_manifest(modules), &NullTypeLibs).expect("bind_program");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm3::Vm3::run(&oxp, &host).expect("run");
    let value = vm.slot(oxp.globals.len())?;
    value
        .as_f64()
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
}

#[test]
fn module_private_fixed_array_global_in_non_entry_module_is_allocated() {
    let main = "Sub Main()\n    Dim r As Long\n    AddTopic 7\n    r = TopicTotal()\nEnd Sub\n";
    let helper = "Private gTopicIds(1 To 2) As Long\nPrivate gTopicCount As Long\n\n\
                  Public Sub AddTopic(ByVal topicId As Long)\n    gTopicCount = gTopicCount + 1\n    gTopicIds(gTopicCount) = topicId\nEnd Sub\n\n\
                  Public Function TopicTotal() As Long\n    TopicTotal = gTopicIds(1)\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("RtdTimer", ModuleKind::Procedural, helper),
        ]),
        Some(7.0)
    );
}

#[test]
fn module_private_fixed_array_global_in_procedureless_module_is_initialized() {
    let program = bind_program(
        &multi_manifest(&[(
            "Globals",
            ModuleKind::Procedural,
            "Private gTopicIds(1 To 2) As Long\n",
        )]),
        &NullTypeLibs,
    )
    .expect("bind procedureless module global");
    assert!(
        program.global_initializer.is_some(),
        "fixed-size module array should allocate even when the project has no procedures"
    );
}

#[test]
fn withevents_raise_event_routes_to_handler() {
    // `Set k.Watched = s` subscribes the sink; `s.Fire` raises the event, which
    // routes to `Watched_Fired` (run with the sink's Me) and sets `k.Got`.
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    s.Fire\n    r = k.Got\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                Private Sub Watched_Fired(ByVal v As Long)\n    Got = v\nEnd Sub\n";
    let source = "Public Event Fired(ByVal v As Long)\n\n\
                  Public Sub Fire()\n    RaiseEvent Fired(99)\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(99.0)
    );
}

#[test]
fn raise_event_outside_class_module_is_bind_error() {
    let err = bind_error("Sub Main()\n    RaiseEvent Tick\nEnd Sub\n");
    assert!(
        err.contains("RaiseEvent outside a class module"),
        "unexpected error: {err}"
    );
}

#[test]
fn raise_event_undeclared_event_is_bind_error() {
    let err = format!(
        "{:?}",
        bind_program(
            &multi_manifest(&[
                ("Main", ModuleKind::Procedural, "Sub Main()\nEnd Sub\n"),
                (
                    "Emitter",
                    ModuleKind::Class,
                    "Public Sub Fire()\n    RaiseEvent Tick\nEnd Sub\n",
                ),
            ]),
            &NullTypeLibs,
        )
        .expect_err("undeclared RaiseEvent target should fail binding")
    );
    assert!(
        err.to_ascii_lowercase().contains("tick"),
        "unexpected error: {err}"
    );
}

#[test]
fn two_sink_classes_same_source_event_route_independently() {
    // Two DISTINCT sink classes each `WithEvents Watched As Source` with their own
    // `Watched_Fired` handler. A single `RaiseEvent Fired` on the shared source must
    // dispatch to BOTH handlers independently. Regression: the binder used to draw a
    // WithEvents field's binding token from a PER-CLASS counter, so both sink fields
    // got token 1 and the bundle's `event_routes[(binding, event)]` collided — vm2
    // panicked at `LoadedBundle::load` (the dedup invariant). A bundle-global binding
    // counter gives each field a distinct token, so both routes coexist.
    //
    // The handlers write DIFFERENT values (v vs v+1) so the asserted result proves
    // each route reached its OWN handler, not that one shadowed the other.
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As SinkA\n    Dim b As SinkB\n    Dim s As Source\n    \
                Set s = New Source\n    Set a = New SinkA\n    Set b = New SinkB\n    \
                Set a.Watched = s\n    Set b.Watched = s\n    s.Fire\n    \
                r = a.Got * 1000 + b.Got\nEnd Sub\n";
    let sink_a = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                  Private Sub Watched_Fired(ByVal v As Long)\n    Got = v\nEnd Sub\n";
    let sink_b = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                  Private Sub Watched_Fired(ByVal v As Long)\n    Got = v + 1\nEnd Sub\n";
    let source = "Public Event Fired(ByVal v As Long)\n\n\
                  Public Sub Fire()\n    RaiseEvent Fired(99)\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("SinkA", ModuleKind::Class, sink_a),
            ("SinkB", ModuleKind::Class, sink_b),
            ("Source", ModuleKind::Class, source),
        ]),
        // a.Got = 99, b.Got = 100  ⇒  99 * 1000 + 100
        Some(99_100.0),
        "both sink classes must route the shared event to their own handler",
    );
}

#[test]
fn same_sink_class_twice_each_instance_dispatches() {
    // Two INSTANCES of ONE sink class, both subscribed to the same source. They share
    // a single binding token (and handler proc) — correctly, since the token names a
    // (sink class, field), not an instance — yet each must dispatch with its own `Me`.
    // The owner identity in the subscription key disambiguates them. Guards the
    // per-instance half of the invariant the bundle-global token change relies on.
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As Sink\n    Dim b As Sink\n    Dim s As Source\n    \
                Set s = New Source\n    Set a = New Sink\n    Set b = New Sink\n    \
                Set a.Watched = s\n    Set b.Watched = s\n    s.Fire\n    \
                r = a.Got + b.Got\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                Private Sub Watched_Fired(ByVal v As Long)\n    Got = v\nEnd Sub\n";
    let source = "Public Event Fired(ByVal v As Long)\n\n\
                  Public Sub Fire()\n    RaiseEvent Fired(99)\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        // Both instances' handlers fire with their own Me: a.Got = 99, b.Got = 99.
        Some(198.0),
        "both instances of one sink class must dispatch independently",
    );
}

#[test]
fn raise_event_byref_param_writes_back_to_raiser() {
    // VBA event parameters default to ByRef, and RaiseEvent is synchronous, so a
    // handler that assigns to a ByRef parameter mutates the raiser's variable
    // after RaiseEvent returns. (RaiseEvent args used to bind without the event
    // signature, defaulting every argument to ByVal and dropping the write-back.)
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    r = s.FireWith(7)\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\n\n\
                Private Sub Watched_Poked(v As Long)\n    v = 1234\nEnd Sub\n";
    let source = "Public Event Poked(v As Long)\n\n\
                  Public Function FireWith(ByVal start As Long) As Long\n    Dim x As Long\n    x = start\n    RaiseEvent Poked(x)\n    FireWith = x\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(1234.0),
        "the handler's write to its ByRef parameter must reach the raiser's variable"
    );
}

#[test]
fn raise_event_prefers_event_over_same_named_property() {
    // `RaiseEvent Result(...)` must target the event declaration even when the
    // class also has a `Result` property. VBA-Web's SpecSuite has this shape.
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    s.Fire\n    r = k.Got\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                Private Sub Watched_Result(ByVal v As Long)\n    Got = v\nEnd Sub\n";
    let source = "Public Event Result(ByVal v As Long)\n\n\
                  Public Property Get Result() As Long\n    Result = -1\nEnd Property\n\n\
                  Public Sub Fire()\n    RaiseEvent Result(7)\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(7.0)
    );
}

#[test]
fn raise_event_byval_param_does_not_write_back() {
    // The event declares its parameter ByVal, so a handler that assigns to it
    // must mutate a copy — never the raiser's variable. (RaiseEvent args used
    // to bind without the event's signature, so an l-value argument took the
    // ByRef default and the handler's write corrupted the raiser's local.)
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    r = s.FireWith(7)\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\n\n\
                Private Sub Watched_Poked(ByVal v As Long)\n    v = 1234\nEnd Sub\n";
    let source = "Public Event Poked(ByVal v As Long)\n\n\
                  Public Function FireWith(ByVal start As Long) As Long\n    Dim x As Long\n    x = start\n    RaiseEvent Poked(x)\n    FireWith = x\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(7.0),
        "the handler's write to its ByVal parameter must not reach the raiser"
    );
}

#[test]
fn raise_event_accepts_bracketed_reserved_event_name() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    s.Fire\n    r = k.Got\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                Private Sub Watched_Exit()\n    Got = 42\nEnd Sub\n";
    let source = "Public Event [Exit]()\n\n\
                  Public Sub Fire()\n    RaiseEvent [Exit]\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(42.0)
    );
}

// ── Class-module UDT fields: per-instance default record-init ────────────────

#[test]
fn class_udt_field_initializes_and_round_trips() {
    // A class module with a scalar UDT field (`Private p As Point`) must bind
    // without faulting `"is not a variable"` (the field resolves through `Me`,
    // unreachable from the entry proc's frame — its default record-init is
    // emitted per-instance into `Class_Initialize`). At runtime the field is a
    // default record, so writing/reading a sub-field round-trips.
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Set b = New Box\n\
                \x20   b.SetX 42\n    r = b.GetX\nEnd Sub\n";
    let class = "Private Type Point\n  X As Long\n  Y As Long\nEnd Type\n\
                 Private p As Point\n\n\
                 Private Sub Class_Initialize()\n    p.Y = 0\nEnd Sub\n\n\
                 Public Sub SetX(ByVal v As Long)\n    p.X = v\nEnd Sub\n\n\
                 Public Property Get GetX() As Long\n    GetX = p.X\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, class),
        ]),
        Some(42.0),
        "a class scalar-UDT field is a default record, so its sub-field round-trips"
    );
}

#[test]
fn class_with_udt_field_and_no_class_initialize_binds() {
    // The same shape with *no* `Class_Initialize`: binding must still SKIP the
    // class field as a bundle global (no `"is not a variable"`). This guards the
    // bind half — the runtime materialization is the gap below.
    let main = "Sub Main()\n    Dim b As Box\n    Set b = New Box\nEnd Sub\n";
    let class = "Private Type Point\n  X As Long\nEnd Type\n\
                 Private p As Point\n\n\
                 Public Sub SetX(ByVal v As Long)\n    p.X = v\nEnd Sub\n";
    // Binds + links + runs (Main never touches the field) without panicking.
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, class),
        ]),
        None
    );
}

// KNOWN GAP: a class scalar-UDT field whose class has NO `Class_Initialize` is
// never default-record-initialized (the per-instance record-init is emitted
// into the `Class_Initialize` prologue; with no such proc there is nowhere to
// emit it). Writing a sub-field then faults type 13 ("record expected"). The
// general fix is to SYNTHESIZE a `Class_Initialize` (a new ProcId wired as the
// class's initialize) when a class with UDT fields has none — a class-build-seam
// change deferred here. ChibiEx HAS a `Class_Initialize`, so this gap does not
// affect the acceptance test.
#[test]
#[ignore = "no-Class_Initialize UDT-field record-init: synthesize-prologue path deferred"]
fn class_udt_field_without_class_initialize_round_trips() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Set b = New Box\n\
                \x20   b.SetX 7\n    r = b.GetX\nEnd Sub\n";
    let class = "Private Type Point\n  X As Long\nEnd Type\n\
                 Private p As Point\n\n\
                 Public Sub SetX(ByVal v As Long)\n    p.X = v\nEnd Sub\n\n\
                 Public Property Get GetX() As Long\n    GetX = p.X\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, class),
        ]),
        Some(7.0)
    );
}

// ── COM-source WithEvents: binder emits EventRoutes for a typelib coclass ────

/// Resolve the fixture `OxVba.TestEventServer` typelib through the real fixture
/// catalog (enabled by the `fixture-typelibs` dev-dep), so the bind test exercises
/// the genuine event classification (OnSimpleEvent/OnValueChanged/OnPairChanged,
/// all `Dispatch` path) rather than a hand-rolled blob.
struct EventServerTypeLibs;
impl TypeLibResolver for EventServerTypeLibs {
    fn resolve(
        &self,
        request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        let identity = oxvba_com::resolve_known_typelib_identity(request)?;
        Some(oxvba_com::build_typelib_metadata(&identity))
    }
}

#[test]
fn withevents_com_source_emits_event_route() {
    // A `WithEvents` field typed as a referenced COM coclass plus a matching
    // `<field>_<EventName>` handler must produce an `EventRoute` bound to that
    // handler, keyed on the event's dispid/token (the runtime subscribe key).
    let main = "Private WithEvents x As OxVba.TestEventServer\n\n\
                Private Sub x_OnValueChanged(ByVal v As Long)\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Class,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "OxVba_TestEventServer".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("oxvba_testeventserver.tlb".into()),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &EventServerTypeLibs).expect("bind_program");

    // OnValueChanged is event token 2 in the fixture typelib; exactly one route is
    // emitted (only OnValueChanged has a matching handler) and it targets the
    // handler proc, keyed on that token.
    assert_eq!(
        program.event_routes.len(),
        1,
        "exactly one EventRoute (only OnValueChanged has a handler): {:?}",
        program.event_routes
    );
    let route = &program.event_routes[0];
    assert_eq!(
        route.event, 2,
        "route keyed on the OnValueChanged dispid/token"
    );
    let handler = &program.procs[route.handler];
    assert!(
        handler.name.eq_ignore_ascii_case("x_OnValueChanged"),
        "route targets the x_OnValueChanged handler, got: {}",
        handler.name
    );
}

#[test]
fn withevents_com_source_without_handler_emits_no_route() {
    // A COM-source `WithEvents` field with no matching handler emits no route
    // (nothing to subscribe).
    let main = "Private WithEvents x As OxVba.TestEventServer\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Class,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "OxVba_TestEventServer".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("oxvba_testeventserver.tlb".into()),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &EventServerTypeLibs).expect("bind_program");
    assert!(
        program.event_routes.is_empty(),
        "no handler ⇒ no EventRoute: {:?}",
        program.event_routes
    );
}

// ── COM early binding: typed receiver lowers a member call to EarlyCom{dispid} ──

#[test]
fn typed_com_receiver_member_call_lowers_to_early_com() {
    // A receiver typed as a referenced COM coclass (`Dim s As OxVba.TestEventServer`)
    // resolves its member against the real fixture typelib, so `s.Ping()` lowers to an
    // early-bound dispatch keyed on Ping's dispid (104 in the fixture) — NOT a
    // by-name LateDispatch (the untyped-Object default). This proves the whole
    // early-bind lowering against a genuine typelib blob, headlessly.
    let main = "Sub Main()\n    Dim s As OxVba.TestEventServer\n    Dim r As Long\n    r = s.Ping()\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "OxVba_TestEventServer".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("oxvba_testeventserver.tlb".into()),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &EventServerTypeLibs).expect("bind_program");

    // Exactly one early-bound COM call, on Ping's dispid; and no late dispatch to Ping
    // (that would mean the typed receiver was treated as an untyped Object).
    let callees = top_level_callees(&program);
    assert!(
        callees
            .iter()
            .any(|c| matches!(c, CoreCallee::EarlyCom { member, .. } if member.token == 104)),
        "expected an EarlyCom on Ping's dispid (104), got: {callees:?}"
    );
    assert!(
        !callees.iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch { name, .. } if name.eq_ignore_ascii_case("Ping")
        )),
        "Ping on a typed receiver must NOT lower to LateDispatch: {callees:?}"
    );
}

struct DefaultValueTypeLibs;
impl TypeLibResolver for DefaultValueTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        Some(oxvba_com::TypeLibMetadataBlob {
            identity: oxvba_com::TypeLibResolvedIdentity {
                reference_name: "Widget".into(),
                requested_coclass: None,
                importlib: "widget".into(),
                libid: None,
                major_version: 1,
                minor_version: 0,
                lcid: None,
                cache_key: "widget".into(),
            },
            activation_prog_id: Some("Widget.Thing".into()),
            member_name_to_token: vec![("Value".into(), 0)],
            members: vec![
                oxvba_com::TypeLibMemberMetadata {
                    name: "Value".into(),
                    token: 0,
                    vtable_slot: None,
                    requires_argument: false,
                    invoke_kind: oxvba_com::TypeLibMemberInvokeKind::PropertyGet,
                    parameter_names: Vec::new(),
                    parameter_optional: Vec::new(),
                    parameter_optional_defaults: Vec::new(),
                    is_default_member: true,
                    parameter_types: Vec::new(),
                    parameter_iids: Vec::new(),
                    return_type: Some(oxvba_com::TypeLibParamType::Long),
                    parameter_wire_types: Vec::new(),
                    return_wire_type: Some(oxvba_com::TypeLibWireType::Automation(
                        oxvba_com::TypeLibParamType::Long,
                    )),
                    callconv_is_stdcall: false,
                    is_dual: true,
                    interface_iid: None,
                    source_typekind: Some(oxvba_com::SourceTypeKind::Dispatch),
                    vtable_slot_bound: None,
                },
                oxvba_com::TypeLibMemberMetadata {
                    name: "Value".into(),
                    token: 0,
                    vtable_slot: None,
                    requires_argument: true,
                    invoke_kind: oxvba_com::TypeLibMemberInvokeKind::PropertyPut,
                    parameter_names: vec!["Value".into()],
                    parameter_optional: vec![false],
                    parameter_optional_defaults: Vec::new(),
                    is_default_member: true,
                    parameter_types: vec![oxvba_com::TypeLibParamType::Long],
                    parameter_wire_types: vec![oxvba_com::TypeLibWireType::Automation(
                        oxvba_com::TypeLibParamType::Long,
                    )],
                    parameter_iids: vec![None],
                    return_type: None,
                    return_wire_type: None,
                    callconv_is_stdcall: false,
                    is_dual: true,
                    interface_iid: None,
                    source_typekind: Some(oxvba_com::SourceTypeKind::Dispatch),
                    vtable_slot_bound: None,
                },
            ],
            events: Vec::new(),
            coclass_names: Vec::new(),
        })
    }
}

struct DefaultValuePutFirstTypeLibs;
impl TypeLibResolver for DefaultValuePutFirstTypeLibs {
    fn resolve(
        &self,
        request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        let mut blob = DefaultValueTypeLibs.resolve(request)?;
        blob.members.reverse();
        Some(blob)
    }
}

struct ApplicationTypeLibs;
impl TypeLibResolver for ApplicationTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        Some(oxvba_com::TypeLibMetadataBlob {
            identity: oxvba_com::TypeLibResolvedIdentity {
                reference_name: "Excel".into(),
                requested_coclass: Some("Application".into()),
                importlib: "excel".into(),
                libid: None,
                major_version: 1,
                minor_version: 0,
                lcid: None,
                cache_key: "excel-application-test".into(),
            },
            activation_prog_id: Some("Excel.Application".into()),
            member_name_to_token: vec![("Run".into(), 10), ("OnTime".into(), 11)],
            members: vec![
                oxvba_com::TypeLibMemberMetadata {
                    name: "Run".into(),
                    token: 10,
                    vtable_slot: None,
                    requires_argument: true,
                    invoke_kind: oxvba_com::TypeLibMemberInvokeKind::Method,
                    parameter_names: vec!["Macro".into(), "Arg1".into()],
                    parameter_optional: vec![false, true],
                    parameter_optional_defaults: vec![
                        oxvba_com::OptionalParamDefault::Required,
                        oxvba_com::OptionalParamDefault::OptionalVariant,
                    ],
                    is_default_member: false,
                    parameter_types: vec![
                        oxvba_com::TypeLibParamType::Variant,
                        oxvba_com::TypeLibParamType::Variant,
                    ],
                    parameter_iids: vec![None, None],
                    return_type: Some(oxvba_com::TypeLibParamType::Variant),
                    parameter_wire_types: vec![
                        oxvba_com::TypeLibWireType::Automation(
                            oxvba_com::TypeLibParamType::Variant,
                        ),
                        oxvba_com::TypeLibWireType::Automation(
                            oxvba_com::TypeLibParamType::Variant,
                        ),
                    ],
                    return_wire_type: Some(oxvba_com::TypeLibWireType::Automation(
                        oxvba_com::TypeLibParamType::Variant,
                    )),
                    callconv_is_stdcall: false,
                    is_dual: true,
                    interface_iid: None,
                    source_typekind: Some(oxvba_com::SourceTypeKind::Dispatch),
                    vtable_slot_bound: None,
                },
                oxvba_com::TypeLibMemberMetadata {
                    name: "OnTime".into(),
                    token: 11,
                    vtable_slot: None,
                    requires_argument: true,
                    invoke_kind: oxvba_com::TypeLibMemberInvokeKind::Method,
                    parameter_names: vec![
                        "EarliestTime".into(),
                        "Procedure".into(),
                        "LatestTime".into(),
                        "Schedule".into(),
                    ],
                    parameter_optional: vec![false, false, true, true],
                    parameter_optional_defaults: vec![
                        oxvba_com::OptionalParamDefault::Required,
                        oxvba_com::OptionalParamDefault::Required,
                        oxvba_com::OptionalParamDefault::OptionalVariant,
                        oxvba_com::OptionalParamDefault::OptionalVariant,
                    ],
                    is_default_member: false,
                    parameter_types: vec![
                        oxvba_com::TypeLibParamType::Variant,
                        oxvba_com::TypeLibParamType::String,
                        oxvba_com::TypeLibParamType::Variant,
                        oxvba_com::TypeLibParamType::Variant,
                    ],
                    parameter_iids: vec![None, None, None, None],
                    return_type: None,
                    parameter_wire_types: vec![
                        oxvba_com::TypeLibWireType::Automation(
                            oxvba_com::TypeLibParamType::Variant,
                        ),
                        oxvba_com::TypeLibWireType::Automation(oxvba_com::TypeLibParamType::String),
                        oxvba_com::TypeLibWireType::Automation(
                            oxvba_com::TypeLibParamType::Variant,
                        ),
                        oxvba_com::TypeLibWireType::Automation(
                            oxvba_com::TypeLibParamType::Variant,
                        ),
                    ],
                    return_wire_type: None,
                    callconv_is_stdcall: false,
                    is_dual: true,
                    interface_iid: None,
                    source_typekind: Some(oxvba_com::SourceTypeKind::Dispatch),
                    vtable_slot_bound: None,
                },
            ],
            events: Vec::new(),
            coclass_names: vec!["Application".into()],
        })
    }
}

#[test]
fn typed_com_default_member_bare_let_get_lowers_to_early_com() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Dim w2 As Widget\n    w = 10\n    Set w2 = w\n    r = w2\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "Widget".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: None,
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &DefaultValueTypeLibs).expect("bind_program");
    let callees = top_level_callees(&program);
    assert_eq!(
        callees
            .iter()
            .filter(|c| matches!(
                c,
                CoreCallee::EarlyCom {
                    kind: Some(oxvba_bundle::ProjectMemberKind::PropertyLet),
                    member,
                    ..
                } if member.token == 0
                    && member.invoke_kind == oxvba_com::TypeLibMemberInvokeKind::PropertyPut
            ))
            .count(),
        1,
        "bare `w = 10` should be one early-bound default PropertyLet: {callees:?}"
    );
    assert_eq!(
        callees
            .iter()
            .filter(|c| matches!(
                c,
                CoreCallee::EarlyCom {
                    kind: Some(oxvba_bundle::ProjectMemberKind::PropertyGet),
                    member,
                    ..
                } if member.token == 0
                    && member.invoke_kind == oxvba_com::TypeLibMemberInvokeKind::PropertyGet
            ))
            .count(),
        1,
        "bare `r = w2` should be one early-bound default PropertyGet, while `Set w2 = w` stays object assignment: {callees:?}"
    );
}

#[test]
fn typed_com_default_member_put_before_get_preserves_accessor_descriptors() {
    let main =
        "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    w = 10\n    r = w\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "Widget".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: None,
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &DefaultValuePutFirstTypeLibs).expect("bind_program");
    let callees = top_level_callees(&program);
    assert_eq!(
        callees
            .iter()
            .filter(|c| matches!(
                c,
                CoreCallee::EarlyCom {
                    kind: Some(oxvba_bundle::ProjectMemberKind::PropertyLet),
                    member,
                    ..
                } if member.token == 0
                    && member.invoke_kind == oxvba_com::TypeLibMemberInvokeKind::PropertyPut
            ))
            .count(),
        1,
        "put-before-get metadata should still carry the PropertyPut descriptor for `w = 10`: {callees:?}"
    );
    assert_eq!(
        callees
            .iter()
            .filter(|c| matches!(
                c,
                CoreCallee::EarlyCom {
                    kind: Some(oxvba_bundle::ProjectMemberKind::PropertyGet),
                    member,
                    ..
                } if member.token == 0
                    && member.invoke_kind == oxvba_com::TypeLibMemberInvokeKind::PropertyGet
            ))
            .count(),
        1,
        "put-before-get metadata should still carry the PropertyGet descriptor for `r = w`: {callees:?}"
    );
}

#[test]
fn host_injected_default_member_bare_let_get_lowers_to_early_com() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Dim w2 As Widget\n    w = 10\n    Set w2 = w\n    r = w2\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::HostInjected {
            referenced_project_name: "Widget".into(),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &DefaultValueTypeLibs).expect("bind_program");
    let callees = top_level_callees(&program);
    assert_eq!(
        callees
            .iter()
            .filter(|c| matches!(
                c,
                CoreCallee::EarlyCom {
                    kind: Some(oxvba_bundle::ProjectMemberKind::PropertyLet),
                    member,
                    ..
                } if member.token == 0
                    && member.invoke_kind == oxvba_com::TypeLibMemberInvokeKind::PropertyPut
            ))
            .count(),
        1,
        "host-injected bare `w = 10` should be one early-bound default PropertyLet: {callees:?}"
    );
    assert_eq!(
        callees
            .iter()
            .filter(|c| matches!(
                c,
                CoreCallee::EarlyCom {
                    kind: Some(oxvba_bundle::ProjectMemberKind::PropertyGet),
                    member,
                    ..
                } if member.token == 0
                    && member.invoke_kind == oxvba_com::TypeLibMemberInvokeKind::PropertyGet
            ))
            .count(),
        1,
        "host-injected bare `r = w2` should be one early-bound default PropertyGet, while `Set w2 = w` stays object assignment: {callees:?}"
    );
}

#[test]
fn host_injected_root_object_member_lowers_through_com_metadata() {
    let main = "Sub Main()\n    Dim r As Long\n    r = Widget.Value\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::HostInjected {
            referenced_project_name: "Widget".into(),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &DefaultValueTypeLibs).expect("bind_program");
    let callees = top_level_callees(&program);
    assert!(
        callees.iter().any(|c| matches!(
            c,
            CoreCallee::EarlyCom {
                kind: Some(oxvba_bundle::ProjectMemberKind::PropertyGet),
                member,
                ..
            } if member.token == 0
        )),
        "host root member should bind against host-injected typelib metadata: {callees:?}"
    );
}

#[test]
fn host_injected_application_run_and_ontime_lower_through_com_metadata() {
    let main = "Sub Main()\n    Dim r As Variant\n    r = Application.Run(\"MacroName\", 1)\n    Application.OnTime 0, \"MacroName\"\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::HostInjected {
            referenced_project_name: "Excel".into(),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let program = bind_program(&manifest, &ApplicationTypeLibs).expect("bind_program");
    let callees = top_level_callees(&program);
    assert!(
        callees.iter().any(|c| matches!(
            c,
            CoreCallee::EarlyCom {
                kind: Some(oxvba_bundle::ProjectMemberKind::Method),
                member,
                ..
            } if member.token == 10
        )),
        "Application.Run should bind through the host-injected Excel metadata: {callees:?}"
    );
    assert!(
        callees.iter().any(|c| matches!(
            c,
            CoreCallee::EarlyCom {
                kind: Some(oxvba_bundle::ProjectMemberKind::Method),
                member,
                ..
            } if member.token == 11
        )),
        "Application.OnTime should bind through the host-injected Excel metadata: {callees:?}"
    );
}

// ── COM late dispatch + Declare (structural — emit-correct, not run) ─────────

/// The callee of a value, unwrapping a `Coerce` wrapper (an assignment to a typed
/// target coerces the call result).
fn callee_of(value: &CoreValue) -> Option<&CoreCallee> {
    match value {
        CoreValue::Call { callee, .. } => Some(callee),
        CoreValue::Coerce { value, .. } => callee_of(value),
        _ => None,
    }
}

/// The callees of top-level `Assign`/`Eval` calls in every procedure.
fn top_level_callees(program: &CoreProgram) -> Vec<&CoreCallee> {
    let mut out = Vec::new();
    for proc in &program.procs {
        for stmt in &proc.body {
            let value = match stmt {
                CoreStmt::Assign { value, .. } => Some(value),
                CoreStmt::Eval(value) => Some(value),
                _ => None,
            };
            if let Some(callee) = value.and_then(callee_of) {
                out.push(callee);
            }
        }
    }
    out
}

#[test]
fn late_bound_member_call_on_object() {
    // A member call on an untyped `Object` (no typelib) becomes a by-name late
    // dispatch — the VBA late-binding default.
    let src = "Sub Main()\n    Dim o As Object\n    Dim r\n    r = o.Compute(1)\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program).iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch { name, .. } if name.eq_ignore_ascii_case("Compute")
        )),
        "expected a LateDispatch to Compute"
    );
}

#[test]
fn late_bound_object_index_lowers_to_default_member_get() {
    let src = "Sub Main()\n    Dim o As Object\n    Dim r\n    r = o(1)\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program).iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch {
                default_member: true,
                kind: Some(oxvba_bundle::ProjectMemberKind::PropertyGet),
                ..
            }
        )),
        "expected Object index read to lower to default-member PropertyGet"
    );
}

#[test]
fn late_bound_property_put_on_object() {
    // `o.Value = 5` on an untyped Object becomes a late-bound Property Let put.
    let src = "Sub Main()\n    Dim o As Object\n    o.Value = 5\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program).iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch {
                name,
                kind: Some(oxvba_bundle::ProjectMemberKind::PropertyLet),
                default_member: false,
            }
                if name.eq_ignore_ascii_case("Value")
        )),
        "expected a late PropertyLet put to Value"
    );
}

#[test]
fn late_bound_object_index_assignment_lowers_to_default_member_put() {
    let src = "Sub Main()\n    Dim o As Object\n    o(1) = 5\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program).iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch {
                default_member: true,
                kind: Some(oxvba_bundle::ProjectMemberKind::PropertyLet),
                ..
            }
        )),
        "expected Object index assignment to lower to default-member PropertyLet"
    );
}

#[test]
fn declare_lib_emits_external_call_descriptor() {
    // `Declare` lowers calls to CoreCallee::Declare and records a descriptor with
    // the source-declared library/name/params (runtime fields are defaults).
    let src = "Declare Function GetTickCount Lib \"kernel32\" () As Long\n\n\
               Sub Main()\n    Dim r As Long\n    r = GetTickCount()\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program)
            .iter()
            .any(|c| matches!(c, CoreCallee::Declare { .. })),
        "expected a Declare callee"
    );
    let desc = program
        .external_calls
        .iter()
        .find(|d| d.declared_name.eq_ignore_ascii_case("GetTickCount"))
        .expect("expected a GetTickCount external-call descriptor");
    assert_eq!(desc.library, "kernel32");
    assert!(desc.return_type.is_some());
}

#[test]
fn declare_function_type_suffix_emits_external_return_type() {
    let src = "DefDbl A-Z\nDeclare PtrSafe Function GetTickCount& Lib \"kernel32\" ()\n\n\
               Sub Main()\n    Dim r As Long\n    r = GetTickCount()\nEnd Sub\n";
    let program = bind(src);
    let desc = program
        .external_calls
        .iter()
        .find(|d| d.declared_name.eq_ignore_ascii_case("GetTickCount"))
        .expect("expected a GetTickCount external-call descriptor");
    assert_eq!(desc.return_type, Some(DeclareParamType::Long));
}

#[test]
fn declare_byref_arg_emits_byref() {
    // A ByRef `Declare` param called with an l-value lowers to CoreArg::ByRef (the
    // write-back target). Exercises bind_args_byref — the same path COM uses.
    let src = "Declare Sub Bump Lib \"k\" (ByRef n As Long)\n\n\
               Sub Main()\n    Dim r As Long\n    r = 5\n    Bump r\nEnd Sub\n";
    let program = bind(src);
    let args = program
        .procs
        .iter()
        .flat_map(|p| &p.body)
        .find_map(|s| match s {
            CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::Declare { .. },
                args,
            }) => Some(args),
            _ => None,
        })
        .expect("a Declare call");
    assert!(
        matches!(args.first(), Some(CoreArg::ByRef(_))),
        "ByRef Declare arg should be CoreArg::ByRef, got {:?}",
        args.first()
    );
}

// ── File I/O (structural — bind emits cross-bundle ExternProc calls; not run) ──

#[test]
fn file_io_lowers_to_vba_filesystem_externs() {
    // Since P4 the funny-syntax file statements lower to cross-bundle `ExternProc`
    // calls into the `VBA` bundle's `FileSystem` module (internal member names
    // `Open`/`Print`/`Close`/…) rather than `CoreCallee::Native(File*)`. We assert the
    // entry bundle imports each member; the special arg-shaping is unchanged.
    let src = "Sub Main()\n    Dim f As Long\n    f = FreeFile\n    Open \"x.txt\" For Output As #1\n    Print #1, \"hi\"\n    Close #1\nEnd Sub\n";
    let program = bind(src);
    for member in ["Open", "Print", "Close"] {
        assert!(
            imports_vba_filesystem(&program, member),
            "expected a VBA/FileSystem import for {member}: {:?}",
            program.imports
        );
    }
    // No file statement remains on the bespoke `CoreCallee::Native` route.
    assert!(
        !contains_file_native(&program),
        "no file statement may lower to CoreCallee::Native after P4"
    );
}

/// True if any statement still lowers a *migrated* file id to `CoreCallee::Native`.
fn contains_file_native(program: &CoreProgram) -> bool {
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
    program.procs.iter().flat_map(|p| &p.body).any(|s| {
        let id = match s {
            CoreStmt::Eval(v) => value_native(v),
            CoreStmt::Assign { value, .. } => value_native(value),
            _ => None,
        };
        id.is_some_and(|id| {
            id.library_member().is_some() || id.library_statement_member().is_some()
        })
    })
}
