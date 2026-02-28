use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundExpr {
    IntConst(i32),
    Var(String),
    AddConst { var: String, delta: i32 },
    SubConst { var: String, delta: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallArg {
    pub name: Option<String>,
    pub expr: BoundExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundStmt {
    Assign {
        target: String,
        expr: BoundExpr,
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
        body: Vec<BoundStmt>,
    },
    ReDim {
        name: String,
        max_index: usize,
        preserve: bool,
    },
    DoWhile {
        cond: BoundCond,
        body: Vec<BoundStmt>,
        post_check: bool,
    },
    ExitDo,
    OnErrorResumeNext,
    OnErrorGoto0,
    OnErrorGotoLabel {
        label: String,
    },
    ResumeNext,
    RaiseError(i32),
    Label {
        name: String,
    },
    GoSub {
        label: String,
    },
    Return,
    Call {
        name: String,
        args: Vec<BoundCallArg>,
    },
    SelectCase {
        expr: BoundExpr,
        arms: Vec<(Vec<i32>, Vec<BoundStmt>)>,
        else_body: Vec<BoundStmt>,
    },
    Unsupported {
        line: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
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
    pub declarations: Vec<String>,
    pub body: Vec<BoundStmt>,
    pub procedures: Vec<BoundProcedure>,
}

#[derive(Debug, Clone)]
pub struct BoundProcedure {
    pub name: String,
    pub params: Vec<BoundParam>,
    pub declarations: Vec<String>,
    pub body: Vec<BoundStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundParam {
    pub name: String,
    pub by_ref: bool,
    pub optional: bool,
    pub default_value: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcKind {
    Sub,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

pub fn resolve_symbols(source: &str) -> BoundModule {
    let mut option_explicit = false;
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('\''))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let module_constants = collect_module_constants(&lines);
    let property_write_routes = collect_property_write_routes(&lines);

    let has_explicit_procs = lines
        .iter()
        .any(|line| detect_proc_kind(&line.to_ascii_lowercase()).is_some());

    let procedures = if has_explicit_procs {
        parse_procedures(
            &lines,
            &mut option_explicit,
            &module_constants,
            &property_write_routes,
        )
    } else {
        let mut declarations: Vec<String> = Vec::new();
        let mut array_bounds: HashMap<String, usize> = HashMap::new();
        let mut index = 0;
        for (name, _) in sorted_module_constants(&module_constants) {
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                declarations.push(name.clone());
            }
        }
        let mut body = parse_block(
            &lines,
            &mut index,
            &mut declarations,
            &mut array_bounds,
            &mut option_explicit,
            &module_constants,
            &property_write_routes,
            &[],
        );
        body.splice(0..0, build_const_prelude(&module_constants));
        vec![BoundProcedure {
            name: "main".to_string(),
            params: Vec::new(),
            declarations,
            body,
        }]
    };

    let entry_idx = procedures
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case("main"))
        .unwrap_or(0);
    let entry = procedures
        .get(entry_idx)
        .cloned()
        .unwrap_or(BoundProcedure {
            name: "main".to_string(),
            params: Vec::new(),
            declarations: Vec::new(),
            body: Vec::new(),
        });

    BoundModule {
        source: source.to_string(),
        option_explicit,
        declarations: entry.declarations.clone(),
        body: entry.body.clone(),
        procedures,
    }
}

fn parse_procedures(
    lines: &[String],
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
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

        let Some(kind) = detect_proc_kind(&lower) else {
            index += 1;
            continue;
        };

        let Some((name, params)) = parse_proc_signature(line, kind) else {
            index += 1;
            continue;
        };

        index += 1;
        let mut declarations: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        for (name, _) in sorted_module_constants(module_constants) {
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                declarations.push(name.clone());
            }
        }
        let mut array_bounds: HashMap<String, usize> = HashMap::new();
        let end_term = kind.end_term();
        let mut body = parse_block(
            lines,
            &mut index,
            &mut declarations,
            &mut array_bounds,
            option_explicit,
            module_constants,
            property_write_routes,
            &[end_term],
        );
        body.splice(0..0, build_const_prelude(module_constants));
        if index < lines.len() && lines[index].eq_ignore_ascii_case(end_term) {
            index += 1;
        }

        procedures.push(BoundProcedure {
            name,
            params,
            declarations,
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

fn parse_proc_signature(line: &str, kind: ProcKind) -> Option<(String, Vec<BoundParam>)> {
    let prefix_len = kind.prefix_len();
    let rest = line.get(prefix_len..)?.trim();
    let name = kind.canonical_name(parse_proc_base_name(line, kind)?);
    let mut params = Vec::new();
    let mut seen_optional = false;

    if let Some(open) = rest.find('(')
        && let Some(close) = rest.rfind(')')
        && close > open
    {
        let params_raw = rest[open + 1..close].trim();
        if !params_raw.is_empty() {
            for item in params_raw.split(',') {
                let mut token = item.trim();
                if token.is_empty() {
                    return None;
                }
                let mut optional = false;
                if token.to_ascii_lowercase().starts_with("optional ") {
                    optional = true;
                    token = token[9..].trim();
                }
                let lower = token.to_ascii_lowercase();
                let (by_ref, remainder) = if lower.starts_with("byval ") {
                    (false, token[6..].trim())
                } else if lower.starts_with("byref ") {
                    (true, token[6..].trim())
                } else {
                    (true, token)
                };
                let (name_text, default_value) = if let Some((lhs, rhs)) = remainder.split_once('=')
                {
                    (lhs.trim(), Some(parse_param_default(rhs.trim())?))
                } else {
                    (remainder, None)
                };

                if default_value.is_some() && !optional {
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

                let param_name = normalize_ident(name_text)?;
                params.push(BoundParam {
                    name: param_name,
                    by_ref,
                    optional,
                    default_value,
                });
            }
        }
    }

    Some((name, params))
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
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
    terminators: &[&str],
) -> Vec<BoundStmt> {
    let mut out = Vec::new();

    while *index < lines.len() {
        let line = lines[*index].as_str();
        let lower = line.to_ascii_lowercase();

        if matches_terminator(&lower, terminators) {
            break;
        }

        if lower == "option explicit" {
            *option_explicit = true;
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

        if lower.starts_with("dim ") {
            parse_declaration(line, declarations, array_bounds);
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
                array_bounds,
                option_explicit,
                module_constants,
                property_write_routes,
                line,
            ));
            continue;
        }

        if lower.starts_with("for ") {
            out.push(parse_for_stmt(
                lines,
                index,
                declarations,
                array_bounds,
                option_explicit,
                module_constants,
                property_write_routes,
                line,
            ));
            continue;
        }

        if lower.starts_with("redim ") {
            if let Some(stmt) = parse_redim_stmt(line, declarations, array_bounds) {
                out.push(stmt);
            } else {
                out.push(BoundStmt::Unsupported {
                    line: line.to_string(),
                });
            }
            *index += 1;
            continue;
        }

        if lower.starts_with("do while ") || lower == "do" {
            out.push(parse_do_stmt(
                lines,
                index,
                declarations,
                array_bounds,
                option_explicit,
                module_constants,
                property_write_routes,
                line,
            ));
            continue;
        }

        if lower == "exit do" {
            out.push(BoundStmt::ExitDo);
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
            if let Some(label) = normalize_ident(raw) {
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

        if lower.starts_with("error ")
            && let Ok(code) = line[6..].trim().parse::<i32>()
        {
            out.push(BoundStmt::RaiseError(code));
            *index += 1;
            continue;
        }

        if let Some(name) = parse_label_declaration(line) {
            out.push(BoundStmt::Label { name });
            *index += 1;
            continue;
        }

        if lower.starts_with("gosub ") {
            if let Some(label) = normalize_ident(line[6..].trim()) {
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
                array_bounds,
                option_explicit,
                module_constants,
                property_write_routes,
                line,
            ));
            continue;
        }

        out.push(parse_assign_or_unsupported(
            line,
            declarations,
            array_bounds,
            property_write_routes,
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
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
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
        array_bounds,
        option_explicit,
        module_constants,
        property_write_routes,
        &["elseif", "else", "end if"],
    );
    let Some(else_body) = parse_if_tail(
        lines,
        index,
        declarations,
        array_bounds,
        option_explicit,
        module_constants,
        property_write_routes,
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
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let Some((var, start, end)) = parse_for_header(line, array_bounds) else {
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
        array_bounds,
        option_explicit,
        module_constants,
        property_write_routes,
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
                body,
            };
        }
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_assign_or_unsupported(
    line: &str,
    declarations: &[String],
    array_bounds: &HashMap<String, usize>,
    property_write_routes: &HashMap<String, String>,
) -> BoundStmt {
    if let Some((lhs_raw, rhs_raw)) = line.split_once('=')
        && let Some(target) = parse_reference_name(lhs_raw, array_bounds)
        && let Some(expr) = parse_expr(rhs_raw, array_bounds)
    {
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
        return BoundStmt::Assign { target, expr };
    }

    let call_token = if line.to_ascii_lowercase().starts_with("call ") {
        line[5..].trim()
    } else {
        line.trim()
    };
    if let Some((name, args)) = parse_call_invocation(call_token, array_bounds) {
        return BoundStmt::Call { name, args };
    }
    if let Some(name) = normalize_ident(call_token) {
        return BoundStmt::Call {
            name,
            args: Vec::new(),
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_call_invocation(
    text: &str,
    array_bounds: &HashMap<String, usize>,
) -> Option<(String, Vec<BoundCallArg>)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    if !text[close + 1..].trim().is_empty() {
        return None;
    }

    let name = normalize_ident(text[..open].trim())?;
    let args_raw = text[open + 1..close].trim();
    if args_raw.is_empty() {
        return Some((name, Vec::new()));
    }

    let mut args = Vec::new();
    for token in args_raw.split(',') {
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

#[allow(clippy::too_many_arguments)]
fn parse_do_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
    line: &str,
) -> BoundStmt {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("do while ") {
        let condition = line[8..].trim();
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
            array_bounds,
            option_explicit,
            module_constants,
            property_write_routes,
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
            array_bounds,
            option_explicit,
            module_constants,
            property_write_routes,
            &["loop"],
        );
        if *index < lines.len() {
            let loop_line = lines[*index].as_str();
            let loop_lower = loop_line.to_ascii_lowercase();
            if loop_lower.starts_with("loop while ") {
                let condition = loop_line[11..].trim();
                if let Some(cond) = parse_condition(condition, array_bounds) {
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
    array_bounds: &HashMap<String, usize>,
) -> Option<(String, BoundExpr, BoundExpr)> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("for ") {
        return None;
    }

    let without_for = line[4..].trim();
    let (lhs_raw, range_raw) = without_for.split_once('=')?;
    let var = normalize_ident(lhs_raw)?;
    let (start_raw, end_raw) = split_ci(range_raw, " to ")?;
    let start = parse_expr(start_raw, array_bounds)?;
    let end = parse_expr(end_raw, array_bounds)?;
    Some((var, start, end))
}

fn parse_redim_stmt(
    line: &str,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
) -> Option<BoundStmt> {
    let mut payload = line[6..].trim();
    let mut preserve = false;
    if payload.to_ascii_lowercase().starts_with("preserve ") {
        preserve = true;
        payload = payload[9..].trim();
    }
    let (name, max_index) = parse_array_declaration(payload)?;
    array_bounds.insert(name.clone(), max_index);
    for idx in 0..=max_index {
        let alias = format!("{name}_{idx}");
        if !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&alias))
        {
            declarations.push(alias);
        }
    }

    Some(BoundStmt::ReDim {
        name,
        max_index,
        preserve,
    })
}

#[allow(clippy::too_many_arguments)]
fn parse_select_case_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
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
    let mut arms: Vec<(Vec<i32>, Vec<BoundStmt>)> = Vec::new();
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
                array_bounds,
                option_explicit,
                module_constants,
                property_write_routes,
                &["end select"],
            );
            continue;
        }

        if lower.starts_with("case ") {
            let values_raw = current[5..].trim();
            let mut values = Vec::new();
            for token in values_raw.split(',') {
                let trimmed = token.trim();
                let Ok(value) = trimmed.parse::<i32>() else {
                    return BoundStmt::Unsupported {
                        line: line.to_string(),
                    };
                };
                values.push(value);
            }

            *index += 1;
            let body = parse_block(
                lines,
                index,
                declarations,
                array_bounds,
                option_explicit,
                module_constants,
                property_write_routes,
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

fn parse_expr(text: &str, array_bounds: &HashMap<String, usize>) -> Option<BoundExpr> {
    let expr = text.trim();
    if let Ok(value) = expr.parse::<i32>() {
        return Some(BoundExpr::IntConst(value));
    }

    if let Some(inner) = parse_intrinsic_conversion_expr(expr, array_bounds) {
        return Some(inner);
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

fn parse_intrinsic_conversion_expr(
    expr: &str,
    array_bounds: &HashMap<String, usize>,
) -> Option<BoundExpr> {
    let open = expr.find('(')?;
    let close = expr.rfind(')')?;
    if close <= open || !expr[close + 1..].trim().is_empty() {
        return None;
    }
    let name = normalize_ident(expr[..open].trim())?;
    if !matches!(
        name.as_str(),
        "cint" | "clng" | "cdbl" | "cstr" | "cbool" | "cdate" | "val" | "str"
    ) {
        return None;
    }
    parse_expr(expr[open + 1..close].trim(), array_bounds)
}

fn parse_condition(text: &str, array_bounds: &HashMap<String, usize>) -> Option<BoundCond> {
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

fn parse_compare_condition(text: &str, array_bounds: &HashMap<String, usize>) -> Option<BoundCond> {
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

fn parse_declaration(
    line: &str,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
) {
    let remainder = line[4..].trim();
    let candidate = remainder
        .split(',')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    if let Some((base, max_index)) = parse_array_declaration(candidate) {
        array_bounds.insert(base.clone(), max_index);
        for idx in 0..=max_index {
            let alias = format!("{base}_{idx}");
            if !declarations
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&alias))
            {
                declarations.push(alias);
            }
        }
        return;
    }

    if let Some(name) = normalize_ident(candidate)
        && !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
    {
        declarations.push(name);
    }
}

fn parse_array_declaration(token: &str) -> Option<(String, usize)> {
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open {
        return None;
    }
    let base = normalize_ident(token[..open].trim())?;
    let max_index = token[open + 1..close].trim().parse::<usize>().ok()?;
    Some((base, max_index))
}

fn parse_reference_name(token: &str, array_bounds: &HashMap<String, usize>) -> Option<String> {
    if token.trim().eq_ignore_ascii_case("err.number") {
        return Some("err_number".to_string());
    }
    if let Some(alias) = parse_array_reference(token, array_bounds) {
        return Some(alias);
    }
    normalize_ident(token)
}

fn parse_array_reference(token: &str, array_bounds: &HashMap<String, usize>) -> Option<String> {
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open {
        return None;
    }
    let base = normalize_ident(token[..open].trim())?;
    let index = token[open + 1..close].trim().parse::<usize>().ok()?;
    let max = array_bounds.get(&base)?;
    if index > *max {
        return None;
    }
    Some(format!("{base}_{index}"))
}

fn normalize_ident(text: &str) -> Option<String> {
    let token = text.trim().trim_end_matches(',').trim();
    if token.is_empty() {
        return None;
    }
    if token.contains(char::is_whitespace) {
        return None;
    }

    let mut chars = token.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(token.to_ascii_lowercase())
}

fn parse_label_declaration(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.ends_with(':') {
        return None;
    }
    normalize_ident(&trimmed[..trimmed.len() - 1])
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

fn parse_if_tail(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
    module_constants: &HashMap<String, i32>,
    property_write_routes: &HashMap<String, String>,
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
            array_bounds,
            option_explicit,
            module_constants,
            property_write_routes,
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
            array_bounds,
            option_explicit,
            module_constants,
            property_write_routes,
            &["elseif", "else", "end if"],
        );
        let nested_else = parse_if_tail(
            lines,
            index,
            declarations,
            array_bounds,
            option_explicit,
            module_constants,
            property_write_routes,
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
    use super::{BoundCond, BoundExpr, BoundStmt, CompareOp, resolve_symbols};

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
    fn resolve_intrinsic_conversion_expression() {
        let source = "Sub Main()\nDim x\nx = CLng(CInt(7))\nEnd Sub";
        let module = resolve_symbols(source);
        let Some(BoundStmt::Assign { expr, .. }) = module.body.first() else {
            panic!("expected assignment");
        };
        assert_eq!(expr, &BoundExpr::IntConst(7));
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
    fn resolve_module_const_injects_const_prelude() {
        let source = "Const BASE = 5\nSub Main()\nDim x\nx = BASE + 2\nEnd Sub";
        let module = resolve_symbols(source);
        assert!(module.declarations.iter().any(|d| d == "base"));
        let Some(BoundStmt::Assign { target, expr }) = module.body.first() else {
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
            |s| matches!(s, BoundStmt::Assign { target, expr } if target == "safe" && expr == &BoundExpr::IntConst(4))
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
}
