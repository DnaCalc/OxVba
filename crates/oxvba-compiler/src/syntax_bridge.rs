use thiserror::Error;

use crate::bytecode::Bytecode;
use crate::resolve::{ArithOp, BoundCallArg, BoundExpr, CompareOp, LogicalBinOp, normalize_ident};
use crate::{CompileError, compile};
use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// Errors produced by the temporary CST-to-legacy bridge.
#[derive(Debug, Error)]
pub enum SyntaxBridgeError {
    #[error("syntax parse failed: {0}")]
    Syntax(String),
    #[error("unsupported syntax bridge shape: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Compile(#[from] CompileError),
}

/// Lower one expression through the v2 CST validation step and into the current
/// legacy `BoundExpr` representation.
///
/// This is intentionally a bridge, not the final HIR binder. It proves that a
/// selected CST construct can be accepted by the new syntax layer and lowered
/// from the CST shape while later FE beads build the real HIR.
pub fn lower_expression_to_legacy_bound_expr(
    expression_source: &str,
) -> Result<BoundExpr, SyntaxBridgeError> {
    let wrapper = format!("Sub __Bridge()\n    __v = {expression_source}\nEnd Sub\n");
    let parsed = oxvba_syntax::parse(&wrapper);
    if !parsed.errors().is_empty() {
        return Err(SyntaxBridgeError::Syntax(format!("{:?}", parsed.errors())));
    }
    if !has_node_kind(&parsed.syntax(), oxvba_syntax::SyntaxKind::AssignStmt) {
        return Err(SyntaxBridgeError::Unsupported(
            "expression wrapper did not produce an assignment statement".to_string(),
        ));
    }
    let assign = find_node_kind(&parsed.syntax(), SyntaxKind::AssignStmt).ok_or_else(|| {
        SyntaxBridgeError::Unsupported("expression wrapper did not expose assignment".to_string())
    })?;
    let expr = assign
        .child_nodes()
        .into_iter()
        .rev()
        .find(|node| is_expression_node(node.kind()))
        .ok_or_else(|| {
            SyntaxBridgeError::Unsupported(
                "expression wrapper did not expose assignment RHS".to_string(),
            )
        })?;
    lower_cst_expr(expr)
}

/// Compile source after first validating that the v2 CST parser accepts it.
///
/// Production still uses the legacy compiler path. This bridge is a temporary
/// transition hook for fixtures that are known to be supported by both the CST
/// parser and the existing lowering.
pub fn compile_source_via_syntax_bridge(source: &str) -> Result<Bytecode, SyntaxBridgeError> {
    validate_source_with_cst(source)?;
    match compile(source) {
        Ok(bytecode) => Ok(bytecode),
        Err(first_error) => {
            let lowered = lower_statement_separators_for_legacy(source);
            if lowered == source {
                return Err(SyntaxBridgeError::Compile(first_error));
            }
            compile(&lowered).map_err(SyntaxBridgeError::Compile)
        }
    }
}

pub fn validate_source_with_cst(source: &str) -> Result<(), SyntaxBridgeError> {
    let parsed = oxvba_syntax::parse(source);
    if !parsed.errors().is_empty() {
        return Err(SyntaxBridgeError::Syntax(format!("{:?}", parsed.errors())));
    }
    Ok(())
}

fn has_node_kind(node: &oxvba_syntax::SyntaxNode<'_>, kind: oxvba_syntax::SyntaxKind) -> bool {
    if node.kind() == kind {
        return true;
    }
    node.child_nodes()
        .iter()
        .any(|child| has_node_kind(child, kind))
}

fn lower_statement_separators_for_legacy(source: &str) -> String {
    let mut lowered = String::with_capacity(source.len());
    for (kind, text) in oxvba_syntax::lexer::tokenize(source) {
        match kind {
            SyntaxKind::Colon => lowered.push('\n'),
            SyntaxKind::Eof => {}
            _ => lowered.push_str(text),
        }
    }
    lowered
}

fn find_node_kind<'a>(node: &SyntaxNode<'a>, kind: SyntaxKind) -> Option<SyntaxNode<'a>> {
    if node.kind() == kind {
        return Some(*node);
    }
    node.child_nodes()
        .into_iter()
        .find_map(|child| find_node_kind(&child, kind))
}

fn is_expression_node(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::BinaryExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::IdentExpr
            | SyntaxKind::LiteralExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::NewExpr
            | SyntaxKind::MemberExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::CallExpr
    )
}

