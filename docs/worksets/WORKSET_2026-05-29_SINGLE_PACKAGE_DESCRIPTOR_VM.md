# Single Package-Descriptor VM Consolidation Workset

Status: `in-progress`
Date: 2026-05-29
Scope owner: OxVBA VM/compiler/host
Epic: `bd-eura`

## Purpose

There is **one** VM: it runs the compiler's bytecode + metadata package directly, and it must
run the **full build-target feature set correctly, without non-object memory leaks**. We are
not migrating lane-by-lane from an old VM to a descriptor-based one, and execution is **not**
gated on a "supported/unsupported-rejected" classification. Anything the VM runs incorrectly
or that leaks non-object memory is a **bug to fix**; legacy/duplicate paths are **housekeeping
to delete**.

This supersedes the lane-migration / strict-gating framing of the prior `bd-7ifr` /
`bd-dpmy` worksets (now abandoned). Old/closed worksets and beads are not back-patched.

## Policy

- The VM executes the package; there is no consumption-evidence execution gate.
- Non-object memory (BStr/Variant/SafeArray) is freed by Rust `Drop` (verified); the
  "no leaks" bar applies to non-object memory and interop refcount balance.
- Object reference-cycle leaks are VBA-consistent and **not** bugs in this scope (a
  beyond-VBA cycle GC is a possible future extension, out of scope).
- The Excel-differential + leak/refcount harness is deferred until the VM is fully
  functional, then runs in parallel with the JIT work.
- Real package descriptors (carrier layout, error/cleanup/interop maps, identity/digests)
  stay — they are what the VM runs from and the future JIT will lower.

## Execution Beads (`bd-eura`)

| Bead | Type | Outcome |
| --- | --- | --- |
| `bd-eura.1` | delivery | Delete the consumption-evidence + support-report gating apparatus and the execution gate. |
| `bd-eura.2` | delivery | Collapse legacy/dual interpreter paths to the single correct path. |
| `bd-eura.3` | delivery | Full build-target functionality: COM in/out (early+late), native in/out, UDTs, events, all internal types. |
| `bd-eura.4` | delivery | Feature-coverage tests across all metadata shapes + the bytecode interpreter. |
| `bd-eura.5` | support | Delete old docs/references to superseded VM mechanisms from live docs. |

## Working Method

After each bead's work: run the impacted crate tests + `cargo clippy --all-targets -- -D
warnings` + `cargo fmt --check` + `./scripts/run-jit-v2-tracer-fixtures.ps1` +
`./scripts/check-governance.ps1`; do a fresh-eyes review for blunders/omissions/bugs; rework
until clean; then commit and proceed.

## Non-Goals

- No consumption-evidence/support-report gating of execution.
- No beyond-VBA object-cycle GC (future, out of scope).
- No leak/Excel-differential harness yet (deferred; post-VM, parallel with JIT).
- No JIT execution / Cranelift activation.
