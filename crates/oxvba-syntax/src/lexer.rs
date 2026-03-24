use crate::syntax_kind::{SyntaxKind, keyword_kind};

/// Tokenize VBA source into a lossless token stream.
///
/// Every byte of input is covered by exactly one token. The returned slices
/// borrow directly from `source` (zero-copy for token text).
pub fn tokenize(source: &str) -> Vec<(SyntaxKind, &str)> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let start = i;
        let b = bytes[i];

        // ── Newline (\r\n or \n or \r) ──────────────────────
        if b == b'\n' {
            i += 1;
            tokens.push((SyntaxKind::Newline, &source[start..i]));
            continue;
        }
        if b == b'\r' {
            i += 1;
            if i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
            tokens.push((SyntaxKind::Newline, &source[start..i]));
            continue;
        }

        // ── Whitespace (not newline) ────────────────────────
        if b == b' ' || b == b'\t' {
            i += 1;
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            tokens.push((SyntaxKind::Whitespace, &source[start..i]));
            continue;
        }

        // ── Comment (' or Rem at line start) ────────────────
        if b == b'\'' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            tokens.push((SyntaxKind::Comment, &source[start..i]));
            continue;
        }

        // ── Line continuation ( _ followed by newline) ──────
        if b == b'_' && is_line_continuation(bytes, i) {
            i += 1; // _
            // consume optional whitespace before newline
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            // consume newline
            if i < bytes.len() && bytes[i] == b'\r' {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
            tokens.push((SyntaxKind::LineContinuation, &source[start..i]));
            continue;
        }

        // ── String literal "..." ────────────────────────────
        if b == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    i += 1;
                    // VBA uses "" for escaped quote inside string
                    if i < bytes.len() && bytes[i] == b'"' {
                        i += 1;
                        continue;
                    }
                    break;
                }
                if bytes[i] == b'\n' || bytes[i] == b'\r' {
                    // Unterminated string — stop before newline
                    break;
                }
                i += 1;
            }
            tokens.push((SyntaxKind::StringLiteral, &source[start..i]));
            continue;
        }

        // ── Date literal #...# ─────────────────────────────
        if b == b'#' && looks_like_date(bytes, i) {
            i += 1;
            while i < bytes.len() && bytes[i] != b'#' && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'#' {
                i += 1;
            }
            tokens.push((SyntaxKind::DateLiteral, &source[start..i]));
            continue;
        }

        // ── Bracketed identifier [name] ─────────────────────
        if b == b'[' {
            i += 1;
            while i < bytes.len() && bytes[i] != b']' && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b']' {
                i += 1;
            }
            tokens.push((SyntaxKind::BracketedIdent, &source[start..i]));
            continue;
        }

        // ── Numeric literals (int, float, hex, oct) ─────────
        if b.is_ascii_digit() {
            lex_number(source, bytes, &mut i);
            tokens.push((classify_number(&source[start..i]), &source[start..i]));
            continue;
        }

        // ── &H hex / &O oct prefix ─────────────────────────
        if b == b'&' && i + 1 < bytes.len() {
            let next = bytes[i + 1].to_ascii_lowercase();
            if next == b'h' {
                i += 2;
                while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                // optional & or type suffix
                if i < bytes.len() && bytes[i] == b'&' {
                    i += 1;
                }
                tokens.push((SyntaxKind::HexLiteral, &source[start..i]));
                continue;
            }
            if next == b'o' {
                i += 2;
                while i < bytes.len() && (bytes[i] >= b'0' && bytes[i] <= b'7') {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'&' {
                    i += 1;
                }
                tokens.push((SyntaxKind::OctLiteral, &source[start..i]));
                continue;
            }
        }

        // ── Identifier / keyword ────────────────────────────
        if b.is_ascii_alphabetic() || b == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let text = &source[start..i];
            let lower = text.to_ascii_lowercase();
            let kind = keyword_kind(&lower).unwrap_or(SyntaxKind::Ident);
            tokens.push((kind, text));

            // Type suffix after identifier: %, &, !, #, @, $
            if kind == SyntaxKind::Ident && i < bytes.len() && is_type_suffix(bytes[i]) {
                let ts = i;
                i += 1;
                tokens.push((SyntaxKind::TypeSuffix, &source[ts..i]));
            }

            continue;
        }

        // ── Multi-character operators ───────────────────────
        if i + 1 < bytes.len() {
            let two = &source[start..start + 2];
            let kind2 = match two {
                "<=" => Some(SyntaxKind::LtEq),
                ">=" => Some(SyntaxKind::GtEq),
                "<>" => Some(SyntaxKind::LtGt),
                ":=" => Some(SyntaxKind::ColonEq),
                _ => None,
            };
            if let Some(k) = kind2 {
                i += 2;
                tokens.push((k, &source[start..i]));
                continue;
            }
        }

        // ── Single-character operators and punctuation ──────
        let kind1 = match b {
            b'+' => SyntaxKind::Plus,
            b'-' => SyntaxKind::Minus,
            b'*' => SyntaxKind::Star,
            b'/' => SyntaxKind::Slash,
            b'\\' => SyntaxKind::Backslash,
            b'^' => SyntaxKind::Caret,
            b'&' => SyntaxKind::Ampersand,
            b'=' => SyntaxKind::Eq,
            b'<' => SyntaxKind::Lt,
            b'>' => SyntaxKind::Gt,
            b'(' => SyntaxKind::LParen,
            b')' => SyntaxKind::RParen,
            b',' => SyntaxKind::Comma,
            b'.' => SyntaxKind::Dot,
            b'!' => SyntaxKind::Bang,
            b':' => SyntaxKind::Colon,
            b';' => SyntaxKind::Semicolon,
            b'#' => SyntaxKind::Hash,
            _ => SyntaxKind::ErrorNode,
        };
        i += 1;
        tokens.push((kind1, &source[start..i]));
    }

    tokens.push((SyntaxKind::Eof, &source[source.len()..]));
    tokens
}

