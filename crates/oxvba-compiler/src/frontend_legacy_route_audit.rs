use crate::resolve::apply_conditional_compilation_to_source;
use crate::syntax_bridge::{SyntaxBridgeProductionRoute, production_route_for_source};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyRouteAuditDisposition {
    HirProduction,
    LegacyFallbackResidual,
    StaticResidual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRouteAuditFinding {
    pub area: &'static str,
    pub evidence: String,
    pub disposition: LegacyRouteAuditDisposition,
    pub owner: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRouteAuditReport {
    pub findings: Vec<LegacyRouteAuditFinding>,
}

impl LegacyRouteAuditReport {
    pub fn terminal_gate_passed(&self) -> bool {
        self.findings
            .iter()
            .all(|finding| finding.disposition == LegacyRouteAuditDisposition::HirProduction)
    }

    pub fn residuals(&self) -> Vec<&LegacyRouteAuditFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.disposition != LegacyRouteAuditDisposition::HirProduction)
            .collect()
    }
}

pub fn run_production_legacy_route_audit() -> LegacyRouteAuditReport {
    let mut findings = Vec::new();

    let scoped_assignment = "Sub Main()\nDim x As Long\nx = 1 + 2\nEnd Sub\n";
    findings.push(route_finding(
        "scoped procedure/local/assignment/arithmetic fixture",
        scoped_assignment,
        "bd-aprs.9.5",
    ));

    let call_statement = "Sub Main()\nCall Worker()\nEnd Sub\nSub Worker()\nEnd Sub\n";
    findings.push(route_finding(
        "procedure call statement fixture",
        call_statement,
        "bd-aprs.9.5",
    ));

    let no_arg_call_statement = "Sub Main()\nCall Worker\nEnd Sub\nSub Worker()\nEnd Sub\n";
    findings.push(route_finding(
        "no-argument procedure call statement fixture",
        no_arg_call_statement,
        "bd-aprs.9.10",
    ));

    let statement_form_call = "Sub Use(ByVal a, ByVal b)\nEnd Sub\nSub Main()\nUse 1, 2\nEnd Sub\n";
    findings.push(route_finding(
        "statement-form procedure call arguments fixture",
        statement_form_call,
        "bd-aprs.9.5",
    ));

    let function_statement =
        "Function Alpha() As Long\nAlpha = 1\nEnd Function\nSub Main()\nEnd Sub\n";
    findings.push(route_finding(
        "function declaration fixture",
        function_statement,
        "bd-aprs.9.5",
    ));

    let if_statement = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nEnd If\nEnd Sub\n";
    findings.push(route_finding(
        "if statement fixture",
        if_statement,
        "bd-aprs.9.5",
    ));

    let if_else_statement =
        "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nElse\nx = 2\nEnd If\nEnd Sub\n";
    findings.push(route_finding(
        "if else statement fixture",
        if_else_statement,
        "bd-aprs.9.5",
    ));

    let elseif_statement = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nElseIf x = 1 Then\nx = 2\nElse\nx = 3\nEnd If\nEnd Sub\n";
    findings.push(route_finding(
        "elseif statement fixture",
        elseif_statement,
        "bd-aprs.9.5",
    ));

    let single_line_if_statement =
        "Sub Main()\nDim x As Long\nIf x = 0 Then x = 1 Else x = 2\nEnd Sub\n";
    findings.push(route_finding(
        "single-line if statement fixture",
        single_line_if_statement,
        "bd-aprs.9.5",
    ));

    let do_while_statement =
        "Sub Main()\nDim x As Long\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub\n";
    findings.push(route_finding(
        "do while statement fixture",
        do_while_statement,
        "bd-aprs.9.5",
    ));

    let select_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase 1\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case statement fixture",
        select_statement,
        "bd-aprs.9.5",
    ));

    let do_until_statement =
        "Sub Main()\nDim x As Long\nDo Until x = 3\nx = x + 1\nLoop\nEnd Sub\n";
    findings.push(route_finding(
        "do until statement fixture",
        do_until_statement,
        "bd-aprs.9.5",
    ));

    let post_check_loop_statement =
        "Sub Main()\nDim x As Long\nDo\nx = x + 1\nLoop Until x = 3\nEnd Sub\n";
    findings.push(route_finding(
        "post-check loop statement fixture",
        post_check_loop_statement,
        "bd-aprs.9.5",
    ));

    let while_wend_statement = "Sub Main()\nDim x As Long\nWhile x < 3\nx = x + 1\nWend\nEnd Sub\n";
    findings.push(route_finding(
        "while wend statement fixture",
        while_wend_statement,
        "bd-aprs.9.5",
    ));

    let for_statement = "Sub Main()\nDim i As Long\nFor i = 1 To 3\ni = i + 1\nNext\nEnd Sub\n";
    findings.push(route_finding(
        "for statement fixture",
        for_statement,
        "bd-aprs.9.5",
    ));

    let select_range_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case range fixture",
        select_range_statement,
        "bd-aprs.9.5",
    ));

    let select_case_is_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase Is < 0\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case is fixture",
        select_case_is_statement,
        "bd-aprs.9.5",
    ));

    let select_multi_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase 1, 2\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case multi-value fixture",
        select_multi_statement,
        "bd-aprs.9.5",
    ));

    let for_each_statement =
        "Sub Main()\nDim item As Variant\nFor Each item In item\nitem = item\nNext\nEnd Sub\n";
    findings.push(route_finding(
        "for each statement fixture",
        for_each_statement,
        "bd-aprs.9.5",
    ));

    let exit_do_statement = "Sub Main()\nDim x As Long\nDo While x < 3\nExit Do\nLoop\nEnd Sub\n";
    findings.push(route_finding(
        "exit do statement fixture",
        exit_do_statement,
        "bd-aprs.9.5",
    ));

    let exit_for_statement = "Sub Main()\nDim i As Long\nFor i = 1 To 3\nExit For\nNext\nEnd Sub\n";
    findings.push(route_finding(
        "exit for statement fixture",
        exit_for_statement,
        "bd-aprs.9.5",
    ));

    let exit_sub_statement = "Sub Main()\nExit Sub\nEnd Sub\n";
    findings.push(route_finding(
        "exit sub statement fixture",
        exit_sub_statement,
        "bd-aprs.9.5",
    ));

    let on_error_resume_next_statement = "Sub Main()\nOn Error Resume Next\nResume Next\nEnd Sub\n";
    findings.push(route_finding(
        "on error resume next statement fixture",
        on_error_resume_next_statement,
        "bd-aprs.9.5",
    ));

    let on_error_goto_zero_statement = "Sub Main()\nOn Error GoTo 0\nResume\nEnd Sub\n";
    findings.push(route_finding(
        "on error goto zero statement fixture",
        on_error_goto_zero_statement,
        "bd-aprs.9.5",
    ));

    let on_error_goto_label_statement =
        "Sub Main()\nOn Error GoTo handler\nhandler:\nResume done\ndone:\nEnd Sub\n";
    findings.push(route_finding(
        "on error goto label statement fixture",
        on_error_goto_label_statement,
        "bd-aprs.9.5",
    ));

    let goto_label_statement = "Sub Main()\nGoTo done\ndone:\nEnd Sub\n";
    findings.push(route_finding(
        "goto label statement fixture",
        goto_label_statement,
        "bd-aprs.9.5",
    ));

    let goto_numeric_label_statement = "Sub Main()\nGoTo 100\n100:\nEnd Sub\n";
    findings.push(route_finding(
        "goto numeric label statement fixture",
        goto_numeric_label_statement,
        "bd-aprs.9.5",
    ));

    let gosub_return_statement = "Sub Main()\nGoSub helper\nhelper:\nReturn\nEnd Sub\n";
    findings.push(route_finding(
        "gosub return statement fixture",
        gosub_return_statement,
        "bd-aprs.9.5",
    ));

    let erase_statement = "Sub Main()\nDim a\nErase a\nEnd Sub\n";
    findings.push(route_finding(
        "erase statement fixture",
        erase_statement,
        "bd-aprs.9.5",
    ));

    let redim_statement =
        "Sub Main()\nDim length As Long\nDim buf() As Byte\nReDim buf(length - 1)\nEnd Sub\n";
    findings.push(route_finding(
        "redim runtime statement fixture",
        redim_statement,
        "bd-aprs.9.5",
    ));

    let redim_multidimensional_statement = "Sub Main()\nDim rows As Long\nDim cols As Long\nDim grid() As Long\nReDim grid(rows - 1, cols - 1)\nEnd Sub\n";
    findings.push(route_finding(
        "redim multidimensional runtime statement fixture",
        redim_multidimensional_statement,
        "bd-aprs.9.5",
    ));

    let redim_explicit_lower_bound_statement =
        "Sub Main()\nDim length As Long\nDim buf() As Byte\nReDim buf(1 To length - 1)\nEnd Sub\n";
    findings.push(route_finding(
        "redim explicit lower-bound runtime statement fixture",
        redim_explicit_lower_bound_statement,
        "bd-aprs.9.5",
    ));

    let dynamic_array_element_read_statement =
        "Sub Main()\nDim buf() As Byte\nDim x As Long\nReDim buf(2)\nx = buf(1)\nEnd Sub\n";
    findings.push(route_finding(
        "dynamic array element read fixture",
        dynamic_array_element_read_statement,
        "bd-aprs.9.8",
    ));

    let dynamic_array_element_write_statement =
        "Sub Main()\nDim buf() As Byte\nReDim buf(2)\nbuf(1) = 7\nEnd Sub\n";
    findings.push(route_finding(
        "dynamic array element write fixture",
        dynamic_array_element_write_statement,
        "bd-aprs.9.8",
    ));

    let multidimensional_dynamic_array_element_statement = "Sub Main()\nDim grid() As Long\nDim x As Long\nReDim grid(1, 1)\ngrid(1, 0) = 7\nx = grid(1, 0)\nEnd Sub\n";
    findings.push(route_finding(
        "multidimensional dynamic array element fixture",
        multidimensional_dynamic_array_element_statement,
        "bd-aprs.9.8",
    ));

    let fixed_array_element_alias_statement =
        "Sub Main()\nDim a(1 To 2) As Integer\nDim x As Long\na(2) = 7\nx = a(2)\nEnd Sub\n";
    findings.push(route_finding(
        "fixed array element alias fixture",
        fixed_array_element_alias_statement,
        "bd-aprs.9.8",
    ));

    let multidimensional_fixed_array_element_alias_statement = "Sub Main()\nDim m(1 To 2, 1 To 2) As Integer\nDim x As Long\nm(2, 1) = 7\nx = m(2, 1)\nEnd Sub\n";
    findings.push(route_finding(
        "multidimensional fixed array element alias fixture",
        multidimensional_fixed_array_element_alias_statement,
        "bd-aprs.9.8",
    ));

    let fixed_array_redim_alias_statement =
        "Sub Main()\nDim a(1)\nDim x As Long\na(0) = 7\nReDim Preserve a(3)\nx = a(3)\nEnd Sub\n";
    findings.push(route_finding(
        "fixed array redim alias rematerialization fixture",
        fixed_array_redim_alias_statement,
        "bd-aprs.9.8",
    ));

    let raise_event_statement = "Sub Main()\nRaiseEvent Tick(1)\nEnd Sub\n";
    findings.push(route_finding(
        "raise event statement fixture",
        raise_event_statement,
        "bd-aprs.9.5",
    ));

    let event_declaration_statement =
        "Event Tick(ByVal value)\nSub Main()\nRaiseEvent Tick(1)\nEnd Sub\n";
    findings.push(route_finding(
        "event declaration and raise event fixture",
        event_declaration_statement,
        "bd-aprs.9.5",
    ));

    let implements_directive = "Implements IFoo\nSub Main()\nEnd Sub\n";
    findings.push(route_finding(
        "single-source implements directive fixture",
        implements_directive,
        "bd-aprs.9.5",
    ));

    let const_statement = "Const CBase = 7, CName = \"a,b\"\nSub Main()\nDim x\nDim y\nx = CBase\ny = CName\nEnd Sub\n";
    findings.push(route_finding(
        "const statement fixture",
        const_statement,
        "bd-aprs.9.5",
    ));

    let const_expression_statement =
        "Const CBase = 1 + 2, CTotal = CBase + 1\nSub Main()\nDim x\nx = CTotal\nEnd Sub\n";
    findings.push(route_finding(
        "const expression statement fixture",
        const_expression_statement,
        "bd-aprs.9.5",
    ));

    let option_explicit_statement = "Option Explicit\nSub Main()\nDim x\nx = 1\nEnd Sub\n";
    findings.push(route_finding(
        "option explicit fixture",
        option_explicit_statement,
        "bd-aprs.9.9",
    ));

    let option_compare_database_statement =
        "Option Compare Database\nSub Main()\nDim x\nx = \"a\" = \"A\"\nEnd Sub\n";
    findings.push(route_finding(
        "option compare database fixture",
        option_compare_database_statement,
        "bd-aprs.9.9",
    ));

    let option_private_module_statement =
        "Option Private Module\nSub Main()\nDim x\nx = 1\nEnd Sub\n";
    findings.push(route_finding(
        "option private module fixture",
        option_private_module_statement,
        "bd-aprs.9.9",
    ));

    let def_type_statement = "DefLng A-Z\nSub Main()\nDim alpha\nalpha = 1\nEnd Sub\n";
    findings.push(route_finding(
        "def type untyped dim fixture",
        def_type_statement,
        "bd-aprs.9.9",
    ));

    let def_type_signature_statement =
        "DefLng A-Z\nFunction alpha(beta%)\nalpha = beta + 1\nEnd Function\n";
    findings.push(route_finding(
        "def type signature fixture",
        def_type_signature_statement,
        "bd-aprs.9.9",
    ));

    let def_type_module_scope_statement = "DefLng A-Z\nDim alpha\nSub Main()\nalpha = 1\nEnd Sub\n";
    findings.push(route_finding(
        "def type module-scope scalar fixture",
        def_type_module_scope_statement,
        "bd-aprs.9.9",
    ));

    let conditional_compilation_statement = "#Const ENABLE = True\nSub Main()\nDim x\n#If ENABLE Then\nx = 7\n#Else\nx = 1\n#End If\nEnd Sub\n";
    findings.push(route_finding(
        "conditional compilation fixture",
        conditional_compilation_statement,
        "bd-aprs.9.9",
    ));

    let module_attribute_statement =
        "Attribute VB_Name = \"Module1\"\nSub Main()\nDim x\nx = 7\nEnd Sub\n";
    findings.push(route_finding(
        "module attribute fixture",
        module_attribute_statement,
        "bd-aprs.9.9",
    ));

    let typed_const_statement = "Const CBase As Long = 7\nSub Main()\nDim x\nx = CBase\nEnd Sub\n";
    findings.push(route_finding(
        "typed const fixture",
        typed_const_statement,
        "bd-aprs.9.9",
    ));

    let mod_like_expression_statement = "Sub Main()\nDim x\nDim y\nDim ok\nx = 17 Mod 3\ny = 17 \\ 3\nok = \"123\" Like \"###\"\nEnd Sub\n";
    findings.push(route_finding(
        "mod integer-division and like expression fixture",
        mod_like_expression_statement,
        "bd-aprs.9.10",
    ));

    let optional_parameter_statement =
        "Sub Use(Optional ByVal n As Long = 7)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
    findings.push(route_finding(
        "optional parameter fixture",
        optional_parameter_statement,
        "bd-aprs.9.10",
    ));

    let optional_integer_default_expression_statement = "Sub Use(Optional ByVal n As Long = &H10 + &O7 - 1)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
    findings.push(route_finding(
        "optional integer default expression fixture",
        optional_integer_default_expression_statement,
        "bd-aprs.9.10",
    ));

    let param_array_statement =
        "Sub Use(ParamArray items() As Variant)\nEnd Sub\nSub Main()\nCall Use(1, 2)\nEnd Sub\n";
    findings.push(route_finding(
        "ParamArray parameter fixture",
        param_array_statement,
        "bd-aprs.9.10",
    ));

    let simple_property_statement = "Sub Main()\nDim x\nx = Value\nValue = x\nEnd Sub\nProperty Get Value() As Long\nValue = 9\nEnd Property\nProperty Let Value(ByRef target)\ntarget = target + 1\nEnd Property\n";
    findings.push(route_finding(
        "simple property fixture",
        simple_property_statement,
        "bd-aprs.9.10",
    ));

    let indexed_property_get_statement = "Sub Main()\nDim x\nx = Value(1)\nEnd Sub\nProperty Get Value(ByVal index As Long) As Long\nValue = index\nEnd Property\n";
    findings.push(route_finding(
        "indexed property get fixture",
        indexed_property_get_statement,
        "bd-aprs.9.9",
    ));

    let enum_member_constants =
        "Public Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub\n";
    findings.push(route_finding(
        "enum member constant fixture",
        enum_member_constants,
        "bd-aprs.9.5",
    ));

    let declared_external_call = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub\n";
    findings.push(route_finding(
        "declared external call fixture",
        declared_external_call,
        "bd-aprs.9.5",
    ));

    let udt_layout = "Type Point\nX As Long\nY As String\nEnd Type\nSub Main()\nDim p As Point\nDim y As Long\np.X = 1\ny = p.X + 2\nEnd Sub\n";
    findings.push(route_finding(
        "UDT layout descriptor fixture",
        udt_layout,
        "bd-aprs.9.5",
    ));

    let member_expression =
        "Sub Main()\nDim obj\nDim x\nDim y\nx = obj.Value\ny = obj.Method(1)\nEnd Sub\n";
    findings.push(route_finding(
        "value-side member expression fixture",
        member_expression,
        "bd-aprs.9.5",
    ));

    let member_assignment =
        "Sub Main()\nDim obj\nDim other\nobj.Value = 1\nSet obj.Ref = other\nEnd Sub\n";
    findings.push(route_finding(
        "member assignment target fixture",
        member_assignment,
        "bd-aprs.9.5",
    ));

    let bang_member_expression = "Sub Main()\nDim obj\nDim x\nx = obj!Value\nEnd Sub\n";
    findings.push(route_finding(
        "value-side bang member expression fixture",
        bang_member_expression,
        "bd-aprs.9.5",
    ));

    let bang_member_assignment = "Sub Main()\nDim obj\nobj!Value = 1\nEnd Sub\n";
    findings.push(route_finding(
        "bang member assignment target fixture",
        bang_member_assignment,
        "bd-aprs.9.5",
    ));

    let typeof_is_expression =
        "Sub Main()\nDim obj As Object\nDim ok\nok = TypeOf obj Is Class1\nEnd Sub\n";
    findings.push(route_finding(
        "TypeOf Is expression fixture",
        typeof_is_expression,
        "bd-aprs.9.5",
    ));

    let time_locale_host_intrinsics = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Date()\nb = Time()\nc = Now()\nd = Timer()\nEnd Sub\n";
    findings.push(route_finding(
        "time-locale host intrinsic fixture",
        time_locale_host_intrinsics,
        "bd-aprs.9.10",
    ));

    let host_utility_intrinsics = "Sub Main()\nDim a\nDim b\nDim c\na = FreeFile()\nb = FreeFile(1)\nc = DoEvents()\nEnd Sub\n";
    findings.push(route_finding(
        "host utility intrinsic fixture",
        host_utility_intrinsics,
        "bd-aprs.9.10",
    ));

    let file_position_host_intrinsics = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = EOF(1)\nb = LOF(1)\nc = Seek(1)\nd = Loc(1)\nEnd Sub\n";
    findings.push(route_finding(
        "file position host intrinsic fixture",
        file_position_host_intrinsics,
        "bd-aprs.9.10",
    ));

    let dialog_host_intrinsics =
        "Sub Main()\nDim a\nDim b\na = MsgBox(7, 3)\nb = InputBox(9, 4)\nEnd Sub\n";
    findings.push(route_finding(
        "dialog host intrinsic fixture",
        dialog_host_intrinsics,
        "bd-aprs.9.10",
    ));

    let process_environment_host_intrinsics = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Shell(7)\nb = Environ(77)\nc = Dir()\nd = Dir(5)\nEnd Sub\n";
    findings.push(route_finding(
        "process/environment host intrinsic fixture",
        process_environment_host_intrinsics,
        "bd-aprs.9.10",
    ));

    let createobject_host_intrinsic =
        "Sub Main()\nDim x\nx = CreateObject(\"Scripting.Dictionary\")\nEnd Sub\n";
    findings.push(route_finding(
        "CreateObject host intrinsic fixture",
        createobject_host_intrinsic,
        "bd-aprs.9.10",
    ));

    let named_dispatchinvoke_intrinsic = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetIndexedValue\", value := 11, lhs := 7)\nEnd Sub\n";
    findings.push(route_finding(
        "named DispatchInvoke host intrinsic fixture",
        named_dispatchinvoke_intrinsic,
        "bd-aprs.9.5",
    ));

    let console_debug_print_statements =
        "Sub Main()\nPrint \"hello\"\nDebug.Print \"left\", \"right\"\nEnd Sub\n";
    findings.push(route_finding(
        "console and debug print statement fixture",
        console_debug_print_statements,
        "bd-aprs.9.10",
    ));

    let file_kill_statement = "Sub Main()\nDim path As String\npath = \"x\"\nKill path\nEnd Sub\n";
    findings.push(route_finding(
        "file kill statement fixture",
        file_kill_statement,
        "bd-aprs.9.10",
    ));

    let file_open_statement = "Sub Main()\nOpen \"x\" For Output As #1\nEnd Sub\n";
    findings.push(route_finding(
        "file open statement fixture",
        file_open_statement,
        "bd-aprs.9.10",
    ));

    let console_input_statement = "Sub Main()\nDim a\nDim b\nInput a, b\nEnd Sub\n";
    findings.push(route_finding(
        "console input statement fixture",
        console_input_statement,
        "bd-aprs.9.10",
    ));

    let console_line_input_statement = "Sub Main()\nDim lineText\nLine Input lineText\nEnd Sub\n";
    findings.push(route_finding(
        "console line input statement fixture",
        console_line_input_statement,
        "bd-aprs.9.10",
    ));

    let file_close_statement = "Sub Main()\nClose #1\nClose\nEnd Sub\n";
    findings.push(route_finding(
        "file close statement fixture",
        file_close_statement,
        "bd-aprs.9.10",
    ));

    let file_print_statement = "Sub Main()\nPrint #1, \"hello\"\nEnd Sub\n";
    findings.push(route_finding(
        "file print statement fixture",
        file_print_statement,
        "bd-aprs.9.10",
    ));

    let file_write_statement = "Sub Main()\nWrite #1, 42, True, \"hello,world\"\nEnd Sub\n";
    findings.push(route_finding(
        "file write statement fixture",
        file_write_statement,
        "bd-aprs.9.10",
    ));

    let file_input_statement = "Sub Main()\nDim a\nDim b\nInput #1, a, b\nEnd Sub\n";
    findings.push(route_finding(
        "file input statement fixture",
        file_input_statement,
        "bd-aprs.9.10",
    ));

    let file_line_input_statement = "Sub Main()\nDim lineText\nLine Input #1, lineText\nEnd Sub\n";
    findings.push(route_finding(
        "file line input statement fixture",
        file_line_input_statement,
        "bd-aprs.9.10",
    ));

    let statement_form_member_call = "Sub Main()\nDim obj\nobj.Method 1, 2\nEnd Sub\n";
    findings.push(route_finding(
        "statement-form member call arguments fixture",
        statement_form_member_call,
        "bd-aprs.9.5",
    ));

    let with_member_read = "Sub Main()\nDim obj\nDim x\nWith obj\nx = .Value\nEnd With\nEnd Sub\n";
    findings.push(route_finding(
        "with member read fixture",
        with_member_read,
        "bd-aprs.9.5",
    ));

    findings.push(LegacyRouteAuditFinding {
        area: "project.rs source-text rewrite bridge",
        evidence: "production project compilation selects ModuleAwareBindPlan unconditionally; RewriteBridge remains only as an internal parity-test strategy".to_string(),
        disposition: LegacyRouteAuditDisposition::HirProduction,
        owner: "bd-aprs.8.*",
    });
    findings.push(LegacyRouteAuditFinding {
        area: "language-service legacy BoundModule compatibility",
        evidence: "oxvba-languageservice SemanticSnapshot no longer retains/exposes or builds a legacy BoundModule; unsupported HIR snapshots report front-end diagnostics instead of rebuilding legacy symbol/callable correlation".to_string(),
        disposition: LegacyRouteAuditDisposition::HirProduction,
        owner: "bd-aprs.10.4",
    });

    LegacyRouteAuditReport { findings }
}

