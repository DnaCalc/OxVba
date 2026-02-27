use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundExpr {
    IntConst(i32),
    Var(String),
    AddConst { var: String, delta: i32 },
    SubConst { var: String, delta: i32 },
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
    DoWhile {
        cond: BoundCond,
        body: Vec<BoundStmt>,
        post_check: bool,
    },
    ExitDo,
    OnErrorResumeNext,
    RaiseError(i32),
    Call {
        name: String,
        args: Vec<BoundExpr>,
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
}

pub fn resolve_symbols(source: &str) -> BoundModule {
    let mut option_explicit = false;
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('\''))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let has_explicit_procs = lines.iter().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.starts_with("sub ") || lower.starts_with("function ")
    });

    let procedures = if has_explicit_procs {
        parse_procedures(&lines, &mut option_explicit)
    } else {
        let mut declarations: Vec<String> = Vec::new();
        let mut array_bounds: HashMap<String, usize> = HashMap::new();
        let mut index = 0;
        let body = parse_block(
            &lines,
            &mut index,
            &mut declarations,
            &mut array_bounds,
            &mut option_explicit,
            &[],
        );
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

fn parse_procedures(lines: &[String], option_explicit: &mut bool) -> Vec<BoundProcedure> {
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

        let is_sub = lower.starts_with("sub ");
        let is_function = lower.starts_with("function ");
        if !(is_sub || is_function) {
            index += 1;
            continue;
        }

        let Some((name, params)) = parse_proc_signature(line, is_function) else {
            index += 1;
            continue;
        };

        index += 1;
        let mut declarations: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut array_bounds: HashMap<String, usize> = HashMap::new();
        let end_term = if is_function {
            "end function"
        } else {
            "end sub"
        };
        let body = parse_block(
            lines,
            &mut index,
            &mut declarations,
            &mut array_bounds,
            option_explicit,
            &[end_term],
        );
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

fn parse_proc_signature(line: &str, is_function: bool) -> Option<(String, Vec<BoundParam>)> {
    let prefix_len = if is_function { 9 } else { 4 };
    let rest = line.get(prefix_len..)?.trim();
    let name_token = rest
        .split('(')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let name = normalize_ident(name_token)?;
    let mut params = Vec::new();

    if let Some(open) = rest.find('(')
        && let Some(close) = rest.rfind(')')
        && close > open
    {
        let params_raw = rest[open + 1..close].trim();
        if !params_raw.is_empty() {
            for item in params_raw.split(',') {
                let token = item.trim();
                let lower = token.to_ascii_lowercase();
                let (by_ref, name_text) = if lower.starts_with("byval ") {
                    (false, token[6..].trim())
                } else if lower.starts_with("byref ") {
                    (true, token[6..].trim())
                } else {
                    (true, token)
                };
                let param_name = normalize_ident(name_text)?;
                params.push(BoundParam {
                    name: param_name,
                    by_ref,
                });
            }
        }
    }

    Some((name, params))
}

fn parse_block(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
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
        {
            *index += 1;
            continue;
        }

        if lower.starts_with("dim ") {
            parse_declaration(line, declarations, array_bounds);
            *index += 1;
            continue;
        }

        if lower.starts_with("if ") && lower.ends_with(" then") {
            out.push(parse_if_stmt(
                lines,
                index,
                declarations,
                array_bounds,
                option_explicit,
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
                line,
            ));
            continue;
        }

        if lower.starts_with("do while ") || lower == "do" {
            out.push(parse_do_stmt(
                lines,
                index,
                declarations,
                array_bounds,
                option_explicit,
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

        if lower.starts_with("error ")
            && let Ok(code) = line[6..].trim().parse::<i32>()
        {
            out.push(BoundStmt::RaiseError(code));
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
                line,
            ));
            continue;
        }

        out.push(parse_assign_or_unsupported(line, array_bounds));
        *index += 1;
    }

    out
}

fn parse_if_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
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
        &["elseif", "else", "end if"],
    );
    let Some(else_body) = parse_if_tail(lines, index, declarations, array_bounds, option_explicit)
    else {
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

fn parse_for_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
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

fn parse_assign_or_unsupported(line: &str, array_bounds: &HashMap<String, usize>) -> BoundStmt {
    if let Some((lhs_raw, rhs_raw)) = line.split_once('=')
        && let Some(target) = parse_reference_name(lhs_raw, array_bounds)
        && let Some(expr) = parse_expr(rhs_raw, array_bounds)
    {
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
) -> Option<(String, Vec<BoundExpr>)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }

    let name = normalize_ident(text[..open].trim())?;
    let args_raw = text[open + 1..close].trim();
    if args_raw.is_empty() {
        return Some((name, Vec::new()));
    }

    let mut args = Vec::new();
    for token in args_raw.split(',') {
        args.push(parse_expr(token.trim(), array_bounds)?);
    }
    Some((name, args))
}

fn parse_do_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
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

fn parse_select_case_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    array_bounds: &mut HashMap<String, usize>,
    option_explicit: &mut bool,
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
            &["elseif", "else", "end if"],
        );
        let nested_else = parse_if_tail(lines, index, declarations, array_bounds, option_explicit)?;
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
}
