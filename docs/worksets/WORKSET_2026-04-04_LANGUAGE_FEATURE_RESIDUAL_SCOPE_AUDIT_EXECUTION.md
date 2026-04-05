# Workset: Language Feature Residual-Scope Audit Execution

Date: 2026-04-04
Owner: Codex
Status: in-progress

## Purpose

Run a careful, repo-wide audit of language/runtime feature coverage so OxVba can
distinguish:

- fully delivered feature areas,
- implemented subsets with remaining accepted scope,
- explicit unsupported boundaries,
- and feature claims whose current wording is broader than the shipped
  implementation.

This workset exists to prevent broad language-feature labels from hiding
important residual scope, as happened with the runtime-sized `ReDim` boundary
only becoming operationally obvious once the SQLiteForExcel integration lane hit
it.

## Why This Exists

Recent integration work showed that OxVba had:

- a real static-array `ReDim` subset,
- explicit diagnostics for runtime-sized `ReDim` gaps,
- and tests for the implemented subset,

but did not yet have a strong enough top-level feature audit that split:

- static `ReDim`,
- runtime-bounds `ReDim`,
- pointer-backed array buffer use,
- and broader dynamic-array semantics

into separate truth-owned coverage slices.

The result was not silent miscompilation, but it was still a truth-surface gap:
the repo did not make the residual scope legible enough at the language-feature
level.

## Governing Goal

Produce a trustworthy feature-gap audit that makes it hard for any major VBA
language/runtime area to remain in one of these states:

- implemented only for a narrow subset but labeled too broadly,
- explicitly unsupported in code but not owned in the planning graph,
- partially tested without a residual-scope owner,
- or differently supported across compiler, VM, JIT, host, and boundary lanes
  without that difference being visible.

## Scope

This workset covers:

- repo-wide inventory of current explicit unsupported/not-yet-supported
  diagnostics across language/runtime/compiler/JIT/host surfaces,
- feature-family grouping of those diagnostics into semantically honest buckets,
- cross-check against existing worksets, beads, specs, and active tests,
- classification of each audited feature family as:
  - implemented,
  - implemented-subset,
  - in-progress,
  - planned,
  - or explicit unsupported boundary,
- identification of residual accepted scope with required owner beads/worksets
  where missing,
- and publication of the resulting audit and next delivery path.

This workset does not itself close feature gaps by implementation.

## Required Outcomes

1. A canonical audit artifact exists for active language/runtime feature
   completeness and residual scope.
2. Major feature families are split into semantically honest subsets where
   necessary.
3. Every accepted residual scope discovered by the audit has an explicit owner:
   workset, epic, bead, or rollout-ready path.
4. Broad claims that currently overstate support are rewritten or narrowed.
5. Unsupported diagnostics that represent real accepted work are no longer
   “stranded” without planning ownership.

## Audit Method

The audit should proceed in this order:

1. Inventory explicit unsupported boundaries from code and tests.
2. Group them into feature families:
   - arrays,
   - control flow,
   - declarations/native interop,
   - string/runtime value semantics,
   - object/property/default-member semantics,
   - error model,
   - file/host/runtime services,
   - and other major VBA surface areas as discovered.
3. For each family, classify current support by semantic subset, not by broad
   label.
4. Cross-check each residual against:
   - existing worksets,
   - open beads,
   - canonical specs,
   - and active regression/integration evidence.
5. Create or refresh missing owner paths where accepted work remains.

## Initial Execution Epics

1. Feature-gap inventory and classification audit
   - audit the repo-wide unsupported/partial language-runtime surfaces and
     produce the first canonical classification ledger
2. Residual-scope ownership repair
   - create or refresh explicit owner paths where the audit finds accepted work
     that is not yet tracked honestly
3. Truth-surface rewrite and claim narrowing
   - repair docs/spec/workset wording where feature claims are too broad for the
     actual shipped subset

## Active Epic And Rollout Mapping

- Epic: `bd-lfa1`
  - title: `Language feature residual-scope audit and ownership repair`
  - purpose: own the first careful feature-gap sweep, residual ownership repair,
    and truth-surface classification for language/runtime coverage
- First rollout bead: `bd-lfa1.1`
  - title: `Roll out language feature residual-scope audit child beads`
  - purpose: create the first executable child bead set for the audit epic,
    grouped by feature families and truth-surface repair lanes

## Current Audit Findings

The current sweep has identified these live residual seams and created owner
beads for each:

1. `bd-lfa1.2` closed
   - `DynamicMemberSelector::Name` no longer lowers to sentinel token `-1`.
   - Token-backed requests still lower directly, while name-backed requests now
     require authoritative metadata resolution or fail honestly before COM
     lowering.
2. `bd-lfa1.3` closed
   - deterministic/projection COM activation now allocates and reuses
     host-owned projection identity instead of hash- or constant-derived raw
     handles;
   - projection object descriptions now preserve truthful ProgID association
     when that state is available;
   - host/runtime evidence no longer treats `5_004`-style handle numerics as
     the first-class truth surface.
