# CORE-1 VMR05 array-shape parser and span provenance

Date: 2026-07-14

Bead: `bd-59co.2.2.18`

Base: `b5592e8d`

Effect: delivery

Result: the VMR05 array-shape fixture is valid VBA. The parser now accepts
`Explicit` as an ordinary identifier outside its contextual role in
`Option Explicit`; the fixture executes to its intended VM3 observable. Exact
UTF-8 source-byte offsets are covered for LF, CRLF, and conditionally blanked
module text. Fresh-eyes non-author review remains required before integration,
and the Core capability profile remains in progress.

## Semantic adjudication

The authority index is `docs/FOUNDATION_SPEC_REFERENCE.md`. Following that
index, the authority used was Microsoft **[MS-VBAL]-250520**, section 3.3.5.2,
"Reserved Identifiers and IDENTIFIER", from the pinned sibling-repository
Foundation extraction:

`../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/docs/discovered-ms-vbal-250520-f945507e/segments.jsonl`

The extracted source identifies the downloaded document and original Office
Protocol Documentation URL. The relevant segment chain is:

- `SEG-001359` identifies the reserved-identifier section;
- `SEG-001363` defines `IDENTIFIER` as a lex-identifier that is not a
  reserved identifier;
- `SEG-001360..SEG-001385` enumerate the reserved-identifier union and all of
  its keyword, reserved-name, special-form, type, literal, implementation, and
  future-reserved constituents; `Explicit` occurs in none of them; and
- `SEG-001583` defines the contextual directive form as `Option Explicit`.

Therefore `explicit` in
`conformance/vm_package/identity_seed/vmr05_array_shape_bounds.bas` is a legal
ordinary identifier. The normative grammar is unambiguous, so no Excel/VBE
oracle run was required. Treating the historical parse failure as a valid
negative case would contradict the published identifier grammar.

## Repair shape

The lexer continues to preserve `KwExplicit`, which lets the option-directive
parser recognize `Option Explicit` directly. The parser's existing
contextual-name classifier now also admits that token in declaration,
statement, label, and expression name positions. This follows the established
`Base`/`Lib` contextual-token design and does not make reserved statement or
operator keywords globally name-like.

No fixture source, binder rule, scanner rule, runtime implementation, or public
interface changed. The stale VMR05 golden row alone changed from seven parser
errors to the executable result already declared by the fixture manifest:

`i32:1|i32:3|i32:0|i32:2|i32:2|i32:4|i32:72`

Those values prove the `Option Base 1` fixed array bounds `1..3`, explicit
array bounds `0..2`, dynamic `ReDim` bounds `2..4`, and element total `72`.

## Source-offset and preprocessing proof

The named `vmr05_array_shape_offsets` parser regression runs the unchanged
fixture as both LF and CRLF, requires lossless round-trip text, and requires
zero parse errors. A paired source keeps `Option Explicit` as a directive while
using `explicit` as a parameter and assignment target.

For each EOL form the test also adds a multi-byte UTF-8 `café` prefix and
creates one deliberate malformed expression. The retained parse diagnostic's
point offset must equal the source-byte index of the exact unexpected `)` and
that byte is checked directly.

The same-named `oxvba-symbol` regression adds conditional directives and a
multi-byte inactive branch before the malformed active token. It proves that
preprocessing:

- preserves total byte length;
- blanks the inactive branch but retains the active branch;
- leaves the active marker at exactly its supplied-module byte offset; and
- passes that exact offset through the parser diagnostic for LF and CRLF.

This pins the product contract: conditional blanking changes inactive bytes to
spaces but does not rebase active token or diagnostic offsets away from the
module text supplied by the host/editor.

## Observable evidence

| Axis | Evidence |
|---|---|
| Result | VMR05 compiles and returns exact Long values `1, 3, 0, 2, 2, 4, 72`; the aggregate golden accepts the one intentional row update without bless mode. |
| Full Err | The valid program completes with `raised=false` and `FinalErr { number: 0, source: "", description: "", last_dll_error: 0 }`. The deliberate syntax controls stop in parsing and do not create or mutate VBA `Err`. |
| Side effects | The fixture writes only its seven ByRef result slots and local arrays. Parser and preprocessing tests allocate owned strings only. |
| Lifecycle/event order | Parse/preprocess tests have no runtime session or events. The VM3 fixture follows compile -> bind -> execute -> capture final results/Err. |
| Transport | Parser offsets are UTF-8 byte offsets relative to the exact LF or CRLF supplied text. Conditional-compilation blanking preserves byte count and active-token position. |
| Balance | No owned runtime carrier, COM reference, BSTR, SAFEARRAY descriptor, or native allocation contract changed. |

## Checks

- `cargo test -p oxvba-syntax vmr05_array_shape_offsets -- --nocapture`:
  named parser test passed; not a zero-test filter.
- `cargo test -p oxvba-symbol vmr05_array_shape_offsets -- --nocapture`:
  named conditional-preprocessing test passed; not a zero-test filter.
- `cargo test -p oxvba-syntax`: 144 unit tests and 2 lexer-corpus tests passed.
- `cargo test -p oxvba-symbol`: 148 unit tests passed.
- `cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`:
  the aggregate VM3 golden passed after the single reviewed VMR05 row edit;
  `OXVBA_BLESS_GOLDEN` was not set.
- `cargo clippy -p oxvba-syntax --all-targets -- -D warnings`: passed.
- `cargo clippy -p oxvba-symbol --all-targets -- -D warnings` stopped on five
  pre-existing strict-baseline findings in `const_eval.rs`, `scanner.rs`, and
  `surface.rs`; none is in the touched conditional-compilation test. The same
  command with those four existing lint classes explicitly allowed passed.
  Strict workspace cleanup remains owned by `bd-59co.2.2.3`.
- `./scripts/validate-line-endings.ps1`: passed V1 for 4,571 tracked files.
- `git diff --check`: passed.

## Residual boundary

- This bounded delivery slice corrects one parser classification and its
  evidence; it does not close any whole compiler, VM3, JIT, language-service,
  or release capability row.
- It supplies no Excel-oracle observation because the normative grammar fully
  decides the identifier question. Future behavioral ambiguity still follows
  the repository's Excel/VBA oracle protocol.
- The negative malformed-expression cases are test-local controls. They do not
  replace or mutate the now-valid VMR05 conformance fixture.
- Strict workspace Clippy and baseline certification remain with
  `bd-59co.2.2.3`; fresh-eyes review is the remaining integration gate for this
  bead.
