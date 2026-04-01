use turbo_vision::views::syntax::{SyntaxHighlighter, Token, TokenType};

pub(crate) struct OxvbaHighlighter;

impl OxvbaHighlighter {
    pub(crate) fn new() -> Self {
        Self
    }

    fn is_keyword(word: &str) -> bool {
        matches!(
            word,
            "as" | "byref"
                | "byval"
                | "call"
                | "case"
                | "const"
                | "declare"
                | "dim"
                | "do"
                | "each"
                | "else"
                | "elseif"
                | "end"
                | "enum"
                | "erase"
                | "event"
                | "exit"
                | "for"
                | "function"
                | "get"
                | "gosub"
                | "goto"
                | "if"
                | "implements"
                | "in"
                | "let"
                | "loop"
                | "me"
                | "new"
                | "next"
                | "not"
                | "nothing"
                | "on"
                | "option"
                | "optional"
                | "paramarray"
                | "preserve"
                | "print"
                | "private"
                | "property"
                | "public"
                | "raiseevent"
                | "redim"
                | "resume"
                | "select"
                | "set"
                | "static"
                | "step"
                | "sub"
                | "then"
                | "to"
                | "until"
                | "wend"
                | "while"
                | "with"
                | "withevents"
        )
    }

    fn is_type_name(word: &str) -> bool {
        matches!(
            word,
            "boolean"
                | "byte"
                | "collection"
                | "currency"
                | "date"
                | "decimal"
                | "double"
                | "integer"
                | "long"
                | "longlong"
                | "longptr"
                | "object"
                | "single"
                | "string"
                | "variant"
        )
    }

    fn is_word_char(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_'
    }

    fn is_comment_rem(chars: &[char], start: usize, end: usize) -> bool {
        if end - start != 3 {
            return false;
        }
        let lower: String = chars[start..end]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        if lower != "rem" {
            return false;
        }
        let prev_is_word = start > 0 && Self::is_word_char(chars[start - 1]);
        let next_is_word = end < chars.len() && Self::is_word_char(chars[end]);
        !prev_is_word && !next_is_word
    }
}

impl SyntaxHighlighter for OxvbaHighlighter {
    fn language(&self) -> &str {
        "oxvba"
    }

    fn highlight_line(&self, line: &str, _line_number: usize) -> Vec<Token> {
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;
        let mut tokens = Vec::new();
        let mut expect_decl_name = false;

        while i < len {
            let ch = chars[i];

            if ch.is_whitespace() {
                let start = i;
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }
                tokens.push(Token::new(start, i, TokenType::Normal));
                continue;
            }

            if ch == '\'' {
                tokens.push(Token::new(i, len, TokenType::Comment));
                break;
            }

            if ch == '"' {
                let start = i;
                i += 1;
                while i < len {
                    if chars[i] == '"' {
                        if i + 1 < len && chars[i + 1] == '"' {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            if ch.is_ascii_digit() {
                let start = i;
                i += 1;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            if ch.is_ascii_alphabetic() || ch == '_' {
                let start = i;
                i += 1;
                while i < len && Self::is_word_char(chars[i]) {
                    i += 1;
                }
                if Self::is_comment_rem(&chars, start, i) {
                    tokens.push(Token::new(start, len, TokenType::Comment));
                    break;
                }
                let word = chars[start..i].iter().collect::<String>();
                let lower = word.to_ascii_lowercase();
                let token_type = if expect_decl_name {
                    expect_decl_name = false;
                    TokenType::Function
                } else if Self::is_keyword(&lower) {
                    if matches!(lower.as_str(), "sub" | "function") {
                        expect_decl_name = true;
                    }
                    TokenType::Keyword
                } else if Self::is_type_name(&lower) {
                    TokenType::Type
                } else {
                    TokenType::Identifier
                };
                tokens.push(Token::new(start, i, token_type));
                continue;
            }

            if i + 1 < len {
                let pair = (chars[i], chars[i + 1]);
                if matches!(pair, ('<', '=') | ('>', '=') | ('<', '>')) {
                    tokens.push(Token::new(i, i + 2, TokenType::Operator));
                    i += 2;
                    continue;
                }
            }

            if matches!(
                ch,
                '+' | '-' | '*' | '/' | '\\' | '^' | '&' | '=' | '<' | '>'
            ) {
                tokens.push(Token::new(i, i + 1, TokenType::Operator));
                i += 1;
                continue;
            }

            if matches!(ch, '(' | ')' | '[' | ']' | ',' | '.' | ':' | ';') {
                tokens.push(Token::new(i, i + 1, TokenType::Special));
                i += 1;
                continue;
            }

            tokens.push(Token::new(i, i + 1, TokenType::Normal));
            i += 1;
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::OxvbaHighlighter;
    use turbo_vision::views::syntax::{SyntaxHighlighter, TokenType};

    #[test]
    fn highlights_core_vba_lexical_categories() {
        let highlighter = OxvbaHighlighter::new();
        let tokens = highlighter.highlight_line("Public Sub Main(): Print \"42\" ' comment", 0);
        assert!(has_token_type(&tokens, TokenType::Keyword));
        assert!(has_token_type(&tokens, TokenType::Function));
        assert!(has_token_type(&tokens, TokenType::String));
        assert!(has_token_type(&tokens, TokenType::Comment));
    }

    #[test]
    fn rem_comment_consumes_rest_of_line() {
        let highlighter = OxvbaHighlighter::new();
        let line = "Rem compile diagnostics";
        let tokens = highlighter.highlight_line(line, 0);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, line.len());
    }

    fn has_token_type(tokens: &[turbo_vision::views::syntax::Token], expected: TokenType) -> bool {
        tokens.iter().any(|token| token.token_type == expected)
    }
}
