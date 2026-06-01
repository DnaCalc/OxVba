use thiserror::Error;

use crate::bytecode::Bytecode;
use crate::resolve::{self, BoundExpr};
use crate::{CompileError, compile};

/// Errors produced by the temporary CST-to-legacy bridge.
#[derive(Debug, Error)]
pub enum SyntaxBridgeError {
    #[error("syntax parse failed: {0}")]
    Syntax(String),
    #[error("unsupported syntax bridge shape: {0}")]
    Unsupported(String),
    #[error("legacy expression lowering failed for `{0}`")]
    LegacyExpression(String),
    #[error(transparent)]
    Compile(#[from] CompileError),
}

/// Lower one expression through the v2 CST validation step and into the current
/// legacy `BoundExpr` representation.
///
/// This is intentionally a bridge, not the final HIR binder. It proves that a
/// selected CST construct can be accepted by the new syntax layer and handed to
/// the old lowering representation while later FE beads build the real HIR.
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
    resolve::parse_expr_for_syntax_bridge(expression_source)
        .ok_or_else(|| SyntaxBridgeError::LegacyExpression(expression_source.to_string()))
}

/// Compile source after first validating that the v2 CST parser accepts it.
///
/// Production still uses the legacy compiler path. This bridge is a temporary
/// transition hook for fixtures that are known to be supported by both the CST
/// parser and the existing lowering.
pub fn compile_source_via_syntax_bridge(source: &str) -> Result<Bytecode, SyntaxBridgeError> {
    validate_source_with_cst(source)?;
    compile(source).map_err(SyntaxBridgeError::Compile)
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