fn lower_cst_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    match node.kind() {
        SyntaxKind::LiteralExpr => lower_literal_expr(node),
        SyntaxKind::IdentExpr => lower_ident_expr(node),
        SyntaxKind::ParenExpr => {
            let inner = expression_children(node)
                .into_iter()
                .next()
                .ok_or_else(|| unsupported_expr(node, "empty parenthesized expression"))?;
            lower_cst_expr(inner)
        }
        SyntaxKind::MemberExpr => lower_member_expr(node),
        SyntaxKind::IndexExpr => lower_index_expr(node),
        SyntaxKind::UnaryExpr => lower_unary_expr(node),
        SyntaxKind::BinaryExpr => lower_binary_expr(node),
        other => Err(unsupported_expr(
            node,
            &format!("unsupported expression node {other:?}"),
        )),
    }
}

fn lower_literal_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    let token = first_nontrivia_token(node)
        .ok_or_else(|| unsupported_expr(node, "literal expression without token"))?;
    match token.kind {
        SyntaxKind::IntLiteral => parse_int_literal(token.text).map(BoundExpr::IntConst),
        SyntaxKind::HexLiteral => parse_prefixed_int_literal(token.text, "&h", 16),
        SyntaxKind::OctLiteral => parse_prefixed_int_literal(token.text, "&o", 8),
        SyntaxKind::FloatLiteral => parse_float_literal(token.text).map(BoundExpr::FloatConst),
        SyntaxKind::StringLiteral => parse_string_literal(token.text).map(BoundExpr::StringConst),
        SyntaxKind::KwTrue => Ok(BoundExpr::BoolConst(true)),
        SyntaxKind::KwFalse => Ok(BoundExpr::BoolConst(false)),
        SyntaxKind::KwEmpty => Ok(BoundExpr::IntrinsicCall {
            name: "__empty".to_string(),
            args: Vec::new(),
        }),
        SyntaxKind::KwNull => Ok(BoundExpr::IntrinsicCall {
            name: "__null".to_string(),
            args: Vec::new(),
        }),
        SyntaxKind::KwNothing => Ok(BoundExpr::IntrinsicCall {
            name: "__nothing".to_string(),
            args: Vec::new(),
        }),
        _ => Err(unsupported_expr(
            node,
            &format!("unsupported literal token {:?}", token.kind),
        )),
    }
}

fn lower_ident_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    let mut text = String::new();
    for token in nontrivia_tokens(node) {
        text.push_str(token.text);
    }
    normalize_ident(&text)
        .map(BoundExpr::Var)
        .ok_or_else(|| unsupported_expr(node, &format!("unsupported identifier `{text}`")))
}

fn lower_member_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    let receiver_node = expression_children(node)
        .into_iter()
        .next()
        .ok_or_else(|| unsupported_expr(node, "member expression without receiver"))?;
    let receiver = Box::new(lower_cst_expr(receiver_node)?);
    let member = direct_member_name(node)
        .and_then(|token| normalize_member_token(token.text))
        .ok_or_else(|| unsupported_expr(node, "member expression without supported member name"))?;
    Ok(BoundExpr::Member {
        receiver,
        member,
        args: Vec::new(),
    })
}

fn lower_index_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    let target_node = expression_children(node)
        .into_iter()
        .next()
        .ok_or_else(|| unsupported_expr(node, "index/call expression without target"))?;
    let args = lower_arg_list(node)?;
    match lower_cst_expr(target_node)? {
        BoundExpr::Var(name) => Ok(BoundExpr::ProcCall { name, args }),
        BoundExpr::Member {
            receiver, member, ..
        } => Ok(BoundExpr::Member {
            receiver,
            member,
            args,
        }),
        other => Err(SyntaxBridgeError::Unsupported(format!(
            "unsupported indexed target `{}` lowered as {other:?}",
            target_node.text().trim()
        ))),
    }
}

fn lower_unary_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    let op = first_nontrivia_token(node)
        .ok_or_else(|| unsupported_expr(node, "unary expression without operator"))?;
    let operand = expression_children(node)
        .into_iter()
        .next()
        .ok_or_else(|| unsupported_expr(node, "unary expression without operand"))?;
    let operand = Box::new(lower_cst_expr(operand)?);
    match op.kind {
        SyntaxKind::Minus => Ok(BoundExpr::UnaryOp {
            op: ArithOp::Neg,
            operand,
        }),
        SyntaxKind::KwNot => Ok(BoundExpr::LogicalNot { operand }),
        _ => Err(unsupported_expr(
            node,
            &format!("unsupported unary operator {:?}", op.kind),
        )),
    }
}

