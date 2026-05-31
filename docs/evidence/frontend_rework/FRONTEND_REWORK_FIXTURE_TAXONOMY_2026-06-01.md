# Front-End Rework Fixture Taxonomy

Date: 2026-06-01
Bead: `bd-aprs.2.4`
Workset: `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`

## Purpose

Frontend v2 needs more than one pass/fail fixture lane. The same source file can be useful for
syntax, binding, lowering, execution, metadata, or live host evidence, but those are different
claims. This taxonomy defines the routing labels for the future semantic/diff harness.

## Fixture Classes

| Class | Claim type | Primary inputs | Required result shape | Phase gate use |
|---|---|---|---|---|
| `syntax_roundtrip` | Lossless CST preserves source text and trivia | `docs/spec/VBA_GRAMMAR_V1.md` rows, new syntax snapshots, selected `conformance/tests/*.bas` | parse tree plus reconstructed text equal to source bytes | FE-2/FE-3/FE-4 parser and lexer gates |
| `syntax_diagnostic` | Parser accepts incomplete/bad source with stable recovery | dedicated negative/incomplete edit fixtures | parse diagnostics with spans and retained error nodes | IDE recovery and diagnostics gates |
| `binder_positive` | Names/scopes/types resolve through HIR/SemanticModel | conformance rows with declarations/calls/members/project references | bound HIR facts plus SemanticModel symbol/type answers | FE-6/FE-7 binder gates |
| `binder_diagnostic` | Invalid source reports correct compile diagnostics | `*_error.bas`, package diagnostic manifests, integration diagnostics | stable diagnostic code/message/span classification | FE-6/FE-7 diagnostic mapping gates |
| `semantic_execution` | New frontend behavior matches Excel/MS-VBAL or current accepted behavior | `conformance/tests_manifest.csv`, `conformance/golden/values.csv`, basic examples | retained `VALUES:` or equivalent observable output | per-construct default flip gates |
| `metadata_contract` | Lowering emits compatible public metadata/contracts | VM package fixtures, host-call descriptor fixtures, XLL/wrapper examples | normalized descriptor/call-site/package projection | FE-5/FE-8 metadata normalization gates |
| `host_integration` | Host/session/project APIs consume compiled output correctly | `crates/oxvba-host/tests/*.rs`, integration projects, examples | Rust integration test outcome and optional evidence artifact | host-facing frontend rollout gates |
| `live_oracle` | Behavior checked against Excel/Office/COM or registered components | oracle scripts, COM/Office fixture catalogs, captured evidence | oracle capture/diff artifact | high-authority semantic disputes and host-sensitive lanes |
| `intentional_improvement` | New frontend intentionally differs from legacy output | classifier rows backed by Excel/MS-VBAL evidence | diff classification with reason and authority reference | prevents byte-identical bytecode from becoming a false gate |
| `residual` | Not currently in frontend v2 scope or not yet fixture-backed | forms/userform edges, missing grammar anchors, optional external corpus | explicit owner bead/workset and disposition | prevents silent coverage claims |

## Routing Rules

- A fixture can have multiple classes, but each class must be asserted independently.
- `semantic_execution` does not imply parser recovery coverage.
- `binder_positive` does not imply metadata compatibility.
- `host_integration` and `live_oracle` rows may be optional in normal CI, but must be available as
  named lanes for closure claims that depend on them.
- `intentional_improvement` rows require authority evidence; they are not an escape hatch for
  unexplained drift.
- `residual` rows must name the missing fixture, unsupported grammar surface, or follow-up bead.

## Initial Source Mapping

| Source | Default classes |
|---|---|
| `conformance/tests/*.bas` | `semantic_execution`, often `binder_positive` or `binder_diagnostic`; selected rows become `syntax_roundtrip` |
| `conformance/integration/projects/**` | `binder_positive`, `binder_diagnostic`, `host_integration`, `semantic_execution` |
| `conformance/vm_package/identity_seed/**` | `metadata_contract`, `binder_diagnostic`, selected `semantic_execution` |
| `conformance/com/**` | `host_integration`, `live_oracle`, selected `binder_positive` |
| `examples/basic/**` | fast `semantic_execution` and smoke `syntax_roundtrip` |
| `examples/xll/**`, `examples/reflection_wrapper/**` | `metadata_contract`, `host_integration` |
| `crates/oxvba-languageservice/tests/**` | `syntax_roundtrip`, `binder_positive`, SemanticModel query checks once shared APIs exist |
| `docs/evidence/conformance/oracle_captures/**` | `live_oracle` authority evidence, not primary source fixtures |
| `.external/sqliteforexcel/fixtures/**` | optional real-world `syntax_roundtrip`, `binder_positive`, and host/API stress lanes |

## Matrix Columns To Add Later

The current grammar matrix already has `parser_status`, `binder_status`, `execution_status`, and
`residual_disposition`. The harness matrix should later add:

- `fixture_class`
- `ci_lane`
- `requires_host`
- `requires_office`
- `authority`
- `diff_class`
- `owner_bead`

## Fresh-Eyes Notes

The main blunder to avoid is collapsing every fixture into an execution result. A frontend rework
can pass execution while still losing trivia, misplacing diagnostics, or failing IDE query
contracts. The taxonomy keeps those claims separate.

## Checks

- Cross-checked against corpus inventory and grammar matrix artifacts.
- `git diff --check`: passed with line-ending warnings only for touched tracked files.
