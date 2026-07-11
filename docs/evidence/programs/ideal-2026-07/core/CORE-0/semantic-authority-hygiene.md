# CORE-0 Semantic Authority Hygiene

Date: 2026-07-11  
Bead: `bd-59co.2.1.2`

Status: verified.

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
| `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md` | Replaced extraction/current-implementation authority with the public-specification and Excel/VBA hierarchy; made the project/provider model conceptual and identity-stable; added source, verified-image, VBA-library, host and COM provider semantics; corrected class initialization, termination/resurrection, property and event rules; replaced obsolete status/pipeline/HAL/next-step prose with current compiler, OxIR/Image, VM3/JIT, Windows, LS and canonical-row ownership. |

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

The parallel project/reference worker additionally passed `docs-check`, a
14-link resolution audit, stale-term scanning, diff/staged-scope checks and a
fresh-eyes reread. Worker-local full governance was unavailable because its
isolated branch intentionally retained the older controller state; no worker
modified `.beads`, AutoRun state, canonical matrices, traces or generated
summaries.

Controller integration results are recorded after the canonical row transition
and final checks. Verification requires the actual authority artifact above and
classifies observables as
`result=verified,full-err=n/a,side-effects=verified,lifecycle-order=verified,transport=n/a,balance=verified`:

- `result`: all five active semantic references use the same authority and
  residual-routing contract;
- `side-effects`: the clean-room review found no proprietary source,
  decompilation, disassembly or Office-internal reverse engineering;
- `lifecycle-order`: semantic decisions precede implementation and unresolved
  behavior remains spec/oracle-owned;
- full Err, runtime transport and runtime balance are not applicable to this
  authority-control row.

Controller integration results:

- all five active semantic references and every repository-relative link in
  them resolve;
- `CORE-READINESS/CORE-AUTHORITY-CLEAN-SPEC-VBA` is `verified`, carries this
  actual artifact and the six-axis classification above, and has no matrix or
  trace residual owner;
- `./scripts/run-truth-reconciliation.ps1 -Refresh` passed at 189 rows, 226
  trace relationships and 78 execution leaves; the other 188 capability and
  evidence rows retain their evidence-backed states;
- `./scripts/check-governance.ps1` and
  `./scripts/test-path-stability.ps1` passed, including all 24 fail-closed
  validator cases and the support/active-claim positive guards;
- `br lint --json`, `br dep cycles` and `git diff --check` passed;
- independent cross-review preserved the full VBA grammar, type, coercion,
  call, property, event, project/reference and boundary semantics. It found and
  repaired the precise `Class_Initialize` return-order and
  `Class_Terminate` delayed-invocation/resurrection/at-most-once/error rules,
  then returned clean on the integrated semantic set.

No compiler, VM3, JIT, Windows interop or language-service capability row is
advanced by this authority-control verification.
