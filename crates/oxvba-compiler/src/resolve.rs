#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundOp {
    AssignConst { name: String, value: i32 },
    AddConst { name: String, value: i32 },
    Unsupported { line: String },
}

#[derive(Debug, Clone)]
pub struct BoundModule {
    pub source: String,
    pub option_explicit: bool,
    pub declarations: Vec<String>,
    pub ops: Vec<BoundOp>,
}

pub fn resolve_symbols(source: &str) -> BoundModule {
    let mut option_explicit = false;
    let mut declarations: Vec<String> = Vec::new();
    let mut ops: Vec<BoundOp> = Vec::new();

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('\'') {
            continue;
        }

        let lower = line.to_ascii_lowercase();
        if lower.starts_with("sub ") || lower == "end sub" {
            continue;
        }

        if lower == "option explicit" {
            option_explicit = true;
            continue;
        }

        if lower.starts_with("dim ") {
            if let Some(name) = line[4..].split_whitespace().next() {
                let normalized = name.to_ascii_lowercase();
                if !declarations
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&normalized))
                {
                    declarations.push(normalized);
                }
            }
            continue;
        }

        if let Some((lhs_raw, rhs_raw)) = line.split_once('=') {
            let lhs = lhs_raw.trim().to_ascii_lowercase();
            let rhs = rhs_raw.trim();

            if let Ok(value) = rhs.parse::<i32>() {
                ops.push(BoundOp::AssignConst { name: lhs, value });
                continue;
            }

            if let Some((left, right)) = rhs.split_once('+') {
                let left = left.trim();
                let right = right.trim();
                if left.eq_ignore_ascii_case(&lhs)
                    && let Ok(value) = right.parse::<i32>()
                {
                    ops.push(BoundOp::AddConst { name: lhs, value });
                    continue;
                }
            }
        }

        ops.push(BoundOp::Unsupported {
            line: line.to_string(),
        });
    }

    BoundModule {
        source: source.to_string(),
        option_explicit,
        declarations,
        ops,
    }
}
