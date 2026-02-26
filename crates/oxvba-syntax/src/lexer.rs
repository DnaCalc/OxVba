use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: String,
}

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token {
                kind: SyntaxKind::Trivia,
                text,
            });
            continue;
        }

        if c == '\'' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token {
                kind: SyntaxKind::Trivia,
                text,
            });
            continue;
        }

        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let lower = text.to_ascii_lowercase();
            let kind = match lower.as_str() {
                "sub" | "end" | "dim" | "if" | "then" | "else" => SyntaxKind::Keyword,
                _ => SyntaxKind::Identifier,
            };
            tokens.push(Token { kind, text });
            continue;
        }

        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token {
                kind: SyntaxKind::Number,
                text,
            });
            continue;
        }

        if c == '"' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token {
                kind: SyntaxKind::StringLiteral,
                text,
            });
            continue;
        }

        tokens.push(Token {
            kind: SyntaxKind::Symbol,
            text: c.to_string(),
        });
        i += 1;
    }

    tokens.push(Token {
        kind: SyntaxKind::EndOfFile,
        text: String::new(),
    });
    tokens
}

#[cfg(test)]
mod tests {
    use super::tokenize;
    use crate::syntax_kind::SyntaxKind;

    #[test]
    fn tokenizes_keywords_identifiers_and_numbers() {
        let toks = tokenize("Sub Main\nDim x 42");
        assert_eq!(toks[0].kind, SyntaxKind::Keyword);
        assert_eq!(toks[2].kind, SyntaxKind::Identifier);
        assert!(toks.iter().any(|t| t.kind == SyntaxKind::Number));
    }

    #[test]
    fn tokenizes_string_and_comment() {
        let toks = tokenize("Dim s\ns = \"abc\" ' c");
        assert!(toks.iter().any(|t| t.kind == SyntaxKind::StringLiteral));
        assert!(toks.iter().any(|t| t.text.starts_with('\'')));
    }
}
