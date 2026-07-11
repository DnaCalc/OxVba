# Ideal Program Matrix Schema V1

Date: 2026-07-10
Program: `bd-59co` / `ideal-2026-07`
Ownership manifest: `IDEAL_MATRIX_OWNERSHIP_V1.csv`
Environment manifest: `IDEAL_ENVIRONMENT_MANIFEST_V1.csv`
Contract-clause disposition: `IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv`

All 15 canonical program matrices use the same identity/authority prefix and closure tail. Matrix-specific columns describe the observable or component states owned by that matrix.

## Required vocabularies

- `truth_role`: `primary`, `projection`, `evidence`, or `quality`.
- component state: `n/a`, `planned`, `in-progress`, `implemented-subset`, `implemented-full`, or `verified`.
- `truth_state`: `planned`, `in-progress`, `implemented-subset`, `implemented-full`, `verified`, or `archived`.
- `residual_disposition`: `remaining-accepted-scope`, `intentional-boundary`, or `external-boundary`.

Bare `implemented` is invalid. A required terminal row closes only at `verified`. Before then, a required row uses `remaining-accepted-scope` and names an open/in-progress descendant of `bd-59co` in `residual_owner_bead`.

Projection and evidence rows name their primary `source_claim_key`. Contract fields contain exact clause IDs rather than wildcard families. Test/evidence anchors must resolve to current files or named external environments; deleted-stack and historical captures may seed provenance but cannot establish current verification.

Test/evidence fields use `;` or `|` between references. A plain reference is a repository file path (an optional `#anchor` or Rust `::test_name` suffix is ignored for the existence check). Typed `matrix:`, `workset:`, and `file:` references resolve their repository path. A planned `artifact:` value must be a safe repository-relative destination and resolves to an existing file when the row becomes verified. Named assertions use a nonempty `br:`, `command:`, `cargo:`, `test:`, `oracle:`, `environment:`, `excel:`, `spec:`, `external:`, `transcript:`, or `observables:` value; a raw command without one of these prefixes is invalid.

Every current executable leaf and every manifest execution epic carries executable acceptance text with all three parts:

- `command:<exact command>`;
- `expected-observable:<specific result>` (the spaced spelling `Expected observable:` is also accepted);
- at least one `artifact:`, `transcript:`, `oracle:`, or `environment:` evidence destination.

Planned work may name a future evidence destination. A verified row may not: `evidence_refs` must include a repository-resolving `artifact:`, `transcript:`, or `oracle:` actual-evidence file. It also classifies the complete shared observable with exactly this grammar (CSV-quote the field because it contains commas):

```text
observables:result=verified,full-err=verified,side-effects=verified,lifecycle-order=verified,transport=verified,balance=verified
```

Each axis is `verified` or explicitly `n/a`; `result` is always `verified`. The six classifications prevent result-only snapshots from being treated as parity evidence. `environment:<environment_id>` references resolve against the environment manifest.

`producer_dependencies` uses a semicolon delimiter and each item names a current-program epic or executable bead. Validators trim each dependency and do not treat commas as dependency separators because CSV quoting must remain unambiguous.

## Target policy

Windows matrices use `target_arch=x64`. `office_bitness` is `64` or `n/a`. x86/32-bit Office, WOW64, ARM64 and other Windows targets are outside the accepted program and must not appear as required rows.

## Traceability

`IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv` maps one bead/matrix/row relationship per record. Delivery beads advance existing rows; support rollout/scaffold beads may use a matrix-level relationship until rows are seeded. Any support bead that exposes remaining accepted capability work must leave a delivery bead owner before closing.

Allowed trace relationships are `owns`, `owns-planned-row`, `advances`, `evidences`, `projects`, and `matrix-scaffold`. `owns-planned-row` gives a rollout owner to a seeded planned row before a delivery successor exists. `evidences` may attach focused evidence to any exact row, while `projects` targets a projection matrix. Only a support `matrix-scaffold` relationship may omit `row_id`; every delivery relationship names an existing row. Every executable profile leaf and every matrix row must have at least one current-program trace relationship.

`matrix-scaffold` and rollout-owned planned rows are directed-rollout states only. When a rollout bead closes, neither may remain: its delivery leaves must exist as sibling/descendant work and each delivery leaf must have an exact row trace. Before switching to AutoRun, all 15 matrices are nonempty, each has at least one required row, and every one of the 42 execution epics has an explicit row relationship.

An execution epic closes only after it has a delivery leaf, every required row it owns or advances is `verified`, and neither those rows nor any descendant residual owner retain remaining accepted scope. Support-only completion cannot close an execution epic.

The trace `profile` is the bead's execution profile. A cross-profile producer/consumer route is valid only when the clause disposition explicitly declares that profile, the parent epic as a producer owner or consumer, and the target matrix. Matrix ownership remains authoritative for the row's profile.

