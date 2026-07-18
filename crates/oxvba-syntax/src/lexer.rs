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

        // ── Comment (' or Rem at line/logical-statement start) ─────────────
        if b == b'\'' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            tokens.push((SyntaxKind::Comment, &source[start..i]));
            continue;
        }
        if is_rem_comment_start(bytes, i) {
            i += 3; // Rem
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
                // optional integer type suffix
                if i < bytes.len() && is_integer_type_suffix(bytes[i]) {
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
                if i < bytes.len() && is_integer_type_suffix(bytes[i]) {
                    i += 1;
                }
                tokens.push((SyntaxKind::OctLiteral, &source[start..i]));
                continue;
            }
        }

        // ── Identifier / keyword ────────────────────────────
        // ASCII letters/`_` start an identifier; VBA also permits non-ASCII
        // Unicode letters in identifiers (common in non-English locales). We
        // keep the ASCII byte fast path and decode whole chars for bytes
        // >= 0x80 so `i` always stays on a UTF-8 char boundary — otherwise the
        // token slices below (here and in the operator arms) would split a
        // multibyte character and panic the host on ordinary VBA source.
        if b.is_ascii_alphabetic() || b == b'_' || b >= 0x80 {
            if b < 0x80 {
                i += 1;
            } else {
                match source[i..].chars().next() {
                    Some(ch) if is_unicode_ident_start(ch) => i += ch.len_utf8(),
                    Some(ch) => {
                        // Non-ASCII, non-letter (symbol/emoji/currency sign):
                        // emit one lossless error token for the whole char so
                        // the stream never splits a multibyte character.
                        i += ch.len_utf8();
                        tokens.push((SyntaxKind::ErrorNode, &source[start..i]));
                        continue;
                    }
                    // Unreachable: a byte >= 0x80 at a boundary always yields a
                    // char. Drain to EOF rather than risk a split slice.
                    None => {
                        i = bytes.len();
                        continue;
                    }
                }
            }
            while i < bytes.len() {
                let cb = bytes[i];
                if cb < 0x80 {
                    if cb.is_ascii_alphanumeric() || cb == b'_' {
                        i += 1;
                    } else {
                        break;
                    }
                } else if let Some(ch) = source[i..].chars().next() {
                    if is_unicode_ident_continue(ch) {
                        i += ch.len_utf8();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            let text = &source[start..i];
            let lower = text.to_ascii_lowercase();
            let kind = keyword_kind(&lower).unwrap_or(SyntaxKind::Ident);
            tokens.push((kind, text));

            // Type suffix after identifier-like word: %, &, !, #, @, $
            if can_have_type_suffix(kind)
                && i < bytes.len()
                && is_type_suffix(bytes[i])
                && !is_bang_member_operator(bytes, i)
            {
                let ts = i;
                i += 1;
                tokens.push((SyntaxKind::TypeSuffix, &source[ts..i]));
            }

            continue;
        }

        // ── Multi-character operators ───────────────────────
        // Match on the raw byte pair rather than slicing `&source[start..start+2]`:
        // the following byte may be a non-ASCII lead byte, and slicing across it
        // would split a multibyte char and panic. All operator bytes are ASCII,
        // so once matched the `start..i` slice lands on char boundaries.
        if i + 1 < bytes.len() {
            let kind2 = match (b, bytes[i + 1]) {
                (b'<', b'=') => Some(SyntaxKind::LtEq),
                (b'>', b'=') => Some(SyntaxKind::GtEq),
                (b'<', b'>') => Some(SyntaxKind::LtGt),
                (b':', b'=') => Some(SyntaxKind::ColonEq),
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
    j < bytes.len() && (bytes[j] == b'\n' || bytes[j] == b'\r')
}

fn is_rem_comment_start(bytes: &[u8], i: usize) -> bool {
    if i + 3 > bytes.len() || !bytes[i..i + 3].eq_ignore_ascii_case(b"rem") {
        return false;
    }
    if i + 3 < bytes.len() {
        let next = bytes[i + 3];
        if next != b' ' && next != b'\t' && next != b'\r' && next != b'\n' {
            return false;
        }
    }

    let mut j = i;
    while j > 0 && (bytes[j - 1] == b' ' || bytes[j - 1] == b'\t') {
        j -= 1;
    }
    j == 0 || matches!(bytes[j - 1], b'\r' | b'\n' | b':')
}

/// Heuristic: does `#` at position `i` open a date literal (as opposed to a
/// file-number sigil that merely has another `#` later on the line)?
///
/// A real date/time literal (`#1/1/2000#`, `#12:30:00 PM#`, `#Jan 1, 2000#`) is
/// built only from date components — digits, the separators `/ : - . ,`, spaces,
/// and month / AM-PM words — and contains at least one genuine separator. The
/// naive "any digit before the next `#`" rule mis-paired the two file-number
/// sigils in `Close #1: Close #2`, `Print #1, #12/31/2020#`, and `Print #1,
/// amount#`, lexing the span between them as one bogus `DateLiteral` and
/// corrupting the statements. Requiring the interior to be all date components
/// (any other identifier, like `Close`/`amount`, disqualifies it) plus a real
/// separator keeps genuine literals while rejecting file-number pairs.
fn looks_like_date(bytes: &[u8], i: usize) -> bool {
    let mut j = i + 1;
    let start = j;
    while j < bytes.len() && bytes[j] != b'#' && bytes[j] != b'\n' && bytes[j] != b'\r' {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'#' {
        return false; // no closing '#' on this physical line
    }
    let interior = &bytes[start..j];
    let mut has_digit = false;
    let mut has_separator = false; // '/', ':', '-', or a month/AM-PM word
    let mut k = 0;
    while k < interior.len() {
        let b = interior[k];
        if b.is_ascii_digit() {
            has_digit = true;
            k += 1;
        } else if matches!(b, b'/' | b':' | b'-') {
            has_separator = true;
            k += 1;
        } else if matches!(b, b'.' | b' ' | b'\t' | b',') {
            k += 1;
        } else if b.is_ascii_alphabetic() {
            let word_start = k;
            while k < interior.len() && interior[k].is_ascii_alphabetic() {
                k += 1;
            }
            if is_date_word(&interior[word_start..k]) {
                has_separator = true;
            } else {
                return false; // an identifier (`Close`, `amount`) — not a date
            }
        } else {
            return false; // an operator/paren/etc. — not a date
        }
    }
    has_digit && has_separator
}

/// A month name (or a prefix of one, so `Jan`/`January` both match) or `AM`/`PM`
/// — the only alphabetic runs a VBA date/time literal may contain.
fn is_date_word(word: &[u8]) -> bool {
    let lower: Vec<u8> = word.iter().map(u8::to_ascii_lowercase).collect();
    let w = lower.as_slice();
    if w == b"am" || w == b"pm" {
        return true;
    }
    const MONTHS: [&[u8]; 12] = [
        b"january",
        b"february",
        b"march",
        b"april",
        b"may",
        b"june",
        b"july",
        b"august",
        b"september",
        b"october",
        b"november",
        b"december",
    ];
    !w.is_empty() && MONTHS.iter().any(|m| m.starts_with(w))
}

fn is_type_suffix(b: u8) -> bool {
    matches!(b, b'%' | b'&' | b'!' | b'#' | b'@' | b'$')
}

fn can_have_type_suffix(kind: SyntaxKind) -> bool {
    kind == SyntaxKind::Ident || kind.is_keyword()
}

fn is_bang_member_operator(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'!'
        && i + 1 < bytes.len()
        && (is_identifier_start(bytes[i + 1]) || bytes[i + 1] == b'[')
}

fn is_identifier_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// A non-ASCII character that may begin a VBA identifier. VBA/VB permit Unicode
/// letters in identifiers, so any Unicode alphabetic char qualifies. (ASCII
/// starts are handled by the byte fast path; this is only consulted for
/// `char >= 0x80`.)
fn is_unicode_ident_start(ch: char) -> bool {
    ch.is_alphabetic()
}

/// A non-ASCII character that may continue a VBA identifier (letters or Unicode
/// digits). ASCII alphanumerics/`_` are handled by the byte fast path; this is
/// only consulted for `char >= 0x80`.
fn is_unicode_ident_continue(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn is_integer_type_suffix(b: u8) -> bool {
    matches!(b, b'%' | b'&' | b'^')
}

fn is_numeric_type_suffix(b: u8) -> bool {
    matches!(b, b'%' | b'&' | b'^' | b'!' | b'#' | b'@')
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
    if *i < bytes.len() && is_numeric_type_suffix(bytes[*i]) {
        *i += 1;
    }
    let _ = source; // used for context only
}

fn classify_number(text: &str) -> SyntaxKind {
    if matches!(text.as_bytes().last(), Some(b'!' | b'#' | b'@')) {
        return SyntaxKind::FloatLiteral;
    }
    let body = text.trim_end_matches(['%', '&', '^']);
    if body.contains('.') || body.contains('e') || body.contains('E') {
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
        assert_eq!(tokenize("&HFF^")[0], (SyntaxKind::HexLiteral, "&HFF^"));
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
    fn numeric_type_suffix_literals_stay_atomic() {
        assert_eq!(tokenize("100&")[0], (SyntaxKind::IntLiteral, "100&"));
        assert_eq!(tokenize("100^")[0], (SyntaxKind::IntLiteral, "100^"));
        assert_eq!(tokenize("2.5!")[0], (SyntaxKind::FloatLiteral, "2.5!"));
        assert_eq!(tokenize("2#")[0], (SyntaxKind::FloatLiteral, "2#"));
        assert_eq!(tokenize("2@")[0], (SyntaxKind::FloatLiteral, "2@"));
        assert_eq!(
            tokenize("x & 2")[2],
            (SyntaxKind::Ampersand, "&"),
            "spaced ampersand must remain concatenation/operator token"
        );
    }

    #[test]
    fn special_literal_keywords_are_classified() {
        assert_eq!(kinds("True"), vec![SyntaxKind::KwTrue, SyntaxKind::Eof]);
        assert_eq!(kinds("False"), vec![SyntaxKind::KwFalse, SyntaxKind::Eof]);
        assert_eq!(
            kinds("Nothing"),
            vec![SyntaxKind::KwNothing, SyntaxKind::Eof]
        );
        assert_eq!(kinds("Empty"), vec![SyntaxKind::KwEmpty, SyntaxKind::Eof]);
        assert_eq!(kinds("Null"), vec![SyntaxKind::KwNull, SyntaxKind::Eof]);
    }

    #[test]
    fn date_literal() {
        assert_eq!(
            kinds("#1/1/2000#"),
            vec![SyntaxKind::DateLiteral, SyntaxKind::Eof]
        );
    }

    #[test]
    fn malformed_string_and_date_literals_recover_losslessly() {
        assert_eq!(
            tokenize("\"unterminated\n")[0],
            (SyntaxKind::StringLiteral, "\"unterminated")
        );
        assert_eq!(tokenize("#1/1/2000\n")[0], (SyntaxKind::Hash, "#"));
        let src = "\"unterminated\n#1/1/2000\n";
        let reconstructed: String = tokenize(src).iter().map(|(_, text)| *text).collect();
        assert_eq!(reconstructed, src);
    }

    #[test]
    fn date_literal_recognizes_real_dates() {
        assert_eq!(tokenize("#1/1/2000#")[0].0, SyntaxKind::DateLiteral);
        assert_eq!(tokenize("#12/31/2020#")[0].0, SyntaxKind::DateLiteral);
        assert_eq!(tokenize("#12:30:00 PM#")[0].0, SyntaxKind::DateLiteral);
        assert_eq!(tokenize("#Jan 1, 2000#")[0].0, SyntaxKind::DateLiteral);
        assert_eq!(
            tokenize("#January 1, 2000 3:30:00 PM#")[0].0,
            SyntaxKind::DateLiteral
        );
        assert_eq!(tokenize("#2020-05-01#")[0].0, SyntaxKind::DateLiteral);
    }

    #[test]
    fn hash_file_numbers_are_not_mislexed_as_dates() {
        // `Close #1: Close #2` — the two file-number sigils must stay Hash+IntLiteral,
        // not be paired into `#1: Close #` as one DateLiteral.
        let toks = tokenize("Close #1: Close #2");
        assert!(
            !toks.iter().any(|(k, _)| *k == SyntaxKind::DateLiteral),
            "file numbers must not lex as a DateLiteral: {toks:?}"
        );
        assert_eq!(toks[2], (SyntaxKind::Hash, "#"));
        assert_eq!(toks[3], (SyntaxKind::IntLiteral, "1"));

        // `Print #1, amount#` — the trailing `#` is a type suffix on `amount`,
        // and `#1` is a file number; neither pairs into a DateLiteral.
        let toks = tokenize("Print #1, amount#");
        assert!(
            !toks.iter().any(|(k, _)| *k == SyntaxKind::DateLiteral),
            "file number + type suffix must not lex as a DateLiteral: {toks:?}"
        );

        // `Print #1, #12/31/2020#` — the real date literal is still recognized,
        // and `#1` is not paired with the date's opening `#`.
        let toks = tokenize("Print #1, #12/31/2020#");
        assert_eq!(
            toks.iter()
                .filter(|(k, _)| *k == SyntaxKind::DateLiteral)
                .count(),
            1,
            "exactly one DateLiteral expected: {toks:?}"
        );
        assert!(
            toks.iter()
                .any(|(k, t)| *k == SyntaxKind::DateLiteral && *t == "#12/31/2020#")
        );
    }

    #[test]
    fn bare_number_between_hashes_is_not_a_date() {
        // `#1, #` (no separator) must not be a DateLiteral.
        assert_ne!(tokenize("#1, #")[0].0, SyntaxKind::DateLiteral);
        // A lone number `#5#` is not a date either (no separator).
        assert_ne!(tokenize("#5#")[0].0, SyntaxKind::DateLiteral);
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
    fn bang_member_is_not_identifier_type_suffix() {
        assert_eq!(
            tokenize("obj!Field")[0..3],
            [
                (SyntaxKind::Ident, "obj"),
                (SyntaxKind::Bang, "!"),
                (SyntaxKind::Ident, "Field")
            ]
        );
        assert_eq!(tokenize("x!")[1], (SyntaxKind::TypeSuffix, "!"));
    }

    #[test]
    fn keyword_colliding_names_keep_attached_type_suffixes() {
        let toks = tokenize("Function Name$()\nEnd Function");
        assert_eq!(toks[0], (SyntaxKind::KwFunction, "Function"));
        assert_eq!(toks[2], (SyntaxKind::KwName, "Name"));
        assert_eq!(toks[3], (SyntaxKind::TypeSuffix, "$"));
    }

    #[test]
    fn identifiers_keywords_and_bracketed_names_are_case_preserving() {
        let src = "Application.[Type]\nDim [Line Input] As String\nVaRiAnT";
        let toks = tokenize(src);
        let reconstructed: String = toks.iter().map(|(_, text)| *text).collect();
        assert_eq!(reconstructed, src);
        assert!(
            toks.iter()
                .any(|(kind, text)| *kind == SyntaxKind::BracketedIdent && *text == "[Type]")
        );
        assert!(toks.iter().any(|(kind, text)| {
            *kind == SyntaxKind::BracketedIdent && *text == "[Line Input]"
        }));
        assert!(
            toks.iter()
                .any(|(kind, text)| *kind == SyntaxKind::Ident && *text == "VaRiAnT")
        );
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
    fn rem_comment_trivia_at_logical_statement_start() {
        let toks = tokenize("Rem module comment\nx: Rem inline logical comment\nRemember = 1\n");
        assert_eq!(toks[0], (SyntaxKind::Comment, "Rem module comment"));
        assert!(
            toks.iter().any(|(kind, text)| *kind == SyntaxKind::Comment
                && *text == "Rem inline logical comment")
        );
        assert!(
            toks.iter()
                .any(|(kind, text)| *kind == SyntaxKind::Ident && *text == "Remember"),
            "Remember must remain an identifier, not a Rem comment"
        );
    }

    #[test]
    fn line_continuation_requires_physical_newline() {
        assert_eq!(kinds("_"), vec![SyntaxKind::Ident, SyntaxKind::Eof]);
        assert_eq!(
            tokenize("_ \r\n")[0],
            (SyntaxKind::LineContinuation, "_ \r\n")
        );
        assert_eq!(tokenize("_ \n")[0], (SyntaxKind::LineContinuation, "_ \n"));
    }

    #[test]
    fn trivia_snapshot_preserves_physical_and_logical_lines() {
        let src = "Sub T()\r\n    x = 1 _\r\n        + 2: Rem after separator\r\n    ' apostrophe\r\nEnd Sub\r\n";
        let tokens = tokenize(src);
        let reconstructed: String = tokens.iter().map(|(_, text)| *text).collect();
        assert_eq!(reconstructed, src);
        assert!(
            tokens
                .iter()
                .any(|(kind, text)| { *kind == SyntaxKind::LineContinuation && *text == "_\r\n" })
        );
        assert!(tokens.iter().any(|(kind, text)| {
            *kind == SyntaxKind::Comment && *text == "Rem after separator"
        }));
        assert!(
            tokens
                .iter()
                .any(|(kind, text)| { *kind == SyntaxKind::Comment && *text == "' apostrophe" })
        );
    }

    #[test]
    fn non_ascii_identifier_is_lexed_as_ident() {
        // VBA permits Unicode-letter identifiers (common in non-English locales).
        let toks = tokenize("Dim café As String");
        assert!(
            toks.iter()
                .any(|(kind, text)| *kind == SyntaxKind::Ident && *text == "café"),
            "accented identifier must lex as a single Ident, got {toks:?}"
        );
        // A 2-byte (é), 3-byte (λ, 日) and mixed identifier all stay atomic.
        assert_eq!(tokenize("λ")[0], (SyntaxKind::Ident, "λ"));
        assert_eq!(tokenize("변수")[0], (SyntaxKind::Ident, "변수"));
        assert_eq!(tokenize("xÄ1")[0], (SyntaxKind::Ident, "xÄ1"));
    }

    #[test]
    fn non_ascii_input_never_panics_and_round_trips() {
        // Regression: the lexer used to byte-slice multibyte chars in the
        // operator/identifier fallthrough and panic ("byte index not a char
        // boundary") on ordinary non-ASCII VBA source. Every one of these must
        // tokenize without panicking and reconstruct exactly.
        let sources = [
            "é",                          // bare 2-byte
            "€",                          // 3-byte non-letter symbol
            "😀",                         // 4-byte non-letter (emoji)
            "+😀",                        // operator immediately followed by a 4-byte char
            "<€",                         // 2-char-operator lookahead across a 3-byte lead byte
            "x = \"café\" ' αβγ comment", // multibyte in string + comment
            "Sub Naïve()\r\n    Dim π As Double\r\nEnd Sub",
            "变量 = 42 : Debug.Print 变量",
        ];
        for src in sources {
            let tokens = tokenize(src);
            let reconstructed: String = tokens.iter().map(|(_, t)| *t).collect();
            assert_eq!(reconstructed, src, "round-trip failed for {src:?}");
        }
    }

    #[test]
    fn non_ascii_symbol_becomes_error_token() {
        // A non-ASCII, non-letter char is a single lossless ErrorNode, not a split.
        assert_eq!(tokenize("€")[0], (SyntaxKind::ErrorNode, "€"));
        assert_eq!(tokenize("😀")[0], (SyntaxKind::ErrorNode, "😀"));
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
