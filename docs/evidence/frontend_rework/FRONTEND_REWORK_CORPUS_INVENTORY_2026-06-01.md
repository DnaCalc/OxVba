# Front-End Rework Corpus Inventory

Date: 2026-06-01
Bead: `bd-aprs.1.3`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Purpose

This inventory identifies existing repo inputs for the future front-end semantic/diff harness. The
new harness should draw from these sources rather than inventing a parallel fixture universe.

Counts below are bounded `rg --files` counts taken on 2026-06-01; they are orientation data, not a
stable contract.

## Corpus Sources

| Source | Current shape | Harness role | Notes / gaps |
|---|---|---|---|
| `conformance/tests/*.bas` | 214 single-file VBA fixtures plus `conformance/tests_manifest.csv` and `conformance/golden/values.csv` | Primary semantic regression corpus for parser, binder, lowering, diagnostics, and execution | Needs grammar-production tagging so parser/binder coverage can be measured per construct, not just by file name |
| `conformance/integration/projects/**` | 26 `.bas` files across cataloged multi-module/multi-project cases | Project/module/reference binding, public member lookup, cross-project resolution, class modules | Good front-end stress surface; needs explicit v2 expected diagnostics/output rows |
| `conformance/vm_package/identity_seed/**` | 17 `.bas` package identity fixtures plus manifests | Descriptor, package identity, call binding, UDT/object/array metadata contracts | Useful for bytecode/metadata normalization; should not require byte-identical bytecode |
| `conformance/com/**` | 21 `.bas` COM/Office fixtures plus lane READMEs/catalogs | Late/early COM binding, Office/JET activation, typelib and host-sensitive boundaries | Some lanes require registered components or Office; mark as optional/live-oracle rows in v2 harness |
| `conformance/jit_v2/tracer_bullets/**` | 9 `.bas` tracer bullet fixtures plus manifest/expected values | Differential seed cases for future VM/JIT/native comparison | Planning-stage inputs; useful as harness shape precedent even before frontend v2 owns them |
| `examples/basic/**` | 9 basic `.bas` examples plus expected CSVs | Smoke examples for parser/lowering/execution with user-facing simple programs | Keep as quick smoke, not exhaustive grammar coverage |
| `examples/xll/**` and `examples/reflection_wrapper/**` | 6 `.bas` files, `.cls`, `.basproj`, expected CSVs | Host callable/XLL projection, application binding, wrapper/reflection examples | Good late-phase metadata/API compatibility checks; not first parser hardening corpus |
| `crates/oxvba-host/tests/*.rs` | Host integration tests that compile/run project snippets and fixture folders | Host/session/project API regression and semantic harness execution driver precedent | Several tests embed Rust-side expectations rather than reusable fixture metadata |
| `crates/oxvba-vm/tests/*.rs` | VM package and feature coverage tests | Package/bytecode/runtime metadata contract regression | Pair with semantic output checks instead of byte-identical bytecode requirements |
| `crates/oxvba-languageservice/tests/dnaoxide_thin_slice_hello.rs` | Current language-service thin-slice test | IDE query and CST-to-semantic correlation precedent | Very small; needs expansion once shared SemanticModel exists |
| `crates/oxvba-syntax/src/*` unit tests | Parser/lexer tests live in crate modules | Syntax substrate regression tests | Needs externalized grammar matrix rows and round-trip corpus coverage |
| `docs/evidence/conformance/oracle_captures/**` | Excel/VBA/COM oracle capture outputs and logs | Correctness authority evidence for ambiguous semantics | Generated evidence, not primary source corpus; use to classify expected behavior and intentional divergences |
| `.external/sqliteforexcel/fixtures/**` referenced from compiler tests | Optional external real-world VBA modules when present | Real-world module stress and Declare/API surface | External dependency may be absent; classify as optional corpus lane |

## Fixture Classes For `frontend_v2`

The semantic/diff harness should classify fixtures into lanes before execution:

- syntax-only round-trip: parse all text, preserve trivia, compare reconstructed source text;
- parser diagnostics: expected parse errors and recovery positions;
- binder diagnostics: symbol lookup, duplicate definitions, argument binding, type/member errors;
- semantic execution: retained `VALUES:` output or equivalent observable behavior;
- metadata contract: descriptors, project/module identity, host-call descriptors, package schema;
- host-sensitive live oracle: Excel/COM/Office or registered-component cases;
- intentional improvement: legacy output differs but Excel/MS-VBAL evidence supports the new result.

## Immediate Gaps

- No single matrix maps grammar productions to existing fixtures.
- Existing conformance manifests are execution-oriented; they do not yet carry parse/bind/HIR
  coverage state for frontend v2.
- Language-service coverage is much thinner than compiler/host coverage.
- External/Office/registered-COM rows need optional-lane metadata so normal CI can remain stable.
- Real-world source samples are limited and partly external; importing any third-party corpus needs
  provenance review before it becomes a checked-in fixture.
- Oracle captures need a small index that points from fixture or construct to the relevant
  authority evidence.

## Recommended First Harness Seeds

1. `conformance/tests_manifest.csv` plus `conformance/tests/*.bas` as the main single-file corpus.
2. `conformance/integration/catalog.psv` plus `conformance/integration/projects/**` for
   project/reference binding.
3. `conformance/vm_package/identity_seed/manifest.csv` and `diagnostic_manifest.csv` for package
   metadata and diagnostic contract rows.
4. `examples/basic/expected.csv` and `examples/basic/projects/expected.csv` for fast smoke.
5. `crates/oxvba-languageservice/tests/dnaoxide_thin_slice_hello.rs` as the initial IDE-query
   regression seed until FE-9.4 expands the shared SemanticModel surface.

## Fresh-Eyes Notes

The main oversight to avoid is treating the existing execution conformance corpus as sufficient for
a compiler front-end. It is a strong base, but frontend v2 also needs syntax recovery,
round-tripping, binding diagnostics, metadata contracts, and optional live-oracle classification.
Those additions belong in later harness/grammar beads; this bead only establishes the source map.

## Checks

- Bounded `rg --files` inventory counts for the corpus rows above.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.
- No code checks were required for this documentation-only corpus inventory.
