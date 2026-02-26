use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: String,
}

pub fn tokenize(source: &str) -> Vec<Token> {
    // Phase-0/1 placeholder tokenizer with deterministic behavior for scaffolding.
    let mut tokens = Vec::new();
    for word in source.split_whitespace() {
        let kind = if word.chars().all(|c| c.is_ascii_digit()) {
            SyntaxKind::Number
        } else {
            SyntaxKind::Identifier
        };
        tokens.push(Token {
            kind,
            text: word.to_string(),
        });
    }
    tokens.push(Token {
        kind: SyntaxKind::EndOfFile,
        text: String::new(),
    });
    tokens
}