3. `bd-lfa1.4` closed
   - the typed `ComHal` callback APIs remain the live path;
   - the old test-only legacy callback interrogation wrapper names have been
     removed from the null/wasm/standard adapters;
   - the HAL draft no longer describes those legacy callback helpers as still
     present.
4. `bd-lfa1.5`
   - `bd-lfa1.5.1` closed: the dominant string-producing intrinsic lane now
     uses typed text coercion instead of legacy-token projection for common
     scalar/date operands, and host evidence now asserts typed string outputs;
   - `bd-lfa1.5.2` closed: the bounded `Split`/`Join` lane now uses typed
     shared helpers instead of token-first coercion, while preserving the
     currently documented `Join(array_tag) -> count` behavior;
   - `bd-lfa1.5.3` closed: `Like` now binds in value position and routes
     through the shared typed text helper instead of a token-first digits path;
   - `bd-lfa1.5.4` closed: the bounded `MidStmt` mutation lane now uses a
     shared typed helper for target and replacement coercion, preserving the
     numeric subset while adding direct string-target evidence;
   - `bd-lfa1.5.5` closed: the shared `Div` / `\` / `Mod` and branch-truthiness
     lane now uses typed numeric helpers instead of token-first coercion, with
     the existing branch/division formal rows still passing;
   - `bd-lfa1.5.6` closed: `StrConv`, `Chr`, `Asc`, `Space`, `String$`, `Hex`,
     `Oct`, and the remaining VM-side `Val` fallback now use shared typed
     helpers instead of token projection, with the focused `formal_v45` lane
     still passing;
   - `bd-lfa1.5.7` closed: the bounded math/date helper cluster (`Abs`, `Sgn`,
     `Round`, `Sqr`, the trig/log helpers, `MonthName`, `DateSerial`,
     `TimeSerial`, `DateAdd`, `DateDiff`) now uses shared typed numeric/date
     coercion helpers in VM and JIT, with direct semantics coverage and focused
     `formal_v48` / `formal_v49` / `formal_v545` evidence;
   - `bd-lfa1.5.8` closed: the remaining JIT tag-introspection (`VarType`,
     `TypeName`, `IsNumeric`, `IsArray`) and PRNG seed (`Rnd`, `Randomize`)
     lane now uses direct runtime-shape inspection plus typed numeric coercion,
     with focused `formal_v50` and `formal_v55x` evidence including numeric-text
     `Randomize`;
   - `bd-lfa1.5.9` closed: the shared scalar arithmetic and comparison lane
     (`+`, `-`, `*`, `^`, unary negation, increment, and the non-string
     comparison fallback) now uses typed numeric compatibility helpers, with
     direct semantics coverage plus focused `formal_v49` evidence for numeric
     text;
  - `bd-lfa1.5.10` closed: the residual non-COM compatibility carrier lane now
    uses explicit typed compatibility or explicit tagged-array handling for
    scalar `text`/`usize`/array-index coercion, `Join(array_tag)`, and
    `DateValue` / `CDate` numeric passthrough, and the dead local VM token
    wrapper is gone;
  - `bd-lfa1.5.11` closed: the remaining JIT finance (`FV`, `PV`, `PMT`,
    `NPV`, `IRR`, `MIRR`, `Rate`, `NPer`) and collection compatibility helpers
    now use explicit typed numeric compatibility instead of
    `runtime_value_legacy_token(...)`, with focused `formal_v49` and
    `formal_v53` evidence;
  - the `bd-lfa1.5` family is now closed: `runtime_value_legacy_token(...)` is
    no longer a live execution dependency in shared VM semantics, the VM
    interpreter, or JIT runtime helpers.
5. `bd-lfa1.6`
  - `bd-lfa1.6.1` closed: the main HAL native COM invoke and metadata rows now
    resolve by member name/request shape instead of hardcoded DISPIDs and
    missing-argument sentinels, and the controlled self-return regression now
    asserts state-owned object identity directly;
  - `bd-lfa1.6.2` closed: the controlled projection rows, native COM event
    rows, and controlled property-put/property-get/exception rows in the
    standard HAL adapter suite now use request-shaped or name-driven invoke
    helpers instead of the scalar `dispatch_invoke_legacy(object, member, arg)`
    wrapper;
  - the `bd-lfa1.6` family is now closed: only the explicit
    `dispatch_invoke_legacy_v2` compatibility comparison row remains, and it is
    now clearly a compatibility-only seam rather than the truth surface for
    ordinary semantic regression rows.
6. `bd-lfa1.13`
  - closed: the last direct shared-semantics COM / `WithEvents` token carrier
    conversions now use explicit typed handle/token carriers (`ObjectHandle`,
    `BindingHandle`, raw `I32` / `I64`, and string-or-token dynamic selectors)
    instead of the generic `runtime_value_legacy_token(...)` projector.
7. `bd-lfa1.7`
  - `oxvba-vm` still leaks formatted return-type strings when projecting
    external descriptor views, justified only by bounded descriptor
    cardinality.
8. `bd-lfa1.8`
  - textual `DateValue` / `CDate` coercion still reports broad
     `...string format is not yet supported` diagnostics for many string
     shapes outside the currently landed bounded subset.
8. `bd-lfa1.9`
   - native declare `ByRef` marshaling still rejects parameter types outside
     the current bounded scalar subset.
9. `bd-lfa1.10`
   - host-backed `Kill` still rejects wildcard paths entirely instead of
     implementing honest wildcard semantics for the supported host scope.
10. `bd-lfa1.11`
   - compiler coverage still explicitly expects the native internal dynamic
     route to keep a transitional token table.

Related residual already owned elsewhere:

- `bd-cmpr1.4.1` already owns the remaining honest `VarPtr` / `ObjPtr`
  unsupported matrix (`Variant` object/array/decimal cases and adjacent
  pointer-helper widening), so the audit does not duplicate that bead here.

Recent completion note:

- `bd-lfa1.7` is now closed.
  - `DynLinkDescriptorView.return_type` now carries owned/cow text rather than
    relying on `Box::leak(...)` in the VM interpreter path.
  - focused dynlink/descriptor contract checks remain green after the shape
    change.

- `bd-lfa1.8` is now closed.
  - the shared semantic date parser now accepts month-first textual forms and
    dot-separated date tokens in addition to the earlier day-month-name subset.
  - direct VM coverage and host execution coverage now prove
    `DateValue("January 1, 2000")` and `CDate("Jan. 1, 2000")`.
  - purely numeric locale-sensitive ambiguity remains explicit rather than
    silently guessed.

- `bd-lfa1.9` is now closed.
  - Windows native declare `ByRef` marshaling now includes honest boundary cells
    for `Currency` and `Date` in addition to the earlier scalar/LongPtr subset.
  - focused VM/JIT evidence now proves `OleAut32.VarCyFromI4` and
    `OleAut32.VarDateFromStr` write back into OxVba variables correctly.
  - residual unsupported `ByRef` shapes still fail explicitly rather than
    pretending to have a native representation.

- `bd-lfa1.10` is now closed.
  - host-backed `Kill` now expands wildcard file-name patterns within a concrete
    parent directory instead of rejecting all wildcard paths outright.
  - focused host-backed end-to-end coverage now proves matching files are
    deleted while non-matching files remain.
  - wildcard patterns in parent-directory path segments remain explicit residue
    rather than silently approximated.

- `bd-lfa1.11` is now closed.
  - internal project dynamic routes no longer populate the compiler’s hardcoded
    transitional token table.
  - compiler and host evidence now assert durable member metadata instead of
    those legacy internal tokens.
  - numeric token routing remains only where explicit dispatch IDs or
    authoritative external COM metadata actually exist.

Completed follow-on delivery note:

- `bd-lfa1.12` is now closed.
  - `bd-lfa1.12.1` delivered host-backed `Dir` wildcard expansion plus
    repeated-call enumeration state, including parent-segment wildcard matching.
  - `bd-lfa1.12.2` delivered the same shared matcher/path-expansion substrate
    for host-backed `Kill`, removing the earlier concrete-parent-only wildcard
    limitation.

- `bd-lfa1.14` is now closed.
  - the remaining live VM/JIT execution uses of direct `to_legacy_i32()` have
    been removed from file-open mode packing, runtime `ReDim` bound evaluation,
    `EOF`/`Loc`, `Int`, `For Each`, and `WithEvents` zero-check paths.
  - the fixed-array `formal_v42` host proofs were updated to match the honest
    post-base-slot snapshot layout instead of the older pre-base-slot index
    assumptions.

- `bd-lfa1.15` is now closed.
  - the residual compatibility projector surface has been renamed repo-wide
    from `from_legacy_i32` / `to_legacy_i32` to
    `from_compat_slot_i32` / `project_compat_slot_i32`.
  - this keeps the remaining host/CLI/HAL/debug/snapshot compatibility lane
    explicit instead of leaving the old token-era method names scattered across
    ordinary maintenance surfaces.
  - focused `oxvba-hal`, `oxvba-vm`, and `oxvba-host formal_v42` checks remain
    green after the rename. `oxvba-cli` still has unrelated pre-existing
    case-normalization failures in its scaffold/entry-point rows; that residue
    is not part of this token-cleanup lane.

- `bd-lfa1.16` is now closed.
  - authored/display casing now survives the compiler/project/CLI naming
    pipeline while lookup remains case-insensitive.
  - the remaining helper-before-`Main` source snapshot seam was also removed:
    source execution now projects entry-procedure slots by runtime metadata
    instead of slicing from register zero.
  - proving rows:
    - `cargo test -p oxvba-cli --quiet`
    - `cargo test -p oxvba-host source_snapshot_uses_entry_procedure_slots_when_helper_precedes_main_vm_and_jit -- --nocapture`
    - `cargo test -p oxvba-host --test pointer_helpers_end_to_end -- --nocapture`

## Exit Condition

This workset is complete only when:

- the audit artifact exists and is populated for the active major feature
  families,
- residual accepted scope is explicitly owned,
- and the repo’s language/runtime completeness claims are aligned to that audit
  rather than broad narrative shorthand.

Current state:

- complete; the ownership audit and follow-on delivery/cleanup beads under
  `bd-lfa1` are closed.