fn lower_binary_expr(node: SyntaxNode<'_>) -> Result<BoundExpr, SyntaxBridgeError> {
    let exprs = expression_children(node);
    if exprs.len() < 2 {
        return Err(unsupported_expr(
            node,
            "binary expression without two expression children",
        ));
    }
    let lhs = lower_cst_expr(exprs[0])?;
    let rhs_node = exprs[1];
    let operator = direct_operator_token(node)
        .ok_or_else(|| unsupported_expr(node, "binary expression without direct operator token"))?;

    if operator.kind == SyntaxKind::KwIs
        && first_nontrivia_token(node).is_some_and(|token| token.kind == SyntaxKind::KwTypeOf)
    {
        return Ok(BoundExpr::IntrinsicCall {
            name: "typeofis".to_string(),
            args: vec![
                lhs,
                BoundExpr::StringConst(rhs_node.text().trim().to_string()),
            ],
        });
    }

    let rhs = lower_cst_expr(rhs_node)?;
    match operator.kind {
        SyntaxKind::Plus => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Add,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Minus => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Sub,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Star => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Mul,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Slash => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Div,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Backslash => Ok(BoundExpr::BinaryOp {
            op: ArithOp::IntDiv,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Caret => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Pow,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::KwMod => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Mod,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Ampersand => Ok(BoundExpr::BinaryOp {
            op: ArithOp::Concat,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }),
        SyntaxKind::Eq => compare_expr(CompareOp::Eq, lhs, rhs),
        SyntaxKind::LtGt => compare_expr(CompareOp::Ne, lhs, rhs),
        SyntaxKind::Lt => compare_expr(CompareOp::Lt, lhs, rhs),
        SyntaxKind::LtEq => compare_expr(CompareOp::Le, lhs, rhs),
        SyntaxKind::Gt => compare_expr(CompareOp::Gt, lhs, rhs),
        SyntaxKind::GtEq => compare_expr(CompareOp::Ge, lhs, rhs),
        SyntaxKind::KwLike => compare_expr(CompareOp::Like, lhs, rhs),
        SyntaxKind::KwIs => Err(unsupported_expr(
            node,
            "bare object `Is` comparison needs binder/object identity lowering",
        )),
        SyntaxKind::KwAnd => logical_expr(LogicalBinOp::And, lhs, rhs),
        SyntaxKind::KwOr => logical_expr(LogicalBinOp::Or, lhs, rhs),
        _ => Err(unsupported_expr(
            node,
            &format!("unsupported binary operator {:?}", operator.kind),
        )),
    }
}

fn compare_expr(
    op: CompareOp,
    lhs: BoundExpr,
    rhs: BoundExpr,
) -> Result<BoundExpr, SyntaxBridgeError> {
    Ok(BoundExpr::CompareOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn logical_expr(
    op: LogicalBinOp,
    lhs: BoundExpr,
    rhs: BoundExpr,
) -> Result<BoundExpr, SyntaxBridgeError> {
    Ok(BoundExpr::LogicalBinaryOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn lower_arg_list(node: SyntaxNode<'_>) -> Result<Vec<BoundCallArg>, SyntaxBridgeError> {
    let arg_list = node
        .child_nodes()
        .into_iter()
        .find(|child| child.kind() == SyntaxKind::ArgList)
        .ok_or_else(|| unsupported_expr(node, "index/call expression without argument list"))?;
    expression_children(arg_list)
        .into_iter()
        .map(|arg| {
            lower_cst_expr(arg).map(|expr| BoundCallArg {
                name: None,
                expr,
                force_byval: false,
            })
        })
        .collect()
}

fn expression_children(node: SyntaxNode<'_>) -> Vec<SyntaxNode<'_>> {
    node.child_nodes()
        .into_iter()
        .filter(|child| is_expression_node(child.kind()))
        .collect()
}

fn direct_member_name(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    let mut after_member_operator = false;
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if token.kind == SyntaxKind::Dot || token.kind == SyntaxKind::Bang =>
            {
                after_member_operator = true;
            }
            SyntaxElement::Token(token)
                if after_member_operator
                    && !token.kind.is_trivia()
                    && (token.kind == SyntaxKind::Ident
                        || token.kind == SyntaxKind::BracketedIdent
                        || token.kind.is_keyword()) =>
            {
                return Some(token);
            }
            SyntaxElement::Token(token) if !token.kind.is_trivia() => {
                after_member_operator = false;
            }
            _ => {}
        }
    }
    None
}

fn normalize_member_token(text: &str) -> Option<String> {
    if let Some(unbracketed) = text
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return normalize_ident(unbracketed);
    }
    normalize_ident(text)
}

fn first_nontrivia_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    nontrivia_tokens(node).into_iter().next()
}

fn nontrivia_tokens(node: SyntaxNode<'_>) -> Vec<SyntaxToken<'_>> {
    node.child_tokens()
        .into_iter()
        .filter(|token| !token.kind.is_trivia())
        .collect()
}

fn direct_operator_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    node.children()
        .into_iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if is_expression_operator(token.kind) => Some(token),
            _ => None,
        })
}

fn is_expression_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Backslash
            | SyntaxKind::Caret
            | SyntaxKind::Ampersand
            | SyntaxKind::Eq
            | SyntaxKind::Lt
            | SyntaxKind::Gt
            | SyntaxKind::LtEq
            | SyntaxKind::GtEq
            | SyntaxKind::LtGt
            | SyntaxKind::KwAnd
            | SyntaxKind::KwOr
            | SyntaxKind::KwMod
            | SyntaxKind::KwLike
            | SyntaxKind::KwIs
    )
}

