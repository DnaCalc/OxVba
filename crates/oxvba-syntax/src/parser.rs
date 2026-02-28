use thiserror::Error;

use crate::green::GreenNode;
use crate::lexer;

#[derive(Debug, Clone)]
pub struct SyntaxTree {
    pub source: String,
    pub root: GreenNode,
}

impl SyntaxTree {
    pub fn to_source(&self) -> &str {
        &self.source
    }
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("input is empty")]
    EmptyInput,
}

pub fn parse(source: &str) -> Result<SyntaxTree, ParseError> {
    if source.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let tokens = lexer::tokenize(source);
    let root = GreenNode::from_tokens(tokens.iter().map(|t| t.text.as_str()));

    Ok(SyntaxTree {
        source: source.to_string(),
        root,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parse_non_empty_source() {
        let source = "Sub Main()\nEnd Sub";
        let tree = parse(source).expect("parser should accept non-empty source");
        assert_eq!(tree.source, source);
        assert_eq!(tree.to_source(), source);
        assert!(tree.root.width > 0);
    }

    #[test]
    fn reject_empty_source() {
        let err = parse("").expect_err("empty source should fail");
        assert_eq!(err.to_string(), "input is empty");
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::parse;

    #[kani::proof]
    fn parse_non_empty_source_roundtrips_input() {
        let len: usize = kani::any();
        kani::assume(len > 0);
        kani::assume(len <= 24);

        let mut source = String::new();
        for _ in 0..len {
            let b: u8 = kani::any();
            let ascii = 32 + (b % 95);
            source.push(char::from(ascii));
        }

        let tree = parse(&source).expect("non-empty source should parse");
        assert_eq!(tree.to_source(), source);
    }
}
