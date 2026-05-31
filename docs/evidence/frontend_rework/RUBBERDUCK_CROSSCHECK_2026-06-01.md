# Rubberduck Grammar Cross-Check Notes

Date: 2026-06-01
Bead: `bd-aprs.2.2`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
Grammar scaffold: `docs/spec/VBA_GRAMMAR_V1.md`

## Scope

Rubberduck is a mature VBA/VB6 IDE tool and parser implementation. For OxVba frontend v2, it is a
quirk checklist and comparison point, not source material to copy into product grammar or parser
code.

Reference checked:

- Rubberduck parsing process wiki:
  `https://github.com/rubberduck-vba/Rubberduck/wiki/The-Parsing-Process`
- Rubberduck repository:
  `https://github.com/rubberduck-vba/Rubberduck`

## Use Policy

- Accept: use Rubberduck to identify real-world VBA/VBE parsing quirks and fixture ideas.
- Accept: compare OxVba coverage categories against Rubberduck's broad parser concerns.
- Reject: copy Rubberduck `.g4` grammar rules into `docs/spec/VBA_GRAMMAR_V1.md` or product code.
- Reject: treat Rubberduck behavior as authority over Excel/MS-VBAL evidence.

## Quirk Checklist Mapping

| Quirk / surface | OxVba grammar row or residual | Harness implication |
|---|---|---|
| VBE code vs exported module code can differ because exported modules include attributes | `attribute_line`, file preamble, residual for metadata details | Need fixtures for exported `.bas`/`.cls` attributes and in-editor text without attributes |
| Physical vs logical line handling | dialect notes, `logical_line`, `trivia`, `LINE_CONTINUATION` | Round-trip tests must preserve physical text while parser consumes logical statements |
| Statement separator `:` | `statement_list` | Include inline multi-statement fixtures and labels next to separators |
| Line labels and line numbers | `line_prefix`, `goto_stmt`, `gosub_stmt`, `resume_stmt` | Binder/control-flow diagnostics need duplicate/missing label rows |
| Comments and `Rem` | dialect notes, `comment_line`, `line_comment`, `COMMENT` | Syntax tests must preserve comments as trivia, including after statements |
| Bracketed identifiers and case-insensitive keywords | `identifier`, dialect notes | Lexer fixtures need keyword-as-identifier and bracketed-name rows |
| Type characters and literal suffixes | dialect notes, `type_ascription`, lexical token classes | Lexer/parser fixtures must disambiguate declaration suffixes from operators and literals |
| Date literals and numeric base forms | `literal`, dialect notes | Lexer snapshot corpus must include date, hex, octal, decimal, and typed numeric forms |
| `Option Base`, `Option Compare`, `Option Explicit`, project-private module option | `option_line`, `option_kind`, `option_value` | Binder/semantic fixtures need option-state rows, not just syntax acceptance |
| Inline and block `If` forms | `inline_if_stmt`, `block_if_stmt` | Parser recovery tests need dangling/incomplete `ElseIf`/`Else` rows |
| `Select Case` selector forms | `case_selector`, `case_compare_op` | Include `Case Else`, comma lists, ranges, and `Case Is <op>` rows |
| Calls with and without `Call`, parentheses, named args, omitted args | `call_stmt`, `argument_syntax`, `argument`, `named_argument` | Harness must classify syntax acceptance separately from binder argument mapping |
| Member, default-member, bang, index/call postfix forms | `postfix_expr`, `member_part`, `bang_part`, `index_or_call_part` | Feed FE-4/FE-7 with member/default-member and dictionary-bang cases |
| `With` block member targets | `with_stmt`, `postfix_expr` | Binder/HIR must preserve receiver context without string rewriting |
| Error handling labels and `Resume` variants | `on_error_stmt`, `resume_stmt` | Include control-flow and runtime-error semantic rows |
| Preprocessor directives | coverage residual | Needs a dedicated grammar/fixture expansion row; current compiler has separate PP parsing precedent |
| Userform/designer/control metadata | coverage residual | Keep as residual unless a later workset expands forms runtime scope |

## Gaps Added To Later Work

The existing `VBA_GRAMMAR_V1` scaffold covers the main syntax categories but intentionally leaves
several Rubberduck-highlighted real-world concerns to later beads:

- detailed attribute/module metadata round-trip;
- preprocessor line grammar and conditional-compilation parse states;
- incomplete-edit parser recovery for IDE usage;
- exported `.cls`/`.frm` module edge cases;
- optional host/form designer surfaces.

## Fresh-Eyes Notes

The main risk is accidentally making Rubberduck a third authority. The correct authority order
remains Excel on Windows, MS-VBAL, then existing OxVba behavior as a regression baseline. Rubberduck
only improves the checklist of awkward source forms that our clean-room grammar and fixtures should
cover.

## Checks

- Public Rubberduck parsing documentation located and reviewed as a checklist source.
- `docs/spec/VBA_GRAMMAR_V1.md` checked for corresponding rows/residuals.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.