/// Check if `_` at position `i` is a line continuation (followed by optional
/// whitespace then a newline).
fn is_line_continuation(bytes: &[u8], i: usize) -> bool {
    let mut j = i + 1;
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }
    // Must be at newline or end of file for it to be a continuation
    j >= bytes.len() || bytes[j] == b'\n' || bytes[j] == b'\r'
}

/// Heuristic: does `#` at position `i` look like the start of a date literal?
/// True if there's at least one digit or `/` before the closing `#`.
fn looks_like_date(bytes: &[u8], i: usize) -> bool {
    let mut j = i + 1;
    let mut has_date_char = false;
    while j < bytes.len() && bytes[j] != b'#' && bytes[j] != b'\n' && bytes[j] != b'\r' {
        if bytes[j].is_ascii_digit() || bytes[j] == b'/' || bytes[j] == b'-' {
            has_date_char = true;
        }
        j += 1;
    }
    has_date_char && j < bytes.len() && bytes[j] == b'#'
}

fn is_type_suffix(b: u8) -> bool {
    matches!(b, b'%' | b'&' | b'!' | b'#' | b'@' | b'$')
}

fn lex_number(source: &str, bytes: &[u8], i: &mut usize) {
    // Integer part
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        *i += 1;
    }
    // Decimal part
    if *i < bytes.len() && bytes[*i] == b'.' {
        // Check it's not a member access (e.g., `1.ToString`)
        if *i + 1 < bytes.len() && bytes[*i + 1].is_ascii_digit() {
            *i += 1; // consume .
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }
    }
    // Exponent part
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        let saved = *i;
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        if *i < bytes.len() && bytes[*i].is_ascii_digit() {
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        } else {
            // Not a valid exponent — backtrack
            *i = saved;
        }
    }
    let _ = source; // used for context only
}

