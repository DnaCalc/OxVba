use std::fs;
use std::path::{Path, PathBuf};

use oxvba_syntax::SyntaxKind;
use oxvba_syntax::lexer::tokenize;

fn snapshot(source: &str) -> Vec<(SyntaxKind, &str)> {
    tokenize(source)
}

fn assert_lossless(name: &str, source: &str) {
    let reconstructed: String = tokenize(source).iter().map(|(_, text)| *text).collect();
    assert_eq!(
        reconstructed, source,
        "lossless tokenization failed for {name}"
    );
}

#[test]
fn grammar_row_token_snapshots_are_stable() {
    assert_eq!(
        snapshot("Option Compare Text\r\n"),
        vec![
            (SyntaxKind::KwOption, "Option"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::KwCompare, "Compare"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Ident, "Text"),
            (SyntaxKind::Newline, "\r\n"),
            (SyntaxKind::Eof, ""),
        ]
    );

    assert_eq!(
        snapshot("Dim [Line Input] As String\r\n"),
        vec![
            (SyntaxKind::KwDim, "Dim"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::BracketedIdent, "[Line Input]"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::KwAs, "As"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Ident, "String"),
            (SyntaxKind::Newline, "\r\n"),
            (SyntaxKind::Eof, ""),
        ]
    );

    assert_eq!(
        snapshot("x = &HFF^: y = 2#: z = \"a\"\"b\"\r\n"),
        vec![
            (SyntaxKind::Ident, "x"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Eq, "="),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::HexLiteral, "&HFF^"),
            (SyntaxKind::Colon, ":"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Ident, "y"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Eq, "="),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::FloatLiteral, "2#"),
            (SyntaxKind::Colon, ":"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Ident, "z"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Eq, "="),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::StringLiteral, "\"a\"\"b\""),
            (SyntaxKind::Newline, "\r\n"),
            (SyntaxKind::Eof, ""),
        ]
    );

    assert_eq!(
        snapshot("Rem c\r\nx _\r\n + 1\r\n"),
        vec![
            (SyntaxKind::Comment, "Rem c"),
            (SyntaxKind::Newline, "\r\n"),
            (SyntaxKind::Ident, "x"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::LineContinuation, "_\r\n"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::Plus, "+"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::IntLiteral, "1"),
            (SyntaxKind::Newline, "\r\n"),
            (SyntaxKind::Eof, ""),
        ]
    );

    assert_eq!(
        snapshot("Function Name$()\nEnd Function\n"),
        vec![
            (SyntaxKind::KwFunction, "Function"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::KwName, "Name"),
            (SyntaxKind::TypeSuffix, "$"),
            (SyntaxKind::LParen, "("),
            (SyntaxKind::RParen, ")"),
            (SyntaxKind::Newline, "\n"),
            (SyntaxKind::KwEnd, "End"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::KwFunction, "Function"),
            (SyntaxKind::Newline, "\n"),
            (SyntaxKind::Eof, ""),
        ]
    );
}

#[test]
fn checked_in_vba_fixture_corpus_tokenizes_losslessly() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let roots = [
        repo_root.join("conformance/tests"),
        repo_root.join("conformance/vm_package/identity_seed"),
        repo_root.join("conformance/com"),
        repo_root.join("conformance/integration/projects"),
        repo_root.join("conformance/jit_v2/tracer_bullets"),
        repo_root.join("crates/oxvba-debug/tests/fixtures"),
    ];

    let mut files = Vec::new();
    for root in roots {
        collect_bas_files(&root, &mut files);
    }
    files.sort();
    assert!(
        files.len() >= 200,
        "expected broad checked-in .bas corpus, got {} files",
        files.len()
    );

    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        assert_lossless(&path.display().to_string(), &source);
    }
}

fn collect_bas_files(root: &Path, out: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    for entry in
        fs::read_dir(root).unwrap_or_else(|err| panic!("failed to read {}: {err}", root.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_bas_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bas"))
        {
            out.push(path);
        }
    }
}