fn route_finding(
    area: &'static str,
    source: &'static str,
    owner: &'static str,
) -> LegacyRouteAuditFinding {
    let route_source = apply_conditional_compilation_to_source(source);
    match production_route_for_source(&route_source) {
        Ok(SyntaxBridgeProductionRoute::HirProduction) => LegacyRouteAuditFinding {
            area,
            evidence: "classified as HIR production".to_string(),
            disposition: LegacyRouteAuditDisposition::HirProduction,
            owner,
        },
        Ok(SyntaxBridgeProductionRoute::HirUnsupportedResidual) => LegacyRouteAuditFinding {
            area,
            evidence: "classified as HIR Unsupported residual; outer default policy may still fall back to legacy"
                .to_string(),
            disposition: LegacyRouteAuditDisposition::LegacyFallbackResidual,
            owner,
        },
        Err(err) => LegacyRouteAuditFinding {
            area,
            evidence: format!("route classification failed: {err}"),
            disposition: LegacyRouteAuditDisposition::LegacyFallbackResidual,
            owner,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_records_hir_production_for_completed_scoped_fixture() {
        let report = run_production_legacy_route_audit();
        let scoped = report
            .findings
            .iter()
            .find(|finding| finding.area.contains("assignment"))
            .expect("scoped fixture finding");
        assert_eq!(
            scoped.disposition,
            LegacyRouteAuditDisposition::HirProduction,
            "{report:#?}"
        );
    }

    #[test]
    fn audit_terminal_gate_passes_after_audited_residuals_retire() {
        let report = run_production_legacy_route_audit();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.area.contains("call statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("statement-form procedure call arguments")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("function declaration")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("project.rs")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("if statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("if else statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("elseif statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("single-line if statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("do until statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("post-check loop statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("while wend statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("for statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("select case range")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("select case multi-value")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("select case is")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("for each statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("exit do statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("exit for statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("exit sub statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("on error resume next")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("on error goto zero")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("on error goto label")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("goto label statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("goto numeric label")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("gosub return")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("erase statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("redim runtime statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("redim multidimensional runtime statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("redim explicit lower-bound runtime statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("dynamic array element read")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("dynamic array element write")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("multidimensional dynamic array element")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("fixed array element alias")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("multidimensional fixed array element alias")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("fixed array redim alias rematerialization")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("value-side member expression")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("member assignment target")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("bang member assignment target")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("statement-form member call arguments")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("event declaration and raise event")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("with member read")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("single-source implements directive")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("option explicit")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("option compare database")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("option private module")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("def type untyped dim")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("def type signature")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("def type module-scope scalar")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("conditional compilation")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("module attribute")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("typed const")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("optional parameter")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("optional integer default expression")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("simple property")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("indexed property get")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.terminal_gate_passed(),
            "terminal gate should pass when audited residuals are retired: {report:#?}"
        );
    }
}
