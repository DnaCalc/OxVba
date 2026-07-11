# CORE-0 Semantic Authority Hygiene

Date: 2026-07-11  
Bead: `bd-59co.2.1.2`

Status: semantic-reference repair complete on the worker branch; canonical row
verification and bead closure remain controller-owned.

## Outcome

The active VBA semantic references now use one authority rule:

1. `CHARTER.md` and `OPERATIONS.md` define compatibility and clean-room method.
2. `docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md` defines the destination and the role
   of semantic references.
3. Public specifications define VBA meaning; reproducible black-box Excel/VBA
   compile and runtime observations adjudicate ambiguity or disagreement.
4. Current OxVba code, helpers, VM3/JIT results, old bundles, and historical
   fixtures are regression evidence or divergences only. They cannot become
   expected VBA behavior without public-spec or Excel/VBA authority.
5. Uncertainty remains an exact canonical row with an active spec/oracle owner.

This repair changes no compiler/runtime capability claim. PROGRAM-0 matrices,
environment roles, migration records, traces, and generated summaries remain
control evidence only.

## File dispositions

| active semantic reference | disposition |
|---|---|
| `docs/spec/VBA_GRAMMAR_V1.md` | Recast implementation/parser observations as non-authoritative coverage inputs; made EBNF explicitly non-normative; routed preprocessing, provenance, typed-declaration, editor-recovery, and forms residuals to exact Core/IDE/extended-profile owners. |
| `docs/spec/VBA_TYPE_SYSTEM_V1.md` | Removed the statement that current code is executable semantic truth; replaced retired executable-package/native-ready/COM-scope companions with current compiler, OxIR/Image, carrier, and Windows contracts; made Rust-like shapes illustrative; replaced bytecode identity with resolved-procedure meaning; removed `Unknown` from verified slot/carrier examples; collapsed the obsolete compiler/Bundle/VM inventory into a historical disposition; routed type/call/artifact/oracle gaps canonically. |
| `docs/spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md` | Made public spec/Excel authority explicit; prohibited current interpreter/helper results from self-authorizing semantics; made DTO shapes non-normative; expressed consumer obligations as semantic distinctions rather than package layout; replaced the OxBundle v15/VMR status table with public-anchor and canonical-row routing while preserving coercion, operator, property, call, event, and COM semantics. |
| `docs/spec/VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md` | Replaced `current_vm_status`-style table guidance with VM3/JIT/evidence/residual fields; made table shapes non-normative; condensed old helper/OxBundle/VMR implementation prose into historical seed-table provenance; preserved the Boolean-string, Null comparison, `&` error swallowing, ASCII `Option Compare Text`, and bounded-shape observations as divergences requiring spec/oracle adjudication. |
| `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md` | Audited in this cycle and assigned to a separate isolated worker to avoid overlapping edits. Its integration must remove current-validation/status claims, correct source provenance, make state/DTO shapes non-normative, replace `Main`-specific lifecycle wording and obsolete truth states, and route pipeline/HAL/next-step material through current compiler, Windows, LS, and oracle rows before this bead verifies. |

## Residual and evidence routing

- Source/preprocessor/CST uncertainty routes to
  `CORE-READINESS/CORE-SYNTAX-CST` and
  `CORE-READINESS/CORE-SOURCE-IDENTITY-PROVENANCE`.
- Declared types, call signatures, argument mapping, properties, default
  members, and project/reference meaning route to
  `CORE-READINESS/CORE-TYPED-BINDING`.
- Executable type/call preservation and backend behavior route to the exact
  `OXIR-BACKENDS`, `OXIMAGE-CONTRACT`, VM3/JIT, and structural differential
  rows.
- VBA-observable ambiguity routes to the applicable current Excel/VBA oracle
  row; historical captures must be replayed on the current stack.
- COM metadata, transport, events, `Declare`, and native ABI questions remain
  in the Windows x64 matrices. IDE projection questions remain in the language
  service matrices. Forms remain `PROFILE-EXT-001` scope.

Historical seed tables remain useful coverage inventories. No helper result or
old VM/package token was promoted to expected behavior. No proprietary source,
decompilation, disassembly, or Office-internal reverse engineering was used.

## Verification

Worker results:

- `./scripts/docs-check.ps1`: passed;
- `./scripts/validate-contract-clause-disposition.ps1`: passed, 60 clauses;
- `./scripts/validate-validation-ownership.ps1`: passed, 15 matrices and three
  profiles;
- targeted stale-term review: no active current-code-as-truth, retired package,
  native-ready, bytecode-entry, old compiler/VM path, or `current_vm_status`
  guidance remains in the four worker-owned semantic references;
- `git diff --check`: passed with line-ending conversion warnings only;
- worker fresh-eyes reread: clean; semantic lists remain present, DTO examples
  are explicitly non-normative, and historical divergence facts retain no
  expected-behavior credit.

The isolated branch's committed `docs/AUTORUN_STATE.md` predates the controller's
required `Mode` field, so `validate-bead-traceability.ps1`, `check-governance.ps1`,
and `run-truth-reconciliation.ps1` stop at that unchanged control-state mismatch.
`test-path-stability.ps1` produced no result before the 124-second worker
timeout. The controller must run those integration gates against current primary
state; this worker did not modify any controller-owned surface to mask the
failure.

The controller must additionally integrate the project/reference companion
patch, attach the actual six-axis authority evidence to
`CORE-READINESS/CORE-AUTHORITY-CLEAN-SPEC-VBA`, clear matrix and trace residual
owners, rerun canonical truth reconciliation, and obtain a non-author fresh-eyes
review before closing the bead.
