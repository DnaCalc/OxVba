# Current Blockers

Date: 2026-07-10
Status: current; no all-path blocker
Authority: blockers only; ordinary unfinished capability work belongs in the three current worksets and canonical matrices

## BLK-BASELINE-001 — Repository release gates are not green

Status: open, progress available
Owner: core workset CORE-1

Impact:

- ordinary `cargo test --workspace` is red in `oxvba-differential`;
- deterministic snapshot line-ending and VM3 policy-error BSTR-balance failures prevent a green baseline;
- parallel carrier counters interfere across some tests;
- strict workspace Clippy is red;
- stale host/JIT assertions fail on current behavior.

Unblocking outcome:

- cross-platform EOL policy;
- isolated/fixture-addressable balance harness;
- minimized/fixed policy-error leak;
- current stable-code assertions;
- strict Clippy and ordinary workspace tests green on required hosts.

This does not block architecture/compiler/package work that does not depend on a green release baseline, but no release/profile closure can pass while it remains open.

## BLK-COM-EVENT-BYREF-001 — Native COM event writeback requires synchronous reentry

Status: open, design and implementation work available
Owner: Windows workset WIN-5
Clauses: `COM-EVENT-001`, `WIN-PLAN-001`

Impact:

- current queue-backed COM callback transport returns before the VBA handler runs;
- source-owned `VT_BYREF` arguments cannot be mutated before Invoke returns;
- Excel-style cancellable events such as `Workbook.BeforeClose(Cancel)` are not conforming;
- VM3/JIT COM-event profile cannot close.

Unblocking outcome:

- authoritative typed source-interface/parameter metadata;
- synchronous scoped callback into the owning project session;
- handler mutation copied to raw `VT_BYREF` storage before return;
- explicit same-thread/cross-apartment/out-of-proc policy and evidence;
- success/error/reentry/lifecycle cleanup under both backends.

## BLK-WINDOWS-CERT-001 — Mandatory Windows/Office certification environments

Status: provisioning prerequisite; not yet demonstrated for the current stack
Owner: Windows workset WIN-14 and core workset CORE-9

Required:

- pinned supported Windows x64 environment with actual 64-bit Excel;
- x64 native/COM fixture runners;
- controlled in-proc/out-of-proc COM fixtures;
- non-default locale profile;
- owned-process UIA/VBE compile-oracle automation and cleanup.

Unblocking outcome: checked-in environment manifest and reproducible current-stack x64 runs with artifact/build/fixture hashes. Non-x64 Windows targets are outside the accepted profile and are not blockers.

## Historical blocker migration

Resolved and superseded March-July blocker narration was removed from this current register. Its provenance remains in git history and evidence artifacts. Valid unfinished residuals must be represented in a current workset/matrix row; a historical blocker ID does not remain active merely because an old document cites it.