Every trace clause must occur in the bead's own contract text. Producer relationships (`owns`, `owns-planned-row`, and `advances`) cover every clause on the target row. Consumer `evidences` and `projects` relationships may carry only the clauses that the evidence or projection actually proves. Across all traces for an executable leaf, the trace-clause union must equal the clause set in the bead contract: no trace may overclaim and no bead clause may remain untraced. Producer relationships use the row's residual owner; selective consumer relationships may retain their own accepted residual owner.

## Named program artifacts

New generated evidence and status belong under `docs/evidence/programs/<program-id>/<profile>/` and `docs/program-status/<program-id>/<profile>/`, where the profile is `core`, `windows-x64`, or `ide` for this manifest. The historical `docs/evidence/profiles/vNNN/` and `docs/profile-status/PROFILE_STATUS_VNNN.md` trees are read-only provenance unless a command supplies an explicit historical version allow-list.

The legacy migration ledger's `status_after` records migration-time state. During directed PROGRAM-0 it must match `br` exactly. After PROGRAM-0 closes, imported delivery rows may advance monotonically from recorded `open` through `in_progress`/`blocked` to `closed`; retired and PROFILE-EXT rows retain their exact recorded terminal/deferred state.

## Environment manifest

`IDEAL_ENVIRONMENT_MANIFEST_V1.csv` distinguishes execution roles rather than treating every Windows run as release evidence. It has one record per named environment and requires:

- stable `environment_id`, execution `role`, accepted `profile`, target architecture and OS build;
- Office product/version/build/channel/bitness or explicit `n/a`;
- locale, immutable `snapshot_or_image` identity and reset policy;
- controlled-fixture manifest and hash or an explicit pending owner;
- separate owned-process and Excel/VBE UI Automation modal policies;
- `evidence_state`, owning current-program bead and notes.

The three required role tokens are `dev-oracle`, `certification-vm` and `linux-ci`. Role names are machine-readable: explanatory boundaries belong in `evidence_state` and `notes`.

The current Windows x64 host is noncertifying development/oracle infrastructure. The clean pinned Windows x64 VM with 64-bit Excel and the pinned Linux x64 CI image are terminal environments. A terminal environment may remain `planned-blocking` during rollout, but terminal release cannot pass until its build/channel/image, fixture hashes, reset proof and evidence are pinned and verified.

The three rows are x64-only; the two Windows rows require Office bitness `64`, while Linux uses `n/a` for every Office field. Any required terminal Windows or Excel/VBA row at `truth_state=verified` cites `environment:<certification-vm environment_id>` and may not use the development/oracle host as release proof. The certification environment itself must already be `verified`. A closed umbrella likewise requires both `certification-vm` and `linux-ci` to be pinned and verified.

## Contract-clause disposition

`IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv` contains every stable clause ID declared by `OXVBA_SYSTEM_CONTRACT_V1.md` exactly once. Allowed dispositions are `in-scope` and `deferred-extended`.

- Every `in-scope` record names one or more accepted profiles (`core`, `windows-x64`, `ide`), accountable producer `owner_epics`, optional non-owning `consumer_epics`, and canonical matrix IDs. Pipe (`|`) is the multi-value delimiter; `consumer_epics=n/a` represents an empty consumer set.
- Producer-owner and consumer sets are disjoint, contain only current manifest execution epics, and collectively include at least one epic in every declared profile.
- A `deferred-extended` record uses `profiles=extended`, `owner_epics=n/a`, `consumer_epics=n/a` and `matrix_ids=n/a`.
- For this umbrella, only `PROFILE-EXT-001`, `DEBUG-CORE-001` and `FORMS-RUNTIME-001` may be `deferred-extended`.
- Contract validation extracts clause IDs from the normative system contract, rejects missing/duplicate/unknown records, resolves every in-scope owner and consumer against the program manifest/graph and every matrix ID against the ownership manifest, and rejects any other deferred clause.
- Every in-scope clause appears in current traceability. Each trace routes its clause through a declared profile, producer/consumer parent epic, and matrix; every declared owner, consumer, and matrix has a clause-bearing witness in both Directed and AutoRun modes. Deferred clauses may be absent only when explicitly classified `deferred-extended`; a trace may never claim a deferred clause. This rule is intentionally mode-independent so the AutoRun transition cannot reveal a coverage defect that Directed rollout accepted.

## LSP advertisement

`LSP_3_18_METHOD_MATRIX_V1.csv` is fail-closed. `capability_advertised` is always the literal `true` or `false`; an unimplemented or partially proved method is `false`. A method may be `true` only when:

- `direct_matrix_row` resolves to a verified primary direct-result row;
- that source row's `direct_state` or `direct_query_state` is `verified`;
- the LSP row's decoded `projection_state`, direct-result `equivalence_state`, and overall `truth_state` are all `verified`;
- its client transcript and verified actual evidence resolve.

The protocol surface therefore cannot advertise ahead of the compiler-owned direct language-service result.
