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
    IfEq {
        lhs: BoundExpr,
        rhs: BoundExpr,
        then_body: Vec<BoundStmt>,
    },
    ForRange {
        var: String,
        start: BoundExpr,
        end: BoundExpr,
        body: Vec<BoundStmt>,
    },
    Unsupported {
        line: String,
    },
}

#[derive(Debug, Clone)]
pub struct BoundModule {
    pub source: String,
    pub option_explicit: bool,
    pub declarations: Vec<String>,
    pub body: Vec<BoundStmt>,
}

pub fn resolve_symbols(source: &str) -> BoundModule {
    let mut option_explicit = false;
    let mut declarations: Vec<String> = Vec::new();
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('\''))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut index = 0;
    let body = parse_block(
        &lines,
        &mut index,
        &mut declarations,
        &mut option_explicit,
        &[],
    );

    BoundModule {
        source: source.to_string(),
        option_explicit,
        declarations,
        body,
    }
}

fn parse_block(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
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

        if lower.starts_with("sub ") || lower == "end sub" {
            *index += 1;
            continue;
        }

        if lower.starts_with("dim ") {
            parse_declaration(line, declarations);
            *index += 1;
            continue;
        }

        if lower.starts_with("if ") && lower.ends_with(" then") {
            out.push(parse_if_stmt(
                lines,
                index,
                declarations,
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
                option_explicit,
                line,
            ));
            continue;
        }

        out.push(parse_assign_or_unsupported(line));
        *index += 1;
    }

    out
}

fn parse_if_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    option_explicit: &mut bool,
    line: &str,
) -> BoundStmt {
    let condition = line[2..line.len() - 4].trim();
    let Some((lhs_raw, rhs_raw)) = condition.split_once('=') else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };
    let Some(lhs) = parse_expr(lhs_raw) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };
    let Some(rhs) = parse_expr(rhs_raw) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let then_body = parse_block(lines, index, declarations, option_explicit, &["end if"]);

    if *index < lines.len() && lines[*index].eq_ignore_ascii_case("end if") {
        *index += 1;
        return BoundStmt::IfEq {
            lhs,
            rhs,
            then_body,
        };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_for_stmt(
    lines: &[String],
    index: &mut usize,
    declarations: &mut Vec<String>,
    option_explicit: &mut bool,
    line: &str,
) -> BoundStmt {
    let Some((var, start, end)) = parse_for_header(line) else {
        *index += 1;
        return BoundStmt::Unsupported {
            line: line.to_string(),
        };
    };

    *index += 1;
    let body = parse_block(lines, index, declarations, option_explicit, &["next"]);

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

fn parse_assign_or_unsupported(line: &str) -> BoundStmt {
    if let Some((lhs_raw, rhs_raw)) = line.split_once('=')
        && let Some(target) = normalize_ident(lhs_raw)
        && let Some(expr) = parse_expr(rhs_raw)
    {
        return BoundStmt::Assign { target, expr };
    }

    BoundStmt::Unsupported {
        line: line.to_string(),
    }
}

fn parse_for_header(line: &str) -> Option<(String, BoundExpr, BoundExpr)> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("for ") {
        return None;
    }

    let without_for = line[4..].trim();
    let (lhs_raw, range_raw) = without_for.split_once('=')?;
    let var = normalize_ident(lhs_raw)?;
    let (start_raw, end_raw) = split_ci(range_raw, " to ")?;
    let start = parse_expr(start_raw)?;
    let end = parse_expr(end_raw)?;
    Some((var, start, end))
}

fn parse_expr(text: &str) -> Option<BoundExpr> {
    let expr = text.trim();
    if let Ok(value) = expr.parse::<i32>() {
        return Some(BoundExpr::IntConst(value));
    }

    if let Some((left_raw, right_raw)) = expr.split_once('+') {
        let var = normalize_ident(left_raw)?;
        let delta = right_raw.trim().parse::<i32>().ok()?;
        return Some(BoundExpr::AddConst { var, delta });
    }

    if let Some((left_raw, right_raw)) = expr.split_once('-') {
        let var = normalize_ident(left_raw)?;
        let delta = right_raw.trim().parse::<i32>().ok()?;
        return Some(BoundExpr::SubConst { var, delta });
    }

    normalize_ident(expr).map(BoundExpr::Var)
}

fn parse_declaration(line: &str, declarations: &mut Vec<String>) {
    let remainder = line[4..].trim();
    let candidate = remainder
        .split(',')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    if let Some(name) = normalize_ident(candidate)
        && !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
    {
        declarations.push(name);
    }
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
        } else {
            lower_line == *term
        }
    })
}

fn split_ci<'a>(text: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let idx = lower.find(marker)?;
    let lhs = text[..idx].trim();
    let rhs = text[idx + marker.len()..].trim();
    Some((lhs, rhs))
}

#[cfg(test)]
mod tests {
    use super::{BoundExpr, BoundStmt, resolve_symbols};

    #[test]
    fn resolve_if_statement_into_structured_body() {
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = x + 2\nEnd If\nEnd Sub";
        let module = resolve_symbols(source);
        assert_eq!(module.declarations, vec!["x"]);
        assert!(
            module
                .body
                .iter()
                .any(|s| matches!(s, BoundStmt::IfEq { .. }))
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
}
