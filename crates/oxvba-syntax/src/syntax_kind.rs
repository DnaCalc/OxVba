#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxKind {
    Identifier,
    Number,
    StringLiteral,
    Keyword,
    Symbol,
    Trivia,
    EndOfFile,
}