fn parse_int_literal(text: &str) -> Result<i32, SyntaxBridgeError> {
    let trimmed = text.trim_end_matches(['%', '&', '^']);
    trimmed.parse::<i32>().map_err(|_| {
        SyntaxBridgeError::Unsupported(format!("unsupported integer literal `{text}`"))
    })
}

fn parse_prefixed_int_literal(
    text: &str,
    prefix: &str,
    radix: u32,
) -> Result<BoundExpr, SyntaxBridgeError> {
    let trimmed = text.trim_end_matches(['%', '&', '^']);
    let digits = trimmed
        .get(prefix.len()..)
        .ok_or_else(|| SyntaxBridgeError::Unsupported(format!("unsupported literal `{text}`")))?;
    i32::from_str_radix(digits, radix)
        .map(BoundExpr::IntConst)
        .map_err(|_| SyntaxBridgeError::Unsupported(format!("unsupported literal `{text}`")))
}

fn parse_float_literal(text: &str) -> Result<u64, SyntaxBridgeError> {
    let trimmed = text.trim_end_matches(['!', '#', '@']);
    trimmed
        .parse::<f64>()
        .map(f64::to_bits)
        .map_err(|_| SyntaxBridgeError::Unsupported(format!("unsupported float literal `{text}`")))
}

fn parse_string_literal(text: &str) -> Result<String, SyntaxBridgeError> {
    if !text.starts_with('"') || !text.ends_with('"') || text.len() < 2 {
        return Err(SyntaxBridgeError::Unsupported(format!(
            "unsupported string literal `{text}`"
        )));
    }
    Ok(text[1..text.len() - 1].replace("\"\"", "\""))
}

