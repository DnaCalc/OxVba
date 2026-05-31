use std::collections::{HashMap, HashSet};

type ArrayBoundsMap = HashMap<String, Vec<(i32, i32)>>;
type ModuleConstMap = HashMap<String, BoundExpr>;
type ParsedArrayDecl = (String, Option<BoundType>, Vec<(i32, i32)>);
#[derive(Debug, Clone)]
struct UdtFieldDef {
    name: String,
    bound_type: BoundType,
    nested_udt_name: Option<String>,
    array_bounds: Option<Vec<(i32, i32)>>,
    fixed_string_len: Option<usize>,
}
type UdtDefMap = HashMap<String, Vec<UdtFieldDef>>;
const UDT_TYPE_MARKER_PREFIX: &str = "__oxvba_udt_type__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEnumDescriptor {
    pub type_name: String,
    pub is_public: bool,
    pub members: Vec<BoundEnumMemberDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEnumMemberDescriptor {
    pub name: String,
    pub value: i32,
    pub ordinal: usize,
    pub explicit_value: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
    Pow,
    Concat,
    Neg,
}

/// Logical operators usable as value-producing expressions (VBA `And`/`Or`).
/// Lowered to the VM's truthy `BoolAnd`/`BoolOr` instructions, consistent with
/// the condition-path semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalBinOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundExpr {
    IntConst(i32),
    BoolConst(bool),
    FloatConst(u64),
    StringConst(String),
    Var(String),
    VarPtrArrayBuffer(String),
    AddConst {
        var: String,
        delta: i32,
    },
    SubConst {
        var: String,
        delta: i32,
    },
    BinaryOp {
        op: ArithOp,
        lhs: Box<BoundExpr>,
        rhs: Box<BoundExpr>,
    },
    CompareOp {
        op: CompareOp,
        lhs: Box<BoundExpr>,
        rhs: Box<BoundExpr>,
    },
    UnaryOp {
        op: ArithOp,
        operand: Box<BoundExpr>,
    },
    LogicalBinaryOp {
        op: LogicalBinOp,
        lhs: Box<BoundExpr>,
        rhs: Box<BoundExpr>,
    },
    LogicalNot {
        operand: Box<BoundExpr>,
    },
    IntrinsicCall {
        name: String,
        args: Vec<BoundExpr>,
    },
    ProcCall {
        name: String,
        args: Vec<BoundCallArg>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundType {
    Variant,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Byte,
    Single,
    Double,
    Currency,
    Decimal,
    Date,
    String,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallArg {
    pub name: Option<String>,
    pub expr: BoundExpr,
    /// When true, the argument was parenthesized at statement level (e.g.
    /// `Foo (x)`) and must be passed ByVal even if the parameter is ByRef.
    pub force_byval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundCallSyntax {
    Unknown,
    StatementNoCall,
    StatementCallKeyword,
    ExpressionCall,
    SyntheticPropertyAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentIntent {
    Implicit,
    Let,
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeArrayDimExpr {
    pub lower_bound: i32,
    pub upper_bound: BoundExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundStmt {
    Assign {
        target: String,
        expr: BoundExpr,
        intent: AssignmentIntent,
    },
    AssignRuntimeArrayElement {
        name: String,
        indices: Vec<BoundExpr>,
        expr: BoundExpr,
        intent: AssignmentIntent,
    },
    UdtAssign {
        target: String,
        source: String,
        fields: Vec<String>,
    },
    MidAssign {
        target: String,
        start: BoundExpr,
        count: Option<BoundExpr>,
        value: BoundExpr,
    },
    IfCond {
        cond: BoundCond,
        then_body: Vec<BoundStmt>,
        else_body: Vec<BoundStmt>,
    },
    ForRange {
        var: String,
        start: BoundExpr,
        end: BoundExpr,
        step: BoundExpr,
        body: Vec<BoundStmt>,
    },
    ForEach {
        var: String,
        items: Vec<BoundExpr>,
        iterable: Option<BoundExpr>,
        body: Vec<BoundStmt>,
    },
    ReDim {
        name: String,
        bounds: Vec<(i32, i32)>,
        previous_bounds: Option<Vec<(i32, i32)>>,
        preserve: bool,
    },
    ReDimRuntime {
        name: String,
        bounds: Vec<RuntimeArrayDimExpr>,
        preserve: bool,
    },
    Erase {
        name: String,
    },
    DoWhile {
        cond: BoundCond,
        body: Vec<BoundStmt>,
        post_check: bool,
    },
    ExitDo,
    ExitFor,
    OnErrorResumeNext,
    OnErrorGoto0,
    OnErrorGotoLabel {
        label: String,
    },
    ResumeNext,
    Resume,
    ResumeLabel {
        label: String,
    },
    RaiseError(i32),
    RaiseEvent {
        name: String,
        args: Vec<BoundCallArg>,
    },
    ErrClear,
    Label {
        name: String,
    },
    GoTo {
        label: String,
    },
    GoSub {
        label: String,
    },
    Return,
    Call {
        name: String,
        args: Vec<BoundCallArg>,
        syntax: BoundCallSyntax,
    },
    AssignFromCall {
        target: String,
        name: String,
        args: Vec<BoundCallArg>,
        intent: AssignmentIntent,
        syntax: BoundCallSyntax,
    },
    SelectCase {
        expr: BoundExpr,
        arms: Vec<(Vec<BoundCaseClause>, Vec<BoundStmt>)>,
        else_body: Vec<BoundStmt>,
    },
    FileOpen {
        path: BoundExpr,
        mode: i32,
        file_number: BoundExpr,
    },
    FileClose {
        file_number: Option<BoundExpr>,
    },
    FileKill {
        path: BoundExpr,
    },
    FilePrint {
        file_number: BoundExpr,
        data: BoundExpr,
    },
    ConsolePrint {
        data: BoundExpr,
    },
    FileWrite {
        file_number: BoundExpr,
        data: Vec<BoundExpr>,
    },
    FileInput {
        file_number: BoundExpr,
        targets: Vec<String>,
    },
    ConsoleInput {
        targets: Vec<String>,
    },
    FileLineInput {
        file_number: BoundExpr,
        target: String,
    },
    ConsoleLineInput {
        target: String,
    },
    Beep,
    ExitProcedure,
    DebugPrint {
        data: BoundExpr,
    },
    Unsupported {
        line: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundCaseClause {
    Value(i32),
    Is { op: CompareOp, value: i32 },
    Range { start: i32, end: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Like,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundCompareMode {
    Binary,
    Text,
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundCond {
    Compare {
        op: CompareOp,
        lhs: BoundExpr,
        rhs: BoundExpr,
    },
    Truthy(BoundExpr),
    Not(Box<BoundCond>),
    And(Box<BoundCond>, Box<BoundCond>),
    Or(Box<BoundCond>, Box<BoundCond>),
}

#[derive(Debug, Clone)]
pub struct BoundModule {
    pub source: String,
    pub option_explicit: bool,
    pub is_class_module: bool,
    pub compare_mode: BoundCompareMode,
    pub default_type_table: [BoundType; 26],
    pub resolution_diagnostics: Vec<String>,
    pub declarations: Vec<String>,
    pub declaration_types: HashMap<String, BoundType>,
    pub array_descriptors: HashMap<String, BoundArrayDescriptor>,
    pub enum_descriptors: Vec<BoundEnumDescriptor>,
    pub external_declarations: HashMap<String, BoundExternalDecl>,
    pub body: Vec<BoundStmt>,
    pub procedures: Vec<BoundProcedure>,
}

#[derive(Debug, Clone)]
pub struct BoundProcedure {
    pub name: String,
    pub source_line_start: usize,
    pub source_line_end: usize,
    pub statement_line_numbers: Vec<usize>,
    pub return_type: BoundType,
    pub params: Vec<BoundParam>,
    pub module_scope_names: Vec<String>,
    pub declarations: Vec<String>,
    pub declaration_types: HashMap<String, BoundType>,
    pub array_descriptors: HashMap<String, BoundArrayDescriptor>,
    pub udt_descriptors: Vec<BoundUdtDescriptor>,
    pub duplicate_declarations: Vec<String>,
    pub body: Vec<BoundStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundArrayDescriptor {
    pub element_type: BoundType,
    pub rank: usize,
    pub bounds: Vec<(i32, i32)>,
    pub dynamic: bool,
    pub option_base: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundUdtDescriptor {
    pub type_name: String,
    pub variable_names: Vec<String>,
    pub fields: Vec<BoundUdtFieldDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundUdtFieldDescriptor {
    pub index: usize,
    pub name: String,
    pub bound_type: BoundType,
    pub nested_udt_name: Option<String>,
    pub array_bounds: Option<Vec<(i32, i32)>>,
    pub fixed_string_len: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundParam {
    pub name: String,
    pub source_mechanism: BoundParamSourceMechanism,
    pub by_ref: bool,
    pub param_array: bool,
    pub optional: bool,
    pub default_value: Option<i32>,
    pub ty: BoundType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundParamSourceMechanism {
    Omitted,
    ExplicitByRef,
    ExplicitByVal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundExternalDecl {
    pub name: String,
    pub library: String,
    pub alias: String,
    pub ptr_safe: bool,
    pub ordinal_alias: bool,
    pub params: Vec<BoundParam>,
    pub return_type: BoundType,
    pub is_function: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcKind {
    Sub,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicSurface {
    DeterministicCore,
    HostSensitive,
}

pub fn resolve_symbols(source: &str) -> BoundModule {
    let mut option_explicit = false;
    let lines = normalize_source_lines(source);
    let compare_mode = collect_option_compare_mode(&lines);
    let option_base = collect_option_base(&lines);
    let default_type_table = collect_default_type_table(&lines);
    let udt_defs = collect_udt_definitions(&lines, &default_type_table);
    let enum_descriptors = collect_enum_descriptors(&lines);
    let module_constants = collect_module_constants(&lines);
    let property_write_routes = collect_property_write_routes(&lines);
    let property_read_routes = collect_property_read_routes(&lines);
    let (declared_externals, external_declarations, external_decl_diagnostics) =
        collect_declared_external_procedures(&lines, &default_type_table);
    let class_model_diagnostics = collect_class_model_diagnostics(&lines);

    let has_explicit_procs = lines
        .iter()
        .any(|line| detect_proc_kind(&line.to_ascii_lowercase()).is_some());

    let top_level_mainline = build_top_level_mainline_procedure(
        &lines,
        &mut option_explicit,
        option_base,
        &default_type_table,
        &udt_defs,
        &module_constants,
        &property_write_routes,
        &property_read_routes,
    );

    let procedures = if has_explicit_procs {
        let mut procedures = parse_procedures(
            &lines,
            &mut option_explicit,
            option_base,
            &default_type_table,
            &udt_defs,
            &module_constants,
            &property_write_routes,
            &property_read_routes,
        );
        if let Some(mainline) = top_level_mainline.clone()
            && !procedures
                .iter()
                .any(|existing| existing.name.eq_ignore_ascii_case("main"))
        {
            procedures.insert(0, mainline);
        }
        procedures
    } else {
        vec![top_level_mainline.unwrap_or_else(|| {
            build_whole_file_main_procedure(
                &lines,
                &mut option_explicit,
                option_base,
                &default_type_table,
                &udt_defs,
                &module_constants,
                &property_write_routes,
                &property_read_routes,
            )
        })]
    };

    let mut procedures = procedures;
    for external in declared_externals {
        if !procedures
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&external.name))
        {
            procedures.push(external);
        }
    }

    let entry_idx = procedures
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case("main"))
        .unwrap_or(0);
    let entry = procedures
        .get(entry_idx)
        .cloned()
        .unwrap_or(BoundProcedure {
            name: "main".to_string(),
            source_line_start: 1,
            source_line_end: lines.len().max(1),
            statement_line_numbers: Vec::new(),
            return_type: BoundType::Variant,
            params: Vec::new(),
            module_scope_names: Vec::new(),
            declarations: Vec::new(),
            declaration_types: HashMap::new(),
            array_descriptors: HashMap::new(),
            udt_descriptors: Vec::new(),
            duplicate_declarations: Vec::new(),
            body: Vec::new(),
        });

    let mut resolution_diagnostics = external_decl_diagnostics;
    for diagnostic in class_model_diagnostics {
        if !resolution_diagnostics
            .iter()
            .any(|existing| existing == &diagnostic)
        {
            resolution_diagnostics.push(diagnostic);
        }
    }

    BoundModule {
        source: source.to_string(),
        option_explicit,
        is_class_module: false,
        compare_mode,
        default_type_table,
        resolution_diagnostics,
        declarations: entry.declarations.clone(),
        declaration_types: entry.declaration_types.clone(),
        array_descriptors: entry.array_descriptors.clone(),
        enum_descriptors,
        external_declarations,
        body: entry.body.clone(),
        procedures,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_whole_file_main_procedure(
    lines: &[String],
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
) -> BoundProcedure {
    build_mainline_procedure_from_lines(
        lines,
        lines,
        1,
        lines.len().max(1),
        collect_candidate_statement_line_numbers(lines, 1),
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
    )
    .unwrap_or(BoundProcedure {
        name: "main".to_string(),
        source_line_start: 1,
        source_line_end: lines.len().max(1),
        statement_line_numbers: collect_candidate_statement_line_numbers(lines, 1),
        return_type: BoundType::Variant,
        params: Vec::new(),
        module_scope_names: Vec::new(),
        declarations: Vec::new(),
        declaration_types: HashMap::new(),
        array_descriptors: HashMap::new(),
        udt_descriptors: Vec::new(),
        duplicate_declarations: Vec::new(),
        body: Vec::new(),
    })
}

#[allow(clippy::too_many_arguments)]
fn build_top_level_mainline_procedure(
    lines: &[String],
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
) -> Option<BoundProcedure> {
    let mainline_lines = extract_top_level_mainline_lines(lines);
    if mainline_lines.is_empty() {
        return None;
    }
    let statement_line_numbers = collect_top_level_mainline_line_numbers(lines);
    let source_line_start = statement_line_numbers.first().copied().unwrap_or(1);
    let source_line_end = statement_line_numbers
        .last()
        .copied()
        .unwrap_or(source_line_start);
    build_mainline_procedure_from_lines(
        lines,
        &mainline_lines,
        source_line_start,
        source_line_end,
        statement_line_numbers,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_mainline_procedure_from_lines(
    module_lines: &[String],
    lines: &[String],
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
) -> Option<BoundProcedure> {
    if lines.is_empty() {
        return None;
    }

    let mut declarations: Vec<String> = Vec::new();
    let mut declaration_types: HashMap<String, BoundType> = HashMap::new();
    let mut duplicate_declarations: Vec<String> = Vec::new();
    let mut array_bounds: ArrayBoundsMap = HashMap::new();
    let mut index = 0;
    seed_module_scope_declarations(
        module_lines,
        &mut declarations,
        &mut declaration_types,
        &mut duplicate_declarations,
        &mut array_bounds,
        option_base,
        default_type_table,
        udt_defs,
    );
    for (name, _) in sorted_module_constants(module_constants) {
        if !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            declarations.push(name.clone());
        }
        let ty = module_constants
            .get(name.as_str())
            .map(module_const_expr_type)
            .unwrap_or(BoundType::Variant);
        declaration_types.insert(name, ty);
    }
    let mut body = parse_block(
        lines,
        &mut index,
        &mut declarations,
        &mut declaration_types,
        &mut duplicate_declarations,
        &mut array_bounds,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
        &[],
    );
    body.splice(0..0, build_const_prelude(module_constants));
    let array_descriptors =
        build_array_descriptors(&array_bounds, &declaration_types, &body, option_base);
    let udt_descriptors = build_udt_descriptors(&declarations, &declaration_types, udt_defs);
    remove_udt_type_markers(&mut declaration_types);
    Some(BoundProcedure {
        name: "main".to_string(),
        source_line_start,
        source_line_end,
        statement_line_numbers,
        return_type: BoundType::Variant,
        params: Vec::new(),
        module_scope_names: Vec::new(),
        declarations,
        declaration_types,
        array_descriptors,
        udt_descriptors,
        duplicate_declarations,
        body,
    })
}

fn collect_top_level_mainline_line_numbers(lines: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut active_proc_end: Option<&'static str> = None;
    let mut active_decl_block_end: Option<&'static str> = None;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if let Some(end_term) = active_proc_end {
            if lower == end_term {
                active_proc_end = None;
            }
            continue;
        }

        if let Some(end_term) = active_decl_block_end {
            if lower == end_term {
                active_decl_block_end = None;
            }
            continue;
        }

        if trimmed.is_empty()
            || trimmed.starts_with('\'')
            || trimmed
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rem "))
        {
            continue;
        }
        if let Some(kind) = detect_proc_kind(&lower) {
            active_proc_end = Some(kind.end_term());
            continue;
        }
        if lower.starts_with("type ") {
            active_decl_block_end = Some("end type");
            continue;
        }
        if lower.starts_with("enum ") {
            active_decl_block_end = Some("end enum");
            continue;
        }
        if lower.starts_with("attribute ") {
            continue;
        }
        out.push(index + 1);
    }

    out
}

fn collect_candidate_statement_line_numbers(
    lines: &[String],
    start_line_number: usize,
) -> Vec<usize> {
    let mut out = Vec::new();
    for (offset, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('\'')
            || trimmed
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rem "))
        {
            continue;
        }
        out.push(start_line_number + offset);
    }
    out
}

fn extract_top_level_mainline_lines(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut active_proc_end: Option<&'static str> = None;
    let mut active_decl_block_end: Option<&'static str> = None;

    for line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if let Some(end_term) = active_proc_end {
            if lower == end_term {
                active_proc_end = None;
            }
            continue;
        }

        if let Some(end_term) = active_decl_block_end {
            if lower == end_term {
                active_decl_block_end = None;
            }
            continue;
        }

        if trimmed.is_empty()
            || trimmed.starts_with('\'')
            || trimmed
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rem "))
        {
            continue;
        }
        if let Some(kind) = detect_proc_kind(&lower) {
            active_proc_end = Some(kind.end_term());
            continue;
        }
        if starts_type_block(&lower) {
            active_decl_block_end = Some("end type");
            continue;
        }
        if starts_enum_block(&lower) {
            active_decl_block_end = Some("end enum");
            continue;
        }
        if is_non_mainline_top_level_directive(trimmed) {
            continue;
        }
        out.push(line.clone());
    }

    out
}

fn is_non_mainline_top_level_directive(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    lower.starts_with("attribute ")
        || lower.starts_with("option ")
        || lower.starts_with("#const ")
        || lower.starts_with("dim ")
        || lower.starts_with("global ")
        || lower.starts_with("static ")
        || lower.starts_with("public ")
        || lower.starts_with("private ")
        || lower.starts_with("friend ")
        || lower.starts_with("implements ")
        || lower.starts_with("event ")
        || lower.starts_with("const ")
        || lower.starts_with("public const ")
        || lower.starts_with("private const ")
        || lower.starts_with("friend const ")
        || lower.starts_with("declare ")
        || lower.starts_with("public declare ")
        || lower.starts_with("private declare ")
        || is_def_type_directive(&lower)
}

#[allow(clippy::too_many_arguments)]
fn seed_module_scope_declarations(
    lines: &[String],
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
) {
    let mut active_proc_end: Option<&'static str> = None;
    let mut active_decl_block_end: Option<&'static str> = None;

    for line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if let Some(end_term) = active_proc_end {
            if lower == end_term {
                active_proc_end = None;
            }
            continue;
        }

        if let Some(end_term) = active_decl_block_end {
            if lower == end_term {
                active_decl_block_end = None;
            }
            continue;
        }

        if let Some(kind) = detect_proc_kind(&lower) {
            active_proc_end = Some(kind.end_term());
            continue;
        }
        if starts_type_block(&lower) {
            active_decl_block_end = Some("end type");
            continue;
        }
        if starts_enum_block(&lower) {
            active_decl_block_end = Some("end enum");
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with('\'')
            || trimmed
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rem "))
        {
            continue;
        }

        parse_variable_declaration_line(
            trimmed,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_base,
            default_type_table,
            udt_defs,
        );
    }
}

fn is_def_type_directive(lower: &str) -> bool {
    [
        "defbool ",
        "defbyte ",
        "defint ",
        "deflng ",
        "deflnglng ",
        "deflngptr ",
        "defsng ",
        "defdbl ",
        "defdec ",
        "defcur ",
        "defdate ",
        "defstr ",
        "defobj ",
        "defvar ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn starts_type_block(lower: &str) -> bool {
    lower.starts_with("type ")
        || lower.starts_with("private type ")
        || lower.starts_with("public type ")
}

fn starts_enum_block(lower: &str) -> bool {
    lower.starts_with("enum ")
        || lower.starts_with("private enum ")
        || lower.starts_with("public enum ")
}

pub(crate) fn normalize_source_lines(source: &str) -> Vec<String> {
    let mut merged = Vec::new();
    let mut pending = String::new();

    for raw in source.lines() {
        let trimmed = strip_inline_comment(raw).trim();
        if trimmed.is_empty() {
            continue;
        }

        let has_line_continuation = trimmed.ends_with(" _");
        let segment = if has_line_continuation {
            trimmed[..trimmed.len() - 2].trim_end()
        } else {
            trimmed
        };

        if !pending.is_empty() && !segment.is_empty() {
            pending.push(' ');
        }
        pending.push_str(segment);

        if has_line_continuation {
            continue;
        }

        let final_line = pending.trim();
        if !final_line.is_empty() {
            merged.push(final_line.to_string());
        }
        pending.clear();
    }

    let final_line = pending.trim();
    if !final_line.is_empty() {
        merged.push(final_line.to_string());
    }

    let conditional_filtered = apply_conditional_compilation(&merged);

    let mut out = Vec::new();
    let mut with_stack: Vec<String> = Vec::new();
    for line in conditional_filtered {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("with ") {
            let raw_target = line[5..].trim();
            let parent = with_stack.last().map(String::as_str);
            if let Some(target) = normalize_with_target(raw_target, parent) {
                with_stack.push(target);
            } else {
                out.push(line);
            }
            continue;
        }

        if lower == "end with" {
            if with_stack.pop().is_none() {
                out.push(line);
            }
            continue;
        }

        if let Some(target) = with_stack.last() {
            out.push(rewrite_with_member_accesses(&line, target));
        } else {
            out.push(line);
        }
    }

    out
}

fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut in_string = false;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                if in_string && index + 1 < bytes.len() && bytes[index + 1] == b'"' {
                    index += 2;
                    continue;
                }
                in_string = !in_string;
            }
            b'\'' if !in_string => return line[..index].trim_end(),
            _ => {}
        }
        index += 1;
    }
    line.trim_end()
}

fn normalize_with_target(raw: &str, parent_target: Option<&str>) -> Option<String> {
    let trimmed = raw.trim();
    if let Some(member_tail) = trimmed.strip_prefix('.') {
        let parent = parent_target?;
        let member_chain = normalize_member_chain(member_tail)?;
        return Some(format!("{parent}_{member_chain}"));
    }
    normalize_member_chain(trimmed)
}

fn rewrite_with_member_accesses(line: &str, target: &str) -> String {
    let chars = line.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(line.len() + 16);
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '.'
            && i + 1 < chars.len()
            && (chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_')
        {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                prev.is_whitespace()
                    || matches!(prev, '(' | ')' | ',' | '=' | '+' | '-' | '*' | '/' | ':')
            };
            if prev_ok {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let member = chars[i + 1..j]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase();
                out.push_str(target);
                out.push('_');
                out.push_str(&member);
                i = j;
                continue;
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

#[derive(Debug, Clone, Copy)]
struct ConditionalFrame {
    parent_active: bool,
    branch_taken: bool,
    current_active: bool,
}

fn apply_conditional_compilation(lines: &[String]) -> Vec<String> {
    let mut constants = builtin_pp_constants();
    let mut frames: Vec<ConditionalFrame> = Vec::new();
    let mut current_active = true;
    let mut out = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        if let Some((name_raw, expr_raw)) = parse_pp_const(trimmed) {
            if current_active && let Some(name) = normalize_ident(name_raw) {
                let value = eval_pp_expr(expr_raw, &constants).unwrap_or(0);
                constants.insert(name, value);
            }
            continue;
        }

        if let Some(expr_raw) = parse_pp_if(trimmed) {
            let parent_active = current_active;
            let branch_active = parent_active && eval_pp_condition(expr_raw, &constants);
            frames.push(ConditionalFrame {
                parent_active,
                branch_taken: branch_active,
                current_active: branch_active,
            });
            current_active = branch_active;
            continue;
        }

        if let Some(expr_raw) = parse_pp_elseif(trimmed) {
            if let Some(frame) = frames.last_mut() {
                let branch_active = frame.parent_active
                    && !frame.branch_taken
                    && eval_pp_condition(expr_raw, &constants);
                frame.current_active = branch_active;
                if branch_active {
                    frame.branch_taken = true;
                }
                current_active = branch_active;
            } else {
                out.push(line.clone());
            }
            continue;
        }

        if is_pp_else(trimmed) {
            if let Some(frame) = frames.last_mut() {
                let branch_active = frame.parent_active && !frame.branch_taken;
                frame.current_active = branch_active;
                frame.branch_taken = true;
                current_active = branch_active;
            } else {
                out.push(line.clone());
            }
            continue;
        }

        if is_pp_end_if(trimmed) {
            if frames.pop().is_some() {
                current_active = frames.last().is_none_or(|f| f.current_active);
            } else {
                out.push(line.clone());
            }
            continue;
        }

        if current_active {
            out.push(line.clone());
        }
    }

    out
}

/// Predefined `#If` compilation constants for the targeted dialect: **VBA 7.1**
/// (Office 2013+). Values follow the VBA predefined-constant rules:
/// - `Vba7` = True (PtrSafe / LongPtr era); `Vba6` = False (the older VBA6 runtime flag).
/// - `Win64`/`Win32` are complementary on Windows and keyed to the build's pointer width
///   (64-bit host => `Win64` True, `Win32` False); `Win16` is always False.
/// - `Mac` is True only on macOS.
///
/// Constants are case-insensitive and may be overridden by source `#Const` directives.
pub(crate) fn builtin_pp_constants() -> HashMap<String, i32> {
    let win64 = cfg!(all(windows, target_pointer_width = "64"));
    let win32 = cfg!(all(windows, target_pointer_width = "32"));
    let bool_to_cc = |flag: bool| if flag { -1 } else { 0 };
    let mut constants = HashMap::new();
    constants.insert("vba7".to_string(), -1);
    constants.insert("vba6".to_string(), 0);
    constants.insert("win64".to_string(), bool_to_cc(win64));
    constants.insert("win32".to_string(), bool_to_cc(win32));
    constants.insert("win16".to_string(), 0);
    constants.insert("mac".to_string(), bool_to_cc(cfg!(target_os = "macos")));
    constants
}

fn parse_pp_const(line: &str) -> Option<(&str, &str)> {
    let rest = strip_directive_prefix_ci(line, "#const")?;
    let (name, expr) = rest.split_once('=')?;
    let name = name.trim();
    let expr = expr.trim();
    if name.is_empty() || expr.is_empty() {
        return None;
    }
    Some((name, expr))
}

fn parse_pp_if(line: &str) -> Option<&str> {
    parse_pp_conditional_directive(line, "#if")
}

fn parse_pp_elseif(line: &str) -> Option<&str> {
    parse_pp_conditional_directive(line, "#elseif")
}

fn parse_pp_conditional_directive<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = strip_directive_prefix_ci(line, keyword)?;
    let lower = rest.to_ascii_lowercase();
    if !lower.ends_with(" then") {
        return None;
    }
    let expr = rest[..rest.len() - 5].trim();
    if expr.is_empty() {
        return None;
    }
    Some(expr)
}

fn is_pp_else(line: &str) -> bool {
    line.eq_ignore_ascii_case("#else")
}

fn is_pp_end_if(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    lowered
        .split_whitespace()
        .eq(["#end", "if"].iter().copied())
}

fn strip_directive_prefix_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let lowered = line.to_ascii_lowercase();
    let marker = format!("{prefix} ");
    if lowered.starts_with(&marker) {
        Some(line[marker.len()..].trim())
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PpToken {
    LParen,
    RParen,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Int(i32),
    Ident(String),
}

fn eval_pp_condition(expr: &str, constants: &HashMap<String, i32>) -> bool {
    eval_pp_expr(expr, constants).is_some_and(|value| value != 0)
}

fn eval_pp_expr(expr: &str, constants: &HashMap<String, i32>) -> Option<i32> {
    let tokens = tokenize_pp_expr(expr)?;
    let mut parser = PpExprParser {
        tokens: &tokens,
        index: 0,
        constants,
    };
    let value = parser.parse_or()?;
    if parser.index == tokens.len() {
        Some(value)
    } else {
        None
    }
}

fn tokenize_pp_expr(expr: &str) -> Option<Vec<PpToken>> {
    let chars = expr.chars().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        match ch {
            '(' => {
                out.push(PpToken::LParen);
                i += 1;
                continue;
            }
            ')' => {
                out.push(PpToken::RParen);
                i += 1;
                continue;
            }
            '=' => {
                out.push(PpToken::Eq);
                i += 1;
                continue;
            }
            '<' => {
                if i + 1 < chars.len() {
                    if chars[i + 1] == '=' {
                        out.push(PpToken::Le);
                        i += 2;
                        continue;
                    }
                    if chars[i + 1] == '>' {
                        out.push(PpToken::Ne);
                        i += 2;
                        continue;
                    }
                }
                out.push(PpToken::Lt);
                i += 1;
                continue;
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(PpToken::Ge);
                    i += 2;
                    continue;
                }
                out.push(PpToken::Gt);
                i += 1;
                continue;
            }
            _ => {}
        }

        if ch == '-' || ch.is_ascii_digit() {
            let mut j = i;
            if chars[j] == '-' {
                j += 1;
                if j >= chars.len() || !chars[j].is_ascii_digit() {
                    return None;
                }
            }
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            let value = chars[i..j].iter().collect::<String>().parse::<i32>().ok()?;
            out.push(PpToken::Int(value));
            i = j;
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let ident = chars[i..j].iter().collect::<String>().to_ascii_lowercase();
            match ident.as_str() {
                "and" => out.push(PpToken::And),
                "or" => out.push(PpToken::Or),
                "not" => out.push(PpToken::Not),
                "true" => out.push(PpToken::Int(-1)),
                "false" => out.push(PpToken::Int(0)),
                _ => out.push(PpToken::Ident(ident)),
            }
            i = j;
            continue;
        }

        return None;
    }

    Some(out)
}

struct PpExprParser<'a> {
    tokens: &'a [PpToken],
    index: usize,
    constants: &'a HashMap<String, i32>,
}

impl<'a> PpExprParser<'a> {
    fn parse_or(&mut self) -> Option<i32> {
        let mut lhs = self.parse_and()?;
        while self.consume(&PpToken::Or) {
            let rhs = self.parse_and()?;
            lhs = pp_bool(lhs != 0 || rhs != 0);
        }
        Some(lhs)
    }

    fn parse_and(&mut self) -> Option<i32> {
        let mut lhs = self.parse_not()?;
        while self.consume(&PpToken::And) {
            let rhs = self.parse_not()?;
            lhs = pp_bool(lhs != 0 && rhs != 0);
        }
        Some(lhs)
    }

    fn parse_not(&mut self) -> Option<i32> {
        if self.consume(&PpToken::Not) {
            return Some(pp_bool(self.parse_not()? == 0));
        }
        self.parse_compare()
    }

    fn parse_compare(&mut self) -> Option<i32> {
        let lhs = self.parse_primary()?;
        let Some(op) = self.peek_compare() else {
            return Some(lhs);
        };
        self.index += 1;
        let rhs = self.parse_primary()?;
        let out = match op {
            PpToken::Eq => lhs == rhs,
            PpToken::Ne => lhs != rhs,
            PpToken::Lt => lhs < rhs,
            PpToken::Le => lhs <= rhs,
            PpToken::Gt => lhs > rhs,
            PpToken::Ge => lhs >= rhs,
            _ => return None,
        };
        Some(pp_bool(out))
    }

    fn parse_primary(&mut self) -> Option<i32> {
        let token = self.tokens.get(self.index)?;
        match token {
            PpToken::Int(value) => {
                self.index += 1;
                Some(*value)
            }
            PpToken::Ident(name) => {
                self.index += 1;
                Some(self.constants.get(name).copied().unwrap_or(0))
            }
            PpToken::LParen => {
                self.index += 1;
                let value = self.parse_or()?;
                if !self.consume(&PpToken::RParen) {
                    return None;
                }
                Some(value)
            }
            _ => None,
        }
    }

    fn consume(&mut self, token: &PpToken) -> bool {
        if self.tokens.get(self.index) == Some(token) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek_compare(&self) -> Option<PpToken> {
        match self.tokens.get(self.index) {
            Some(PpToken::Eq) => Some(PpToken::Eq),
            Some(PpToken::Ne) => Some(PpToken::Ne),
            Some(PpToken::Lt) => Some(PpToken::Lt),
            Some(PpToken::Le) => Some(PpToken::Le),
            Some(PpToken::Gt) => Some(PpToken::Gt),
            Some(PpToken::Ge) => Some(PpToken::Ge),
            _ => None,
        }
    }
}

fn pp_bool(value: bool) -> i32 {
    if value { -1 } else { 0 }
}

#[allow(clippy::too_many_arguments)]
fn parse_procedures(
    lines: &[String],
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
) -> Vec<BoundProcedure> {
    let mut procedures = Vec::new();
    let mut index = 0;
    let mut module_declarations: Vec<String> = Vec::new();
    let mut module_declaration_types: HashMap<String, BoundType> = HashMap::new();
    let mut module_duplicate_declarations: Vec<String> = Vec::new();
    let mut module_array_bounds: ArrayBoundsMap = HashMap::new();
    seed_module_scope_declarations(
        lines,
        &mut module_declarations,
        &mut module_declaration_types,
        &mut module_duplicate_declarations,
        &mut module_array_bounds,
        option_base,
        default_type_table,
        udt_defs,
    );

    while index < lines.len() {
        let line = lines[index].as_str();
        let lower = line.to_ascii_lowercase();

        if lower == "option explicit" {
            *option_explicit = true;
            index += 1;
            continue;
        }
        if parse_option_base_directive(line).is_some() {
            index += 1;
            continue;
        }

        let Some(kind) = detect_proc_kind(&lower) else {
            index += 1;
            continue;
        };

        let Some((name, params, return_type)) =
            parse_proc_signature(line, kind, default_type_table)
        else {
            index += 1;
            continue;
        };

        let source_line_start = index + 1;
        index += 1;
        let mut declarations: Vec<String> = module_declarations.clone();
        let mut declaration_types: HashMap<String, BoundType> = module_declaration_types.clone();
        let mut duplicate_declarations: Vec<String> = module_duplicate_declarations.clone();
        let mut array_bounds: ArrayBoundsMap = module_array_bounds.clone();
        let mut module_scope_names = declarations.clone();
        for param in &params {
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&param.name))
            {
                declarations.push(param.name.clone());
            }
            declaration_types.insert(param.name.clone(), param.ty);
            if param.ty == BoundType::Array {
                // Procedure array parameters do not carry compile-time extents here,
                // but they still need array identity so `VarPtr(arg(0))` and similar
                // parser paths recognize them as array-backed values.
                array_bounds
                    .entry(param.name.clone())
                    .or_insert_with(|| vec![(0, 0)]);
            }
        }
        if matches!(kind, ProcKind::Function | ProcKind::PropertyGet) {
            // For Property Get, the return-value slot must use the base name
            // (e.g. "value") so that assignments like `Value = 9` inside the
            // body write to the correct slot.  For plain Functions the
            // canonical name equals the base name already.
            let return_decl_name = match kind {
                ProcKind::PropertyGet => name
                    .strip_prefix("property_get_")
                    .unwrap_or(&name)
                    .to_string(),
                _ => name.clone(),
            };
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&return_decl_name))
            {
                declarations.push(return_decl_name.clone());
                declaration_types.insert(return_decl_name, return_type);
            }
        }
        for (name, _) in sorted_module_constants(module_constants) {
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                declarations.push(name.clone());
            }
            if !module_scope_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                module_scope_names.push(name.clone());
            }
            let ty = module_constants
                .get(name.as_str())
                .map(module_const_expr_type)
                .unwrap_or(BoundType::Variant);
            declaration_types.entry(name).or_insert(ty);
        }
        let end_term = kind.end_term();
        let mut body = parse_block(
            lines,
            &mut index,
            &mut declarations,
            &mut declaration_types,
            &mut duplicate_declarations,
            &mut array_bounds,
            option_explicit,
            option_base,
            default_type_table,
            udt_defs,
            module_constants,
            property_write_routes,
            property_read_routes,
            &[end_term],
        );
        body.splice(0..0, build_const_prelude(module_constants));
        let array_descriptors =
            build_array_descriptors(&array_bounds, &declaration_types, &body, option_base);
        let udt_descriptors = build_udt_descriptors(&declarations, &declaration_types, udt_defs);
        remove_udt_type_markers(&mut declaration_types);
        let source_line_end = if index < lines.len() && lines[index].eq_ignore_ascii_case(end_term)
        {
            index + 1
        } else {
            index.max(source_line_start)
        };
        let mut statement_line_numbers = collect_candidate_statement_line_numbers(
            &lines[source_line_start..index],
            source_line_start + 1,
        );
        if statement_line_numbers.is_empty() {
            statement_line_numbers.push(source_line_start);
        }
        if index < lines.len() && lines[index].eq_ignore_ascii_case(end_term) {
            index += 1;
        }

        procedures.push(BoundProcedure {
            name,
            source_line_start,
            source_line_end,
            statement_line_numbers,
            return_type,
            params,
            module_scope_names,
            declarations,
            declaration_types,
            array_descriptors,
            udt_descriptors,
            duplicate_declarations,
            body,
        });
    }

    procedures
}

impl ProcKind {
    fn prefix_len(self) -> usize {
        match self {
            Self::Sub => 4,
            Self::Function => 9,
            Self::PropertyGet | Self::PropertyLet | Self::PropertySet => 13,
        }
    }

    fn end_term(self) -> &'static str {
        match self {
            Self::Sub => "end sub",
            Self::Function => "end function",
            Self::PropertyGet | Self::PropertyLet | Self::PropertySet => "end property",
        }
    }

    fn canonical_name(self, base: String) -> String {
        match self {
            Self::Sub | Self::Function => base,
            Self::PropertyGet => format!("property_get_{base}"),
            Self::PropertyLet => format!("property_let_{base}"),
            Self::PropertySet => format!("property_set_{base}"),
        }
    }
}

pub fn detect_proc_kind(lower: &str) -> Option<ProcKind> {
    let lower = strip_proc_scope_prefixes_ci(lower);
    if lower.starts_with("sub ") {
        Some(ProcKind::Sub)
    } else if lower.starts_with("function ") {
        Some(ProcKind::Function)
    } else if lower.starts_with("property get ") {
        Some(ProcKind::PropertyGet)
    } else if lower.starts_with("property let ") {
        Some(ProcKind::PropertyLet)
    } else if lower.starts_with("property set ") {
        Some(ProcKind::PropertySet)
    } else {
        None
    }
}

fn parse_proc_base_name(line: &str, kind: ProcKind) -> Option<String> {
    let line = strip_proc_scope_prefixes_ci(line);
    let rest = line.get(kind.prefix_len()..)?.trim();
    let name_token = rest
        .split('(')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    normalize_ident(name_token)
}

pub fn parse_proc_signature(
    line: &str,
    kind: ProcKind,
    default_type_table: &[BoundType; 26],
) -> Option<(String, Vec<BoundParam>, BoundType)> {
    let line = strip_proc_scope_prefixes_ci(line);
    let prefix_len = kind.prefix_len();
    let rest = line.get(prefix_len..)?.trim();
    let name_token = rest
        .split('(')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let (base_name, name_type_char) = normalize_ident_with_type_char(name_token)?;
    let name = kind.canonical_name(base_name.clone());
    let mut params = Vec::new();
    let mut seen_optional = false;
    let mut seen_param_array = false;
    let mut explicit_return_ty = None;

    if let Some(open) = rest.find('(')
        && let Some(close) = find_matching_paren(rest, open)
        && close > open
    {
        let params_raw = rest[open + 1..close].trim();
        if !params_raw.is_empty() {
            for item in params_raw.split(',') {
                if seen_param_array {
                    return None;
                }
                let mut token = item.trim();
                if token.is_empty() {
                    return None;
                }
                let mut optional = false;
                if token.to_ascii_lowercase().starts_with("optional ") {
                    optional = true;
                    token = token[9..].trim();
                }
                let mut param_array = false;
                if token.to_ascii_lowercase().starts_with("paramarray ") {
                    param_array = true;
                    token = token[11..].trim();
                }
                let lower = token.to_ascii_lowercase();
                let (source_mechanism, by_ref, remainder) = if param_array {
                    if lower.starts_with("byval ") || lower.starts_with("byref ") {
                        return None;
                    }
                    (BoundParamSourceMechanism::Omitted, false, token)
                } else if lower.starts_with("byval ") {
                    (
                        BoundParamSourceMechanism::ExplicitByVal,
                        false,
                        token[6..].trim(),
                    )
                } else if lower.starts_with("byref ") {
                    (
                        BoundParamSourceMechanism::ExplicitByRef,
                        true,
                        token[6..].trim(),
                    )
                } else {
                    (BoundParamSourceMechanism::Omitted, true, token)
                };
                let (decl_text, default_value) = if let Some((lhs, rhs)) = remainder.split_once('=')
                {
                    (lhs.trim(), Some(parse_param_default(rhs.trim())?))
                } else {
                    (remainder, None)
                };

                let (name_text, explicit_ty) =
                    if let Some((lhs, rhs)) = split_keyword_ci(decl_text, "as") {
                        (
                            lhs.trim(),
                            Some(parse_declared_type(rhs.trim()).unwrap_or(BoundType::Variant)),
                        )
                    } else {
                        (decl_text, None)
                    };

                if default_value.is_some() && !optional {
                    return None;
                }
                if param_array && (optional || default_value.is_some()) {
                    return None;
                }
                // `Optional` parameters are ByRef by default in VBA (e.g. `Optional b As Long`);
                // `Optional ByRef`/`Optional ByVal` are both legal. Do not reject ByRef here.
                if optional {
                    seen_optional = true;
                } else if seen_optional {
                    return None;
                }

                let trimmed_name_text = name_text.trim();
                let is_array_param = trimmed_name_text.ends_with("()");
                let normalized_name_text = if param_array || is_array_param {
                    trimmed_name_text.strip_suffix("()")?
                } else {
                    name_text
                };
                let (param_name, type_char_ty) =
                    normalize_ident_with_type_char(normalized_name_text)?;
                let ty = if param_array {
                    if explicit_ty.is_some() && explicit_ty != Some(BoundType::Variant) {
                        return None;
                    }
                    BoundType::Array
                } else if is_array_param {
                    BoundType::Array
                } else {
                    resolve_declared_type(
                        &param_name,
                        explicit_ty,
                        type_char_ty,
                        default_type_table,
                    )
                };
                params.push(BoundParam {
                    name: param_name,
                    source_mechanism,
                    by_ref,
                    param_array,
                    optional,
                    default_value,
                    ty,
                });
                if param_array {
                    seen_param_array = true;
                }
            }
        }

        if matches!(kind, ProcKind::Function | ProcKind::PropertyGet) {
            let tail = rest[close + 1..].trim();
            if let Some(ty_text) = strip_keyword_prefix_ci(tail, "as") {
                explicit_return_ty =
                    Some(parse_declared_or_array_type(ty_text).unwrap_or(BoundType::Variant));
            }
        }
    } else if matches!(kind, ProcKind::Function | ProcKind::PropertyGet)
        && let Some((_, rhs)) = split_keyword_ci(rest, "as")
    {
        explicit_return_ty =
            Some(parse_declared_or_array_type(rhs.trim()).unwrap_or(BoundType::Variant));
    }

    let return_type = if matches!(kind, ProcKind::Function | ProcKind::PropertyGet) {
        resolve_declared_type(
            &base_name,
            explicit_return_ty,
            name_type_char,
            default_type_table,
        )
    } else {
        BoundType::Variant
    };

    Some((name, params, return_type))
}

fn parse_param_default(text: &str) -> Option<i32> {
    text.trim().parse::<i32>().ok()
}

fn sorted_module_constants(constants: &ModuleConstMap) -> Vec<(String, BoundExpr)> {
    let mut out = constants
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect::<Vec<_>>();
    out.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    out
}

fn build_const_prelude(constants: &ModuleConstMap) -> Vec<BoundStmt> {
    sorted_module_constants(constants)
        .into_iter()
        .map(|(name, expr)| BoundStmt::Assign {
            target: name,
            expr,
            intent: AssignmentIntent::Implicit,
        })
        .collect()
}

fn module_const_expr_type(expr: &BoundExpr) -> BoundType {
    match expr {
        BoundExpr::IntConst(_) | BoundExpr::AddConst { .. } | BoundExpr::SubConst { .. } => {
            BoundType::Long
        }
        BoundExpr::BoolConst(_) => BoundType::Boolean,
        BoundExpr::FloatConst(_) => BoundType::Double,
        BoundExpr::StringConst(_) => BoundType::String,
        BoundExpr::Var(_)
        | BoundExpr::BinaryOp { .. }
        | BoundExpr::CompareOp { .. }
        | BoundExpr::UnaryOp { .. }
        | BoundExpr::LogicalBinaryOp { .. }
        | BoundExpr::LogicalNot { .. }
        | BoundExpr::IntrinsicCall { .. }
        | BoundExpr::ProcCall { .. }
        | BoundExpr::VarPtrArrayBuffer(_) => BoundType::Variant,
    }
}

fn collect_module_constants(lines: &[String]) -> ModuleConstMap {
    let mut constants = HashMap::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].as_str();
        let lower = line.to_ascii_lowercase();
        if let Some(kind) = detect_proc_kind(&lower) {
            let end_term = kind.end_term();
            index += 1;
            while index < lines.len() && !lines[index].eq_ignore_ascii_case(end_term) {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }

        if parse_enum_header(line).is_some() {
            parse_enum_block(lines, &mut index, &mut constants);
            continue;
        }

        if let Some((name, expr)) = parse_const_declaration(line) {
            constants.insert(name, expr);
        }
        index += 1;
    }

    constants
}

fn collect_enum_descriptors(lines: &[String]) -> Vec<BoundEnumDescriptor> {
    let mut descriptors = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].trim();
        let lower = line.to_ascii_lowercase();
        if let Some(kind) = detect_proc_kind(&lower) {
            let end_term = kind.end_term();
            index += 1;
            while index < lines.len() && !lines[index].eq_ignore_ascii_case(end_term) {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }
        if let Some((type_name, is_public)) = parse_enum_header(line) {
            index += 1;
            let mut members = Vec::new();
            let mut next_value = 0i32;
            while index < lines.len() {
                let line = lines[index].trim();
                if line.eq_ignore_ascii_case("end enum") {
                    break;
                }
                if let Some((name, explicit)) = parse_enum_member(line) {
                    let value = explicit.unwrap_or(next_value);
                    members.push(BoundEnumMemberDescriptor {
                        name,
                        value,
                        ordinal: members.len(),
                        explicit_value: explicit.is_some(),
                    });
                    next_value = value.saturating_add(1);
                }
                index += 1;
            }
            descriptors.push(BoundEnumDescriptor {
                type_name,
                is_public,
                members,
            });
        }
        index += 1;
    }

    descriptors.sort_by(|left, right| {
        left.type_name
            .to_ascii_lowercase()
            .cmp(&right.type_name.to_ascii_lowercase())
    });
    descriptors
}

fn parse_enum_header(line: &str) -> Option<(String, bool)> {
    if let Some(rest) = strip_keyword_prefix_ci(line, "public enum") {
        return normalize_ident(rest).map(|name| (name, true));
    }
    if let Some(rest) = strip_keyword_prefix_ci(line, "private enum") {
        return normalize_ident(rest).map(|name| (name, false));
    }
    strip_keyword_prefix_ci(line, "enum")
        .and_then(|rest| normalize_ident(rest).map(|name| (name, false)))
}

fn collect_property_write_routes(lines: &[String]) -> HashMap<String, String> {
    let mut routes = HashMap::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        let Some(kind) = detect_proc_kind(&lower) else {
            continue;
        };
        if matches!(kind, ProcKind::PropertyLet | ProcKind::PropertySet)
            && let Some(base) = parse_proc_base_name(line, kind)
        {
            routes.insert(base.clone(), kind.canonical_name(base));
        }
    }
    routes
}

fn collect_property_read_routes(lines: &[String]) -> HashMap<String, String> {
    let mut routes = HashMap::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        let Some(kind) = detect_proc_kind(&lower) else {
            continue;
        };
        if matches!(kind, ProcKind::PropertyGet)
            && let Some(base) = parse_proc_base_name(line, kind)
        {
            routes.insert(base.clone(), kind.canonical_name(base));
        }
    }
    routes
}

fn collect_declared_external_procedures(
    lines: &[String],
    default_type_table: &[BoundType; 26],
) -> (
    Vec<BoundProcedure>,
    HashMap<String, BoundExternalDecl>,
    Vec<String>,
) {
    let mut procedures = Vec::new();
    let mut externals = HashMap::new();
    let mut diagnostics = Vec::new();
    for line in lines {
        let declare = match parse_declare_signature_line(line, default_type_table) {
            Ok(Some(declare)) => declare,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(format!("{diagnostic}: `{}`", line.trim()));
                continue;
            }
        };
        let name = declare.name.clone();
        let params = declare.params.clone();
        let return_type = declare.return_type;
        let mut declarations: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut declaration_types: HashMap<String, BoundType> =
            params.iter().map(|p| (p.name.clone(), p.ty)).collect();
        if !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            declarations.push(name.clone());
            declaration_types.insert(name.clone(), return_type);
        }
        let external_params = params.clone();
        procedures.push(BoundProcedure {
            name,
            source_line_start: 1,
            source_line_end: 1,
            statement_line_numbers: vec![1],
            return_type,
            params,
            module_scope_names: Vec::new(),
            declarations,
            declaration_types,
            array_descriptors: HashMap::new(),
            udt_descriptors: Vec::new(),
            duplicate_declarations: Vec::new(),
            body: Vec::new(),
        });
        externals.insert(
            declare.name.to_ascii_lowercase(),
            BoundExternalDecl {
                name: declare.name,
                library: declare.library,
                alias: declare.alias,
                ptr_safe: declare.ptr_safe,
                ordinal_alias: declare.ordinal_alias,
                params: external_params,
                return_type,
                is_function: declare.is_function,
            },
        );
    }
    (procedures, externals, diagnostics)
}

