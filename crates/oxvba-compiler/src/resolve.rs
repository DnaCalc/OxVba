use std::collections::{HashMap, HashSet};

type ArrayBoundsMap = HashMap<String, Vec<(i32, i32)>>;
type ParsedArrayDecl = (String, Option<BoundType>, Vec<(i32, i32)>);
type UdtDefMap = HashMap<String, Vec<(String, BoundType)>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundExpr {
    IntConst(i32),
    Var(String),
    AddConst {
        var: String,
        delta: i32,
    },
    SubConst {
        var: String,
        delta: i32,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentIntent {
    Implicit,
    Let,
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundStmt {
    Assign {
        target: String,
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
        body: Vec<BoundStmt>,
    },
    ReDim {
        name: String,
        bounds: Vec<(i32, i32)>,
        previous_bounds: Option<Vec<(i32, i32)>>,
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
    },
    AssignFromCall {
        target: String,
        name: String,
        args: Vec<BoundCallArg>,
        intent: AssignmentIntent,
    },
    SelectCase {
        expr: BoundExpr,
        arms: Vec<(Vec<BoundCaseClause>, Vec<BoundStmt>)>,
        else_body: Vec<BoundStmt>,
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
    pub compare_mode: BoundCompareMode,
    pub default_type_table: [BoundType; 26],
    pub resolution_diagnostics: Vec<String>,
    pub declarations: Vec<String>,
    pub declaration_types: HashMap<String, BoundType>,
    pub array_descriptors: HashMap<String, BoundArrayDescriptor>,
    pub external_declarations: HashMap<String, BoundExternalDecl>,
    pub body: Vec<BoundStmt>,
    pub procedures: Vec<BoundProcedure>,
}

#[derive(Debug, Clone)]
pub struct BoundProcedure {
    pub name: String,
    pub return_type: BoundType,
    pub params: Vec<BoundParam>,
    pub declarations: Vec<String>,
    pub declaration_types: HashMap<String, BoundType>,
    pub array_descriptors: HashMap<String, BoundArrayDescriptor>,
    pub duplicate_declarations: Vec<String>,
    pub body: Vec<BoundStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundArrayDescriptor {
    pub element_type: BoundType,
    pub rank: usize,
    pub bounds: Vec<(i32, i32)>,
    pub dynamic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundParam {
    pub name: String,
    pub by_ref: bool,
    pub param_array: bool,
    pub optional: bool,
    pub default_value: Option<i32>,
    pub ty: BoundType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundExternalDecl {
    pub name: String,
    pub library: String,
    pub alias: String,
    pub ptr_safe: bool,
    pub ordinal_alias: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcKind {
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
    let module_constants = collect_module_constants(&lines);
    let property_write_routes = collect_property_write_routes(&lines);
    let property_read_routes = collect_property_read_routes(&lines);
    let (declared_externals, external_declarations, external_decl_diagnostics) =
        collect_declared_external_procedures(&lines, &default_type_table);
    let class_model_diagnostics = collect_class_model_diagnostics(&lines);

    let has_explicit_procs = lines
        .iter()
        .any(|line| detect_proc_kind(&line.to_ascii_lowercase()).is_some());

    let procedures = if has_explicit_procs {
        parse_procedures(
            &lines,
            &mut option_explicit,
            option_base,
            &default_type_table,
            &udt_defs,
            &module_constants,
            &property_write_routes,
            &property_read_routes,
        )
    } else {
        let mut declarations: Vec<String> = Vec::new();
        let mut declaration_types: HashMap<String, BoundType> = HashMap::new();
        let mut duplicate_declarations: Vec<String> = Vec::new();
        let mut array_bounds: ArrayBoundsMap = HashMap::new();
        let mut index = 0;
        for (name, _) in sorted_module_constants(&module_constants) {
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                declarations.push(name.clone());
            }
            declaration_types.insert(name, BoundType::Long);
        }
        let mut body = parse_block(
            &lines,
            &mut index,
            &mut declarations,
            &mut declaration_types,
            &mut duplicate_declarations,
            &mut array_bounds,
            &mut option_explicit,
            option_base,
            &default_type_table,
            &udt_defs,
            &module_constants,
            &property_write_routes,
            &property_read_routes,
            &[],
        );
        body.splice(0..0, build_const_prelude(&module_constants));
        let array_descriptors = build_array_descriptors(&array_bounds, &declaration_types, &body);
        vec![BoundProcedure {
            name: "main".to_string(),
            return_type: BoundType::Variant,
            params: Vec::new(),
            declarations,
            declaration_types,
            array_descriptors,
            duplicate_declarations,
            body,
        }]
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
            return_type: BoundType::Variant,
            params: Vec::new(),
            declarations: Vec::new(),
            declaration_types: HashMap::new(),
            array_descriptors: HashMap::new(),
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
        compare_mode,
        default_type_table,
        resolution_diagnostics,
        declarations: entry.declarations.clone(),
        declaration_types: entry.declaration_types.clone(),
        array_descriptors: entry.array_descriptors.clone(),
        external_declarations,
        body: entry.body.clone(),
        procedures,
    }
}

fn normalize_source_lines(source: &str) -> Vec<String> {
    let mut merged = Vec::new();
    let mut pending = String::new();

    for raw in source.lines() {
        let trimmed = raw.trim();
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
        if !final_line.is_empty() && !final_line.starts_with('\'') {
            merged.push(final_line.to_string());
        }
        pending.clear();
    }

    let final_line = pending.trim();
    if !final_line.is_empty() && !final_line.starts_with('\'') {
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
    let mut constants: HashMap<String, i32> = HashMap::new();
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
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
) -> Vec<BoundProcedure> {
    let mut procedures = Vec::new();
    let mut index = 0;

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

        index += 1;
        let mut declarations: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut declaration_types: HashMap<String, BoundType> =
            params.iter().map(|p| (p.name.clone(), p.ty)).collect();
        if matches!(kind, ProcKind::Function | ProcKind::PropertyGet)
            && !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            declarations.push(name.clone());
            declaration_types.insert(name.clone(), return_type);
        }
        let mut duplicate_declarations: Vec<String> = Vec::new();
        for (name, _) in sorted_module_constants(module_constants) {
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                declarations.push(name.clone());
            }
            declaration_types.entry(name).or_insert(BoundType::Long);
        }
        let mut array_bounds: ArrayBoundsMap = HashMap::new();
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
        let array_descriptors = build_array_descriptors(&array_bounds, &declaration_types, &body);
        if index < lines.len() && lines[index].eq_ignore_ascii_case(end_term) {
            index += 1;
        }

        procedures.push(BoundProcedure {
            name,
            return_type,
            params,
            declarations,
            declaration_types,
            array_descriptors,
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

fn detect_proc_kind(lower: &str) -> Option<ProcKind> {
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

fn parse_proc_signature(
    line: &str,
    kind: ProcKind,
    default_type_table: &[BoundType; 26],
) -> Option<(String, Vec<BoundParam>, BoundType)> {
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
        && let Some(close) = rest.rfind(')')
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
                let (by_ref, remainder) = if param_array {
                    if lower.starts_with("byval ") || lower.starts_with("byref ") {
                        return None;
                    }
                    (false, token)
                } else if lower.starts_with("byval ") {
                    (false, token[6..].trim())
                } else if lower.starts_with("byref ") {
                    (true, token[6..].trim())
                } else {
                    (true, token)
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
                if optional && by_ref {
                    return None;
                }
                if optional {
                    seen_optional = true;
                } else if seen_optional {
                    return None;
                }

                let normalized_name_text = if param_array {
                    let trimmed = name_text.trim();
                    trimmed.strip_suffix("()")?
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
                    Some(parse_declared_type(ty_text).unwrap_or(BoundType::Variant));
            }
        }
    } else if matches!(kind, ProcKind::Function | ProcKind::PropertyGet)
        && let Some((_, rhs)) = split_keyword_ci(rest, "as")
    {
        explicit_return_ty = Some(parse_declared_type(rhs.trim()).unwrap_or(BoundType::Variant));
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

fn sorted_module_constants(constants: &HashMap<String, i32>) -> Vec<(String, i32)> {
    let mut out = constants
        .iter()
        .map(|(name, value)| (name.clone(), *value))
        .collect::<Vec<_>>();
    out.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    out
}

fn build_const_prelude(constants: &HashMap<String, i32>) -> Vec<BoundStmt> {
    sorted_module_constants(constants)
        .into_iter()
        .map(|(name, value)| BoundStmt::Assign {
            target: name,
            expr: BoundExpr::IntConst(value),
            intent: AssignmentIntent::Implicit,
        })
        .collect()
}

fn collect_module_constants(lines: &[String]) -> HashMap<String, i32> {
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

        if lower.starts_with("enum ") {
            parse_enum_block(lines, &mut index, &mut constants);
            continue;
        }

        if let Some((name, value)) = parse_const_declaration(line) {
            constants.insert(name, value);
        }
        index += 1;
    }

    constants
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
        procedures.push(BoundProcedure {
            name,
            return_type,
            params,
            declarations,
            declaration_types,
            array_descriptors: HashMap::new(),
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
    let Some(rest) = strip_keyword_prefix_ci(trimmed, "declare") else {
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

    if params.iter().any(|param| param.param_array) {
        return Err(
            "external procedure declaration rejected: ParamArray is not supported in dynamic-link subset"
                .to_string(),
        );
    }
    if params.iter().any(|param| param.optional) {
        return Err(
            "external procedure declaration rejected: Optional parameters are not supported in dynamic-link subset"
                .to_string(),
        );
    }
    if params.iter().any(|param| param.by_ref) {
        return Err(
            "external procedure declaration rejected: ByRef parameters are not supported in dynamic-link subset"
                .to_string(),
        );
    }
    if params.len() > 1 {
        return Err(
            "external procedure declaration rejected: only one argument is supported in dynamic-link subset"
                .to_string(),
        );
    }
    if params.iter().any(|param| param.ty != BoundType::Long) {
        return Err(
            "external procedure declaration rejected: only `Long` parameter type is supported in dynamic-link subset"
                .to_string(),
        );
    }
    if matches!(kind, ProcKind::Function) && return_type != BoundType::Long {
        return Err(
            "external procedure declaration rejected: only `Long` return type is supported in dynamic-link subset"
                .to_string(),
        );
    }

    let lib = extract_quoted_after_keyword(tail, "lib").ok_or_else(|| {
        "external procedure declaration rejected: missing `Lib \"...\"` clause".to_string()
    })?;
    let alias_raw = extract_quoted_after_keyword(tail, "alias").unwrap_or_else(|| name.clone());
    let (alias, ordinal_alias) = normalize_external_alias(alias_raw.as_str())?;

    Ok(Some(ParsedDeclareSignature {
        name,
        params,
        return_type,
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
    Ok((alias.to_ascii_lowercase(), false))
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
                let (field_name_raw, explicit_ty) =
                    if let Some((lhs, rhs)) = split_keyword_ci(raw, "as") {
                        (
                            lhs.trim(),
                            parse_declared_type(rhs.trim()).unwrap_or(BoundType::Variant),
                        )
                    } else {
                        (raw, BoundType::Variant)
                    };
                if let Some(field_name) = normalize_ident(field_name_raw) {
                    let field_ty = if explicit_ty == BoundType::Variant {
                        default_type_for_name(&field_name, default_type_table)
                    } else {
                        explicit_ty
                    };
                    fields.push((field_name, field_ty));
                }
            }
            index += 1;
        }
        defs.insert(type_name, fields);
        if index < lines.len() {
            index += 1;
        }
    }
    defs
}

fn parse_enum_block(lines: &[String], index: &mut usize, constants: &mut HashMap<String, i32>) {
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
            constants.insert(name, value);
            next_value = value.saturating_add(1);
        }
        *index += 1;
    }
}

fn parse_const_declaration(line: &str) -> Option<(String, i32)> {
    let trimmed = line.trim();
    let rhs = strip_keyword_prefix_ci(trimmed, "public const")
        .or_else(|| strip_keyword_prefix_ci(trimmed, "private const"))
        .or_else(|| strip_keyword_prefix_ci(trimmed, "const"))?;
    let (lhs, rhs_value) = rhs.split_once('=')?;
    let name = lhs.split_whitespace().next().and_then(normalize_ident)?;
    let value = rhs_value.trim().parse::<i32>().ok()?;
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
    module_constants: &HashMap<String, i32>,
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

        if lower.starts_with("sub ")
            || lower == "end sub"
            || lower.starts_with("function ")
            || lower == "end function"
            || lower.starts_with("property get ")
            || lower.starts_with("property let ")
            || lower.starts_with("property set ")
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

        if lower.starts_with("dim ") {
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

        if lower.starts_with("enum ") {
            *index += 1;
            while *index < lines.len() && !lines[*index].eq_ignore_ascii_case("end enum") {
                *index += 1;
            }
            if *index < lines.len() {
                *index += 1;
            }
            continue;
        }

        if lower.starts_with("type ") {
            *index += 1;
            while *index < lines.len() && !lines[*index].eq_ignore_ascii_case("end type") {
                *index += 1;
            }
            if *index < lines.len() {
                *index += 1;
            }
            continue;
        }

        if lower.starts_with("if ") && lower.ends_with(" then") {
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

        out.push(parse_assign_or_unsupported(
            line,
            declarations,
            array_bounds,
            property_write_routes,
            property_read_routes,
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
    module_constants: &HashMap<String, i32>,
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
    module_constants: &HashMap<String, i32>,
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
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let Some((var, items)) = parse_for_each_header(line, array_bounds) else {
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
            return BoundStmt::ForEach { var, items, body };
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
    module_constants: &HashMap<String, i32>,
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
    array_bounds: &ArrayBoundsMap,
    property_write_routes: &HashMap<String, String>,
    property_read_routes: &HashMap<String, String>,
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

    if let Some((lhs_raw, rhs_raw)) = assignment_line.split_once('=')
        && let Some(target) = parse_reference_name(lhs_raw, array_bounds)
    {
        if parse_reference_name(rhs_raw.trim(), array_bounds).is_none()
            && let Some((call_name, args)) =
                parse_dispatch_invoke_call_invocation(rhs_raw.trim(), array_bounds)
        {
            return BoundStmt::AssignFromCall {
                target,
                name: call_name,
                args,
                intent: assignment_intent,
            };
        }
        if parse_reference_name(rhs_raw.trim(), array_bounds).is_none()
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
            };
        }

        if let Some(expr) = parse_expr(rhs_raw, array_bounds) {
            if let BoundExpr::Var(source) = &expr
                && let Some(fields) =
                    shared_udt_fields_for_assignment(&target, source, declarations)
            {
                return BoundStmt::UdtAssign {
                    target,
                    source: source.clone(),
                    fields,
                };
            }
            if let Some(route_proc) = property_write_routes.get(&target)
                && !declarations
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&target))
            {
                return BoundStmt::Call {
                    name: route_proc.clone(),
                    args: vec![BoundCallArg { name: None, expr }],
                };
            }
            return BoundStmt::Assign {
                target,
                expr,
                intent: assignment_intent,
            };
        }
    }

    let call_token = if assignment_line.to_ascii_lowercase().starts_with("call ") {
        assignment_line[5..].trim()
    } else {
        assignment_line.trim()
    };
    if let Some((name, args)) = parse_dispatch_invoke_call_invocation(call_token, array_bounds) {
        return BoundStmt::Call { name, args };
    }
    if let Some((name, args)) = parse_call_invocation(call_token, array_bounds) {
        return BoundStmt::Call { name, args };
    }
    if let Some(name) = normalize_ident(call_token).or_else(|| parse_member_reference(call_token)) {
        return BoundStmt::Call {
            name,
            args: Vec::new(),
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn shared_udt_fields_for_assignment(
    target: &str,
    source: &str,
    declarations: &[String],
) -> Option<Vec<String>> {
    let target_fields = collect_udt_field_suffixes(target, declarations);
    if target_fields.is_empty() {
        return None;
    }
    let source_fields = collect_udt_field_suffixes(source, declarations);
    if source_fields.is_empty() || source_fields != target_fields {
        return None;
    }
    Some(target_fields)
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
    for token in split_call_args(args_raw)? {
        let trimmed = token.trim();
        if let Some((lhs, rhs)) = trimmed.split_once(":=") {
            args.push(BoundCallArg {
                name: Some(normalize_ident(lhs)?),
                expr: parse_expr(rhs.trim(), array_bounds)?,
            });
        } else {
            args.push(BoundCallArg {
                name: None,
                expr: parse_expr(trimmed, array_bounds)?,
            });
        }
    }
    Some((name, args))
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
    if name != "dispatchinvoke" {
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
    });
    args.push(BoundCallArg {
        name: None,
        expr: parse_dispatch_member_arg(args_text[1], array_bounds)?,
    });
    for token in &args_text[2..] {
        let trimmed = token.trim();
        if let Some((lhs, rhs)) = trimmed.split_once(":=") {
            args.push(BoundCallArg {
                name: Some(normalize_ident(lhs)?),
                expr: parse_expr(rhs.trim(), array_bounds)?,
            });
        } else {
            args.push(BoundCallArg {
                name: None,
                expr: parse_expr(trimmed, array_bounds)?,
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
                });
            } else {
                args.push(BoundCallArg {
                    name: None,
                    expr: parse_expr(trimmed, array_bounds)?,
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
    module_constants: &HashMap<String, i32>,
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

fn parse_for_each_header(
    line: &str,
    array_bounds: &ArrayBoundsMap,
) -> Option<(String, Vec<BoundExpr>)> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("for each ") {
        return None;
    }

    let payload = line[9..].trim();
    let (var_raw, iterable_raw) = split_keyword_ci(payload, "in")?;
    let var = normalize_ident(var_raw)?;
    let iterable = iterable_raw.trim();

    let iterable_lower = iterable.to_ascii_lowercase();
    if iterable_lower.starts_with("array(") && iterable.ends_with(')') {
        let inner = &iterable[6..iterable.len() - 1];
        let items = split_call_args(inner)?
            .iter()
            .map(|token| parse_expr(token, array_bounds))
            .collect::<Option<Vec<_>>>()?;
        if items.is_empty() {
            return None;
        }
        return Some((var, items));
    }

    let base = normalize_ident(iterable)?;
    let bounds = array_bounds.get(&base)?;
    let element_count = array_element_count(bounds)?;
    let mut items = Vec::with_capacity(element_count);
    for idx in 0..element_count {
        items.push(BoundExpr::Var(format!("{base}_{idx}")));
    }
    Some((var, items))
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
    let (name, _, bounds) = parse_array_declaration(payload, option_base)?;
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
    module_constants: &HashMap<String, i32>,
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
            let Some(values) = parse_case_clauses(values_raw) else {
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

fn parse_case_clauses(values_raw: &str) -> Option<Vec<BoundCaseClause>> {
    let tokens = split_call_args(values_raw)?;
    let mut out = Vec::new();
    for token in tokens {
        let clause = parse_case_clause(token.trim())?;
        out.push(clause);
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn parse_case_clause(token: &str) -> Option<BoundCaseClause> {
    if token.is_empty() {
        return None;
    }

    let lower = token.to_ascii_lowercase();
    if lower.starts_with("is ") {
        return parse_case_is_clause(token[3..].trim());
    }

    if let Some((start_raw, end_raw)) = split_keyword_ci(token, "to") {
        let start = start_raw.trim().parse::<i32>().ok()?;
        let end = end_raw.trim().parse::<i32>().ok()?;
        return Some(BoundCaseClause::Range { start, end });
    }

    token.parse::<i32>().ok().map(BoundCaseClause::Value)
}

fn parse_case_is_clause(token: &str) -> Option<BoundCaseClause> {
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
            let value = rest.trim().parse::<i32>().ok()?;
            return Some(BoundCaseClause::Is { op, value });
        }
    }
    None
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
        return Some(BoundExpr::IntConst(0));
    }
    if expr.eq_ignore_ascii_case("null") {
        return Some(BoundExpr::IntConst(-1));
    }
    if expr.eq_ignore_ascii_case("nothing") {
        return Some(BoundExpr::IntConst(0));
    }
    if let Ok(value) = expr.parse::<i32>() {
        return Some(BoundExpr::IntConst(value));
    }

    if let Some(inner) = parse_intrinsic_conversion_expr(expr, array_bounds) {
        return Some(inner);
    }
    if let Some(call) = parse_stdlib_intrinsic_call_expr(expr, array_bounds) {
        return Some(call);
    }
    if parse_reference_name(expr, array_bounds).is_none()
        && let Some((name, args)) = parse_call_invocation(expr, array_bounds)
        && !is_intrinsic_call_name(&name)
    {
        return Some(BoundExpr::ProcCall { name, args });
    }

    if let Some((left_raw, right_raw)) = expr.split_once('+') {
        let var = parse_reference_name(left_raw, array_bounds)?;
        let delta = right_raw.trim().parse::<i32>().ok()?;
        return Some(BoundExpr::AddConst { var, delta });
    }

    if let Some((left_raw, right_raw)) = expr.split_once('-') {
        let var = parse_reference_name(left_raw, array_bounds)?;
        let delta = right_raw.trim().parse::<i32>().ok()?;
        return Some(BoundExpr::SubConst { var, delta });
    }

    parse_reference_name(expr, array_bounds).map(BoundExpr::Var)
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
    if name == "cverr" {
        return Some(BoundExpr::IntrinsicCall {
            name,
            args: vec![inner],
        });
    }
    Some(inner)
}

#[derive(Debug, Clone, Copy)]
struct IntrinsicSpec {
    min_arity: usize,
    max_arity: usize,
    surface: IntrinsicSurface,
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

fn intrinsic_spec(name: &str) -> Option<IntrinsicSpec> {
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
        | "eof" | "lof" | "seek" => Some(IntrinsicSpec::fixed(1, DeterministicCore)),
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
        "shell" | "environ" | "dir" | "createobject" => {
            Some(IntrinsicSpec::fixed(1, HostSensitive))
        }
        "dispatchinvoke" => Some(IntrinsicSpec::range(2, usize::MAX, HostSensitive)),
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
    if close <= open || !expr[close + 1..].trim().is_empty() {
        return None;
    }
    let name = normalize_ident(expr[..open].trim())?;
    let spec = intrinsic_spec(name.as_str())?;

    let args_raw = expr[open + 1..close].trim();
    let args_text = split_call_args(args_raw)?;
    let args = match name.as_str() {
        "createobject" => args_text
            .iter()
            .map(|arg| parse_createobject_arg(arg, array_bounds))
            .collect::<Option<Vec<_>>>()?,
        "dispatchinvoke" => parse_dispatch_invoke_args(&args_text, array_bounds)?,
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

fn parse_createobject_arg(arg: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    if let Some(expr) = parse_expr(arg, array_bounds) {
        return Some(expr);
    }
    let literal = parse_quoted_string_literal(arg)?;
    let token = map_createobject_literal_token(literal.as_str())?;
    Some(BoundExpr::IntConst(token))
}

fn parse_dispatch_member_arg(arg: &str, array_bounds: &ArrayBoundsMap) -> Option<BoundExpr> {
    if let Some(expr) = parse_expr(arg, array_bounds) {
        return Some(expr);
    }
    let literal = parse_quoted_string_literal(arg)?;
    let token = map_dispatch_member_literal_token(literal.as_str())?;
    Some(BoundExpr::IntConst(token))
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
    out.push(parse_dispatch_member_arg(args_text[1], array_bounds)?);
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
    Some(body.replace("\"\"", "\""))
}

fn map_createobject_literal_token(text: &str) -> Option<i32> {
    let canonical = text.trim().to_ascii_lowercase();
    match canonical.as_str() {
        "scripting.dictionary" => Some(4),
        "oxvba.testdispatch" => Some(4),
        _ => None,
    }
}

fn map_dispatch_member_literal_token(text: &str) -> Option<i32> {
    let canonical = text.trim().to_ascii_lowercase();
    match canonical.as_str() {
        "count" => Some(1),
        "exists" => Some(2),
        "firechanged" => Some(3),
        "firechangedpair" => Some(4),
        "firechangedsourceinterface" => Some(11),
        "ping" => Some(5),
        "lookup" => Some(6),
        "setvalue" => Some(7),
        "setvalueref" => Some(8),
        "value" => Some(9),
        "quit" => Some(10),
        "sumpair" => Some(12),
        "lookuppair" => Some(13),
        "setindexedvalue" => Some(14),
        "setindexedvalueref" => Some(15),
        "echovariant" => Some(16),
        "raiseexception" => Some(17),
        "returnsmallint" => Some(18),
        "returnunsignedword" => Some(19),
        "returnsmallintarray" => Some(20),
        "returnboolarray" => Some(21),
        "returnstringarray" => Some(22),
        "returnselfdispatch" => Some(23),
        "returnselfunknown" => Some(24),
        "classifyvariantarg" => Some(25),
        "classifyvariantarrayfirstelementarg" => Some(26),
        "returnselfdispatcharray" => Some(27),
        "returnselftypeddispatcharray" => Some(28),
        "returnselftypedunknownarray" => Some(29),
        "returnsmallintmatrix" => Some(30),
        "returnplainunknown" => Some(31),
        "returnplainunknownarray" => Some(32),
        "returnlongarray" => Some(33),
        "returnunsignedlongarray" => Some(34),
        "returnlong" => Some(35),
        "returnunsignedlong" => Some(36),
        "returnbyte" => Some(37),
        "returnbytearray" => Some(38),
        "returnsignedbyte" => Some(39),
        "returnsignedbytearray" => Some(40),
        "returnplatformint" => Some(41),
        "returnplatformuint" => Some(42),
        "returnplatformintarray" => Some(43),
        "returnplatformuintarray" => Some(44),
        _ => None,
    }
}

fn split_call_args(args_raw: &str) -> Option<Vec<&str>> {
    if args_raw.trim().is_empty() {
        return Some(Vec::new());
    }

    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in args_raw.char_indices() {
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
                if part.is_empty() {
                    return None;
                }
                out.push(part);
                start = idx + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let tail = args_raw[start..].trim();
    if tail.is_empty() {
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
        let rhs = parse_expr(rhs_raw, array_bounds)?;
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
    let remainder = line[4..].trim();
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
            .any(|existing| existing.eq_ignore_ascii_case(&format!("{base}_0")))
            || array_bounds.contains_key(&base)
        {
            duplicate_declarations.push(base);
            return;
        }
        array_bounds.insert(base.clone(), bounds.clone());
        let Some(element_count) = array_element_count(&bounds) else {
            duplicate_declarations.push(base);
            return;
        };
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

        if let Some(udt_name) = explicit_udt_name
            && let Some(fields) = udt_defs.get(&udt_name)
        {
            for (field_name, field_ty) in fields {
                let alias = format!("{name}_{field_name}");
                if declarations
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&alias))
                {
                    duplicate_declarations.push(alias);
                    continue;
                }
                declarations.push(alias.clone());
                declaration_types.insert(alias, *field_ty);
            }
        }
    }
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

fn normalize_ident(text: &str) -> Option<String> {
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

fn collect_option_base(lines: &[String]) -> i32 {
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
                rank: bounds.len(),
                bounds: bounds.clone(),
                dynamic: redim_targets.contains(name),
            },
        );
    }
    descriptors
}

fn collect_redim_targets(stmts: &[BoundStmt], targets: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            BoundStmt::ReDim { name, .. } => {
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
    module_constants: &HashMap<String, i32>,
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

#[cfg(test)]
mod tests {
    use super::{
        BoundCompareMode, BoundCond, BoundExpr, BoundStmt, CompareOp, IntrinsicSurface,
        intrinsic_surface, resolve_symbols,
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
        let Some(BoundStmt::Call { name, args }) = main_proc.body.first() else {
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
        let Some(BoundStmt::Call { name, args }) = main_proc.body.first() else {
            panic!("expected call statement");
        };
        assert_eq!(name, "mathmodule_ping");
        assert!(args.is_empty());
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
        assert_eq!(ext.alias, "ping");
        assert!(ext.ptr_safe);
        assert!(!ext.ordinal_alias);
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
    fn resolve_declare_byref_parameter_adds_resolution_diagnostic() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByRef x As Long) As Long\nSub Main()\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.external_declarations.len(), 0);
        assert!(
            module
                .resolution_diagnostics
                .iter()
                .any(|diag| diag.contains("ByRef parameters are not supported"))
        );
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
}