fn unsupported_expr(node: SyntaxNode<'_>, reason: &str) -> SyntaxBridgeError {
    SyntaxBridgeError::Unsupported(format!("{reason}: `{}`", node.text().trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Instruction;
    use crate::resolve::ArithOp;

    #[test]
    fn bridge_lowers_expression_cst_to_legacy_bound_expr() {
        let expr = lower_expression_to_legacy_bound_expr("1 + 2 * 3")
            .expect("expression should lower through bridge");
        match expr {
            BoundExpr::BinaryOp {
                op: ArithOp::Add,
                rhs,
                ..
            } => {
                assert!(
                    matches!(
                        *rhs,
                        BoundExpr::BinaryOp {
                            op: ArithOp::Mul,
                            ..
                        }
                    ),
                    "expected multiplication on RHS, got {rhs:?}"
                );
            }
            other => panic!("expected additive expression, got {other:?}"),
        }
    }

    #[test]
    fn bridge_lowers_expression_parity_scope_from_cst_without_legacy_expr_parser() {
        let pow = lower_expression_to_legacy_bound_expr("2 ^ 3 ^ 4")
            .expect("power expression should lower");
        match pow {
            BoundExpr::BinaryOp {
                op: ArithOp::Pow,
                rhs,
                ..
            } => assert!(
                matches!(
                    *rhs,
                    BoundExpr::BinaryOp {
                        op: ArithOp::Pow,
                        ..
                    }
                ),
                "expected right-associative power RHS, got {rhs:?}"
            ),
            other => panic!("expected power expression, got {other:?}"),
        }

        let compare = lower_expression_to_legacy_bound_expr("(name Like \"A*\") And Not done")
            .expect("logical comparison expression should lower");
        assert!(
            matches!(
                compare,
                BoundExpr::LogicalBinaryOp {
                    op: crate::resolve::LogicalBinOp::And,
                    ..
                }
            ),
            "expected top-level logical And, got {compare:?}"
        );

        let type_of = lower_expression_to_legacy_bound_expr("TypeOf obj Is Class1")
            .expect("TypeOf expression should lower");
        assert!(
            matches!(type_of, BoundExpr::IntrinsicCall { ref name, ref args } if name == "typeofis" && args.len() == 2),
            "expected typeofis intrinsic expression, got {type_of:?}"
        );

        let bare_is = lower_expression_to_legacy_bound_expr("a Is b")
            .expect_err("bare object identity needs binder lowering");
        assert!(
            bare_is.to_string().contains("bare object `Is` comparison"),
            "unexpected error: {bare_is}"
        );
    }

    #[test]
    fn bridge_lowers_postfix_scope_from_cst() {
        let call = lower_expression_to_legacy_bound_expr("Name$(1, 2)")
            .expect("keyword-colliding suffixed call should lower");
        assert!(
            matches!(call, BoundExpr::ProcCall { ref name, ref args } if name == "name" && args.len() == 2),
            "expected ProcCall, got {call:?}"
        );

        let member_call = lower_expression_to_legacy_bound_expr("obj.Method(1)")
            .expect("member call should lower");
        assert!(
            matches!(member_call, BoundExpr::Member { ref member, ref args, .. } if member == "method" && args.len() == 1),
            "expected member call, got {member_call:?}"
        );

        let chain = lower_expression_to_legacy_bound_expr("obj!Field(0).Value")
            .expect("bang/index/member chain should lower");
        assert!(
            matches!(chain, BoundExpr::Member { ref member, .. } if member == "value"),
            "expected outer Value member, got {chain:?}"
        );
    }

    #[test]
    fn bridge_validates_statement_coverage_corpus_with_cst() {
        let sources = [
            "Attribute VB_Name = \"Module1\"\nSub T()\nEnd Sub\n",
            "Sub T()\n    x = 1: y = 2: RaiseEvent Tick\nEnd Sub\n",
            "Sub T()\n    On Error Resume Next: Resume Next\nEnd Sub\n",
            "Sub T()\n    With obj\n        .Value = 1\n    End With\nEnd Sub\n",
            "Public Property Get Value() As Long\n    Value = 1\nEnd Property\n",
            "Declare PtrSafe Function GetTickCount Lib \"kernel32\" () As Long\n",
            "Type Point\n    X As Long\n    Y As Long\nEnd Type\n",
            "Enum Color\n    Red = 1\n    Blue = 2\nEnd Enum\n",
        ];

        for source in sources {
            validate_source_with_cst(source).unwrap_or_else(|err| {
                panic!("statement corpus should validate: {source:?}: {err}")
            });
        }
    }

    #[test]
    fn bridge_compiles_supported_statement_sequence_after_cst_validation() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1\n    x = x + 1\nEnd Sub\n";
        let bytecode = compile_source_via_syntax_bridge(source)
            .expect("multiline assignment sequence should compile through bridge");
        assert!(
            !bytecode.instructions.is_empty(),
            "expected bytecode for assignment sequence"
        );
    }

    #[test]
    fn bridge_lowers_inline_statement_separators_for_legacy_compile() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        validate_source_with_cst(source).expect("CST parser should accept inline statements");
        let bytecode = compile_source_via_syntax_bridge(source)
            .expect("bridge should lower colon separators before legacy compile");
        assert!(
            !bytecode.instructions.is_empty(),
            "expected bytecode for inline assignment sequence"
        );
    }

    #[test]
    fn bridge_rejects_recovered_syntax_errors_before_legacy_lowering() {
        let source = "Sub Main()\n    x = \n    y = 2\nEnd Sub\n";
        let parsed = oxvba_syntax::parse(source);
        assert_eq!(parsed.syntax().text(), source);
        assert!(
            parsed
                .errors()
                .iter()
                .any(|error| error.message == "expected expression after `=`"),
            "expected missing-expression parse diagnostic, got {:?}",
            parsed.errors()
        );
        assert!(
            has_node_kind(&parsed.syntax(), SyntaxKind::ErrorNode),
            "expected recovery ErrorNode"
        );

        let err = compile_source_via_syntax_bridge(source)
            .expect_err("CST diagnostics should stop bridge before legacy lowering");
        assert!(
            err.to_string().contains("syntax parse failed")
                && err.to_string().contains("expected expression after `=`"),
            "unexpected bridge error: {err}"
        );
    }

    #[test]
    fn bridge_compiles_supported_assignment_family_through_legacy_lowering() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let bytecode =
            compile_source_via_syntax_bridge(source).expect("bridge compile should succeed");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::CopySlot { .. } | Instruction::AddConstI32 { .. }
            )),
            "expected compiled assignment/arithmetic bytecode: {:?}",
            bytecode.instructions
        );
    }
}