fn collect_class_model_diagnostics(lines: &[String]) -> Vec<String> {
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("option private module") && !is_procedural_module_context(lines) {
            let message = "PMR-E-OPTION-PRIVATE-MODULE-KIND-UNRESOLVED: `Option Private Module` requires project/module-kind integration";
            push_class_model_diagnostic(&mut diagnostics, &mut seen, message, trimmed);
        }
    }

    diagnostics
}

fn push_class_model_diagnostic(
    diagnostics: &mut Vec<String>,
    seen: &mut HashSet<String>,
    message: &str,
    source_line: &str,
) {
    let formatted = format!("{message}: `{source_line}`");
    if seen.insert(formatted.clone()) {
        diagnostics.push(formatted);
    }
}

fn is_procedural_module_context(lines: &[String]) -> bool {
    !lines.iter().any(|line| {
        let lowered = line.trim().to_ascii_lowercase();
        lowered.starts_with("class ") || lowered.starts_with("attribute vb_name")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedDeclareSignature {
    name: String,
    params: Vec<BoundParam>,
    return_type: BoundType,
    is_function: bool,
    library: String,
    alias: String,
    ptr_safe: bool,
    ordinal_alias: bool,
}

fn parse_declare_signature_line(
    line: &str,
    default_type_table: &[BoundType; 26],
) -> Result<Option<ParsedDeclareSignature>, String> {
    let trimmed = line.trim();
    let visibility_trimmed = strip_keyword_prefix_ci(trimmed, "private")
        .or_else(|| strip_keyword_prefix_ci(trimmed, "public"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "friend"))
        .unwrap_or(trimmed)
        .trim();
    let Some(rest) = strip_keyword_prefix_ci(visibility_trimmed, "declare") else {
        return Ok(None);
    };
    let rest = rest.trim();
    let rest_lower = rest.to_ascii_lowercase();
    let (ptr_safe, rest) = if rest_lower.starts_with("ptrsafe ") {
        (true, rest[7..].trim())
    } else {
        (false, rest)
    };
    if !ptr_safe {
        return Err(
            "external procedure declaration rejected: PtrSafe keyword is required".to_string(),
        );
    }

    let lower = rest.to_ascii_lowercase();
    let (kind, tail) = if lower.starts_with("function ") {
        (ProcKind::Function, rest[9..].trim())
    } else if lower.starts_with("sub ") {
        (ProcKind::Sub, rest[4..].trim())
    } else {
        return Err(
            "external procedure declaration rejected: expected `Function` or `Sub`".to_string(),
        );
    };

    let name_token = tail
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or_default()
        .trim();
    if name_token.is_empty() {
        return Err("external procedure declaration rejected: missing procedure name".to_string());
    }

    let open = tail.find('(').ok_or_else(|| {
        "external procedure declaration rejected: missing parameter list".to_string()
    })?;
    let close = tail.rfind(')').ok_or_else(|| {
        "external procedure declaration rejected: missing closing `)`".to_string()
    })?;
    if close <= open {
        return Err(
            "external procedure declaration rejected: malformed parameter list".to_string(),
        );
    }

    let params_text = &tail[open..=close];
    let return_clause = if matches!(kind, ProcKind::Function) {
        let after_params = tail[close + 1..].trim();
        if let Some(rhs) = strip_keyword_prefix_ci(after_params, "as") {
            format!(" As {}", rhs.trim())
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let synthetic = match kind {
        ProcKind::Sub => format!("Sub {name_token}{params_text}"),
        ProcKind::Function => format!("Function {name_token}{params_text}{return_clause}"),
        _ => {
            return Err(
                "external procedure declaration rejected: invalid declaration kind".to_string(),
            );
        }
    };
    let (name, params, return_type) = parse_proc_signature(&synthetic, kind, default_type_table)
        .ok_or_else(|| {
            "external procedure declaration rejected: unable to parse signature".to_string()
        })?;

    let lib = extract_quoted_after_keyword(tail, "lib").ok_or_else(|| {
        "external procedure declaration rejected: missing `Lib \"...\"` clause".to_string()
    })?;
    let alias_raw =
        extract_quoted_after_keyword(tail, "alias").unwrap_or_else(|| name_token.to_string());
    let (alias, ordinal_alias) = normalize_external_alias(alias_raw.as_str())?;

    Ok(Some(ParsedDeclareSignature {
        name,
        params,
        return_type,
        is_function: matches!(kind, ProcKind::Function),
        library: lib.trim().to_ascii_lowercase(),
        alias,
        ptr_safe,
        ordinal_alias,
    }))
}

fn extract_quoted_after_keyword(text: &str, keyword: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let key = keyword.to_ascii_lowercase();
    let needle = format!("{key} ");
    let pos = lower.find(&needle)?;
    let after = &text[pos + needle.len()..];
    let first_quote = after.find('"')?;
    let rest = &after[first_quote + 1..];
    let second_quote = rest.find('"')?;
    Some(rest[..second_quote].to_string())
}

fn normalize_external_alias(alias: &str) -> Result<(String, bool), String> {
    let alias = alias.trim();
    if alias.is_empty() {
        return Err("external procedure declaration rejected: alias must not be empty".to_string());
    }
    if let Some(ordinal_digits) = alias.strip_prefix('#') {
        if ordinal_digits.is_empty() || !ordinal_digits.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(
                "external procedure declaration rejected: ordinal alias must be `#` followed by digits"
                    .to_string(),
            );
        }
        let canonical_digits = ordinal_digits.trim_start_matches('0');
        let canonical_digits = if canonical_digits.is_empty() {
            "0"
        } else {
            canonical_digits
        };
        return Ok((format!("#{canonical_digits}"), true));
    }
    Ok((alias.to_string(), false))
}

/// Parse UDT field array bounds from a parenthesized expression like `(10)` or `(1 To 5)`.
/// Returns `Some(vec![(lo, hi)])` on success, `None` if unparseable.
fn parse_udt_field_array_bounds(bounds_str: &str) -> Option<Vec<(i32, i32)>> {
    let inner = bounds_str
        .trim()
        .strip_prefix('(')?
        .strip_suffix(')')?
        .trim();
    if inner.is_empty() {
        return None;
    }
    let mut result = Vec::new();
    for dim in inner.split(',') {
        let dim = dim.trim();
        let lower_dim = dim.to_ascii_lowercase();
        if let Some((lo_str, hi_str)) = lower_dim.split_once(" to ") {
            let lo: i32 = lo_str.trim().parse().ok()?;
            let hi: i32 = hi_str.trim().parse().ok()?;
            result.push((lo, hi));
        } else {
            let hi: i32 = dim.parse().ok()?;
            result.push((0, hi));
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn collect_udt_definitions(lines: &[String], default_type_table: &[BoundType; 26]) -> UdtDefMap {
    let mut defs = HashMap::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].as_str();
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with("type ") {
            index += 1;
            continue;
        }
        let Some(type_name) = normalize_ident(line[5..].trim()) else {
            index += 1;
            continue;
        };
        index += 1;
        let mut fields = Vec::new();
        while index < lines.len() && !lines[index].eq_ignore_ascii_case("end type") {
            let raw = lines[index].trim();
            if !raw.is_empty() {
                let (field_name_raw, explicit_ty, nested_udt_name, fixed_string_len) =
                    if let Some((lhs, rhs)) = split_keyword_ci(raw, "as") {
                        let rhs_trimmed = rhs.trim();
                        let fixed_string_len = parse_fixed_string_declared_type(rhs_trimmed);
                        let primitive = fixed_string_len
                            .map(|_| BoundType::String)
                            .or_else(|| parse_declared_type(rhs_trimmed));
                        let nested_name = if primitive.is_none() {
                            normalize_ident(rhs_trimmed)
                        } else {
                            None
                        };
                        (
                            lhs.trim(),
                            primitive.unwrap_or(BoundType::Variant),
                            nested_name,
                            fixed_string_len,
                        )
                    } else {
                        (raw, BoundType::Variant, None, None)
                    };
                // Strip array bounds from field name if present, e.g. "Items(10)" → "Items"
                let (field_name_clean, field_array_bounds) =
                    if let Some(paren_pos) = field_name_raw.find('(') {
                        let name_part = field_name_raw[..paren_pos].trim();
                        let bounds_part = field_name_raw[paren_pos..].trim();
                        let parsed = parse_udt_field_array_bounds(bounds_part);
                        (name_part, parsed)
                    } else {
                        (field_name_raw, None)
                    };
                if let Some(field_name) = normalize_ident(field_name_clean) {
                    let field_ty = if explicit_ty == BoundType::Variant && nested_udt_name.is_none()
                    {
                        default_type_for_name(&field_name, default_type_table)
                    } else {
                        explicit_ty
                    };
                    fields.push(UdtFieldDef {
                        name: field_name,
                        bound_type: field_ty,
                        nested_udt_name: nested_udt_name.clone(),
                        array_bounds: field_array_bounds,
                        fixed_string_len,
                    });
                }
            }
            index += 1;
        }
        defs.insert(type_name, fields);
        if index < lines.len() {
            index += 1;
        }
    }
    // Expand nested UDT fields: if a field's type matches another UDT name,
    // recursively flatten its fields as sub-fields.
    expand_nested_udt_fields(&mut defs);
    defs
}

/// Recursively expand nested UDT fields. If type `Rect` has field `TopLeft As Point` and
/// `Point` has fields `X, Y`, then `Rect` gets expanded to include `topleft_x`, `topleft_y`
/// alongside the original `topleft` field.
fn expand_nested_udt_fields(defs: &mut UdtDefMap) {
    // Snapshot UDT names for nested lookup.
    let udt_names: Vec<String> = defs.keys().cloned().collect();
    let snapshot: HashMap<String, Vec<UdtFieldDef>> = defs.clone();
    for udt_name in &udt_names {
        let mut expanded = Vec::new();
        let fields = match snapshot.get(udt_name) {
            Some(f) => f.clone(),
            None => continue,
        };
        for field in &fields {
            expanded.push(field.clone());
            // Use the explicit nested_udt_name (from "As SomeType") to look up nested fields,
            // rather than the field name itself, which was a bug.
            let lookup_key = match &field.nested_udt_name {
                Some(name) => name,
                None => continue,
            };
            if let Some(nested_fields) = snapshot.get(lookup_key) {
                // Skip self-referential types to avoid infinite recursion.
                if lookup_key == udt_name {
                    continue;
                }
                for sub_field in nested_fields {
                    expanded.push(UdtFieldDef {
                        name: format!("{}_{}", field.name, sub_field.name),
                        bound_type: sub_field.bound_type,
                        nested_udt_name: sub_field.nested_udt_name.clone(),
                        array_bounds: sub_field.array_bounds.clone(),
                        fixed_string_len: sub_field.fixed_string_len,
                    });
                }
            }
        }
        defs.insert(udt_name.clone(), expanded);
    }
}

fn parse_enum_block(lines: &[String], index: &mut usize, constants: &mut ModuleConstMap) {
    *index += 1;
    let mut next_value = 0i32;

    while *index < lines.len() {
        let line = lines[*index].as_str();
        if line.eq_ignore_ascii_case("end enum") {
            *index += 1;
            return;
        }
        if let Some((name, explicit)) = parse_enum_member(line) {
            let value = explicit.unwrap_or(next_value);
            constants.insert(name, BoundExpr::IntConst(value));
            next_value = value.saturating_add(1);
        }
        *index += 1;
    }
}

fn parse_const_declaration(line: &str) -> Option<(String, BoundExpr)> {
    let trimmed = line.trim();
    let rhs = strip_keyword_prefix_ci(trimmed, "public const")
        .or_else(|| strip_keyword_prefix_ci(trimmed, "private const"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "const"))?;
    let (lhs, rhs_value) = rhs.split_once('=')?;
    let name = lhs.split_whitespace().next().and_then(normalize_ident)?;
    let value = parse_expr(rhs_value.trim(), &HashMap::new())?;
    Some((name, value))
}

fn parse_enum_member(line: &str) -> Option<(String, Option<i32>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((lhs, rhs)) = trimmed.split_once('=') {
        let name = normalize_ident(lhs.trim())?;
        let value = rhs.trim().parse::<i32>().ok()?;
        return Some((name, Some(value)));
    }
    normalize_ident(trimmed).map(|name| (name, None))
}

#[allow(clippy::too_many_arguments)]
fn parse_block(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    terminators: &[&str],
) -> Vec<BoundStmt> {
    let mut out = Vec::new();

    while *index < lines.len() {
        let original_line = lines[*index].as_str();
        let mut line_owned = original_line.to_string();
        let mut lower = line_owned.to_ascii_lowercase();

        if let Some((label, rest)) = parse_line_number_statement(original_line) {
            out.push(BoundStmt::Label { name: label });
            if let Some(rest) = rest {
                line_owned = rest;
                lower = line_owned.to_ascii_lowercase();
            } else {
                *index += 1;
                continue;
            }
        }
        let line = line_owned.as_str();

        if matches_terminator(&lower, terminators) {
            break;
        }

        if lower == "option explicit" {
            *option_explicit = true;
            *index += 1;
            continue;
        }
        if parse_option_base_directive(line).is_some() {
            *index += 1;
            continue;
        }
        if parse_option_compare_directive(line).is_some() {
            *index += 1;
            continue;
        }

        if detect_proc_kind(&lower).is_some()
            || lower == "end sub"
            || lower == "end function"
            || lower == "end property"
        {
            *index += 1;
            continue;
        }

        if lower.starts_with("implements ")
            || lower.starts_with("event ")
            || lower.starts_with("public event ")
            || lower.starts_with("private event ")
        {
            *index += 1;
            continue;
        }

        if parse_variable_declaration_line(
            line,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_base,
            default_type_table,
            udt_defs,
        ) {
            *index += 1;
            continue;
        }

        if parse_def_type_directive(line).is_some() {
            *index += 1;
            continue;
        }

        if parse_const_declaration(line).is_some() {
            *index += 1;
            continue;
        }

        if starts_enum_block(&lower) {
            *index += 1;
            while *index < lines.len() && !lines[*index].eq_ignore_ascii_case("end enum") {
                *index += 1;
            }
            if *index < lines.len() {
                *index += 1;
            }
            continue;
        }

        if starts_type_block(&lower) {
            *index += 1;
            while *index < lines.len() && !lines[*index].eq_ignore_ascii_case("end type") {
                *index += 1;
            }
            if *index < lines.len() {
                *index += 1;
            }
            continue;
        }

        if lower.starts_with("if ") {
            if lower.ends_with(" then") {
                out.push(parse_if_stmt(
                    lines,
                    index,
                    declarations,
                    declaration_types,
                    duplicate_declarations,
                    array_bounds,
                    option_explicit,
                    option_base,
                    default_type_table,
                    udt_defs,
                    module_constants,
                    property_write_routes,
                    property_read_routes,
                    line,
                ));
            } else {
                out.push(parse_single_line_if_stmt(
                    line,
                    declarations,
                    declaration_types,
                    array_bounds,
                    property_write_routes,
                    property_read_routes,
                    udt_defs,
                ));
                *index += 1;
            }
            continue;
        }

        if lower.starts_with("for each ") {
            out.push(parse_for_each_stmt(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                line,
            ));
            continue;
        }

        if lower.starts_with("for ") {
            out.push(parse_for_stmt(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                line,
            ));
            continue;
        }

        if lower.starts_with("while ") {
            out.push(parse_while_wend_stmt(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                line,
            ));
            continue;
        }

        if lower.starts_with("redim ") {
            if let Some(stmt) = parse_redim_stmt(
                line,
                declarations,
                declaration_types,
                array_bounds,
                option_base,
            ) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("erase ") {
            let raw = line[6..].trim();
            if let Some(name) = normalize_ident(raw) {
                out.push(BoundStmt::Erase { name });
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower == "randomize" || lower.starts_with("randomize ") {
            let args = if lower == "randomize" {
                Vec::new()
            } else {
                let raw = line[10..].trim();
                if let Some(expr) = parse_expr(raw, array_bounds) {
                    vec![BoundCallArg {
                        name: None,
                        expr,
                        force_byval: true,
                    }]
                } else {
                    Vec::new()
                }
            };
            out.push(BoundStmt::Call {
                name: "randomize".to_string(),
                args,
                syntax: BoundCallSyntax::StatementNoCall,
            });
            *index += 1;
            continue;
        }

        if lower.starts_with("do while ") || lower.starts_with("do until ") || lower == "do" {
            out.push(parse_do_stmt(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                line,
            ));
            continue;
        }

        if lower == "exit do" {
            out.push(BoundStmt::ExitDo);
            *index += 1;
            continue;
        }

        if lower == "exit for" {
            out.push(BoundStmt::ExitFor);
            *index += 1;
            continue;
        }

        if lower == "exit sub" || lower == "exit function" || lower == "exit property" {
            out.push(BoundStmt::ExitProcedure);
            *index += 1;
            continue;
        }

        if lower == "on error resume next" {
            out.push(BoundStmt::OnErrorResumeNext);
            *index += 1;
            continue;
        }

        if lower == "on error goto 0" {
            out.push(BoundStmt::OnErrorGoto0);
            *index += 1;
            continue;
        }

        if lower.starts_with("on error goto ") {
            let raw = line[14..].trim();
            if let Some(label) = parse_jump_target_label(raw) {
                out.push(BoundStmt::OnErrorGotoLabel { label });
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower == "resume next" {
            out.push(BoundStmt::ResumeNext);
            *index += 1;
            continue;
        }

        if lower == "resume" {
            out.push(BoundStmt::Resume);
            *index += 1;
            continue;
        }

        if lower.starts_with("resume ") {
            if let Some(label) = parse_jump_target_label(line[7..].trim()) {
                out.push(BoundStmt::ResumeLabel { label });
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("raiseevent ") {
            if let Some((name, args)) = parse_raiseevent_invocation(line, array_bounds) {
                out.push(BoundStmt::RaiseEvent { name, args });
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("error ")
            && let Ok(code) = line[6..].trim().parse::<i32>()
        {
            out.push(BoundStmt::RaiseError(code));
            *index += 1;
            continue;
        }

        if lower.starts_with("err.raise ")
            && let Ok(code) = line[10..].trim().parse::<i32>()
        {
            out.push(BoundStmt::RaiseError(code));
            *index += 1;
            continue;
        }

        if lower == "err.clear" {
            out.push(BoundStmt::ErrClear);
            *index += 1;
            continue;
        }

        if let Some(name) = parse_label_declaration(line) {
            out.push(BoundStmt::Label { name });
            *index += 1;
            continue;
        }

        if lower.starts_with("goto ") {
            let raw = line[5..].trim();
            if let Some(label) = parse_jump_target_label(raw) {
                out.push(BoundStmt::GoTo { label });
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("gosub ") {
            if let Some(label) = parse_jump_target_label(line[6..].trim()) {
                out.push(BoundStmt::GoSub { label });
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower == "return" {
            out.push(BoundStmt::Return);
            *index += 1;
            continue;
        }

        if lower.starts_with("select case ") {
            out.push(parse_select_case_stmt(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                line,
            ));
            continue;
        }

        // VBA file I/O statements
        if lower.starts_with("open ") {
            if let Some(stmt) = parse_file_open_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower == "close" {
            out.push(BoundStmt::FileClose { file_number: None });
            *index += 1;
            continue;
        }

        if lower.starts_with("close ") {
            if let Some(stmt) = parse_file_close_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("kill ") {
            if let Some(stmt) = parse_file_kill_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("print #") {
            if let Some(stmt) = parse_file_print_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower == "print" || lower.starts_with("print ") {
            if let Some(stmt) = parse_console_print_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("write #") {
            if let Some(stmt) = parse_file_write_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("line input #") {
            if let Some(stmt) = parse_file_line_input_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("line input ") {
            if let Some(stmt) = parse_console_line_input_stmt(line) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("input #") {
            if let Some(stmt) = parse_file_input_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("input ") {
            if let Some(stmt) = parse_console_input_stmt(line) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower == "beep" {
            out.push(BoundStmt::Beep);
            *index += 1;
            continue;
        }

        if lower == "debug.print" || lower.starts_with("debug.print ") {
            if let Some(stmt) = parse_debug_print_stmt(line, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        out.push(parse_assign_or_unsupported(
            line,
            declarations,
            declaration_types,
            array_bounds,
            property_write_routes,
            property_read_routes,
            udt_defs,
        ));
        *index += 1;
    }

    out
}

#[allow(clippy::too_many_arguments)]
fn parse_if_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let condition = line[2..line.len() - 4].trim();
    let Some(cond) = parse_condition(condition, array_bounds) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let then_body = parse_block(
        lines,
        index,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
        &["elseif", "else", "end if"],
    );
    let Some(else_body) = parse_if_tail(
        lines,
        index,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
    ) else {
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    BoundStmt::IfCond {
        cond,
        then_body,
        else_body,
    }
}

fn parse_single_line_if_stmt(
    line: &str,
    declarations: &[String],
    declaration_types: &HashMap<String, BoundType>,
    array_bounds: &ArrayBoundsMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    udt_defs: &UdtDefMap,
) -> BoundStmt {
    let Some((condition, tail)) = split_ci(&line[2..], " then ") else {
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };
    let Some(cond) = parse_condition(condition, array_bounds) else {
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    let (then_tail, else_tail) = if let Some((then_tail, else_tail)) = split_ci(tail, " else ") {
        (then_tail, Some(else_tail))
    } else {
        (tail, None)
    };

    let then_stmt = parse_inline_stmt_or_unsupported(
        then_tail,
        declarations,
        declaration_types,
        array_bounds,
        property_write_routes,
        property_read_routes,
        udt_defs,
    );
    if matches!(then_stmt, BoundStmt::Unsupported { .. }) {
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    }

    let else_body = if let Some(else_tail) = else_tail {
        let else_stmt = parse_inline_stmt_or_unsupported(
            else_tail,
            declarations,
            declaration_types,
            array_bounds,
            property_write_routes,
            property_read_routes,
            udt_defs,
        );
        if matches!(else_stmt, BoundStmt::Unsupported { .. }) {
            return BoundStmt::Unsupported {
                line: line.to_string(),
            };
        }
        vec![else_stmt]
    } else {
        Vec::new()
    };

    BoundStmt::IfCond {
        cond,
        then_body: vec![then_stmt],
        else_body,
    }
}

fn parse_inline_stmt_or_unsupported(
    line: &str,
    declarations: &[String],
    declaration_types: &HashMap<String, BoundType>,
    array_bounds: &ArrayBoundsMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    udt_defs: &UdtDefMap,
) -> BoundStmt {
    let lower = line.trim().to_ascii_lowercase();
    if lower.starts_with("error ")
        && let Ok(code) = line.trim()[6..].trim().parse::<i32>()
    {
        return BoundStmt::RaiseError(code);
    }
    if lower.starts_with("err.raise ")
        && let Ok(code) = line.trim()[10..].trim().parse::<i32>()
    {
        return BoundStmt::RaiseError(code);
    }
    if lower == "err.clear" {
        return BoundStmt::ErrClear;
    }
    if lower == "exit do" {
        return BoundStmt::ExitDo;
    }
    if lower == "exit for" {
        return BoundStmt::ExitFor;
    }
    if lower == "exit sub" || lower == "exit function" || lower == "exit property" {
        return BoundStmt::ExitProcedure;
    }
    parse_assign_or_unsupported(
        line.trim(),
        declarations,
        declaration_types,
        array_bounds,
        property_write_routes,
        property_read_routes,
        udt_defs,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_for_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let Some((var, start, end, step)) = parse_for_header(line, array_bounds) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let body = parse_block(
        lines,
        index,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
        &["next"],
    );

    if *index < lines.len() {
        let lower = lines[*index].to_ascii_lowercase();
        if lower == "next" || lower.starts_with("next ") {
            *index += 1;
            return BoundStmt::ForRange {
                var,
                start,
                end,
                step,
                body,
            };
        }
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_for_each_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let Some(ForEachHeader {
        var,
        items,
        iterable,
    }) = parse_for_each_header(line, array_bounds)
    else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let body = parse_block(
        lines,
        index,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
        &["next"],
    );

    if *index < lines.len() {
        let lower = lines[*index].to_ascii_lowercase();
        if lower == "next" || lower.starts_with("next ") {
            *index += 1;
            return BoundStmt::ForEach {
                var,
                items,
                iterable,
                body,
            };
        }
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_while_wend_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let condition = line[6..].trim();
    let Some(cond) = parse_condition(condition, array_bounds) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let body = parse_block(
        lines,
        index,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_explicit,
        option_base,
        default_type_table,
        udt_defs,
        module_constants,
        property_write_routes,
        property_read_routes,
        &["wend"],
    );
    if *index < lines.len() && lines[*index].eq_ignore_ascii_case("wend") {
        *index += 1;
        return BoundStmt::DoWhile {
            cond,
            body,
            post_check: false,
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_assign_or_unsupported(
    line: &str,
    declarations: &[String],
    declaration_types: &HashMap<String, BoundType>,
    array_bounds: &ArrayBoundsMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    udt_defs: &UdtDefMap,
) -> BoundStmt {
    let lowered = line.trim_start().to_ascii_lowercase();
    let (assignment_intent, assignment_line) = if lowered.starts_with("set ") {
        (AssignmentIntent::Set, line.trim_start()[4..].trim_start())
    } else if lowered.starts_with("let ") {
        (AssignmentIntent::Let, line.trim_start()[4..].trim_start())
    } else {
        (AssignmentIntent::Implicit, line)
    };

    if let Some(stmt) = parse_mid_assign_stmt(assignment_line, array_bounds) {
        return stmt;
    }

    if let Some((lhs_raw, rhs_raw)) = split_assignment_once_top_level(assignment_line)
        && let Some((name, indices)) = parse_runtime_array_index_target(lhs_raw, array_bounds)
        && let Some(expr) = parse_expr(rhs_raw, array_bounds)
    {
        return BoundStmt::AssignRuntimeArrayElement {
            name,
            indices,
            expr,
            intent: assignment_intent,
        };
    }

    if let Some((lhs_raw, rhs_raw)) = split_assignment_once_top_level(assignment_line)
        && let Some(target) = parse_reference_name(lhs_raw, array_bounds)
    {
        let runtime_array_read = parse_runtime_array_index_expr(rhs_raw.trim(), array_bounds);
        if parse_reference_name(rhs_raw.trim(), array_bounds).is_none()
            && runtime_array_read.is_none()
            && let Some((call_name, args)) =
                parse_dispatch_invoke_call_invocation(rhs_raw.trim(), array_bounds)
        {
            return BoundStmt::AssignFromCall {
                target,
                name: call_name,
                args,
                intent: assignment_intent,
                syntax: BoundCallSyntax::ExpressionCall,
            };
        }
        if parse_reference_name(rhs_raw.trim(), array_bounds).is_none()
            && runtime_array_read.is_none()
            && let Some((call_name, args)) = parse_call_invocation(rhs_raw.trim(), array_bounds)
            && !is_intrinsic_call_name(&call_name)
        {
            let name = property_read_routes
                .get(&call_name)
                .cloned()
                .unwrap_or(call_name);
            return BoundStmt::AssignFromCall {
                target,
                name,
                args,
                intent: assignment_intent,
                syntax: BoundCallSyntax::ExpressionCall,
            };
        }

        if let Some(base_name) = normalize_ident(rhs_raw.trim())
            && let Some(route_proc) = property_read_routes.get(&base_name)
            && !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&base_name))
        {
            return BoundStmt::AssignFromCall {
                target,
                name: route_proc.clone(),
                args: Vec::new(),
                intent: assignment_intent,
                syntax: BoundCallSyntax::ExpressionCall,
            };
        }

        if let Some(expr) = runtime_array_read.or_else(|| parse_expr(rhs_raw, array_bounds)) {
            if let BoundExpr::Var(source) = &expr {
                match resolve_udt_assignment(
                    &target,
                    source,
                    declarations,
                    declaration_types,
                    udt_defs,
                ) {
                    UdtAssignmentResolution::Copy(fields) => {
                        return BoundStmt::UdtAssign {
                            target,
                            source: source.clone(),
                            fields,
                        };
                    }
                    UdtAssignmentResolution::CrossType {
                        target_type,
                        source_type,
                    } => {
                        return BoundStmt::Unsupported {
                            line: format!(
                                "cross-type UDT assignment from {source_type} to {target_type}"
                            ),
                        };
                    }
                    UdtAssignmentResolution::NoMatch => {}
                }
            }
            if let Some(route_proc) = property_write_routes.get(&target)
                && !declarations
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&target))
            {
                return BoundStmt::Call {
                    name: route_proc.clone(),
                    args: vec![BoundCallArg {
                        name: None,
                        expr,
                        // Property Let/Set value parameter must not write back
                        // through ByRef — the assigned value is always by-value.
                        force_byval: true,
                    }],
                    syntax: BoundCallSyntax::SyntheticPropertyAssignment,
                };
            }
            return BoundStmt::Assign {
                target,
                expr,
                intent: assignment_intent,
            };
        }
    }

    let has_call_keyword = assignment_line.to_ascii_lowercase().starts_with("call ");
    let call_token = if has_call_keyword {
        assignment_line[5..].trim()
    } else {
        assignment_line.trim()
    };
    if let Some((name, args)) = parse_dispatch_invoke_call_invocation(call_token, array_bounds) {
        return BoundStmt::Call {
            name,
            args,
            syntax: call_syntax_from_keyword(has_call_keyword),
        };
    }
    if let Some((name, mut args)) = parse_call_invocation(call_token, array_bounds) {
        // VBA rule: at statement level (no Call keyword), parentheses around a
        // single argument force ByVal evaluation.  E.g. `AddOne (x)` passes x
        // ByVal, while `Call AddOne(x)` passes x ByRef.
        if !has_call_keyword && args.len() == 1 {
            args[0].force_byval = true;
        }
        return BoundStmt::Call {
            name,
            args,
            syntax: call_syntax_from_keyword(has_call_keyword),
        };
    }
    if !has_call_keyword
        && let Some((name, args)) = parse_statement_call_invocation(call_token, array_bounds)
    {
        return BoundStmt::Call {
            name,
            args,
            syntax: BoundCallSyntax::StatementNoCall,
        };
    }
    if let Some(name) = normalize_ident(call_token).or_else(|| parse_member_reference(call_token)) {
        return BoundStmt::Call {
            name,
            args: Vec::new(),
            syntax: call_syntax_from_keyword(has_call_keyword),
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn call_syntax_from_keyword(has_call_keyword: bool) -> BoundCallSyntax {
    if has_call_keyword {
        BoundCallSyntax::StatementCallKeyword
    } else {
        BoundCallSyntax::StatementNoCall
    }
}

fn split_assignment_once_top_level(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut idx = 0usize;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch == '"' {
            in_string = !in_string;
            idx += 1;
            continue;
        }
        if in_string {
            idx += 1;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '=' if depth == 0 => return Some((&line[..idx], &line[idx + 1..])),
            _ => {}
        }
        idx += 1;
    }
    None
}

enum UdtAssignmentResolution {
    Copy(Vec<String>),
    CrossType {
        target_type: String,
        source_type: String,
    },
    NoMatch,
}

fn resolve_udt_assignment(
    target: &str,
    source: &str,
    declarations: &[String],
    declaration_types: &HashMap<String, BoundType>,
    udt_defs: &UdtDefMap,
) -> UdtAssignmentResolution {
    let target_fields = collect_udt_field_suffixes(target, declarations);
    if target_fields.is_empty() {
        return UdtAssignmentResolution::NoMatch;
    }
    let source_fields = collect_udt_field_suffixes(source, declarations);
    if source_fields.is_empty() || source_fields != target_fields {
        return UdtAssignmentResolution::NoMatch;
    }

    let target_type = declared_udt_type_for_variable(target, declaration_types)
        .or_else(|| infer_udt_type_from_fields(&target_fields, udt_defs));
    let source_type = declared_udt_type_for_variable(source, declaration_types)
        .or_else(|| infer_udt_type_from_fields(&source_fields, udt_defs));
    if let (Some(tt), Some(st)) = (&target_type, &source_type)
        && !tt.eq_ignore_ascii_case(st)
    {
        return UdtAssignmentResolution::CrossType {
            target_type: tt.clone(),
            source_type: st.clone(),
        };
    }
    UdtAssignmentResolution::Copy(target_fields)
}

fn udt_type_marker_key(variable_name: &str, udt_name: &str) -> String {
    format!(
        "{UDT_TYPE_MARKER_PREFIX}{}::{}",
        variable_name.to_ascii_lowercase(),
        udt_name.to_ascii_lowercase()
    )
}

fn insert_udt_type_marker(
    declaration_types: &mut HashMap<String, BoundType>,
    variable_name: &str,
    udt_name: &str,
) {
    declaration_types.insert(
        udt_type_marker_key(variable_name, udt_name),
        BoundType::Variant,
    );
}

fn declared_udt_type_for_variable(
    variable_name: &str,
    declaration_types: &HashMap<String, BoundType>,
) -> Option<String> {
    let prefix = format!(
        "{UDT_TYPE_MARKER_PREFIX}{}::",
        variable_name.to_ascii_lowercase()
    );
    declaration_types
        .keys()
        .find_map(|key| key.strip_prefix(&prefix).map(ToString::to_string))
}

fn remove_udt_type_markers(declaration_types: &mut HashMap<String, BoundType>) {
    declaration_types.retain(|name, _| !name.starts_with(UDT_TYPE_MARKER_PREFIX));
}

/// Infer which UDT type a variable belongs to by matching its field suffixes
/// against known UDT definitions.
fn infer_udt_type_from_fields(fields: &[String], udt_defs: &UdtDefMap) -> Option<String> {
    for (type_name, type_fields) in udt_defs {
        let mut def_field_names: Vec<String> = type_fields
            .iter()
            .map(|f| f.name.to_ascii_lowercase())
            .collect();
        def_field_names.sort();
        def_field_names.dedup();
        if def_field_names == *fields {
            return Some(type_name.clone());
        }
    }
    None
}

fn collect_udt_field_suffixes(base: &str, declarations: &[String]) -> Vec<String> {
    let prefix = format!("{}_", base.to_ascii_lowercase());
    let mut fields = Vec::new();
    for declaration in declarations {
        let lower = declaration.to_ascii_lowercase();
        if let Some(suffix) = lower.strip_prefix(&prefix) {
            if suffix.is_empty() || suffix.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            fields.push(suffix.to_string());
        }
    }
    fields.sort();
    fields.dedup();
    fields
}

fn parse_mid_assign_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let trimmed = line.trim();
    let (lhs, rhs) = trimmed.split_once('=')?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    if rhs.is_empty() {
        return None;
    }

    let open = lhs.find('(')?;
    let close = lhs.rfind(')')?;
    if close <= open || close != lhs.len() - 1 {
        return None;
    }
    let name = normalize_ident(lhs[..open].trim())?;
    if name != "mid" {
        return None;
    }

    let args = split_call_args(lhs[open + 1..close].trim())?;
    if !(args.len() == 2 || args.len() == 3) {
        return None;
    }
    let target = parse_reference_name(args[0], array_bounds)?;
    let start = parse_expr(args[1], array_bounds)?;
    let count = if args.len() == 3 {
        Some(parse_expr(args[2], array_bounds)?)
    } else {
        None
    };
    let value = parse_expr(rhs, array_bounds)?;

    Some(BoundStmt::MidAssign {
        target,
        start,
        count,
        value,
    })
}

fn parse_call_invocation(
    text: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, Vec<BoundCallArg>)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    if !text[close + 1..].trim().is_empty() {
        return None;
    }

    let name = normalize_ident(text[..open].trim())
        .or_else(|| parse_member_reference(text[..open].trim()))?;
    let args_raw = text[open + 1..close].trim();
    if args_raw.is_empty() {
        return Some((name, Vec::new()));
    }

    let mut args = Vec::new();
    for token in split_call_args_allowing_omitted(args_raw)? {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            // Omitted positional argument via bare commas, e.g. `Foo(1, , 5)`.
            // Bind a sentinel; call lowering resolves it to the parameter's
            // Optional default (or Missing for an Optional Variant).
            args.push(BoundCallArg {
                name: None,
                expr: omitted_argument_sentinel(),
                force_byval: false,
            });
        } else if let Some((lhs, rhs)) = trimmed.split_once(":=") {
            args.push(BoundCallArg {
                name: Some(normalize_ident(lhs)?),
                expr: parse_expr(rhs.trim(), array_bounds)?,
                force_byval: false,
            });
        } else {
            args.push(BoundCallArg {
                name: None,
                expr: parse_expr(trimmed, array_bounds)?,
                force_byval: false,
            });
        }
    }
    Some((name, args))
}

/// Sentinel expression for an argument omitted via bare commas (`Foo(1, , 5)`).
/// Resolved during call lowering to the target parameter's Optional default.
pub(crate) fn omitted_argument_sentinel() -> BoundExpr {
    BoundExpr::IntrinsicCall {
        name: "__omitted".to_string(),
        args: Vec::new(),
    }
}

pub(crate) fn is_omitted_argument_expr(expr: &BoundExpr) -> bool {
    matches!(expr, BoundExpr::IntrinsicCall { name, args } if name == "__omitted" && args.is_empty())
}

fn parse_dispatch_invoke_call_invocation(
    text: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, Vec<BoundCallArg>)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open || !text[close + 1..].trim().is_empty() {
        return None;
    }
    let name = normalize_ident(text[..open].trim())?;
    if name != "dispatchinvoke" && name != "__oxvbaearlyinvoke" {
        return None;
    }
    let args_raw = text[open + 1..close].trim();
    let args_text = split_call_args(args_raw)?;
    if args_text.len() < 2 {
        return None;
    }
    let mut args = Vec::with_capacity(args_text.len());
    args.push(BoundCallArg {
        name: None,
        expr: parse_expr(args_text[0], array_bounds)?,
        force_byval: false,
    });
    args.push(BoundCallArg {
        name: None,
        expr: parse_dispatch_member_arg(args_text[0], args_text[1], array_bounds)?,
        force_byval: false,
    });
    for token in &args_text[2..] {
        let trimmed = token.trim();
        if let Some((lhs, rhs)) = trimmed.split_once(":=") {
            args.push(BoundCallArg {
                name: Some(normalize_ident(lhs)?),
                expr: parse_expr(rhs.trim(), array_bounds)?,
                force_byval: false,
            });
        } else {
            args.push(BoundCallArg {
                name: None,
                expr: parse_expr(trimmed, array_bounds)?,
                force_byval: false,
            });
        }
    }
    Some((name, args))
}

fn parse_statement_call_invocation(
    text: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, Vec<BoundCallArg>)> {
    let trimmed = text.trim();
    let split_at = trimmed.find(char::is_whitespace)?;
    let name = normalize_ident(trimmed[..split_at].trim())
        .or_else(|| parse_member_reference(trimmed[..split_at].trim()))?;
    let args_raw = trimmed[split_at..].trim();
    if args_raw.is_empty() {
        return Some((name, Vec::new()));
    }

    let mut args = Vec::new();
    for token in split_call_args_allowing_omitted(args_raw)? {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            args.push(BoundCallArg {
                name: None,
                expr: omitted_argument_sentinel(),
                force_byval: false,
            });
        } else if let Some((lhs, rhs)) = trimmed.split_once(":=") {
            args.push(BoundCallArg {
                name: Some(normalize_ident(lhs)?),
                expr: parse_expr(rhs.trim(), array_bounds)?,
                force_byval: false,
            });
        } else {
            args.push(BoundCallArg {
                name: None,
                expr: parse_expr(trimmed, array_bounds)?,
                force_byval: false,
            });
        }
    }
    Some((name, args))
}

fn parse_raiseevent_invocation(
    line: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, Vec<BoundCallArg>)> {
    let payload = line.trim()[10..].trim();
    if payload.is_empty() {
        return None;
    }
    if let Some(open) = payload.find('(') {
        let close = payload.rfind(')')?;
        if close <= open || !payload[close + 1..].trim().is_empty() {
            return None;
        }
        let name = normalize_ident(payload[..open].trim())?;
        let args_raw = payload[open + 1..close].trim();
        if args_raw.is_empty() {
            return Some((name, Vec::new()));
        }
        let mut args = Vec::new();
        for token in split_call_args(args_raw)? {
            let trimmed = token.trim();
            if let Some((lhs, rhs)) = trimmed.split_once(":=") {
                args.push(BoundCallArg {
                    name: Some(normalize_ident(lhs)?),
                    expr: parse_expr(rhs.trim(), array_bounds)?,
                    force_byval: false,
                });
            } else {
                args.push(BoundCallArg {
                    name: None,
                    expr: parse_expr(trimmed, array_bounds)?,
                    force_byval: false,
                });
            }
        }
        return Some((name, args));
    }
    Some((normalize_ident(payload)?, Vec::new()))
}

fn is_intrinsic_call_name(name: &str) -> bool {
    if intrinsic_spec(name).is_some() {
        return true;
    }
    matches!(
        name,
        "cint"
            | "clng"
            | "cdbl"
            | "cstr"
            | "cbool"
            | "cdate"
            | "csng"
            | "cbyte"
            | "ccur"
            | "cdec"
            | "val"
            | "str"
            | "cverr"
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_do_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("do while ") || lower.starts_with("do until ") {
        let is_until = lower.starts_with("do until ");
        let condition = if is_until {
            line[9..].trim()
        } else {
            line[8..].trim()
        };
        let Some(cond) = parse_condition(condition, array_bounds) else {
            *index += 1;
            return BoundStmt::Unsupported {
                line: line.to_string(),
            };
        };
        let cond = if is_until {
            BoundCond::Not(Box::new(cond))
        } else {
            cond
        };

        *index += 1;
        let body = parse_block(
            lines,
            index,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_explicit,
            option_base,
            default_type_table,
            udt_defs,
            module_constants,
            property_write_routes,
            property_read_routes,
            &["loop"],
        );
        if *index < lines.len() {
            let loop_line = lines[*index].to_ascii_lowercase();
            if loop_line == "loop" {
                *index += 1;
                return BoundStmt::DoWhile {
                    cond,
                    body,
                    post_check: false,
                };
            }
        }

        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    }

    if lower == "do" {
        *index += 1;
        let body = parse_block(
            lines,
            index,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_explicit,
            option_base,
            default_type_table,
            udt_defs,
            module_constants,
            property_write_routes,
            property_read_routes,
            &["loop"],
        );
        if *index < lines.len() {
            let loop_line = lines[*index].as_str();
            let loop_lower = loop_line.to_ascii_lowercase();
            if loop_lower.starts_with("loop while ") || loop_lower.starts_with("loop until ") {
                let is_until = loop_lower.starts_with("loop until ");
                let condition = loop_line[11..].trim();
                if let Some(cond) = parse_condition(condition, array_bounds) {
                    let cond = if is_until {
                        BoundCond::Not(Box::new(cond))
                    } else {
                        cond
                    };
                    *index += 1;
                    return BoundStmt::DoWhile {
                        cond,
                        body,
                        post_check: true,
                    };
                }
            }
        }

        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_for_header(
    line: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, BoundExpr, BoundExpr, BoundExpr)> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("for ") {
        return None;
    }

    let without_for = line[4..].trim();
    let (lhs_raw, range_raw) = without_for.split_once('=')?;
    let var = normalize_ident(lhs_raw)?;
    let (start_raw, to_tail_raw) = split_ci(range_raw, " to ")?;
    let (end_raw, step) = if let Some((end_raw, step_raw)) = split_keyword_ci(to_tail_raw, "step") {
        (end_raw, parse_expr(step_raw, array_bounds)?)
    } else {
        (to_tail_raw, BoundExpr::IntConst(1))
    };
    let start = parse_expr(start_raw, array_bounds)?;
    let end = parse_expr(end_raw, array_bounds)?;
    Some((var, start, end, step))
}

struct ForEachHeader {
    var: String,
    items: Vec<BoundExpr>,
    iterable: Option<BoundExpr>,
}

fn parse_for_each_header(line: &str, array_bounds: &ArrayBoundsMap) -> Option<ForEachHeader> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("for each ") {
        return None;
    }

    let payload = line[9..].trim();
    let (var_raw, iterable_raw) = split_keyword_ci(payload, "in")?;
    let var = normalize_ident(var_raw)?;
    let iterable = iterable_raw.trim();

    if let Some(base) = normalize_ident(iterable)
        && let Some(bounds) = array_bounds.get(&base)
    {
        let element_count = array_element_count(bounds)?;
        let mut items = Vec::with_capacity(element_count);
        for idx in 0..element_count {
            items.push(BoundExpr::Var(format!("{base}_{idx}")));
        }
        return Some(ForEachHeader {
            var,
            items,
            iterable: None,
        });
    }

    Some(ForEachHeader {
        var,
        items: Vec::new(),
        iterable: Some(parse_expr(iterable, array_bounds)?),
    })
}

/// Parse `Open path For mode As [#]filenum`
fn parse_file_open_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let lower = line.to_ascii_lowercase();
    // Find " for " separator
    let for_pos = lower.find(" for ")?;
    let path_raw = line[5..for_pos].trim();
    let after_for = &line[for_pos + 5..];
    let after_for_lower = after_for.to_ascii_lowercase();
    // Find " as " separator
    let as_pos = after_for_lower.find(" as ")?;
    let mode_raw = after_for[..as_pos].trim();
    let filenum_raw = after_for[as_pos + 4..].trim();

    let path = parse_expr(path_raw, array_bounds)?;

    let mode = match mode_raw.to_ascii_lowercase().as_str() {
        "input" => 0,
        "output" => 1,
        "append" => 2,
        "binary" => 3,
        "random" => 4,
        _ => return None,
    };

    let filenum_clean = filenum_raw.strip_prefix('#').unwrap_or(filenum_raw).trim();
    let file_number = parse_expr(filenum_clean, array_bounds)?;

    Some(BoundStmt::FileOpen {
        path,
        mode,
        file_number,
    })
}

/// Parse `Close [#filenum]`
fn parse_file_close_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let rest = line[6..].trim();
    let clean = rest.strip_prefix('#').unwrap_or(rest).trim();
    let file_number = parse_expr(clean, array_bounds)?;
    Some(BoundStmt::FileClose {
        file_number: Some(file_number),
    })
}

fn parse_file_kill_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let path = parse_expr(line[5..].trim(), array_bounds)?;
    Some(BoundStmt::FileKill { path })
}

/// Parse `Print #filenum, data`
fn parse_file_print_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let after_print = line[7..].trim(); // skip "Print #"
    let comma_pos = after_print.find(',')?;
    let filenum_raw = after_print[..comma_pos].trim();
    let data_raw = after_print[comma_pos + 1..].trim();

    let file_number = parse_expr(filenum_raw, array_bounds)?;
    let data = if data_raw.is_empty() {
        BoundExpr::StringConst(String::new())
    } else {
        parse_expr(data_raw, array_bounds)?
    };

    Some(BoundStmt::FilePrint { file_number, data })
}

/// Parse `Print [expr]`
fn parse_console_print_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let payload = line[5..].trim();
    let data = if payload.is_empty() {
        BoundExpr::StringConst(String::new())
    } else {
        parse_expr(payload, array_bounds)?
    };
    Some(BoundStmt::ConsolePrint { data })
}

/// Parse `Write #filenum, data`
fn parse_file_write_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let after_write = line[7..].trim(); // skip "Write #"
    let comma_pos = after_write.find(',')?;
    let filenum_raw = after_write[..comma_pos].trim();
    let data_raw = after_write[comma_pos + 1..].trim();

    let file_number = parse_expr(filenum_raw, array_bounds)?;
    let data = if data_raw.is_empty() {
        vec![BoundExpr::StringConst(String::new())]
    } else {
        split_top_level_stmt_args(data_raw)?
            .into_iter()
            .map(|part| parse_expr(part.as_str(), array_bounds))
            .collect::<Option<Vec<_>>>()?
    };

    Some(BoundStmt::FileWrite { file_number, data })
}

/// Parse `Input #filenum, var1[, var2, ...]`
fn parse_file_input_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let after_input = line[7..].trim(); // skip "Input #"
    let comma_pos = after_input.find(',')?;
    let filenum_raw = after_input[..comma_pos].trim();
    let targets_raw = after_input[comma_pos + 1..].trim();

    let file_number = parse_expr(filenum_raw, array_bounds)?;
    let targets: Vec<String> = targets_raw
        .split(',')
        .filter_map(|t| normalize_ident(t.trim()))
        .collect();
    if targets.is_empty() {
        return None;
    }

    Some(BoundStmt::FileInput {
        file_number,
        targets,
    })
}

/// Parse `Input var1[, var2, ...]`
fn parse_console_input_stmt(line: &str) -> Option<BoundStmt> {
    let targets_raw = line[5..].trim();
    let targets: Vec<String> = targets_raw
        .split(',')
        .filter_map(|t| normalize_ident(t.trim()))
        .collect();
    if targets.is_empty() {
        return None;
    }
    Some(BoundStmt::ConsoleInput { targets })
}

/// Parse `Line Input #filenum, var`
fn parse_file_line_input_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let after_li = line[12..].trim(); // skip "Line Input #"
    let comma_pos = after_li.find(',')?;
    let filenum_raw = after_li[..comma_pos].trim();
    let target_raw = after_li[comma_pos + 1..].trim();

    let file_number = parse_expr(filenum_raw, array_bounds)?;
    let target = normalize_ident(target_raw)?;

    Some(BoundStmt::FileLineInput {
        file_number,
        target,
    })
}

/// Parse `Line Input var`
fn parse_console_line_input_stmt(line: &str) -> Option<BoundStmt> {
    let target = normalize_ident(line[10..].trim())?;
    Some(BoundStmt::ConsoleLineInput { target })
}

/// Parse `Debug.Print [expr]`
fn parse_debug_print_stmt(line: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundStmt> {
    let payload = line[11..].trim();
    let data = if payload.is_empty() {
        BoundExpr::StringConst(String::new())
    } else {
        let parts = split_top_level_stmt_args(payload)?;
        let exprs = parts
            .into_iter()
            .map(|part| parse_expr(part.as_str(), array_bounds))
            .collect::<Option<Vec<_>>>()?;
        concat_exprs_with_delimiter(exprs, "\t")
    };
    Some(BoundStmt::DebugPrint { data })
}

fn concat_exprs_with_delimiter(mut exprs: Vec<BoundExpr>, delimiter: &str) -> BoundExpr {
    let mut acc = if exprs.is_empty() {
        BoundExpr::StringConst(String::new())
    } else {
        exprs.remove(0)
    };
    for expr in exprs {
        acc = BoundExpr::BinaryOp {
            op: ArithOp::Concat,
            lhs: Box::new(BoundExpr::BinaryOp {
                op: ArithOp::Concat,
                lhs: Box::new(acc),
                rhs: Box::new(BoundExpr::StringConst(delimiter.to_string())),
            }),
            rhs: Box::new(expr),
        };
    }
    acc
}

fn split_top_level_stmt_args(args: &str) -> Option<Vec<String>> {
    if args.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let chars = args.as_bytes();
    let mut idx = 0usize;
    while idx < chars.len() {
        let ch = chars[idx] as char;
        if ch == '"' {
            in_string = !in_string;
            idx += 1;
            continue;
        }
        if in_string {
            idx += 1;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(args[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
        idx += 1;
    }
    if depth != 0 || in_string {
        return None;
    }
    out.push(args[start..].trim().to_string());
    Some(out)
}

fn parse_redim_stmt(
    line: &str,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    array_bounds: &mut ArrayBoundsMap,
    option_base: i32,
) -> Option<BoundStmt> {
    let mut payload = line[6..].trim();
    let mut preserve = false;
    if payload.to_ascii_lowercase().starts_with("preserve ") {
        preserve = true;
        payload = payload[9..].trim();
    }
    let Some((name, _, bounds)) = parse_array_declaration(payload, option_base) else {
        if let Some(runtime_stmt) =
            parse_runtime_redim_stmt(payload, preserve, declarations, array_bounds, option_base)
        {
            return Some(runtime_stmt);
        }
        if let Some(detail) = describe_unsupported_redim_expression_bounds(payload) {
            return Some(BoundStmt::Unsupported { line: detail });
        }
        return None;
    };
    if array_bounds
        .get(&name)
        .is_some_and(|existing_bounds| existing_bounds.is_empty())
    {
        return Some(BoundStmt::ReDimRuntime {
            name,
            bounds: bounds
                .into_iter()
                .map(|(lower_bound, upper_bound)| RuntimeArrayDimExpr {
                    lower_bound,
                    upper_bound: BoundExpr::IntConst(upper_bound),
                })
                .collect(),
            preserve,
        });
    }
    let element_prefix = format!("{name}_");
    let element_ty = declaration_types
        .iter()
        .find_map(|(key, ty)| {
            if key.starts_with(&element_prefix) {
                Some(*ty)
            } else {
                None
            }
        })
        .unwrap_or(BoundType::Variant);
    let previous_bounds = array_bounds.insert(name.clone(), bounds.clone());
    let element_count = array_element_count(&bounds)?;
    for idx in 0..element_count {
        let alias = format!("{name}_{idx}");
        if !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&alias))
        {
            declarations.push(alias.clone());
        }
        declaration_types.insert(alias, element_ty);
    }

    Some(BoundStmt::ReDim {
        name,
        bounds,
        previous_bounds,
        preserve,
    })
}

fn parse_runtime_redim_stmt(
    payload: &str,
    preserve: bool,
    declarations: &[String],
    array_bounds: &ArrayBoundsMap,
    option_base: i32,
) -> Option<BoundStmt> {
    let open = payload.find('(')?;
    let close = payload.rfind(')')?;
    if close <= open || !payload[close + 1..].trim().is_empty() {
        return None;
    }
    let name = normalize_ident(payload[..open].trim())?;
    if !declarations
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&name))
    {
        return None;
    }
    let raw_bounds = payload[open + 1..close].trim();
    let dims = split_call_args(raw_bounds)?;
    if dims.is_empty() {
        return None;
    }

    Some(BoundStmt::ReDimRuntime {
        name,
        bounds: dims
            .into_iter()
            .map(|dim| {
                let dim = dim.trim();
                let (lower_bound, upper_raw) = if let Some((lhs, rhs)) = split_keyword_ci(dim, "to")
                {
                    let lower = lhs.trim().parse::<i32>().ok()?;
                    (lower, rhs.trim())
                } else {
                    (option_base, dim)
                };
                let upper_bound = parse_expr(upper_raw, array_bounds)?;
                Some(RuntimeArrayDimExpr {
                    lower_bound,
                    upper_bound,
                })
            })
            .collect::<Option<Vec<_>>>()?,
        preserve,
    })
}

fn describe_unsupported_redim_expression_bounds(payload: &str) -> Option<String> {
    let open = payload.find('(')?;
    let close = payload.rfind(')')?;
    if close <= open || !payload[close + 1..].trim().is_empty() {
        return None;
    }
    let (name, _) = normalize_ident_with_type_char(payload[..open].trim())?;
    let raw_bounds = payload[open + 1..close].trim();
    if raw_bounds.is_empty() {
        return None;
    }
    Some(format!(
        "ReDim with runtime expression bounds is not yet supported for array `{name}`: {payload}"
    ))
}

#[allow(clippy::too_many_arguments)]
fn parse_select_case_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let Some((_, expr_raw)) = split_ci(line, "case") else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };
    let Some(expr) = parse_expr(expr_raw, array_bounds) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let mut arms: Vec<(Vec<BoundCaseClause>, Vec<BoundStmt>)> = Vec::new();
    let mut else_body: Vec<BoundStmt> = Vec::new();

    while *index < lines.len() {
        let current = lines[*index].as_str();
        let lower = current.to_ascii_lowercase();

        if lower == "end select" {
            *index += 1;
            return BoundStmt::SelectCase {
                expr,
                arms,
                else_body,
            };
        }

        if lower.starts_with("case else") {
            *index += 1;
            else_body = parse_block(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                &["end select"],
            );
            continue;
        }

        if lower.starts_with("case ") {
            let values_raw = current[5..].trim();
            let Some(values) = parse_case_clauses(values_raw, module_constants) else {
                return BoundStmt::Unsupported {
                    line: line.to_string(),
                };
            };

            *index += 1;
            let body = parse_block(
                lines,
                index,
                declarations,
                declaration_types,
                duplicate_declarations,
                array_bounds,
                option_explicit,
                option_base,
                default_type_table,
                udt_defs,
                module_constants,
                property_write_routes,
                property_read_routes,
                &["case", "end select"],
            );
            arms.push((values, body));
            continue;
        }

        return BoundStmt::Unsupported {
            line: current.to_string(),
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_case_clauses(
    values_raw: &str,
    module_constants: &ModuleConstMap,
) -> Option<Vec<BoundCaseClause>> {
    let tokens = split_call_args(values_raw)?;
    let mut out = Vec::new();
    for token in tokens {
        let clause = parse_case_clause(token.trim(), module_constants)?;
        out.push(clause);
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn parse_case_clause(token: &str, module_constants: &ModuleConstMap) -> Option<BoundCaseClause> {
    let token = token.trim().trim_end_matches(':').trim();
    if token.is_empty() {
        return None;
    }

    let lower = token.to_ascii_lowercase();
    if lower.starts_with("is ") {
        return parse_case_is_clause(token[3..].trim(), module_constants);
    }

    if let Some((start_raw, end_raw)) = split_keyword_ci(token, "to") {
        let start = parse_case_i32_value(start_raw.trim(), module_constants)?;
        let end = parse_case_i32_value(end_raw.trim(), module_constants)?;
        return Some(BoundCaseClause::Range { start, end });
    }

    parse_case_i32_value(token, module_constants).map(BoundCaseClause::Value)
}

fn parse_case_is_clause(token: &str, module_constants: &ModuleConstMap) -> Option<BoundCaseClause> {
    let pairs = [
        ("<>", CompareOp::Ne),
        ("<=", CompareOp::Le),
        (">=", CompareOp::Ge),
        ("=", CompareOp::Eq),
        ("<", CompareOp::Lt),
        (">", CompareOp::Gt),
    ];
    for (op_text, op) in pairs {
        if let Some(rest) = token.strip_prefix(op_text) {
            let value = parse_case_i32_value(rest.trim(), module_constants)?;
            return Some(BoundCaseClause::Is { op, value });
        }
    }
    None
}

fn parse_case_i32_value(token: &str, module_constants: &ModuleConstMap) -> Option<i32> {
    if let Ok(value) = token.parse::<i32>() {
        return Some(value);
    }
    let name = normalize_ident(token)?;
    let expr = module_constants.get(&name)?;
    match expr {
        BoundExpr::IntConst(value) => Some(*value),
        BoundExpr::UnaryOp {
            op: ArithOp::Neg,
            operand,
        } => match operand.as_ref() {
            BoundExpr::IntConst(value) => Some(value.saturating_neg()),
            _ => None,
        },
        _ => None,
    }
}

/// Scan right-to-left at paren depth 0, quote-aware, to find the operator
/// with the lowest precedence.  Precedence (low→high):
///   `&`  →  `+`/`-`  →  `Mod`  →  `\`  →  `*`/`/`  →  `^`
/// Returns `(ArithOp, byte-position-of-operator)` or `None`.
fn split_at_lowest_precedence_op(expr: &str) -> Option<(ArithOp, usize)> {
    fn precedence(op: ArithOp) -> u8 {
        match op {
            ArithOp::Concat => 1,
            ArithOp::Add | ArithOp::Sub => 2,
            ArithOp::Mod => 3,
            ArithOp::IntDiv => 4,
            ArithOp::Mul | ArithOp::Div => 5,
            ArithOp::Pow => 6,
            ArithOp::Neg => 7,
        }
    }

    let bytes = expr.as_bytes();
    let len = bytes.len();
    let mut best: Option<(ArithOp, usize)> = None;
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut i = len;

    while i > 0 {
        i -= 1;
        let ch = bytes[i];

        if ch == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }

        if ch == b')' {
            depth += 1;
            continue;
        }
        if ch == b'(' {
            depth -= 1;
            continue;
        }
        if depth != 0 {
            continue;
        }

        let candidate = match ch {
            b'^' => Some((ArithOp::Pow, i)),
            b'*' => Some((ArithOp::Mul, i)),
            b'/' => Some((ArithOp::Div, i)),
            b'\\' => Some((ArithOp::IntDiv, i)),
            b'+' => {
                // Guard: unary `+` at position 0 or after another operator — skip
                let before = expr[..i].trim_end();
                if before.is_empty() {
                    None
                } else {
                    Some((ArithOp::Add, i))
                }
            }
            b'-' => {
                // Guard: unary `-` at position 0 or after another operator — skip
                let before = expr[..i].trim_end();
                if before.is_empty() {
                    None
                } else {
                    let last = before.as_bytes()[before.len() - 1];
                    if matches!(last, b'+' | b'-' | b'*' | b'/' | b'\\' | b'^' | b'(') {
                        None // unary minus after operator or open-paren
                    } else {
                        Some((ArithOp::Sub, i))
                    }
                }
            }
            b'&' => {
                // Guard: `&H`, `&O`, `&B` hex/octal/binary literal prefixes
                if i + 1 < len && matches!(bytes[i + 1], b'H' | b'h' | b'O' | b'o' | b'B' | b'b') {
                    None
                } else {
                    Some((ArithOp::Concat, i))
                }
            }
            _ => None,
        };

        // Check for `Mod` keyword (case-insensitive, requires word boundaries)
        if candidate.is_none() && i + 3 <= len {
            let slice = &expr[i..i + 3];
            if slice.eq_ignore_ascii_case("mod") {
                let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                let after_ok = i + 3 >= len || !bytes[i + 3].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    let mod_candidate = Some((ArithOp::Mod, i));
                    if let Some((op, _)) = mod_candidate {
                        let p = precedence(op);
                        // We scan right-to-left. On equal precedence we keep the
                        // first match we saw, which is the rightmost operator, so
                        // `a - b + 1` parses as `(a - b) + 1`.
                        if best.is_none_or(|(b_op, _)| p < precedence(b_op)) {
                            best = mod_candidate;
                        }
                    }
                    continue;
                }
            }
        }

        if let Some((op, _)) = candidate {
            let p = precedence(op);
            // Same-precedence operators are left-associative; because we scan
            // right-to-left, keeping the first equal-precedence match preserves
            // the rightmost split point.
            if best.is_none_or(|(b_op, _)| p < precedence(b_op)) {
                best = candidate;
            }
        }
    }

    // Only return a split if both sides are non-empty
    if let Some((op, pos)) = best {
        let left = expr[..pos].trim();
        let right_start = match op {
            ArithOp::Mod => pos + 3,
            _ => pos + 1,
        };
        let right = if right_start <= expr.len() {
            expr[right_start..].trim()
        } else {
            ""
        };
        if !left.is_empty() && !right.is_empty() {
            return Some((op, pos));
        }
    }
    None
}

/// Resolves an always-available VBA intrinsic constant (the `vbConstants` family from the
/// VBA runtime library) to its literal value. These are predeclared in every VBA project
/// regardless of references, so a bare `vbCrLf`/`vbBinaryCompare`/`vbYesNo`/etc. must bind
/// here instead of falling through to "use of undeclared variable". `vbNullString`, `Empty`,
/// and `Null` are modeled as dedicated intrinsics elsewhere and are intentionally omitted.
fn intrinsic_vb_constant(name: &str) -> Option<BoundExpr> {
    let s = |text: &str| Some(BoundExpr::StringConst(text.to_string()));
    let i = |value: i32| Some(BoundExpr::IntConst(value));
    match name.to_ascii_lowercase().as_str() {
        // String control characters
        "vbcr" => s("\r"),
        "vblf" => s("\n"),
        "vbcrlf" | "vbnewline" => s("\r\n"),
        "vbtab" => s("\t"),
        "vbnullchar" => s("\0"),
        "vbback" => s("\u{0008}"),
        "vbformfeed" => s("\u{000C}"),
        "vbverticaltab" => s("\u{000B}"),
        // Comparison (VbCompareMethod)
        "vbbinarycompare" => i(0),
        "vbtextcompare" => i(1),
        "vbdatabasecompare" => i(2),
        // String conversion (VbStrConv)
        "vbuppercase" => i(1),
        "vblowercase" => i(2),
        "vbpropercase" => i(3),
        "vbwide" => i(4),
        "vbnarrow" => i(8),
        "vbkatakana" => i(16),
        "vbhiragana" => i(32),
        "vbunicode" => i(64),
        "vbfromunicode" => i(128),
        // VarType (VbVarType)
        "vbempty" => i(0),
        "vbnull" => i(1),
        "vbinteger" => i(2),
        "vblong" => i(3),
        "vbsingle" => i(4),
        "vbdouble" => i(5),
        "vbcurrency" => i(6),
        "vbdate" => i(7),
        "vbstring" => i(8),
        "vbobject" => i(9),
        "vberror" => i(10),
        "vbboolean" => i(11),
        "vbvariant" => i(12),
        "vbdataobject" => i(13),
        "vbdecimal" => i(14),
        "vbbyte" => i(17),
        "vblonglong" => i(20),
        "vbuserdefinedtype" => i(36),
        "vbarray" => i(8192),
        // Tristate / boolean-ish
        "vbtrue" => i(-1),
        "vbfalse" => i(0),
        "vbusedefault" => i(-2),
        // MsgBox buttons / icons / defaults / modality (VbMsgBoxStyle)
        "vbokonly" => i(0),
        "vbokcancel" => i(1),
        "vbabortretryignore" => i(2),
        "vbyesnocancel" => i(3),
        "vbyesno" => i(4),
        "vbretrycancel" => i(5),
        "vbcritical" => i(16),
        "vbquestion" => i(32),
        "vbexclamation" => i(48),
        "vbinformation" => i(64),
        "vbdefaultbutton1" => i(0),
        "vbdefaultbutton2" => i(256),
        "vbdefaultbutton3" => i(512),
        "vbdefaultbutton4" => i(768),
        "vbapplicationmodal" => i(0),
        "vbsystemmodal" => i(4096),
        "vbmsgboxhelpbutton" => i(16384),
        "vbmsgboxsetforeground" => i(65536),
        "vbmsgboxright" => i(524288),
        "vbmsgboxrtlreading" => i(1048576),
        // MsgBox results (VbMsgBoxResult)
        "vbok" => i(1),
        "vbcancel" => i(2),
        "vbabort" => i(3),
        "vbretry" => i(4),
        "vbignore" => i(5),
        "vbyes" => i(6),
        "vbno" => i(7),
        // Colors (VbColorConstants), RGB-packed
        "vbblack" => i(0),
        "vbred" => i(255),
        "vbgreen" => i(65280),
        "vbyellow" => i(65535),
        "vbblue" => i(16711680),
        "vbmagenta" => i(16711935),
        "vbcyan" => i(16776960),
        "vbwhite" => i(16777215),
        // Date format (VbDateTimeFormat)
        "vbgeneraldate" => i(0),
        "vblongdate" => i(1),
        "vbshortdate" => i(2),
        "vblongtime" => i(3),
        "vbshorttime" => i(4),
        // Day of week / first week (VbDayOfWeek / VbFirstWeekOfYear)
        "vbusesystemdayofweek" => i(0),
        "vbsunday" => i(1),
        "vbmonday" => i(2),
        "vbtuesday" => i(3),
        "vbwednesday" => i(4),
        "vbthursday" => i(5),
        "vbfriday" => i(6),
        "vbsaturday" => i(7),
        "vbusesystem" => i(0),
        "vbfirstjan1" => i(1),
        "vbfirstfourdays" => i(2),
        "vbfirstfullweek" => i(3),
        // File attributes (VbFileAttribute)
        "vbnormal" => i(0),
        "vbreadonly" => i(1),
        "vbhidden" => i(2),
        "vbsystem" => i(4),
        "vbvolume" => i(8),
        "vbdirectory" => i(16),
        "vbarchive" => i(32),
        // Automation/object error base
        "vbobjecterror" => i(-2147221504),
        _ => None,
    }
}

fn parse_expr(text: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    let expr = text.trim();
    if expr.eq_ignore_ascii_case("vbnullstring") {
        return Some(BoundExpr::IntrinsicCall {
            name: "vbnullstring".to_string(),
            args: Vec::new(),
        });
    }
    if expr.eq_ignore_ascii_case("empty") {
        return Some(BoundExpr::IntrinsicCall {
            name: "__empty".to_string(),
            args: Vec::new(),
        });
    }
    if expr.eq_ignore_ascii_case("null") {
        return Some(BoundExpr::IntrinsicCall {
            name: "__null".to_string(),
            args: Vec::new(),
        });
    }
    if expr.eq_ignore_ascii_case("nothing") {
        // The null object reference. Typed as Object so `Set obj = Nothing` is accepted (and a
        // non-object `Let`/arithmetic use is rejected), while still lowering to runtime 0 so it
        // reads as a cleared object slot.
        return Some(BoundExpr::IntrinsicCall {
            name: "__nothing".to_string(),
            args: Vec::new(),
        });
    }
    // Always-available VBA intrinsic constants (vbCrLf, vbBinaryCompare, vbYesNo, ...).
    // Guarded on a `vb` prefix to keep the common path cheap.
    if expr.len() >= 4
        && expr.as_bytes()[..2].eq_ignore_ascii_case(b"vb")
        && let Some(constant) = intrinsic_vb_constant(expr)
    {
        return Some(constant);
    }
    if let Ok(value) = expr.parse::<i32>() {
        return Some(BoundExpr::IntConst(value));
    }
    if (expr.contains('.') || expr.contains('e') || expr.contains('E'))
        && let Ok(value) = expr.parse::<f64>()
    {
        return Some(BoundExpr::FloatConst(value.to_bits()));
    }

    // String literals: "hello", "he""llo" (escaped quotes)
    if let Some(s) = parse_quoted_string_literal(expr) {
        return Some(BoundExpr::StringConst(s));
    }

    // Boolean keywords are preserved as booleans so runtime lanes that care
    // about logical shape, like Write#/Input# roundtrips and CStr(), can
    // distinguish them from numeric -1/0.
    if expr.eq_ignore_ascii_case("true") {
        return Some(BoundExpr::BoolConst(true));
    }
    if expr.eq_ignore_ascii_case("false") {
        return Some(BoundExpr::BoolConst(false));
    }

    // Hex literals: &HFF → 255, &O77 → 63
    if let Some(hex_val) = parse_numeric_prefix_literal(expr) {
        return Some(BoundExpr::IntConst(hex_val));
    }

    // Type-suffix numeric literals: 2# (Double), 2! (Single), 2@ (Currency),
    // 2% (Integer), 2& (Long), 2^ (LongLong).
    if let Some(suffixed) = parse_typed_suffix_literal(expr) {
        return Some(suffixed);
    }

    // Bare parenthesized expression: `(expr)` — strip and recurse
    if expr.starts_with('(') && expr.ends_with(')') {
        let inner = &expr[1..expr.len() - 1];
        // Only strip if the parens are balanced (not a function call)
        let mut depth = 0i32;
        let mut balanced = true;
        for (idx, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        balanced = false;
                        break;
                    }
                }
                _ => {}
            }
            // If we close all parens before the end, these aren't wrapping parens
            let _ = idx;
        }
        if balanced && depth == 0 {
            if let Some(parsed) = parse_expr(inner, array_bounds) {
                return Some(parsed);
            }
            if let Some(parsed) = parse_compare_expr(inner, array_bounds) {
                return Some(parsed);
            }
        }
    }

    // Logical operators have the lowest precedence (VBA: Or below And below Not,
    // all below comparison). Split outermost so operands recurse into the
    // comparison/arithmetic grammar. Lowered to truthy BoolOr/BoolAnd/BoolNot,
    // matching the condition path's semantics.
    if let Some((lhs_raw, rhs_raw)) = split_compare_keyword_top_level(expr, "or") {
        let lhs = parse_expr(lhs_raw, array_bounds)?;
        let rhs = parse_expr(rhs_raw, array_bounds)?;
        return Some(BoundExpr::LogicalBinaryOp {
            op: LogicalBinOp::Or,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        });
    }
    if let Some((lhs_raw, rhs_raw)) = split_compare_keyword_top_level(expr, "and") {
        let lhs = parse_expr(lhs_raw, array_bounds)?;
        let rhs = parse_expr(rhs_raw, array_bounds)?;
        return Some(BoundExpr::LogicalBinaryOp {
            op: LogicalBinOp::And,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        });
    }
    if let Some(rest) = strip_not_prefix(expr)
        && let Some(operand) = parse_expr(rest, array_bounds)
    {
        return Some(BoundExpr::LogicalNot {
            operand: Box::new(operand),
        });
    }

    if let Some(inner) = parse_intrinsic_conversion_expr(expr, array_bounds) {
        return Some(inner);
    }
    if let Some(call) = parse_stdlib_intrinsic_call_expr(expr, array_bounds) {
        return Some(call);
    }
    if let Some(array_index) = parse_runtime_array_index_expr(expr, array_bounds) {
        return Some(array_index);
    }
    if parse_reference_name(expr, array_bounds).is_none()
        && let Some((name, args)) = parse_call_invocation(expr, array_bounds)
        && !is_intrinsic_call_name(&name)
    {
        return Some(BoundExpr::ProcCall { name, args });
    }
    if let Some(compare) = parse_compare_expr(expr, array_bounds) {
        return Some(compare);
    }

    if let Some((op, split_pos)) = split_at_lowest_precedence_op(expr) {
        let left_raw = &expr[..split_pos];
        let right_raw = match op {
            ArithOp::Mod => &expr[split_pos + 3..],    // "Mod" is 3 chars
            ArithOp::Concat => &expr[split_pos + 1..], // "&"
            _ => &expr[split_pos + 1..],
        };

        // Preserve AddConst/SubConst fast-path for simple `var + const` / `var - const`
        if matches!(op, ArithOp::Add | ArithOp::Sub)
            && let Some(var) = parse_reference_name(left_raw, array_bounds)
            && let Ok(delta) = right_raw.trim().parse::<i32>()
        {
            return match op {
                ArithOp::Add => Some(BoundExpr::AddConst { var, delta }),
                ArithOp::Sub => Some(BoundExpr::SubConst { var, delta }),
                _ => unreachable!(),
            };
        }

        let lhs = parse_expr(left_raw, array_bounds)?;
        let rhs = parse_expr(right_raw, array_bounds)?;
        return Some(BoundExpr::BinaryOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        });
    }

    // Unary negation: starts with `-` and the remainder is a valid expression
    if let Some(rest) = expr.strip_prefix('-') {
        let rest = rest.trim();
        if !rest.is_empty()
            && let Some(operand) = parse_expr(rest, array_bounds)
        {
            return Some(BoundExpr::UnaryOp {
                op: ArithOp::Neg,
                operand: Box::new(operand),
            });
        }
    }

    parse_reference_name(expr, array_bounds).map(BoundExpr::Var)
}

/// Parse a VBA type-suffix numeric literal (e.g. `2#`, `2.5!`, `100&`). Only
/// fires when the text before the suffix is itself a valid number, so it does
/// not interfere with the `&` concatenation operator (`x & 2`).
fn parse_typed_suffix_literal(expr: &str) -> Option<BoundExpr> {
    let last = *expr.as_bytes().last()?;
    let body = expr[..expr.len() - 1].trim();
    if body.is_empty() {
        return None;
    }
    match last {
        // Double / Single / Currency carriers — represented as f64 constants.
        b'#' | b'!' | b'@' => body
            .parse::<f64>()
            .ok()
            .map(|v| BoundExpr::FloatConst(v.to_bits())),
        // Integer / Long / LongLong carriers — represented as i32 constants for
        // the common literal range.
        b'%' | b'&' | b'^' => body.parse::<i32>().ok().map(BoundExpr::IntConst),
        _ => None,
    }
}

/// Strip a leading `Not` logical-operator keyword (word-bounded), returning the
/// remaining operand text. Does not match identifiers like `Nothing`/`Notify`.
fn strip_not_prefix(expr: &str) -> Option<&str> {
    let trimmed = expr.trim_start();
    if trimmed.len() >= 3 && trimmed[..3].eq_ignore_ascii_case("not") {
        let after = &trimmed[3..];
        if after.starts_with(|c: char| c.is_whitespace() || c == '(') {
            let rest = after.trim_start();
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

fn parse_compare_expr(expr: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    if let Some((lhs_raw, rhs_raw)) = split_compare_keyword_top_level(expr, "like") {
        let lhs = parse_expr(lhs_raw, array_bounds)?;
        let rhs = parse_expr(rhs_raw, array_bounds)?;
        return Some(BoundExpr::CompareOp {
            op: CompareOp::Like,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        });
    }
    let (op, pos, width) = split_compare_operator_top_level(expr)?;
    let lhs = parse_expr(expr[..pos].trim(), array_bounds)?;
    let rhs = parse_expr(expr[pos + width..].trim(), array_bounds)?;
    Some(BoundExpr::CompareOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn split_compare_keyword_top_level<'a>(text: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let bytes = text.as_bytes();
    let keyword_len = keyword.len();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut idx = 0usize;
    while idx + keyword_len <= bytes.len() {
        let ch = bytes[idx] as char;
        if ch == '"' {
            if in_string && idx + 1 < bytes.len() && bytes[idx + 1] == b'"' {
                idx += 2;
                continue;
            }
            in_string = !in_string;
            idx += 1;
            continue;
        }
        if in_string {
            idx += 1;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0
            && lower[idx..].starts_with(keyword)
            && is_keyword_boundary(text, idx.saturating_sub(1))
            && is_keyword_boundary(text, idx + keyword_len)
        {
            let lhs = text[..idx].trim();
            let rhs = text[idx + keyword_len..].trim();
            if !lhs.is_empty() && !rhs.is_empty() {
                return Some((lhs, rhs));
            }
        }
        idx += 1;
    }
    None
}

fn is_keyword_boundary(text: &str, index: usize) -> bool {
    if index >= text.len() {
        return true;
    }
    !text.as_bytes()[index].is_ascii_alphanumeric() && text.as_bytes()[index] != b'_'
}

fn split_compare_operator_top_level(text: &str) -> Option<(CompareOp, usize, usize)> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut idx = 0usize;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch == '"' {
            if in_string && idx + 1 < bytes.len() && bytes[idx + 1] == b'"' {
                idx += 2;
                continue;
            }
            in_string = !in_string;
            idx += 1;
            continue;
        }
        if in_string {
            idx += 1;
            continue;
        }
        match ch {
            '(' => {
                depth += 1;
                idx += 1;
                continue;
            }
            ')' => {
                depth -= 1;
                idx += 1;
                continue;
            }
            _ => {}
        }
        if depth == 0 {
            let tail = &text[idx..];
            let candidate = if tail.starts_with("<>") {
                Some((CompareOp::Ne, 2))
            } else if tail.starts_with("<=") {
                Some((CompareOp::Le, 2))
            } else if tail.starts_with(">=") {
                Some((CompareOp::Ge, 2))
            } else if tail.starts_with("=") {
                Some((CompareOp::Eq, 1))
            } else if tail.starts_with("<") {
                Some((CompareOp::Lt, 1))
            } else if tail.starts_with(">") {
                Some((CompareOp::Gt, 1))
            } else {
                None
            };
            if let Some((op, width)) = candidate {
                let lhs = text[..idx].trim();
                let rhs = text[idx + width..].trim();
                if !lhs.is_empty() && !rhs.is_empty() {
                    return Some((op, idx, width));
                }
            }
        }
        idx += 1;
    }
    None
}

fn parse_intrinsic_conversion_expr(expr: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    let open = expr.find('(')?;
    let close = expr.rfind(')')?;
    if close <= open || !expr[close + 1..].trim().is_empty() {
        return None;
    }
    let name = normalize_ident(expr[..open].trim())?;
    if !matches!(
        name.as_str(),
        "cint"
            | "clng"
            | "cdbl"
            | "cstr"
            | "cbool"
            | "cdate"
            | "csng"
            | "cbyte"
            | "ccur"
            | "cdec"
            | "val"
            | "str"
            | "cverr"
    ) {
        return None;
    }
    let inner = parse_expr(expr[open + 1..close].trim(), array_bounds)?;
    if matches!(name.as_str(), "cverr" | "cstr" | "str" | "val" | "cdate") {
        return Some(BoundExpr::IntrinsicCall {
            name,
            args: vec![inner],
        });
    }
    Some(inner)
}

#[derive(Debug, Clone, Copy)]
pub struct IntrinsicSpec {
    pub min_arity: usize,
    pub max_arity: usize,
    pub surface: IntrinsicSurface,
}

impl IntrinsicSpec {
    const fn fixed(arity: usize, surface: IntrinsicSurface) -> Self {
        Self {
            min_arity: arity,
            max_arity: arity,
            surface,
        }
    }

    const fn range(min_arity: usize, max_arity: usize, surface: IntrinsicSurface) -> Self {
        Self {
            min_arity,
            max_arity,
            surface,
        }
    }

    fn arity_allows(self, count: usize) -> bool {
        (self.min_arity..=self.max_arity).contains(&count)
    }
}

pub fn intrinsic_surface(name: &str) -> Option<IntrinsicSurface> {
    let normalized = normalize_ident(name)?;
    intrinsic_spec(normalized.as_str()).map(|spec| spec.surface)
}

pub fn intrinsic_spec(name: &str) -> Option<IntrinsicSpec> {
    use IntrinsicSurface::{DeterministicCore, HostSensitive};

    match name {
        "rnd" | "randomize" => Some(IntrinsicSpec::range(0, 1, DeterministicCore)),
        "date" | "time" | "now" | "timer" => Some(IntrinsicSpec::fixed(0, HostSensitive)),
        "freefile" => Some(IntrinsicSpec::range(0, 1, HostSensitive)),
        "doevents" => Some(IntrinsicSpec::fixed(0, HostSensitive)),
        "msgbox" | "inputbox" => Some(IntrinsicSpec::range(1, 2, HostSensitive)),
        "len" | "lcase" | "ucase" | "trim" | "ltrim" | "rtrim" | "datevalue" | "timevalue"
        | "abs" | "int" | "fix" | "sgn" | "sqr" | "sin" | "cos" | "log" | "exp" | "hex" | "oct"
        | "atn" | "tan" | "year" | "month" | "day" | "weekday" | "space" | "chr" | "asc"
        | "lbound" | "ubound" | "isarray" | "vartype" | "typename" | "isnumeric" | "isdate"
        | "isobject" | "isempty" | "isnull" | "iserror" | "monthname" | "collectioncount"
        | "strreverse" | "strptr" | "varptr" | "objptr" => {
            Some(IntrinsicSpec::fixed(1, DeterministicCore))
        }
        "eof" | "lof" | "loc" | "seek" => Some(IntrinsicSpec::fixed(1, HostSensitive)),
        "format" => Some(IntrinsicSpec::range(1, 2, DeterministicCore)),
        "strconv" => Some(IntrinsicSpec::range(2, 3, DeterministicCore)),
        "left" | "right" | "instr" | "instrrev" | "split" | "join" | "strcomp" => {
            Some(IntrinsicSpec::fixed(2, DeterministicCore))
        }
        "string" | "typeofis" => Some(IntrinsicSpec::fixed(2, DeterministicCore)),
        "mid" => Some(IntrinsicSpec::range(2, 3, DeterministicCore)),
        "round" => Some(IntrinsicSpec::range(1, 2, DeterministicCore)),
        "replace" | "dateserial" | "timeserial" | "dateadd" | "datediff" => {
            Some(IntrinsicSpec::fixed(3, DeterministicCore))
        }
        "mirr" => Some(IntrinsicSpec::fixed(3, DeterministicCore)),
        "collectionadd" | "collectionitem" | "collectionremove" => {
            Some(IntrinsicSpec::range(2, 3, DeterministicCore))
        }
        "fv" | "pv" | "pmt" => Some(IntrinsicSpec::range(3, 5, DeterministicCore)),
        "rate" => Some(IntrinsicSpec::range(3, 6, DeterministicCore)),
        "nper" => Some(IntrinsicSpec::range(3, 5, DeterministicCore)),
        "irr" => Some(IntrinsicSpec::range(1, 2, DeterministicCore)),
        "npv" => Some(IntrinsicSpec::range(2, usize::MAX, DeterministicCore)),
        "array" => Some(IntrinsicSpec::range(1, usize::MAX, DeterministicCore)),
        "__oxvba_array_append" => Some(IntrinsicSpec::fixed(2, DeterministicCore)),
        // Internal carrier for a freshly instantiated project-class instance. Typed `Object`
        // and lowered (via `LoadProjectObjectRef`) to materialise the instance's
        // reference-counted `ObjectRef` as an Object Variant, so `Set <var> = New <ProjectClass>`
        // assigns a real object reference (refcounted on Set/scope via the COM `Variant` path).
        "__oxvba_project_instance" => Some(IntrinsicSpec::fixed(1, DeterministicCore)),
        "shell" | "environ" | "createobject" => Some(IntrinsicSpec::fixed(1, HostSensitive)),
        "dir" => Some(IntrinsicSpec::range(0, 1, HostSensitive)),
        "dispatchinvoke" | "__oxvbaearlyinvoke" => {
            Some(IntrinsicSpec::range(2, usize::MAX, HostSensitive))
        }
        "__oxvba_com_subscribe_event" => Some(IntrinsicSpec::fixed(2, HostSensitive)),
        "__oxvba_com_unsubscribe_event" => Some(IntrinsicSpec::fixed(1, HostSensitive)),
        "__oxvba_com_callback_subscription" => Some(IntrinsicSpec::fixed(1, HostSensitive)),
        "__oxvba_com_callback_arg" => Some(IntrinsicSpec::fixed(2, HostSensitive)),
        "__oxvba_com_release_callback" => Some(IntrinsicSpec::fixed(1, HostSensitive)),
        "__oxvba_withevents_get" => Some(IntrinsicSpec::fixed(2, DeterministicCore)),
        "__oxvba_withevents_set" => Some(IntrinsicSpec::fixed(3, DeterministicCore)),
        "__oxvba_withevents_clear_owner" => Some(IntrinsicSpec::fixed(1, DeterministicCore)),
        "__oxvba_withevents_first_owner" => Some(IntrinsicSpec::fixed(2, DeterministicCore)),
        "__oxvba_withevents_next_owner" => Some(IntrinsicSpec::fixed(0, DeterministicCore)),
        _ => None,
    }
}

fn parse_stdlib_intrinsic_call_expr(
    expr: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<BoundExpr> {
    let open = expr.find('(')?;
    let close = expr.rfind(')')?;
    if close < open || !expr[close + 1..].trim().is_empty() {
        return None;
    }
    let name = normalize_ident(expr[..open].trim())?;
    let spec = intrinsic_spec(name.as_str())?;

    let args_raw = expr[open + 1..close].trim();
    let args_text = if args_raw.is_empty() {
        Vec::new()
    } else {
        split_call_args(args_raw)?
    };
    let args = match name.as_str() {
        "createobject" => args_text
            .iter()
            .map(|arg| parse_createobject_arg(arg, array_bounds))
            .collect::<Option<Vec<_>>>()?,
        "varptr" => args_text
            .iter()
            .map(|arg| parse_varptr_arg(arg, array_bounds))
            .collect::<Option<Vec<_>>>()?,
        "dispatchinvoke" | "__oxvbaearlyinvoke" => {
            parse_dispatch_invoke_args(&args_text, array_bounds)?
        }
        _ => args_text
            .iter()
            .map(|arg| parse_expr(arg, array_bounds))
            .collect::<Option<Vec<_>>>()?,
    };

    if !spec.arity_allows(args.len()) {
        return None;
    }

    Some(BoundExpr::IntrinsicCall { name, args })
}

fn starts_with_call_name_ci(text: &str, name: &str) -> bool {
    text.trim_start()
        .get(..name.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(name))
        && text.trim_start()[name.len()..]
            .trim_start()
            .starts_with('(')
}

fn parse_varptr_arg(arg: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    if let Some(base) = parse_array_element_base_for_varptr(arg, array_bounds) {
        return Some(BoundExpr::VarPtrArrayBuffer(base));
    }
    parse_expr(arg, array_bounds)
}

fn parse_runtime_array_index_expr(expr: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    let (base, indices) = parse_runtime_array_index_target(expr, array_bounds)?;
    let mut args = Vec::with_capacity(indices.len() + 1);
    args.push(BoundExpr::Var(base));
    args.extend(indices);
    Some(BoundExpr::IntrinsicCall {
        name: "__oxvba_array_get".to_string(),
        args,
    })
}

fn parse_runtime_array_index_target(
    expr: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, Vec<BoundExpr>)> {
    let open = expr.find('(')?;
    let close = expr.rfind(')')?;
    if close <= open || !expr[close + 1..].trim().is_empty() {
        return None;
    }
    let base = normalize_ident(expr[..open].trim())?;
    let args = split_call_args(expr[open + 1..close].trim())?;
    if args.is_empty() {
        return None;
    }
    match array_bounds.get(&base) {
        Some(bounds) if bounds.is_empty() => {}
        Some(_) => return None,
        None => {
            let dynamic_bound_arg = args.iter().any(|arg| {
                let trimmed = arg.trim();
                (starts_with_call_name_ci(trimmed, "lbound")
                    || starts_with_call_name_ci(trimmed, "ubound"))
                    && trimmed
                        .find('(')
                        .and_then(|open| trimmed.rfind(')').map(|close| (open, close)))
                        .is_some_and(|(open, close)| {
                            close > open
                                && normalize_ident(trimmed[open + 1..close].trim())
                                    .is_some_and(|name| name.eq_ignore_ascii_case(&base))
                        })
            });
            if !dynamic_bound_arg {
                return None;
            }
        }
    }
    let indices = args
        .into_iter()
        .map(|arg| parse_expr(arg, array_bounds))
        .collect::<Option<Vec<_>>>()?;
    Some((base, indices))
}

fn parse_array_element_base_for_varptr(
    token: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<String> {
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open || !token[close + 1..].trim().is_empty() {
        return None;
    }
    let base = normalize_ident(token[..open].trim())?;
    let bounds = array_bounds.get(&base)?;
    let indices = split_call_args(token[open + 1..close].trim())?;
    if indices.len() != 1 || indices[0].trim() != "0" {
        return None;
    }
    if !bounds.is_empty() && (bounds.len() != 1 || bounds[0].0 != 0) {
        return None;
    }
    Some(base)
}

fn parse_createobject_arg(arg: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    parse_expr(arg, array_bounds)
}

fn parse_dispatch_member_arg(
    _object_arg: &str,
    arg: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<BoundExpr> {
    // Preserve quoted DispatchInvoke selectors as strings. Lowering them to member
    // tokens here was a fixture-specific optimization and does not belong on the
    // general compiler path.
    if let Some(literal) = parse_quoted_string_literal(arg) {
        return Some(BoundExpr::StringConst(literal));
    }
    parse_expr(arg, array_bounds)
}

fn parse_dispatch_invoke_args(
    args_text: &[&str],
    array_bounds: &ArrayBoundsMap,
) -> Option<Vec<BoundExpr>> {
    if args_text.len() < 2 {
        return None;
    }
    let mut out = Vec::with_capacity(args_text.len());
    out.push(parse_expr(args_text[0], array_bounds)?);
    out.push(parse_dispatch_member_arg(
        args_text[0],
        args_text[1],
        array_bounds,
    )?);
    for arg in &args_text[2..] {
        out.push(parse_expr(arg, array_bounds)?);
    }
    Some(out)
}

fn parse_quoted_string_literal(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('"') || !trimmed.ends_with('"') || trimmed.len() < 2 {
        return None;
    }
    let body = &trimmed[1..trimmed.len() - 1];
    // Validate: every `"` inside must be doubled (`""`)
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut result = String::new();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                result.push('"');
                i += 2;
            } else {
                return None; // unescaped quote — not a valid string literal
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    Some(result)
}

fn parse_numeric_prefix_literal(text: &str) -> Option<i32> {
    let t = text.trim();
    if t.len() < 3 || !t.starts_with('&') {
        return None;
    }
    let prefix = t.as_bytes()[1].to_ascii_lowercase();
    let digits = &t[2..];
    match prefix {
        b'h' => i32::from_str_radix(digits, 16).ok(),
        b'o' => i32::from_str_radix(digits, 8).ok(),
        _ => None,
    }
}

/// Splits a comma-separated argument list at top level (respecting nested parens and
/// string literals). Empty positions are rejected — appropriate for array indices,
/// `ReDim` bounds, and value lists where bare commas are not valid.
fn split_call_args(args_raw: &str) -> Option<Vec<&str>> {
    split_call_args_inner(args_raw, false)
}

/// Like [`split_call_args`], but keeps omitted positions (bare commas) as empty `""`
/// tokens instead of rejecting them. Used for procedure/method call arguments, where
/// `Foo(1, , 5)` legally omits an optional parameter.
fn split_call_args_allowing_omitted(args_raw: &str) -> Option<Vec<&str>> {
    split_call_args_inner(args_raw, true)
}

fn split_call_args_inner(args_raw: &str, allow_omitted: bool) -> Option<Vec<&str>> {
    if args_raw.trim().is_empty() {
        return Some(Vec::new());
    }

    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let bytes = args_raw.as_bytes();
    let mut idx = 0usize;
    let mut in_string = false;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch == '"' {
            if in_string && idx + 1 < bytes.len() && bytes[idx + 1] == b'"' {
                idx += 2;
                continue;
            }
            in_string = !in_string;
            idx += 1;
            continue;
        }
        if in_string {
            idx += 1;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            ',' if depth == 0 => {
                let part = args_raw[start..idx].trim();
                if part.is_empty() && !allow_omitted {
                    return None;
                }
                out.push(part);
                start = idx + 1;
            }
            _ => {}
        }
        idx += 1;
    }
    if depth != 0 || in_string {
        return None;
    }
    let tail = args_raw[start..].trim();
    if tail.is_empty() && !allow_omitted {
        return None;
    }
    out.push(tail);
    Some(out)
}

fn parse_condition(text: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundCond> {
    if let Some((lhs_raw, rhs_raw)) = split_keyword_ci(text, "or") {
        let lhs = parse_condition(lhs_raw, array_bounds)?;
        let rhs = parse_condition(rhs_raw, array_bounds)?;
        return Some(BoundCond::Or(Box::new(lhs), Box::new(rhs)));
    }

    if let Some((lhs_raw, rhs_raw)) = split_keyword_ci(text, "and") {
        let lhs = parse_condition(lhs_raw, array_bounds)?;
        let rhs = parse_condition(rhs_raw, array_bounds)?;
        return Some(BoundCond::And(Box::new(lhs), Box::new(rhs)));
    }

    let trimmed = text.trim();
    if let Some(rest) = strip_keyword_prefix_ci(trimmed, "not") {
        let inner = parse_condition(rest, array_bounds)?;
        return Some(BoundCond::Not(Box::new(inner)));
    }

    parse_compare_condition(trimmed, array_bounds)
}

fn parse_compare_condition(text: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundCond> {
    if let Some((lhs_raw, rhs_raw)) = split_keyword_ci(text, "like") {
        let lhs = parse_expr(lhs_raw, array_bounds)?;
        let rhs = parse_expr(rhs_raw, array_bounds)?;
        return Some(BoundCond::Compare {
            op: CompareOp::Like,
            lhs,
            rhs,
        });
    }

    if let Some(rest) = strip_keyword_prefix_ci(text.trim(), "typeof")
        && let Some((lhs_raw, rhs_raw)) = split_keyword_ci(rest, "is")
    {
        let lhs = parse_expr(lhs_raw, array_bounds)?;
        // VBA requires a literal type name after Is — treat RHS as a string constant,
        // not an evaluated expression.
        let type_name = rhs_raw.trim().to_string();
        let rhs = BoundExpr::StringConst(type_name);
        return Some(BoundCond::Truthy(BoundExpr::IntrinsicCall {
            name: "typeofis".to_string(),
            args: vec![lhs, rhs],
        }));
    }

    let pairs = [
        ("<>", CompareOp::Ne),
        ("<=", CompareOp::Le),
        (">=", CompareOp::Ge),
        ("=", CompareOp::Eq),
        ("<", CompareOp::Lt),
        (">", CompareOp::Gt),
    ];

    for (op_text, op) in pairs {
        if let Some((lhs_raw, rhs_raw)) = text.split_once(op_text) {
            let lhs = parse_expr(lhs_raw, array_bounds)?;
            let rhs = parse_expr(rhs_raw, array_bounds)?;
            return Some(BoundCond::Compare { op, lhs, rhs });
        }
    }

    parse_expr(text, array_bounds).map(BoundCond::Truthy)
}

#[allow(clippy::too_many_arguments)]
fn parse_declaration(
    line: &str,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
) {
    let Some(remainder) = strip_variable_declaration_prefix_ci(line) else {
        return;
    };
    parse_declaration_remainder(
        remainder,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_base,
        default_type_table,
        udt_defs,
    );
}

#[allow(clippy::too_many_arguments)]
fn parse_variable_declaration_line(
    line: &str,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
) -> bool {
    if strip_variable_declaration_prefix_ci(line).is_none() {
        return false;
    }
    parse_declaration(
        line,
        declarations,
        declaration_types,
        duplicate_declarations,
        array_bounds,
        option_base,
        default_type_table,
        udt_defs,
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn parse_declaration_remainder(
    remainder: &str,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
) {
    let remainder = if remainder.len() >= 11 && remainder[..11].eq_ignore_ascii_case("withevents ")
    {
        remainder[11..].trim_start()
    } else {
        remainder
    };
    let first_decl = split_first_decl_segment(remainder)
        .unwrap_or_default()
        .trim();
    let (name_part, explicit_ty, explicit_udt_name) =
        if let Some((lhs, rhs)) = split_keyword_ci(first_decl, "as") {
            let explicit_raw = rhs.trim();
            let primitive = parse_declared_type(explicit_raw);
            let udt_name = if primitive.is_none() {
                normalize_ident(explicit_raw)
            } else {
                None
            };
            (
                lhs.trim(),
                Some(primitive.unwrap_or(BoundType::Variant)),
                udt_name,
            )
        } else {
            (first_decl, None, None)
        };

    if let Some((base, type_char_ty, bounds)) = parse_array_declaration(name_part, option_base) {
        let declared_ty =
            resolve_declared_type(&base, explicit_ty, type_char_ty, default_type_table);
        if declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&base))
            || declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&format!("{base}_0")))
            || array_bounds.contains_key(&base)
        {
            duplicate_declarations.push(base);
            return;
        }
        array_bounds.insert(base.clone(), bounds.clone());
        if bounds.is_empty() {
            declarations.push(base.clone());
            declaration_types.insert(base.clone(), BoundType::Array);
            declaration_types.insert(format!("{base}_0"), declared_ty);
            return;
        }
        let Some(element_count) = array_element_count(&bounds) else {
            duplicate_declarations.push(base);
            return;
        };
        declarations.push(base.clone());
        declaration_types.insert(base.clone(), BoundType::Array);
        for idx in 0..element_count {
            let alias = format!("{base}_{idx}");
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&alias))
            {
                declarations.push(alias.clone());
            }
            declaration_types.insert(alias, declared_ty);
        }
        return;
    }

    if let Some((name, type_char_ty)) = normalize_ident_with_type_char(name_part) {
        let declared_ty =
            resolve_declared_type(&name, explicit_ty, type_char_ty, default_type_table);
        if declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            duplicate_declarations.push(name);
            return;
        }
        declaration_types.insert(name.clone(), declared_ty);
        declarations.push(name.clone());

        if let Some(udt_name) = explicit_udt_name.as_deref() {
            insert_udt_type_marker(declaration_types, &name, udt_name);
        }

        if let Some(udt_name) = explicit_udt_name
            && let Some(fields) = udt_defs.get(&udt_name)
        {
            for field in fields {
                if let Some(ref bounds) = field.array_bounds {
                    // Expand array-bounded UDT fields as indexed slot aliases.
                    for &(lo, hi) in bounds {
                        for idx in lo..=hi {
                            let alias = format!("{name}_{}_{idx}", field.name);
                            if declarations
                                .iter()
                                .any(|existing| existing.eq_ignore_ascii_case(&alias))
                            {
                                duplicate_declarations.push(alias);
                                continue;
                            }
                            declarations.push(alias.clone());
                            declaration_types.insert(alias, field.bound_type);
                        }
                    }
                } else {
                    let alias = format!("{name}_{}", field.name);
                    if declarations
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&alias))
                    {
                        duplicate_declarations.push(alias);
                        continue;
                    }
                    declarations.push(alias.clone());
                    declaration_types.insert(alias, field.bound_type);
                }
            }
        }
    }
}

fn strip_variable_declaration_prefix_ci(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    strip_keyword_prefix_ci(trimmed, "dim")
        .or_else(|| strip_keyword_prefix_ci(trimmed, "public"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "private"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "global"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "static"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "friend"))
        .filter(|remainder| {
            let lower = remainder.to_ascii_lowercase();
            !lower.starts_with("sub ")
                && !lower.starts_with("function ")
                && !lower.starts_with("property ")
                && !lower.starts_with("const ")
                && !lower.starts_with("declare ")
                && !lower.starts_with("enum ")
                && !lower.starts_with("type ")
                && !lower.starts_with("event ")
        })
}

fn split_first_decl_segment(text: &str) -> Option<&str> {
    let mut depth = 0i32;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(text[..idx].trim()),
            _ => {}
        }
    }
    Some(text.trim())
}

fn parse_array_declaration(token: &str, option_base: i32) -> Option<ParsedArrayDecl> {
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open {
        return None;
    }
    let (base, type_char_ty) = normalize_ident_with_type_char(token[..open].trim())?;
    let bounds = parse_array_bounds_spec(token[open + 1..close].trim(), option_base)?;
    Some((base, type_char_ty, bounds))
}

fn parse_array_bounds_spec(raw: &str, option_base: i32) -> Option<Vec<(i32, i32)>> {
    if raw.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut bounds = Vec::new();
    for dim in split_call_args(raw)? {
        let trimmed = dim.trim();
        if trimmed.is_empty() {
            return None;
        }
        let (lower, upper) = if let Some((lhs, rhs)) = split_keyword_ci(trimmed, "to") {
            let lower = lhs.trim().parse::<i32>().ok()?;
            let upper = rhs.trim().parse::<i32>().ok()?;
            (lower, upper)
        } else {
            let upper = trimmed.parse::<i32>().ok()?;
            (option_base, upper)
        };
        if upper < lower {
            return None;
        }
        bounds.push((lower, upper));
    }
    if bounds.is_empty() {
        return None;
    }
    Some(bounds)
}

fn array_element_count(bounds: &[(i32, i32)]) -> Option<usize> {
    let mut total = 1usize;
    for (lower, upper) in bounds {
        if upper < lower {
            return None;
        }
        let width = (*upper as i64 - *lower as i64 + 1) as usize;
        total = total.checked_mul(width)?;
    }
    Some(total)
}

fn linearize_array_index(bounds: &[(i32, i32)], indices: &[i32]) -> Option<usize> {
    if bounds.len() != indices.len() {
        return None;
    }
    let mut offset = 0usize;
    let mut stride = 1usize;
    for dim in (0..bounds.len()).rev() {
        let (lower, upper) = bounds[dim];
        let idx = indices[dim];
        if idx < lower || idx > upper {
            return None;
        }
        let normalized = (idx - lower) as usize;
        offset = offset.checked_add(normalized.checked_mul(stride)?)?;
        let width = (upper as i64 - lower as i64 + 1) as usize;
        stride = stride.checked_mul(width)?;
    }
    Some(offset)
}

fn parse_declared_type(token: &str) -> Option<BoundType> {
    match token.trim().to_ascii_lowercase().as_str() {
        "variant" => Some(BoundType::Variant),
        "integer" => Some(BoundType::Integer),
        "long" => Some(BoundType::Long),
        "longlong" => Some(BoundType::LongLong),
        "longptr" => Some(BoundType::LongPtr),
        "byte" => Some(BoundType::Byte),
        "single" => Some(BoundType::Single),
        "double" => Some(BoundType::Double),
        "currency" => Some(BoundType::Currency),
        "decimal" => Some(BoundType::Decimal),
        "date" => Some(BoundType::Date),
        "string" => Some(BoundType::String),
        "boolean" => Some(BoundType::Boolean),
        "object" => Some(BoundType::Object),
        _ => None,
    }
}

fn parse_fixed_string_declared_type(token: &str) -> Option<usize> {
    let (kind, len) = token.trim().split_once('*')?;
    if !kind.trim().eq_ignore_ascii_case("string") {
        return None;
    }
    len.trim().parse::<usize>().ok().filter(|value| *value > 0)
}

fn parse_reference_name(token: &str, array_bounds: &ArrayBoundsMap) -> Option<String> {
    if let Some(alias) = parse_err_member_reference(token) {
        return Some(alias);
    }
    if let Some(alias) = parse_array_reference(token, array_bounds) {
        return Some(alias);
    }
    if let Some(alias) = parse_member_reference(token) {
        return Some(alias);
    }
    normalize_ident(token)
}

fn parse_err_member_reference(token: &str) -> Option<String> {
    let trimmed = token.trim();
    let mut parts = trimmed.split('.');
    let root = parts.next()?.trim();
    let member = parts.next()?.trim();
    if parts.next().is_some() || !root.eq_ignore_ascii_case("err") {
        return None;
    }

    match member.to_ascii_lowercase().as_str() {
        "number" => Some("err_number".to_string()),
        "description" => Some("err_description".to_string()),
        "source" => Some("err_source".to_string()),
        "helpcontext" => Some("err_helpcontext".to_string()),
        "helpfile" => Some("err_helpfile".to_string()),
        "lastdllerror" => Some("err_lastdllerror".to_string()),
        _ => None,
    }
}

fn parse_array_reference(token: &str, array_bounds: &ArrayBoundsMap) -> Option<String> {
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open {
        return None;
    }
    let base = normalize_ident(token[..open].trim())?;
    let bounds = array_bounds.get(&base)?;
    let indices = split_call_args(token[open + 1..close].trim())?
        .iter()
        .map(|text| text.trim().parse::<i32>().ok())
        .collect::<Option<Vec<_>>>()?;
    let linear = linearize_array_index(bounds, &indices)?;
    Some(format!("{base}_{linear}"))
}

fn parse_member_reference(token: &str) -> Option<String> {
    let trimmed = token.trim();
    if !trimmed.contains('.') {
        return None;
    }
    normalize_member_chain(trimmed)
}

fn normalize_member_chain(text: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in text.split('.') {
        let normalized = normalize_ident(part)?;
        parts.push(normalized);
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("_"))
}

pub fn normalize_ident(text: &str) -> Option<String> {
    normalize_ident_with_type_char(text).map(|(name, _)| name)
}

fn normalize_ident_with_type_char(text: &str) -> Option<(String, Option<BoundType>)> {
    let token = text.trim().trim_end_matches(',').trim();
    if token.is_empty() {
        return None;
    }
    if token.contains(char::is_whitespace) {
        return None;
    }

    let (core_token, type_char_ty) = split_type_char(token);
    let mut chars = core_token.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((core_token.to_ascii_lowercase(), type_char_ty))
}

fn split_type_char(token: &str) -> (&str, Option<BoundType>) {
    let Some(last) = token.chars().last() else {
        return (token, None);
    };
    let Some(ty) = type_declaration_char(last) else {
        return (token, None);
    };
    let cutoff = token.len() - last.len_utf8();
    (&token[..cutoff], Some(ty))
}

fn find_matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_declared_or_array_type(token: &str) -> Option<BoundType> {
    let trimmed = token.trim();
    if trimmed.ends_with("()") {
        return Some(BoundType::Array);
    }
    parse_declared_type(trimmed)
}

fn type_declaration_char(ch: char) -> Option<BoundType> {
    match ch {
        '%' => Some(BoundType::Integer),
        '&' => Some(BoundType::Long),
        '^' => Some(BoundType::LongLong),
        '!' => Some(BoundType::Single),
        '#' => Some(BoundType::Double),
        '@' => Some(BoundType::Currency),
        '$' => Some(BoundType::String),
        _ => None,
    }
}

fn resolve_declared_type(
    name: &str,
    explicit_ty: Option<BoundType>,
    type_char_ty: Option<BoundType>,
    default_type_table: &[BoundType; 26],
) -> BoundType {
    if let Some(ty) = explicit_ty {
        return ty;
    }
    if let Some(ty) = type_char_ty {
        return ty;
    }
    default_type_for_name(name, default_type_table)
}

fn default_type_for_name(name: &str, default_type_table: &[BoundType; 26]) -> BoundType {
    let Some(first) = name.chars().next() else {
        return BoundType::Variant;
    };
    if !first.is_ascii_alphabetic() {
        return BoundType::Variant;
    }
    let idx = (first.to_ascii_lowercase() as u8 - b'a') as usize;
    default_type_table
        .get(idx)
        .copied()
        .unwrap_or(BoundType::Variant)
}

fn collect_option_compare_mode(lines: &[String]) -> BoundCompareMode {
    let mut mode = BoundCompareMode::Binary;
    for line in lines {
        if let Some(parsed) = parse_option_compare_directive(line) {
            mode = parsed;
        }
    }
    mode
}

pub(crate) fn collect_option_base(lines: &[String]) -> i32 {
    let mut base = 0i32;
    for line in lines {
        if let Some(parsed) = parse_option_base_directive(line) {
            base = parsed;
        }
    }
    base
}

fn parse_option_base_directive(line: &str) -> Option<i32> {
    let trimmed = line.trim();
    let tail = strip_keyword_prefix_ci(trimmed, "option base")?;
    match tail {
        "0" => Some(0),
        "1" => Some(1),
        _ => None,
    }
}

fn parse_option_compare_directive(line: &str) -> Option<BoundCompareMode> {
    let trimmed = line.trim();
    let tail = strip_keyword_prefix_ci(trimmed, "option compare")?;
    match tail.to_ascii_lowercase().as_str() {
        "binary" => Some(BoundCompareMode::Binary),
        "text" => Some(BoundCompareMode::Text),
        "database" => Some(BoundCompareMode::Database),
        _ => None,
    }
}

fn collect_default_type_table(lines: &[String]) -> [BoundType; 26] {
    let mut table = [BoundType::Variant; 26];
    for line in lines {
        if let Some((ty, indices)) = parse_def_type_directive(line) {
            for idx in indices {
                table[idx] = ty;
            }
        }
    }
    table
}

fn parse_def_type_directive(line: &str) -> Option<(BoundType, Vec<usize>)> {
    let trimmed = line.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let keyword = parts.next()?.to_ascii_lowercase();
    let spec = parts.next()?.trim();
    let ty = match keyword.as_str() {
        "defbool" => BoundType::Boolean,
        "defbyte" => BoundType::Byte,
        "defint" => BoundType::Integer,
        "deflng" => BoundType::Long,
        "deflnglng" => BoundType::LongLong,
        "deflngptr" => BoundType::LongPtr,
        "defsng" => BoundType::Single,
        "defdbl" => BoundType::Double,
        "defdec" => BoundType::Decimal,
        "defcur" => BoundType::Currency,
        "defdate" => BoundType::Date,
        "defstr" => BoundType::String,
        "defobj" => BoundType::Object,
        "defvar" => BoundType::Variant,
        _ => return None,
    };

    let mut indices = Vec::new();
    for raw in spec.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            return None;
        }
        if let Some((lhs, rhs)) = token.split_once('-') {
            let start = parse_letter_index(lhs.trim())?;
            let end = parse_letter_index(rhs.trim())?;
            let (from, to) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            for idx in from..=to {
                indices.push(idx);
            }
        } else {
            indices.push(parse_letter_index(token)?);
        }
    }
    Some((ty, indices))
}

fn parse_letter_index(token: &str) -> Option<usize> {
    let mut chars = token.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || !ch.is_ascii_alphabetic() {
        return None;
    }
    Some((ch.to_ascii_lowercase() as u8 - b'a') as usize)
}

fn build_array_descriptors(
    array_bounds: &ArrayBoundsMap,
    declaration_types: &HashMap<String, BoundType>,
    body: &[BoundStmt],
    option_base: i32,
) -> HashMap<String, BoundArrayDescriptor> {
    let mut redim_targets = HashSet::new();
    collect_redim_targets(body, &mut redim_targets);

    let mut descriptors = HashMap::new();
    for (name, bounds) in array_bounds {
        let element_alias = format!("{name}_0");
        let element_type = declaration_types
            .get(&element_alias)
            .copied()
            .unwrap_or(BoundType::Variant);
        descriptors.insert(
            name.clone(),
            BoundArrayDescriptor {
                element_type,
                rank: bounds.len().max(1),
                bounds: bounds.clone(),
                dynamic: bounds.is_empty() || redim_targets.contains(name),
                option_base,
            },
        );
    }
    descriptors
}

fn build_udt_descriptors(
    declarations: &[String],
    declaration_types: &HashMap<String, BoundType>,
    udt_defs: &UdtDefMap,
) -> Vec<BoundUdtDescriptor> {
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for name in declarations {
        if let Some(udt_name) = declared_udt_type_for_variable(name, declaration_types) {
            grouped.entry(udt_name).or_default().push(name.clone());
        }
    }

    let mut required_type_names = grouped.keys().cloned().collect::<HashSet<_>>();
    loop {
        let before = required_type_names.len();
        for type_name in required_type_names.clone() {
            if let Some(fields) = udt_defs.get(&type_name) {
                for field in fields {
                    if let Some(nested_name) = &field.nested_udt_name
                        && udt_defs.contains_key(nested_name)
                    {
                        required_type_names.insert(nested_name.clone());
                    }
                }
            }
        }
        if required_type_names.len() == before {
            break;
        }
    }

    let mut descriptors = required_type_names
        .into_iter()
        .filter_map(|type_name| {
            let mut variable_names = grouped.remove(&type_name).unwrap_or_default();
            variable_names.sort();
            variable_names.dedup();
            let fields = udt_defs.get(&type_name)?;
            Some(BoundUdtDescriptor {
                type_name,
                variable_names,
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| BoundUdtFieldDescriptor {
                        index,
                        name: field.name.clone(),
                        bound_type: field.bound_type,
                        nested_udt_name: field.nested_udt_name.clone(),
                        array_bounds: field.array_bounds.clone(),
                        fixed_string_len: field.fixed_string_len,
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| left.type_name.cmp(&right.type_name));
    descriptors
}

fn collect_redim_targets(stmts: &[BoundStmt], targets: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            BoundStmt::ReDim { name, .. } => {
                targets.insert(name.clone());
            }
            BoundStmt::ReDimRuntime { name, .. } => {
                targets.insert(name.clone());
            }
            BoundStmt::IfCond {
                then_body,
                else_body,
                ..
            } => {
                collect_redim_targets(then_body, targets);
                collect_redim_targets(else_body, targets);
            }
            BoundStmt::ForRange { body, .. } | BoundStmt::DoWhile { body, .. } => {
                collect_redim_targets(body, targets);
            }
            BoundStmt::ForEach { body, .. } => {
                collect_redim_targets(body, targets);
            }
            BoundStmt::SelectCase {
                arms, else_body, ..
            } => {
                for (_, body) in arms {
                    collect_redim_targets(body, targets);
                }
                collect_redim_targets(else_body, targets);
            }
            _ => {}
        }
    }
}

fn parse_line_number_statement(line: &str) -> Option<(String, Option<String>)> {
    let trimmed = line.trim_start();
    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 {
        return None;
    }
    let rest_slice = &trimmed[digit_count..];
    if !rest_slice.is_empty()
        && rest_slice
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace())
    {
        return None;
    }
    let line_no = trimmed[..digit_count].parse::<i32>().ok()?;
    if line_no < 0 {
        return None;
    }
    let label = line_number_label(line_no);
    let rest = rest_slice.trim();
    if rest.is_empty() {
        Some((label, None))
    } else {
        Some((label, Some(rest.to_string())))
    }
}

fn parse_label_declaration(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.ends_with(':') {
        return None;
    }
    parse_jump_target_label(&trimmed[..trimmed.len() - 1])
}

fn parse_jump_target_label(raw: &str) -> Option<String> {
    let token = raw.trim();
    if token.is_empty() {
        return None;
    }
    if let Ok(line_no) = token.parse::<i32>()
        && line_no >= 0
    {
        return Some(line_number_label(line_no));
    }
    normalize_ident(token)
}

fn line_number_label(line_no: i32) -> String {
    format!("__line_{line_no}")
}

fn matches_terminator(lower_line: &str, terminators: &[&str]) -> bool {
    terminators.iter().any(|term| {
        if *term == "next" {
            lower_line == "next" || lower_line.starts_with("next ")
        } else if *term == "loop" {
            lower_line == "loop" || lower_line.starts_with("loop ")
        } else if *term == "case" {
            lower_line.starts_with("case ")
        } else if *term == "elseif" {
            lower_line.starts_with("elseif ")
        } else {
            lower_line == *term
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn parse_if_tail(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    duplicate_declarations: &mut Vec<String>,
    array_bounds: &mut ArrayBoundsMap,
    option_explicit: &mut bool,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    udt_defs: &UdtDefMap,
    module_constants: &ModuleConstMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
) -> Option<Vec<BoundStmt>> {
    if *index >= lines.len() {
        return None;
    }

    let line = lines[*index].as_str();
    let lower = line.to_ascii_lowercase();

    if lower == "end if" {
        *index += 1;
        return Some(Vec::new());
    }

    if lower == "else" {
        *index += 1;
        let else_body = parse_block(
            lines,
            index,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_explicit,
            option_base,
            default_type_table,
            udt_defs,
            module_constants,
            property_write_routes,
            property_read_routes,
            &["end if"],
        );
        if *index < lines.len() && lines[*index].eq_ignore_ascii_case("end if") {
            *index += 1;
            return Some(else_body);
        }
        return None;
    }

    if lower.starts_with("elseif ") && lower.ends_with(" then") {
        let condition = line[6..line.len() - 4].trim();
        let cond = parse_condition(condition, array_bounds)?;
        *index += 1;
        let then_body = parse_block(
            lines,
            index,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_explicit,
            option_base,
            default_type_table,
            udt_defs,
            module_constants,
            property_write_routes,
            property_read_routes,
            &["elseif", "else", "end if"],
        );
        let nested_else = parse_if_tail(
            lines,
            index,
            declarations,
            declaration_types,
            duplicate_declarations,
            array_bounds,
            option_explicit,
            option_base,
            default_type_table,
            udt_defs,
            module_constants,
            property_write_routes,
            property_read_routes,
        )?;
        return Some(vec![BoundStmt::IfCond {
            cond,
            then_body,
            else_body: nested_else,
        }]);
    }

    None
}

fn split_ci<'a>(text: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let idx = lower.find(marker)?;
    let lhs = text[..idx].trim();
    let rhs = text[idx + marker.len()..].trim();
    Some((lhs, rhs))
}

fn split_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let marker = format!(" {keyword} ");
    let idx = lower.find(&marker)?;
    let lhs = text[..idx].trim();
    let rhs = text[idx + marker.len()..].trim();
    Some((lhs, rhs))
}

fn strip_keyword_prefix_ci<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    let marker = format!("{keyword} ");
    if lower.starts_with(&marker) {
        Some(text[marker.len()..].trim())
    } else {
        None
    }
}

fn strip_proc_scope_prefixes_ci(mut text: &str) -> &str {
    loop {
        if let Some(stripped) = strip_keyword_prefix_ci(text, "public") {
            text = stripped;
            continue;
        }
        if let Some(stripped) = strip_keyword_prefix_ci(text, "private") {
            text = stripped;
            continue;
        }
        if let Some(stripped) = strip_keyword_prefix_ci(text, "friend") {
            text = stripped;
            continue;
        }
        if let Some(stripped) = strip_keyword_prefix_ci(text, "static") {
            text = stripped;
            continue;
        }
        return text.trim();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArithOp, BoundCompareMode, BoundCond, BoundExpr, BoundStmt, CompareOp, IntrinsicSurface,
        UDT_TYPE_MARKER_PREFIX, intrinsic_surface, resolve_symbols,
    };

    #[test]
    fn resolve_if_statement_into_structured_body() {
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = x + 2\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.declarations, vec!["x"]);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::IfCond { .. }))
        );
    }

    #[test]
    fn resolve_for_loop_into_structured_body() {
        let source = "Sub Main()\nDim x\nDim i\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.declarations, vec!["x", "i"]);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::ForRange { .. }))
        );
    }

    #[test]
    fn resolve_variable_copy_expression() {
        let source = "Sub Main()\nDim x\nDim y\nx = y\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(expr, &BoundExpr::Var("y".to_string()));
    }

    #[test]
    fn resolve_line_continuation_assignment_expression() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + _\n2\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.get(1) else {
            panic!("expected assignment");
        };
        assert_eq!(
            expr,
            &BoundExpr::AddConst {
                var: "x".to_string(),
                delta: 2,
            }
        );
    }

    #[test]
    fn resolve_with_block_member_assignments() {
        let source =
            "Sub Main()\nDim x\nWith x\n.Value = 1\n.Value = .Value + 2\nEnd With\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(target, "x_value");
        assert_eq!(expr, &BoundExpr::IntConst(1));
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.get(1) else {
            panic!("expected second assignment");
        };
        assert_eq!(target, "x_value");
        assert_eq!(
            expr,
            &BoundExpr::AddConst {
                var: "x_value".to_string(),
                delta: 2,
            }
        );
    }

    #[test]
    fn resolve_nested_with_block_member_assignments() {
        let source =
            "Sub Main()\nDim x\nWith x\nWith .inner\n.Value = 9\nEnd With\nEnd With\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { target, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(target, "x_inner_value");
    }

    #[test]
    fn resolve_with_block_direct_member_target_assignment() {
        let source = "Sub Main()\nDim x\nWith x.inner\n.Value = 4\n.Value = .Value + 3\nx = .Value\nEnd With\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(target, "x_inner_value");
        assert_eq!(expr, &BoundExpr::IntConst(4));
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.get(1) else {
            panic!("expected second assignment");
        };
        assert_eq!(target, "x_inner_value");
        assert_eq!(
            expr,
            &BoundExpr::AddConst {
                var: "x_inner_value".to_string(),
                delta: 3,
            }
        );
    }

    #[test]
    fn resolve_conditional_compilation_if_else_branch() {
        let source = "#Const ENABLE = True\nSub Main()\nDim x\n#If ENABLE Then\nx = 7\n#Else\nx = 1\n#End If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(target, "x");
        assert_eq!(expr, &BoundExpr::IntConst(7));
    }

    #[test]
    fn resolve_conditional_compilation_elseif_branch() {
        let source = "#Const A = False\n#Const B = True\nSub Main()\nDim x\n#If A Then\nx = 1\n#ElseIf B Then\nx = 9\n#Else\nx = 3\n#End If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(target, "x");
        assert_eq!(expr, &BoundExpr::IntConst(9));
    }

    #[test]
    fn resolve_intrinsic_vb_constants_to_literals() {
        // Always-available vbConstants must bind to their literal values under
        // Option Explicit rather than reporting "use of undeclared variable".
        let source = "Option Explicit\nSub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\n\
            a = vbBinaryCompare\nb = vbFromUnicode\nc = vbCrLf\nd = vbObjectError\ne = vbYesNo\nEnd Sub";
        let module = resolve_symbols(source);
        let expect = |idx: usize, want: &BoundExpr| {
            let Some(BoundStmt::Assign { expr, .. }) = module.body.get(idx) else {
                panic!("expected assignment at {idx}");
            };
            assert_eq!(expr, want, "binding mismatch at statement {idx}");
        };
        expect(0, &BoundExpr::IntConst(0)); // vbBinaryCompare
        expect(1, &BoundExpr::IntConst(128)); // vbFromUnicode
        expect(2, &BoundExpr::StringConst("\r\n".to_string())); // vbCrLf
        expect(3, &BoundExpr::IntConst(-2147221504)); // vbObjectError
        expect(4, &BoundExpr::IntConst(4)); // vbYesNo
    }

    #[test]
    fn resolve_conditional_compilation_skips_inactive_const_definition() {
        let source = "#Const OUTER = False\n#If OUTER Then\n#Const FLAG = True\n#End If\nSub Main()\nDim x\n#If FLAG Then\nx = 1\n#Else\nx = 5\n#End If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(target, "x");
        assert_eq!(expr, &BoundExpr::IntConst(5));
    }

    #[test]
    fn resolve_intrinsic_surface_classification() {
        assert_eq!(
            intrinsic_surface("Len"),
            Some(IntrinsicSurface::DeterministicCore)
        );
        assert_eq!(
            intrinsic_surface("Date"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("FreeFile"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("EOF"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("Seek"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("MsgBox"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("DoEvents"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(
            intrinsic_surface("Shell"),
            Some(IntrinsicSurface::HostSensitive)
        );
        assert_eq!(intrinsic_surface("UnknownIntrinsic"), None);
    }

    #[test]
    fn resolve_intrinsic_conversion_expression() {
        let source = "Sub Main()\nDim x\nx = CLng(CInt(7))\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(expr, &BoundExpr::IntConst(7));
    }

    #[test]
    fn resolve_cverr_preserves_intrinsic_call_node() {
        let source = "Sub Main()\nDim x\nx = CVErr(7)\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "cverr" && args == &vec![BoundExpr::IntConst(7)]
        ));
    }

    #[test]
    fn resolve_stdlib_intrinsic_call_expression() {
        let source = "Sub Main()\nDim x\nx = Len(1234)\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "len" && args.len() == 1
        ));
    }

    #[test]
    fn resolve_dir_continuation_intrinsic_call_expression() {
        let source = "Sub Main()\nDim x\nx = Dir()\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "dir" && args.is_empty()
        ));
    }

    #[test]
    fn resolve_stdlib_dollar_suffix_intrinsic_call_expression() {
        let source = "Sub Main()\nDim x\nx = Left$(1234, 2)\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "left" && args.len() == 2
        ));
    }

    #[test]
    fn resolve_mid_statement_assignment() {
        let source = "Sub Main()\nDim x\nx = 12345\nMid(x, 2, 2) = 99\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::MidAssign { target, .. }) = module.body.get(1) else {
            panic!("expected mid assignment");
        };
        assert_eq!(target, "x");
    }

    #[test]
    fn resolve_stdlib_advanced_intrinsic_call_expression() {
        let source = "Sub Main()\nDim x\nx = Replace(12345, 23, 67)\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "replace" && args.len() == 3
        ));
    }

    #[test]
    fn resolve_stdlib_date_math_intrinsic_expression() {
        let source = "Sub Main()\nDim x\nx = DateAdd(1, 2, DateSerial(2026, 2, 28))\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "dateadd" && args.len() == 3
        ));
    }

    #[test]
    fn resolve_stdlib_collection_intrinsic_expression() {
        let source = "Sub Main()\nDim x\nx = CollectionCount(CollectionAdd(0, 7, 0))\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "collectioncount" && args.len() == 1
        ));
    }

    #[test]
    fn resolve_if_with_boolean_operators() {
        let source = "Sub Main()\nDim x\nIf Not x = 0 Or x < 2 Then\nx = 1\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::IfCond { cond, .. }) = module.body.first() else {
            panic!("expected if");
        };
        assert!(matches!(cond, BoundCond::Or(_, _)));
    }

    #[test]
    fn resolve_if_with_non_eq_comparison() {
        let source = "Sub Main()\nDim x\nIf x >= 1 Then\nx = 2\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::IfCond { cond, .. }) = module.body.first() else {
            panic!("expected if");
        };
        assert!(matches!(
            cond,
            BoundCond::Compare {
                op: CompareOp::Ge,
                ..
            }
        ));
    }

    #[test]
    fn resolve_like_condition_as_compare_op() {
        let source = "Sub Main()\nDim x\nIf 12 Like 12 Then\nx = 1\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::IfCond { cond, .. }) = module.body.first() else {
            panic!("expected if");
        };
        assert!(matches!(
            cond,
            BoundCond::Compare {
                op: CompareOp::Like,
                ..
            }
        ));
    }

    #[test]
    fn resolve_option_compare_text_directive_sets_module_mode() {
        let source = "Option Compare Text\nSub Main()\nDim x\nx = StrComp(1, 1)\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.compare_mode, BoundCompareMode::Text);
    }

    #[test]
    fn resolve_if_else_if_else_chain() {
        let source = "Sub Main()\nDim x\nIf x = 1 Then\nx = 2\nElseIf x = 2 Then\nx = 3\nElse\nx = 4\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::IfCond { else_body, .. }) = module.body.first() else {
            panic!("expected if");
        };
        assert!(!else_body.is_empty());
    }

    #[test]
    fn resolve_single_line_if_statement_routes_inline_call_body() {
        let source = "Sub Main()\nIf 1 = 1 Then Err.Raise 5\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::IfCond {
            then_body,
            else_body,
            ..
        }) = module.body.first()
        else {
            panic!("expected if");
        };
        assert!(else_body.is_empty());
        let Some(BoundStmt::RaiseError(code)) = then_body.first() else {
            panic!("expected inline raise body");
        };
        assert_eq!(*code, 5);
    }

    #[test]
    fn resolve_single_line_if_statement_routes_inline_exit_do_body() {
        let source =
            "Sub Main()\nDim x\nDo While x < 5\nx = x + 1\nIf x = 3 Then Exit Do\nLoop\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::DoWhile { body, .. }) = module.body.first() else {
            panic!("expected do while");
        };
        let Some(BoundStmt::IfCond { then_body, .. }) =
            body.iter().find(|s| matches!(s, BoundStmt::IfCond { .. }))
        else {
            panic!("expected inline if");
        };
        assert!(matches!(then_body.first(), Some(BoundStmt::ExitDo)));
    }

    #[test]
    fn resolve_do_while_and_exit_do() {
        let source = "Sub Main()\nDim x\nDo While x < 5\nx = x + 1\nExit Do\nLoop\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::DoWhile { .. }))
        );
    }

    #[test]
    fn resolve_vbnullstring_intrinsic_constant_expression() {
        let source = "Sub Main()\nDim x\nx = vbNullString\nEnd Sub";
        let module = resolve_symbols(source);
        let BoundStmt::Assign { expr, .. } = &module.body[0] else {
            panic!("expected assignment");
        };
        assert!(matches!(
            expr,
            BoundExpr::IntrinsicCall { name, args } if name == "vbnullstring" && args.is_empty()
        ));
    }

    #[test]
    fn resolve_select_case_statement() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase 1\nx = 10\nCase 2, 3\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::SelectCase { .. }))
        );
    }

    #[test]
    fn resolve_named_procedures_and_call_sites() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall Foo\nEnd Sub\nSub Foo()\nDim y\ny = 2\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.procedures.len(), 2);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure should exist");
        assert!(
            main_proc
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::Call { name, .. } if name == "foo"))
        );
    }

    #[test]
    fn resolve_procedure_params_and_call_args() {
        let source = "Sub Main()\nDim x\nx = 1\nCall AddOne(x)\nEnd Sub\nSub AddOne(ByRef a)\na = a + 1\nEnd Sub";
        let module = resolve_symbols(source);
        let add_one = module
            .procedures
            .iter()
            .find(|p| p.name == "addone")
            .expect("addone procedure expected");
        assert_eq!(add_one.params.len(), 1);
        assert_eq!(add_one.params[0].name, "a");
        assert!(add_one.params[0].by_ref);
        assert!(!add_one.params[0].optional);
        assert_eq!(add_one.params[0].default_value, None);
    }

    #[test]
    fn resolve_optional_params_with_default_literals() {
        let source = "Sub Main()\nDim x\nCall Fill(x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let module = resolve_symbols(source);
        let fill = module
            .procedures
            .iter()
            .find(|p| p.name == "fill")
            .expect("fill procedure expected");
        assert_eq!(fill.params.len(), 2);
        assert_eq!(fill.params[0].name, "target");
        assert!(fill.params[0].by_ref);
        assert!(!fill.params[0].optional);
        assert_eq!(fill.params[0].default_value, None);
        assert_eq!(fill.params[1].name, "value");
        assert!(!fill.params[1].by_ref);
        assert!(fill.params[1].optional);
        assert_eq!(fill.params[1].default_value, Some(7));
    }

    #[test]
    fn resolve_optional_byref_param_is_accepted() {
        // `Optional b As Long` is ByRef by default in VBA; the signature must be accepted
        // (regression: a prior `optional && by_ref => reject` rule made every such
        // procedure unresolvable, so callers reported "unknown procedure").
        let source = "Sub Main()\nFoo 1\nEnd Sub\nSub Foo(a As Long, Optional b As Long)\nEnd Sub";
        let module = resolve_symbols(source);
        let foo = module
            .procedures
            .iter()
            .find(|p| p.name == "foo")
            .expect("foo procedure should register with an Optional ByRef parameter");
        assert_eq!(foo.params.len(), 2);
        assert!(foo.params[1].optional);
        assert!(
            foo.params[1].by_ref,
            "Optional parameters default to ByRef and must be allowed"
        );
    }

    #[test]
    fn resolve_omitted_positional_arguments_bind_sentinel() {
        // `Foo 1, , 5` omits the middle optional argument via bare commas.
        let source = "Sub Main()\nFoo 1, , 5\nEnd Sub\nSub Foo(a, Optional b, Optional c)\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Call { name, args, .. }) = module.body.first() else {
            panic!("expected call statement, got {:?}", module.body.first());
        };
        assert_eq!(name, "foo");
        assert_eq!(args.len(), 3);
        assert_eq!(args[0].expr, BoundExpr::IntConst(1));
        assert!(
            super::is_omitted_argument_expr(&args[1].expr),
            "middle argument should bind the omitted sentinel, got {:?}",
            args[1].expr
        );
        assert_eq!(args[2].expr, BoundExpr::IntConst(5));
    }

    #[test]
    fn resolve_paramarray_signature_marks_last_param_as_array_pack() {
        let source = "Sub Main()\nCall Capture(1, 2, 3)\nEnd Sub\nSub Capture(ParamArray items() As Variant)\nEnd Sub";
        let module = resolve_symbols(source);
        let capture = module
            .procedures
            .iter()
            .find(|p| p.name == "capture")
            .expect("capture procedure expected");
        assert_eq!(capture.params.len(), 1);
        assert_eq!(capture.params[0].name, "items");
        assert!(capture.params[0].param_array);
        assert!(!capture.params[0].by_ref);
        assert_eq!(capture.params[0].ty, super::BoundType::Array);
    }

    #[test]
    fn resolve_typed_param_and_dim_declarations() {
        let source = "Sub Main(ByVal a As Integer)\nDim x As Long\nx = a\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(main_proc.params.len(), 1);
        assert_eq!(main_proc.params[0].name, "a");
        assert_eq!(main_proc.params[0].ty, super::BoundType::Integer);
        assert_eq!(
            main_proc
                .declaration_types
                .get("x")
                .copied()
                .expect("typed dim should be recorded"),
            super::BoundType::Long
        );
    }

    #[test]
    fn resolve_typed_array_dim_records_element_alias_types() {
        let source = "Sub Main()\nDim a(2) As Integer\na(1) = 7\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(
            main_proc.declaration_types.get("a_0").copied(),
            Some(super::BoundType::Integer)
        );
        assert_eq!(
            main_proc.declaration_types.get("a_2").copied(),
            Some(super::BoundType::Integer)
        );
    }

    #[test]
    fn resolve_array_descriptor_records_bounds_and_type() {
        let source = "Sub Main()\nDim a(2) As Integer\na(1) = 7\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let descriptor = main_proc
            .array_descriptors
            .get("a")
            .expect("array descriptor should be present");
        assert_eq!(descriptor.element_type, super::BoundType::Integer);
        assert_eq!(descriptor.rank, 1);
        assert_eq!(descriptor.bounds, vec![(0, 2)]);
        assert!(!descriptor.dynamic);
    }

    #[test]
    fn resolve_redim_marks_array_descriptor_dynamic() {
        let source = "Sub Main()\nDim a(1)\nReDim Preserve a(3)\na(3) = 5\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let descriptor = main_proc
            .array_descriptors
            .get("a")
            .expect("array descriptor should be present");
        assert_eq!(descriptor.bounds, vec![(0, 3)]);
        assert!(descriptor.dynamic);
    }

    #[test]
    fn resolve_redim_preserve_records_previous_bounds_snapshot() {
        let source = "Sub Main()\nDim a(0 To 3)\nReDim Preserve a(0 To 1)\nEnd Sub";
        let module = resolve_symbols(source);
        let stmt = module
            .body
            .iter()
            .find(|s| matches!(s, BoundStmt::ReDim { .. }))
            .expect("redim statement expected");
        let BoundStmt::ReDim {
            bounds,
            previous_bounds,
            preserve,
            ..
        } = stmt
        else {
            panic!("expected redim statement");
        };
        assert!(*preserve);
        assert_eq!(bounds, &vec![(0, 1)]);
        assert_eq!(previous_bounds, &Some(vec![(0, 3)]));
    }

    #[test]
    fn resolve_runtime_redim_expression_bounds_on_dynamic_array() {
        let source =
            "Sub Main()\nDim length As Long\nDim buf() As Byte\nReDim buf(length - 1)\nEnd Sub";
        let module = resolve_symbols(source);
        let stmt = module
            .body
            .iter()
            .find(|s| matches!(s, BoundStmt::ReDimRuntime { .. }))
            .expect("runtime redim statement expected");
        let BoundStmt::ReDimRuntime {
            name,
            preserve,
            bounds,
        } = stmt
        else {
            panic!("expected runtime redim statement");
        };
        assert_eq!(name, "buf");
        assert!(!preserve);
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].lower_bound, 0);
        assert!(matches!(
            bounds[0].upper_bound,
            BoundExpr::SubConst { ref var, delta } if var == "length" && delta == 1
        ));
    }

    #[test]
    fn resolve_option_base_one_applies_to_array_declaration_bounds() {
        let source = "Option Base 1\nSub Main()\nDim a(3)\na(1) = 7\na(3) = 9\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let descriptor = main_proc
            .array_descriptors
            .get("a")
            .expect("array descriptor should be present");
        assert_eq!(descriptor.rank, 1);
        assert_eq!(descriptor.bounds, vec![(1, 3)]);
        assert!(main_proc.declarations.iter().any(|d| d == "a_0"));
        assert!(main_proc.declarations.iter().any(|d| d == "a_2"));
    }

    #[test]
    fn resolve_explicit_lower_bound_maps_to_linear_slot_alias() {
        let source = "Sub Main()\nDim a(5 To 7)\na(6) = 4\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let descriptor = main_proc
            .array_descriptors
            .get("a")
            .expect("array descriptor should be present");
        assert_eq!(descriptor.bounds, vec![(5, 7)]);
        assert!(matches!(
            main_proc.body.first(),
            Some(BoundStmt::Assign { target, .. }) if target == "a_1"
        ));
    }

    #[test]
    fn resolve_multidim_reference_linearizes_indices() {
        let source = "Sub Main()\nDim m(1 To 2, 1 To 3)\nDim x\nm(2, 3) = 9\nx = m(2, 3)\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let descriptor = main_proc
            .array_descriptors
            .get("m")
            .expect("array descriptor should be present");
        assert_eq!(descriptor.rank, 2);
        assert_eq!(descriptor.bounds, vec![(1, 2), (1, 3)]);
        assert!(matches!(
            &main_proc.body[0],
            BoundStmt::Assign { target, .. } if target == "m_5"
        ));
        assert!(matches!(
            &main_proc.body[1],
            BoundStmt::Assign {
                expr: BoundExpr::Var(name),
                ..
            } if name == "m_5"
        ));
    }

    #[test]
    fn resolve_def_type_applies_to_untyped_dim() {
        let source = "DefLng A-Z\nSub Main()\nDim alpha\nalpha = 1\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(
            main_proc.declaration_types.get("alpha").copied(),
            Some(super::BoundType::Long)
        );
    }

    #[test]
    fn resolve_type_char_overrides_def_type_for_dim() {
        let source = "DefObj A-Z\nSub Main()\nDim alpha%\nalpha = 1\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(
            main_proc.declaration_types.get("alpha").copied(),
            Some(super::BoundType::Integer)
        );
    }

    #[test]
    fn resolve_explicit_as_overrides_type_char_and_def_type() {
        let source = "DefInt A-Z\nSub Main()\nDim alpha% As Object\nalpha = 1\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(
            main_proc.declaration_types.get("alpha").copied(),
            Some(super::BoundType::Object)
        );
    }

    #[test]
    fn resolve_def_type_and_type_char_precedence_for_params() {
        let source = "DefObj A-Z\nSub Use(alpha, beta%, gamma% As Object)\nEnd Sub";
        let module = resolve_symbols(source);
        let proc = module
            .procedures
            .iter()
            .find(|p| p.name == "use")
            .expect("use procedure expected");
        assert_eq!(proc.params.len(), 3);
        assert_eq!(proc.params[0].ty, super::BoundType::Object);
        assert_eq!(proc.params[1].ty, super::BoundType::Integer);
        assert_eq!(proc.params[2].ty, super::BoundType::Object);
    }

    #[test]
    fn resolve_function_return_type_uses_type_char_and_def_type_precedence() {
        let source = "DefObj A-Z\nFunction alpha%()\nalpha = 1\nEnd Function";
        let module = resolve_symbols(source);
        let proc = module
            .procedures
            .iter()
            .find(|p| p.name == "alpha")
            .expect("alpha function expected");
        assert_eq!(proc.return_type, super::BoundType::Integer);
        assert_eq!(
            proc.declaration_types.get("alpha").copied(),
            Some(super::BoundType::Integer)
        );
    }

    #[test]
    fn resolve_function_return_explicit_as_overrides_type_char_and_def_type() {
        let source = "DefInt A-Z\nFunction alpha%() As Object\nalpha = 1\nEnd Function";
        let module = resolve_symbols(source);
        let proc = module
            .procedures
            .iter()
            .find(|p| p.name == "alpha")
            .expect("alpha function expected");
        assert_eq!(proc.return_type, super::BoundType::Object);
        assert_eq!(
            proc.declaration_types.get("alpha").copied(),
            Some(super::BoundType::Object)
        );
    }

    #[test]
    fn resolve_duplicate_dim_is_recorded_for_diagnostics() {
        let source = "Sub Main()\nDim x\nDim x\nx = 1\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(
            main_proc.duplicate_declarations,
            vec!["x".to_string()],
            "duplicate dim should be captured for downstream diagnostics"
        );
    }

    #[test]
    fn resolve_duplicate_array_dim_is_recorded_for_diagnostics() {
        let source = "Sub Main()\nDim a(1) As Integer\nDim a(2) As Integer\na(0) = 1\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert_eq!(
            main_proc.duplicate_declarations,
            vec!["a".to_string()],
            "duplicate array dim should be captured for downstream diagnostics"
        );
    }

    #[test]
    fn resolve_named_call_arguments() {
        let source = "Sub Main()\nDim x\nCall Fill(target := x, value := 9)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let Some(BoundStmt::Call { args, .. }) = main_proc.body.first() else {
            panic!("expected call statement");
        };
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name.as_deref(), Some("target"));
        assert_eq!(args[1].name.as_deref(), Some("value"));
    }

    #[test]
    fn resolve_module_qualified_call_with_parentheses_normalizes_member_chain() {
        let source =
            "Sub Main()\nCall MathModule.Add(1, 2)\nEnd Sub\nSub Add(ByVal a, ByVal b)\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let Some(BoundStmt::Call { name, args, .. }) = main_proc.body.first() else {
            panic!("expected call statement");
        };
        assert_eq!(name, "mathmodule_add");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn resolve_module_qualified_call_without_parentheses_normalizes_member_chain() {
        let source = "Sub Main()\nCall MathModule.Ping\nEnd Sub\nSub Ping()\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let Some(BoundStmt::Call { name, args, .. }) = main_proc.body.first() else {
            panic!("expected call statement");
        };
        assert_eq!(name, "mathmodule_ping");
        assert!(args.is_empty());
    }

    #[test]
    fn resolve_statement_level_call_without_parentheses_preserves_arguments() {
        let source = "Sub Main()\nDim ptr\nDim length\nDim buf() As Byte\nRtlMoveMemory VarPtr(buf(0)), ptr, length\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        let Some(BoundStmt::Call { name, args, .. }) = main_proc.body.first() else {
            panic!("expected call statement");
        };
        assert_eq!(name, "rtlmovememory");
        assert_eq!(args.len(), 3);
        assert!(matches!(
            &args[0].expr,
            BoundExpr::IntrinsicCall { name, .. } if name == "varptr"
        ));
        assert!(matches!(&args[1].expr, BoundExpr::Var(var) if var == "ptr"));
        assert!(matches!(&args[2].expr, BoundExpr::Var(var) if var == "length"));
    }

    #[test]
    fn resolve_fixture_shaped_demo_module_keeps_later_testversion_procedure_visible() {
        let source = "Option Explicit\nDim TestFile As String\nPublic Sub AllTests()\nDim InitReturn As Long\n#If Win64 Then\nInitReturn = SQLite3Initialize(ThisWorkbook.Path + \"\\\\x64\")\n#Else\nInitReturn = SQLite3Initialize\n#End If\nIf InitReturn <> SQLITE_INIT_OK Then\nDebug.Print \"Error Initializing SQLite. Error: \" & Err.LastDllError\nExit Sub\nEnd If\nTestVersion\nEnd Sub\nPublic Sub TestVersion()\nDebug.Print SQLite3LibVersion()\nEnd Sub";
        let module = resolve_symbols(source);
        let names = module
            .procedures
            .iter()
            .map(|proc| proc.name.clone())
            .collect::<Vec<_>>();
        assert!(
            names.iter().any(|name| name == "alltests"),
            "expected alltests procedure, got {names:?}"
        );
        assert!(
            names.iter().any(|name| name == "testversion"),
            "expected testversion procedure, got {names:?}"
        );
    }

    #[test]
    fn normalize_source_lines_strips_inline_apostrophe_comments() {
        let lines = super::normalize_source_lines(
            "Sub Main()\nvalue = String(3, \"*\") ' trailing comment\nmessage = \"don't strip\"\nEnd Sub",
        );
        assert!(lines.iter().any(|line| line == "value = String(3, \"*\")"));
        assert!(lines.iter().any(|line| line == "message = \"don't strip\""));
    }

    #[test]
    fn resolve_module_const_injects_const_prelude() {
        let source = "Const BASE = 5\nSub Main()\nDim x\nx = BASE + 2\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "base"));
        let Some(BoundStmt::Assign { target, expr, .. }) = module.body.first() else {
            panic!("expected const prelude assignment");
        };
        assert_eq!(target, "base");
        assert_eq!(expr, &BoundExpr::IntConst(5));
    }

    #[test]
    fn resolve_enum_members_as_module_constants() {
        let source =
            "Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "fast"));
        assert!(module.declarations.iter().any(|d| d == "safe"));
        assert!(module.body.iter().any(
            |s| matches!(s, BoundStmt::Assign { target, expr, .. } if target == "safe" && expr == &BoundExpr::IntConst(4))
        ));
    }

    #[test]
    fn resolve_udt_block_is_accepted_and_ignored_for_mvp() {
        let source =
            "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim x\nx = 9\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "x"));
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::Assign { target, .. } if target == "x"))
        );
    }

    #[test]
    fn resolve_udt_whole_assignment_emits_struct_copy_stmt() {
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim a As Point\nDim b As Point\na.X = 7\na.Y = 9\nb = a\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| {
            matches!(
                stmt,
                BoundStmt::UdtAssign {
                    target,
                    source,
                    fields
                } if target == "b" && source == "a" && fields == &vec!["x".to_string(), "y".to_string()]
            )
        }));
    }

    #[test]
    fn resolve_udt_cross_type_whole_assignment_is_unsupported() {
        let source = "Type PairA\nX As Integer\nY As Integer\nEnd Type\nType PairB\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim a As PairA\nDim b As PairB\nb = a\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| {
            matches!(
                stmt,
                BoundStmt::Unsupported { line }
                    if line.contains("cross-type UDT assignment")
            )
        }));
        assert!(
            module
                .declaration_types
                .keys()
                .all(|name| !name.starts_with(UDT_TYPE_MARKER_PREFIX))
        );
    }

    #[test]
    fn resolve_property_let_assignment_routes_to_property_call() {
        let source = "Sub Main()\nDim x\nx = 1\nValue = x\nEnd Sub\nProperty Let Value(ByRef target)\ntarget = target + 2\nEnd Property";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert!(
            main_proc
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::Call { name, .. } if name == "property_let_value"))
        );
    }

    #[test]
    fn resolve_property_set_assignment_routes_to_property_call() {
        let source = "Sub Main()\nDim x\nx = 2\nObj = x\nEnd Sub\nProperty Set Obj(ByRef target)\ntarget = target + 5\nEnd Property";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        assert!(
            main_proc
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::Call { name, .. } if name == "property_set_obj"))
        );
    }

    #[test]
    fn resolve_property_get_procedure_is_parsed() {
        let source =
            "Sub Main()\nDim x\nx = 4\nEnd Sub\nProperty Get Value()\nDim y\ny = 1\nEnd Property";
        let module = resolve_symbols(source);
        assert!(
            module
                .procedures
                .iter()
                .any(|proc| proc.name == "property_get_value")
        );
    }

    #[test]
    fn resolve_gosub_and_label_statements() {
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nx = x + 1\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|p| p.name == "main")
            .expect("main procedure expected");
        fn has_label(stmts: &[BoundStmt], needle: &str) -> bool {
            for stmt in stmts {
                match stmt {
                    BoundStmt::Label { name } if name == needle => return true,
                    BoundStmt::IfCond {
                        then_body,
                        else_body,
                        ..
                    } => {
                        if has_label(then_body, needle) || has_label(else_body, needle) {
                            return true;
                        }
                    }
                    BoundStmt::ForRange { body, .. } | BoundStmt::DoWhile { body, .. } => {
                        if has_label(body, needle) {
                            return true;
                        }
                    }
                    BoundStmt::SelectCase {
                        arms, else_body, ..
                    } => {
                        if arms.iter().any(|(_, body)| has_label(body, needle))
                            || has_label(else_body, needle)
                        {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        fn has_return(stmts: &[BoundStmt]) -> bool {
            for stmt in stmts {
                match stmt {
                    BoundStmt::Return => return true,
                    BoundStmt::IfCond {
                        then_body,
                        else_body,
                        ..
                    } => {
                        if has_return(then_body) || has_return(else_body) {
                            return true;
                        }
                    }
                    BoundStmt::ForRange { body, .. } | BoundStmt::DoWhile { body, .. } => {
                        if has_return(body) {
                            return true;
                        }
                    }
                    BoundStmt::SelectCase {
                        arms, else_body, ..
                    } => {
                        if arms.iter().any(|(_, body)| has_return(body)) || has_return(else_body) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        assert!(
            main_proc
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::GoSub { label } if label == "add_two"))
        );
        assert!(has_label(&main_proc.body, "add_two"));
        assert!(has_return(&main_proc.body));
    }

    #[test]
    fn resolve_array_references_into_element_slots() {
        let source = "Sub Main()\nDim a(2)\nDim x\na(1) = 7\nx = a(1)\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "a_0"));
        assert!(module.declarations.iter().any(|d| d == "a_1"));
        assert!(module.declarations.iter().any(|d| d == "a_2"));
        assert!(module.declarations.iter().any(|d| d == "x"));
    }

    #[test]
    fn resolve_redim_and_redim_preserve_statements() {
        let source = "Sub Main()\nDim a(1)\nReDim Preserve a(3)\nReDim a(2)\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::ReDim { preserve: true, .. }))
        );
        assert!(module.body.iter().any(|s| matches!(
            s,
            BoundStmt::ReDim {
                preserve: false,
                ..
            }
        )));
        assert!(module.declarations.iter().any(|d| d == "a_3"));
    }

    #[test]
    fn resolve_runtime_redim_preserve_expression_bounds_on_dynamic_array() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nReDim Preserve buf(length)\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::ReDimRuntime {
                name,
                preserve: true,
                ..
            } if name == "buf"
        )));
    }

    #[test]
    fn resolve_literal_redim_on_dynamic_array_keeps_runtime_array_model() {
        let source = "Sub Main()\nDim buf() As Byte\nReDim buf(2)\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::ReDimRuntime {
                name,
                bounds,
                preserve: false,
            } if name == "buf"
                && bounds.len() == 1
                && bounds[0].lower_bound == 0
                && matches!(bounds[0].upper_bound, BoundExpr::IntConst(2))
        )));
    }

    #[test]
    fn resolve_runtime_array_index_assignment_target() {
        let source = "Sub Main()\nDim buf() As Byte\nDim length As Long\nlength = 1\nReDim buf(length)\nbuf(1) = 7\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::AssignRuntimeArrayElement {
                name,
                indices,
                expr: BoundExpr::IntConst(7),
                ..
            } if name == "buf"
                && matches!(indices.as_slice(), [BoundExpr::IntConst(1)])
        )));
    }

    #[test]
    fn resolve_runtime_multidim_redim_keeps_runtime_array_model() {
        let source = "Sub Main()\nDim buf() As Byte\nReDim buf(1 To 2, 3 To 4)\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::ReDimRuntime {
                name,
                bounds,
                preserve: false,
            } if name == "buf"
                && bounds.len() == 2
                && bounds[0].lower_bound == 1
                && matches!(bounds[0].upper_bound, BoundExpr::IntConst(2))
                && bounds[1].lower_bound == 3
                && matches!(bounds[1].upper_bound, BoundExpr::IntConst(4))
        )));
    }

    #[test]
    fn resolve_runtime_multidim_array_index_assignment_target() {
        let source =
            "Sub Main()\nDim buf() As Byte\nReDim buf(1 To 2, 1 To 2)\nbuf(2, 1) = 7\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::AssignRuntimeArrayElement {
                name,
                indices,
                expr: BoundExpr::IntConst(7),
                ..
            } if name == "buf"
                && matches!(
                    indices.as_slice(),
                    [BoundExpr::IntConst(2), BoundExpr::IntConst(1)]
                )
        )));
    }

    #[test]
    fn resolve_on_error_resume_next_and_error_stmt() {
        let source = "Sub Main()\nOn Error Resume Next\nError 5\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::OnErrorResumeNext))
        );
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::RaiseError(5)))
        );
    }

    #[test]
    fn resolve_on_error_goto_zero_and_resume_next_stmt() {
        let source = "Sub Main()\nOn Error GoTo 0\nResume Next\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::OnErrorGoto0))
        );
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::ResumeNext))
        );
    }

    #[test]
    fn resolve_on_error_goto_label_stmt() {
        let source = "Sub Main()\nOn Error GoTo handler\nIf Err.Number = -1 Then\nhandler:\nResume Next\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::OnErrorGotoLabel { label } if label == "handler"))
        );
    }

    #[test]
    fn resolve_for_step_parses_explicit_step() {
        let source = "Sub Main()\nDim i\nFor i = 5 To 1 Step -2\ni = i + 1\nNext i\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::ForRange { step, .. }) = module.body.first() else {
            panic!("expected for-range statement");
        };
        assert_eq!(step, &BoundExpr::IntConst(-2));
    }

    #[test]
    fn resolve_while_wend_parses_as_loop() {
        let source = "Sub Main()\nDim x\nWhile x < 3\nx = x + 1\nWend\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::DoWhile {
                post_check: false,
                ..
            }
        )));
    }

    #[test]
    fn resolve_do_until_and_loop_until_are_supported() {
        let source = "Sub Main()\nDim x\nDo Until x = 3\nx = x + 1\nLoop\nDo\nx = x + 1\nLoop Until x = 7\nEnd Sub";
        let module = resolve_symbols(source);
        let loops = module
            .body
            .iter()
            .filter(|stmt| matches!(stmt, BoundStmt::DoWhile { .. }))
            .count();
        assert_eq!(loops, 2);
    }

    #[test]
    fn resolve_select_case_is_and_range_clauses() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase Is < 0\nx = 1\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::SelectCase { arms, .. }) = module.body.first() else {
            panic!("expected select-case statement");
        };
        assert!(matches!(
            arms[0].0[0],
            super::BoundCaseClause::Is {
                op: CompareOp::Lt,
                value: 0
            }
        ));
        assert!(matches!(
            arms[1].0[0],
            super::BoundCaseClause::Range { start: 1, end: 3 }
        ));
    }

    #[test]
    fn resolve_select_case_accepts_module_constants_and_trailing_colons() {
        let source = concat!(
            "Public Const SQLITE_INTEGER = 1\n",
            "Sub Main()\n",
            "Dim x\n",
            "Select Case x\n",
            "Case SQLITE_INTEGER:\n",
            "x = 1\n",
            "End Select\n",
            "End Sub\n",
        );
        let module = resolve_symbols(source);
        let Some(BoundStmt::SelectCase { arms, .. }) = module
            .body
            .iter()
            .find(|stmt| matches!(stmt, BoundStmt::SelectCase { .. }))
        else {
            panic!("expected select-case statement");
        };
        assert!(matches!(arms[0].0[0], super::BoundCaseClause::Value(1)));
    }

    #[test]
    fn resolve_assignment_from_call_ignores_equals_inside_sql_string_literals() {
        let source = concat!(
            "Sub Main()\n",
            "Dim RetVal As Long\n",
            "Dim myDbHandle As Long\n",
            "Dim myStmtHandle As Long\n",
            "RetVal = SQLite3PrepareV2(myDbHandle, \"SELECT * FROM MyTable WHERE TheDate <= @FindThisDate\", myStmtHandle)\n",
            "End Sub\n",
        );
        let module = resolve_symbols(source);
        let Some(BoundStmt::AssignFromCall { name, .. }) = module
            .body
            .iter()
            .find(|stmt| matches!(stmt, BoundStmt::AssignFromCall { .. }))
        else {
            panic!("expected assign-from-call statement");
        };
        assert_eq!(name, "sqlite3preparev2");
    }

    #[test]
    fn resolve_assignment_from_call_ignores_commas_inside_sql_string_literals() {
        let source = concat!(
            "Sub Main()\n",
            "Dim RetVal As Long\n",
            "Dim myDbHandle As Long\n",
            "Dim myStmtHandle As Long\n",
            "RetVal = SQLite3PrepareV2(myDbHandle, \"SELECT TheId, datetime(TheDate), TheText FROM MyTable\", myStmtHandle)\n",
            "End Sub\n",
        );
        let module = resolve_symbols(source);
        let Some(BoundStmt::AssignFromCall { name, args, .. }) = module
            .body
            .iter()
            .find(|stmt| matches!(stmt, BoundStmt::AssignFromCall { .. }))
        else {
            panic!("expected assign-from-call statement");
        };
        assert_eq!(name, "sqlite3preparev2");
        assert_eq!(args.len(), 3);
    }

    #[test]
    fn resolve_parenthesized_compare_expression_as_value_without_breaking_named_args() {
        let source = concat!(
            "Sub Main()\n",
            "Dim result\n",
            "result = \"same: \" & (1 = 1)\n",
            "Call Fill(value := 9)\n",
            "End Sub\n",
            "Sub Fill(Optional ByVal value = 7)\n",
            "End Sub\n",
        );
        let module = resolve_symbols(source);
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::Assign { expr, .. }
            if matches!(
                expr,
                BoundExpr::BinaryOp {
                    op: ArithOp::Concat,
                    rhs,
                    ..
                } if matches!(rhs.as_ref(), BoundExpr::CompareOp { op: CompareOp::Eq, .. })
            )
        )));
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::Call { name, args, .. }
            if name == "fill" && args.len() == 1 && args[0].name.as_deref() == Some("value")
        )));
    }

    #[test]
    fn resolve_proc_signature_accepts_regular_array_parameter_shape() {
        let source = concat!(
            "Public Function BindBlob(ByVal stmtHandle As LongPtr, ByRef Value() As Byte) As Long\n",
            "BindBlob = 0\n",
            "End Function\n",
        );
        let module = resolve_symbols(source);
        let proc = module
            .procedures
            .iter()
            .find(|proc| proc.name == "bindblob")
            .expect("array-parameter function should be resolved");
        assert_eq!(proc.params.len(), 2);
        assert_eq!(proc.params[1].name, "value");
        assert_eq!(proc.params[1].ty, super::BoundType::Array);
        assert!(proc.params[1].by_ref);
    }

    #[test]
    fn resolve_fixed_array_declaration_keeps_base_symbol_for_whole_array_passing() {
        let source = concat!(
            "Sub Main()\n",
            "Dim myBlob(2) As Byte\n",
            "myBlob(0) = 90\n",
            "Call UseBlob(myBlob)\n",
            "End Sub\n",
            "Sub UseBlob(ByRef value() As Byte)\n",
            "End Sub\n",
        );
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|name| name == "myblob"));
        assert_eq!(
            module.declaration_types.get("myblob"),
            Some(&super::BoundType::Array)
        );
    }

    #[test]
    fn parse_runtime_array_index_expr_prefers_array_read_over_proc_call() {
        let source = concat!(
            "Sub Main()\n",
            "Dim buf() As Byte\n",
            "Dim length As Long\n",
            "Dim value As Long\n",
            "length = 2\n",
            "ReDim buf(length)\n",
            "value = buf(0)\n",
            "End Sub\n",
        );
        let module = resolve_symbols(source);
        let proc = module
            .procedures
            .iter()
            .find(|proc| proc.name == "main")
            .expect("main procedure expected");
        let Some(BoundStmt::Assign { expr, .. }) = proc
            .body
            .iter()
            .find(|stmt| matches!(stmt, BoundStmt::Assign { target, .. } if target == "value"))
        else {
            panic!("expected value assignment");
        };
        assert!(
            matches!(
                expr,
                BoundExpr::IntrinsicCall { name, args }
                    if name == "__oxvba_array_get"
                        && matches!(args.first(), Some(BoundExpr::Var(name)) if name == "buf")
            ),
            "expr lowered as {expr:?}"
        );
    }

    #[test]
    fn resolve_goto_numeric_target_uses_line_label_key() {
        let source = "Sub Main()\nGoTo 100\n100:\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::GoTo { label } if label == "__line_100"))
        );
        assert!(
            module
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::Label { name } if name == "__line_100"))
        );
    }

    #[test]
    fn resolve_resume_and_resume_label_statements() {
        let source = "Sub Main()\nOn Error GoTo handler\nError 5\nhandler:\nResume\nResume done\ndone:\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::Resume))
        );
        assert!(
            module
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::ResumeLabel { label } if label == "done"))
        );
    }

    #[test]
    fn resolve_err_clear_and_erase_statements() {
        let source = "Sub Main()\nDim a(2)\nErr.Clear\nErase a\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(
            module
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::ErrClear))
        );
        assert!(
            module
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::Erase { name } if name == "a"))
        );
    }

    #[test]
    fn resolve_err_surface_member_aliases() {
        let source = "Sub Main()\nDim a\nDim b\na = Err.Description\nb = Err.HelpContext\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "a"));
        assert!(module.declarations.iter().any(|d| d == "b"));

        let assign_vars = module
            .body
            .iter()
            .filter_map(|stmt| match stmt {
                BoundStmt::Assign {
                    expr: BoundExpr::Var(name),
                    ..
                } => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(assign_vars.contains(&"err_description"));
        assert!(assign_vars.contains(&"err_helpcontext"));
    }

    #[test]
    fn resolve_declare_alias_is_canonicalized_and_ptrsafe_recorded() {
        let source = "Declare PtrSafe Function HostPing Lib \"HOST\" Alias \"Ping\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        let ext = module
            .external_declarations
            .get("hostping")
            .expect("external declaration should be registered");
        assert_eq!(ext.library, "host");
        assert_eq!(ext.alias, "Ping");
        assert!(ext.ptr_safe);
        assert!(!ext.ordinal_alias);
        assert!(module.resolution_diagnostics.is_empty());
    }

    #[test]
    fn resolve_private_declare_alias_is_canonicalized_and_ptrsafe_recorded() {
        let source = "Private Declare PtrSafe Function HostPing Lib \"HOST\" Alias \"Ping\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        let ext = module
            .external_declarations
            .get("hostping")
            .expect("private external declaration should be registered");
        assert_eq!(ext.library, "host");
        assert_eq!(ext.alias, "Ping");
        assert!(ext.ptr_safe);
        assert!(!ext.ordinal_alias);
        assert!(module.resolution_diagnostics.is_empty());
    }

    #[test]
    fn resolve_debug_print_multiple_exprs_is_lowered_as_concat_chain() {
        let source = "Sub Main()\nDebug.Print \"trace\", Err.LastDllError\nEnd Sub";
        let module = resolve_symbols(source);
        let stmt = module
            .procedures
            .iter()
            .find(|proc| proc.name == "main")
            .and_then(|proc| proc.body.first())
            .expect("debug print stmt");
        match stmt {
            BoundStmt::DebugPrint { data } => match data {
                BoundExpr::BinaryOp {
                    op: ArithOp::Concat,
                    ..
                } => {}
                other => panic!("expected concat chain, got {:?}", other),
            },
            other => panic!("expected DebugPrint stmt, got {:?}", other),
        }
        assert!(module.resolution_diagnostics.is_empty());
    }

    #[test]
    fn resolve_declare_ordinal_alias_is_normalized() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"#0007\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        let ext = module
            .external_declarations
            .get("hostping")
            .expect("external declaration should be registered");
        assert_eq!(ext.alias, "#7");
        assert!(ext.ordinal_alias);
        assert!(module.resolution_diagnostics.is_empty());
    }

    #[test]
    fn resolve_declare_without_ptrsafe_adds_resolution_diagnostic() {
        let source = "Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.external_declarations.len(), 0);
        assert!(
            module
                .resolution_diagnostics
                .iter()
                .any(|diag| diag.contains("PtrSafe keyword is required"))
        );
    }

    #[test]
    fn resolve_declare_byref_parameter_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByRef x As Long) As Long\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.external_declarations.len(), 1);
        let decl = &module.external_declarations["hostping"];
        assert!(decl.params[0].by_ref);
    }

    #[test]
    fn resolve_withevents_declaration_is_parsed_as_regular_object_declaration() {
        let source = "Sub Main()\nDim WithEvents app As Object\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.resolution_diagnostics.is_empty());
        assert!(module.declarations.iter().any(|name| name == "app"));
    }

    #[test]
    fn resolve_implements_directive_is_ignored_in_single_module_resolve() {
        let source = "Implements IFoo\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.resolution_diagnostics.is_empty());
    }

    #[test]
    fn resolve_raiseevent_statement_is_captured_for_downstream_lowering() {
        let source = "Sub Main()\nRaiseEvent Tick\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.resolution_diagnostics.is_empty());
        assert!(module.body.iter().any(|stmt| matches!(
            stmt,
            BoundStmt::RaiseEvent { name, args } if name == "tick" && args.is_empty()
        )));
    }

    #[test]
    fn parse_expr_mul_const() {
        let source = "Sub Main()\nDim x\nx = 3 * 4\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(
            expr,
            &BoundExpr::BinaryOp {
                op: ArithOp::Mul,
                lhs: Box::new(BoundExpr::IntConst(3)),
                rhs: Box::new(BoundExpr::IntConst(4)),
            }
        );
    }

    #[test]
    fn parse_expr_div_var() {
        let source = "Sub Main()\nDim x\nDim y\nx = y / 2\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(
            expr,
            &BoundExpr::BinaryOp {
                op: ArithOp::Div,
                lhs: Box::new(BoundExpr::Var("y".to_string())),
                rhs: Box::new(BoundExpr::IntConst(2)),
            }
        );
    }

    #[test]
    fn parse_expr_precedence() {
        // a + b * c → splits at `+` first (lower precedence)
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim x\nx = a + b * c\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        // Should be Add(a, Mul(b, c))
        match expr {
            BoundExpr::BinaryOp {
                op: ArithOp::Add,
                rhs,
                ..
            } => {
                assert!(matches!(
                    rhs.as_ref(),
                    BoundExpr::BinaryOp {
                        op: ArithOp::Mul,
                        ..
                    }
                ));
            }
            other => panic!("expected BinaryOp::Add, got {:?}", other),
        }
    }

    #[test]
    fn parse_expr_add_sub_is_left_associative() {
        let source = "Sub Main()\nDim a\nDim b\nDim x\nx = a - b + 1\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        match expr {
            BoundExpr::BinaryOp {
                op: ArithOp::Add,
                lhs,
                rhs,
            } => {
                assert!(matches!(rhs.as_ref(), BoundExpr::IntConst(1)));
                assert!(matches!(
                    lhs.as_ref(),
                    BoundExpr::BinaryOp {
                        op: ArithOp::Sub,
                        ..
                    }
                ));
            }
            other => panic!("expected left-associative add/sub tree, got {:?}", other),
        }
    }

    #[test]
    fn parse_expr_paren_override() {
        // (a + b) * c → splits at `*` (parens protect `+`)
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim x\nx = (a + b) * c\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        match expr {
            BoundExpr::BinaryOp {
                op: ArithOp::Mul,
                lhs,
                ..
            } => {
                // lhs should be Add(a, b) — but AddConst since a + b where b is not const...
                // Actually: (a + b) where a and b are vars → BinaryOp::Add
                assert!(matches!(
                    lhs.as_ref(),
                    BoundExpr::BinaryOp {
                        op: ArithOp::Add,
                        ..
                    }
                ));
            }
            other => panic!("expected BinaryOp::Mul, got {:?}", other),
        }
    }

    #[test]
    fn parse_expr_mod_keyword() {
        let source = "Sub Main()\nDim x\nx = 17 Mod 3\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(
            expr,
            &BoundExpr::BinaryOp {
                op: ArithOp::Mod,
                lhs: Box::new(BoundExpr::IntConst(17)),
                rhs: Box::new(BoundExpr::IntConst(3)),
            }
        );
    }

    #[test]
    fn parse_expr_concat() {
        let source = "Sub Main()\nDim x\nDim y\nx = x & y\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(
            expr,
            &BoundExpr::BinaryOp {
                op: ArithOp::Concat,
                lhs: Box::new(BoundExpr::Var("x".to_string())),
                rhs: Box::new(BoundExpr::Var("y".to_string())),
            }
        );
    }

    #[test]
    fn parse_expr_string_literal() {
        let source = "Sub Main()\nDim x\nx = \"hello\"\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(expr, &BoundExpr::StringConst("hello".to_string()));
    }

    #[test]
    fn parse_expr_string_escaped_quote() {
        let source = "Sub Main()\nDim x\nx = \"he\"\"llo\"\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(expr, &BoundExpr::StringConst("he\"llo".to_string()));
    }

    #[test]
    fn parse_expr_empty_string() {
        let source = "Sub Main()\nDim x\nx = \"\"\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(expr, &BoundExpr::StringConst("".to_string()));
    }

    #[test]
    fn parse_expr_string_concat_literals() {
        let source = "Sub Main()\nDim x\nx = \"a\" & \"b\"\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(
            expr,
            &BoundExpr::BinaryOp {
                op: ArithOp::Concat,
                lhs: Box::new(BoundExpr::StringConst("a".to_string())),
                rhs: Box::new(BoundExpr::StringConst("b".to_string())),
            }
        );
    }

    #[test]
    fn resolve_nested_udt_expansion_uses_explicit_type_name() {
        // Item 1 regression: TopLeft As Point should expand using the "Point" type name,
        // not the "topleft" field name.
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nType Rect\nTopLeft As Point\nBottomRight As Point\nEnd Type\nSub Main()\nDim r As Rect\nr.TopLeft_X = 1\nr.TopLeft_Y = 2\nr.BottomRight_X = 3\nr.BottomRight_Y = 4\nEnd Sub";
        let module = resolve_symbols(source);
        // Verify that all nested fields are declared
        for expected in &[
            "r",
            "r_topleft",
            "r_topleft_x",
            "r_topleft_y",
            "r_bottomright",
            "r_bottomright_x",
            "r_bottomright_y",
        ] {
            assert!(
                module.declarations.iter().any(|d| d == expected),
                "expected declaration '{}' not found in {:?}",
                expected,
                module.declarations
            );
        }
    }

    #[test]
    fn resolve_nested_udt_no_expansion_for_non_udt_type() {
        // If a field says "Foo As Bar" and Bar is not a known UDT, no sub-fields should expand.
        let source = "Type MyType\nFoo As String\nEnd Type\nSub Main()\nDim m As MyType\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "m"));
        assert!(module.declarations.iter().any(|d| d == "m_foo"));
        // Should NOT have any sub-fields of foo
        assert!(
            !module.declarations.iter().any(|d| d.starts_with("m_foo_")),
            "non-UDT field should not have sub-field expansion"
        );
    }

    #[test]
    fn resolve_udt_array_field_parses_without_error() {
        let source =
            "Type Scores\nItems(10) As Integer\nEnd Type\nSub Main()\nDim s As Scores\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "s"));
        // Array field Items(10) should create indexed aliases s_items_0 through s_items_10
        assert!(
            module.declarations.iter().any(|d| d == "s_items_0"),
            "expected s_items_0 in {:?}",
            module.declarations
        );
        assert!(
            module.declarations.iter().any(|d| d == "s_items_10"),
            "expected s_items_10 in {:?}",
            module.declarations
        );
    }

    #[test]
    fn resolve_udt_descriptors_record_nested_fixed_string_and_array_members() {
        let source = concat!(
            "Type Point\n",
            "X As Long\n",
            "Y As Long\n",
            "End Type\n",
            "Type Record\n",
            "Name As String * 5\n",
            "Scores(1 To 2) As Long\n",
            "Inner As Point\n",
            "End Type\n",
            "Sub Main()\n",
            "Dim r As Record\n",
            "End Sub",
        );
        let module = resolve_symbols(source);
        let main_proc = module
            .procedures
            .iter()
            .find(|proc| proc.name == "main")
            .expect("main procedure expected");
        let record = main_proc
            .udt_descriptors
            .iter()
            .find(|descriptor| descriptor.type_name == "record")
            .expect("record UDT descriptor expected");
        assert_eq!(record.variable_names, vec!["r".to_string()]);
        assert!(record.fields.iter().any(|field| field.name == "name"
            && field.bound_type == super::BoundType::String
            && field.fixed_string_len == Some(5)));
        assert!(
            record
                .fields
                .iter()
                .any(|field| field.name == "scores" && field.array_bounds == Some(vec![(1, 2)]))
        );
        assert!(record.fields.iter().any(
            |field| field.name == "inner" && field.nested_udt_name.as_deref() == Some("point")
        ));
        assert!(
            main_proc
                .udt_descriptors
                .iter()
                .any(|descriptor| descriptor.type_name == "point"),
            "nested UDT type descriptor should be retained for package evidence"
        );
    }
}