fn classify_number(text: &str) -> SyntaxKind {
    if text.contains('.') || text.contains('e') || text.contains('E') {
        SyntaxKind::FloatLiteral
    } else {
        SyntaxKind::IntLiteral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<SyntaxKind> {
        tokenize(source).into_iter().map(|(k, _)| k).collect()
    }

    fn texts(source: &str) -> Vec<&str> {
        tokenize(source).into_iter().map(|(_, t)| t).collect()
    }

    #[test]
    fn round_trip_lossless() {
        let sources = vec![
            "Sub Main()\r\nEnd Sub",
            "Dim x As Long ' comment\nDim y%",
            "If x >= 10 Then\n    y = x + 1\nEnd If",
            "&HFF& + &O77",
            "#1/1/2000# = d",
            "s = \"hello \"\"world\"\"\"",
            "[Sheet1].Range(\"A1\")",
            "x = 1.5E-3",
            "y = _ \r\n  42",
        ];
        for src in sources {
            let tokens = tokenize(src);
            let reconstructed: String = tokens.iter().map(|(_, t)| *t).collect();
            assert_eq!(reconstructed, src, "round-trip failed for: {src:?}");
        }
    }

    #[test]
    fn keywords_are_classified() {
        let toks = tokenize("Sub Main\nDim x As Long");
        assert_eq!(toks[0].0, SyntaxKind::KwSub);
        assert_eq!(toks[2].0, SyntaxKind::Ident); // Main
        assert_eq!(toks[4].0, SyntaxKind::KwDim);
        assert_eq!(toks[8].0, SyntaxKind::KwAs);
        // Long is a type name, not a keyword — appears at index 10
        assert_eq!(toks[10].0, SyntaxKind::Ident);
        assert_eq!(toks[10].1, "Long");
    }

    #[test]
    fn hex_and_oct_literals() {
        assert_eq!(kinds("&HFF"), vec![SyntaxKind::HexLiteral, SyntaxKind::Eof]);
        assert_eq!(kinds("&O77"), vec![SyntaxKind::OctLiteral, SyntaxKind::Eof]);
        assert_eq!(
            kinds("&HFF&"),
            vec![SyntaxKind::HexLiteral, SyntaxKind::Eof]
        );
    }

    #[test]
    fn float_and_exponent() {
        assert_eq!(
            kinds("1.5"),
            vec![SyntaxKind::FloatLiteral, SyntaxKind::Eof]
        );
        assert_eq!(
            kinds("1E10"),
            vec![SyntaxKind::FloatLiteral, SyntaxKind::Eof]
        );
        assert_eq!(
            kinds("1.5E-3"),
            vec![SyntaxKind::FloatLiteral, SyntaxKind::Eof]
        );
    }

    #[test]
    fn date_literal() {
        assert_eq!(
            kinds("#1/1/2000#"),
            vec![SyntaxKind::DateLiteral, SyntaxKind::Eof]
        );
    }

    #[test]
    fn bracketed_identifier() {
        assert_eq!(
            kinds("[Sheet1]"),
            vec![SyntaxKind::BracketedIdent, SyntaxKind::Eof]
        );
    }

    #[test]
    fn multi_char_operators() {
        assert_eq!(
            kinds("<= >= <> :="),
            vec![
                SyntaxKind::LtEq,
                SyntaxKind::Whitespace,
                SyntaxKind::GtEq,
                SyntaxKind::Whitespace,
                SyntaxKind::LtGt,
                SyntaxKind::Whitespace,
                SyntaxKind::ColonEq,
                SyntaxKind::Eof,
            ]
        );
    }

    #[test]
    fn type_suffix() {
        let toks = tokenize("x%");
        assert_eq!(toks[0], (SyntaxKind::Ident, "x"));
        assert_eq!(toks[1], (SyntaxKind::TypeSuffix, "%"));
    }

    #[test]
    fn line_continuation() {
        let toks = tokenize("x _\ny");
        assert!(toks.iter().any(|(k, _)| *k == SyntaxKind::LineContinuation));
    }

    #[test]
    fn comment_trivia() {
        let toks = tokenize("x ' comment\ny");
        assert!(toks.iter().any(|(k, _)| *k == SyntaxKind::Comment));
    }

    #[test]
    fn eof_always_present() {
        let toks = tokenize("");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].0, SyntaxKind::Eof);

        let toks = tokenize("Sub");
        assert_eq!(toks.last().unwrap().0, SyntaxKind::Eof);
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn tokenize_always_appends_eof_token() {
        let len: usize = kani::any();
        kani::assume(len > 0);
        kani::assume(len <= 24);

        let mut source = String::new();
        for _ in 0..len {
            let b: u8 = kani::any();
            let ascii = 32 + (b % 95);
            source.push(char::from(ascii));
        }

        let tokens = tokenize(&source);
        assert!(!tokens.is_empty());
        assert!(matches!(
            tokens.last().map(|(k, _)| *k),
            Some(SyntaxKind::Eof)
        ));
    }

    #[kani::proof]
    fn tokenize_is_lossless() {
        let len: usize = kani::any();
        kani::assume(len > 0);
        kani::assume(len <= 16);

        let mut source = String::new();
        for _ in 0..len {
            let b: u8 = kani::any();
            let ascii = 32 + (b % 95);
            source.push(char::from(ascii));
        }

        let tokens = tokenize(&source);
        let reconstructed: String = tokens.iter().map(|(_, t)| *t).collect();
        assert_eq!(reconstructed, source);
    }
}
