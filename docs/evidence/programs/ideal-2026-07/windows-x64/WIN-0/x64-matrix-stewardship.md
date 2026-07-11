# WIN-0 x64 Matrix Stewardship

Date: 2026-07-11
Bead: `bd-59co.3.1.3`
Effect: support only
Clauses: `CONF-MATRIX-001`, `DOC-AUTH-001`, `DOC-TRACE-001`, `PROFILE-WIN-001`

## Outcome

The Windows control surface is now guarded by
`scripts/validate-windows-x64-control-surfaces.ps1`. The validator fails closed
unless the manifest exposes exactly the six accepted Windows matrices and their
57 required row identities, with the pinned matrix stewardship and per-row
owner, evidence-owner and accepted-residual routes.

This bead changes no compiler, VM3, JIT, COM, native-import or output behavior.
All 57 rows remain `planned` with `remaining-accepted-scope`; no capability
state or compatibility claim advanced.

## Locked inventory

| matrix | role | stewardship epic | required rows |
|---|---|---:|---:|
| `WIN-COM-CLIENT` | primary | `bd-59co.3.4` | 9 |
| `WIN-COM-EVENTS` | primary | `bd-59co.3.6` | 7 |
| `WIN-COM-SERVER` | primary | `bd-59co.3.7` | 7 |
| `WIN-NATIVE-IMPORT` | primary | `bd-59co.3.10` | 8 |
| `WIN-NATIVE-EXPORT` | primary | `bd-59co.3.13` | 8 |
| `WIN-ABI-CARRIER` | quality | `bd-59co.3.2` | 18 |

The validator rejects a missing, additional, renamed or duplicate row. Every
row must be required, use profile `windows-x64`, retain its matrix role and row
prefix, and cite the normative system contract through resolving authority
references.

The embedded route contract pins all 57 authority bundles and
`owner_epic`/`evidence_owner_bead`/non-verified `residual_owner_bead` triples.
Owners must exist in the current program; evidence and residual beads must
belong to the row's owner epic; and a non-verified row's accepted-residual owner
must remain active. Evidence-owner beads may close after producing their
evidence contract, but their exact route remains pinned. A verified row is
instead rejected if it retains accepted-residual ownership.

## x64-only boundary

Every row carries all three target-control fields:

- `target_arch=x64`;
- `office_bitness=64` or `n/a` only where Office is not part of that row; and
- a nonblank process shape.

Process shape uses an explicit current x64 vocabulary. The complete active row
text is rejected if it introduces x86/i686, WOW64, ARM64/ARM64EC/aarch64,
32-bit Windows/Office/Excel/process/artifact/host spellings, Office32 or Excel32
artifacts. Conventional x64 target triples using `x86_64` or `x86-64` remain
valid. `InprocServer32` and `LocalServer32` remain valid x64 COM registry value
names; they are not target architecture claims and are intentionally not
classified as x86 artifacts.

## Separate claim and output classes

The two umbrella claim gates remain independently represented:

| gate | canonical row | exact profile clause |
|---|---|---|
| Windows x64 VBA compatibility | `WIN-ABI-CARRIER/WAC-PROFILE-TERMINAL` | `PROFILE-WIN-001` with completion/documentation clauses |
| standalone Windows tooling/native output | `WIN-NATIVE-EXPORT/WNE-PROFILE-TOOL-TERMINAL` | `PROFILE-TOOL-001` |

The output rows are also locked to honest classes and backends:

- `WrapperExe`, `WrapperLibrary` and `WrappedComServer` remain package-backed
  `JIT-session` wrappers with `BUILD-PACKAGE-001`, never `BUILD-NATIVE-001`;
- `NativeDll`, `NativeExe` and `NativeDll-and-NativeExe` remain Cranelift
  object/linker outputs with `BUILD-NATIVE-001` and `JIT-AOT-001`, never
  package-backed wrappers.

Changing either terminal clause set or relabelling a native row as a wrapper is
a validation failure.

## Focused failure evidence

`scripts/test-windows-x64-control-surfaces.ps1` builds process-unique temporary
fixtures and proves clean positive copies, including `x86_64` and `x86-64`
artifact triples, plus twelve fail-closed mutations:

1. required-row removal;
2. `target_arch=x86`;
3. `office_bitness=32`;
4. a WOW64 process artifact;
5. a `32-bit-process` alias;
6. an `Office-32bit` order variant;
7. ARM64EC/aarch64 artifacts;
8. evidence-owner drift;
9. residual-owner drift;
10. authority-route drift;
11. compatibility/tooling terminal-clause collapse; and
12. native-output relabelling as a wrapper.

The test owns and verifies its temporary path before recursive cleanup. It does
not edit canonical matrices or bead truth.

The acceptance run also exposed and repaired a shared validation portability
bug: execution-control field parsing accepted LF but rejected a normal CRLF
Windows checkout. The shared execution-mode reader, active-program reader and
profile-artifact reader now accept either line ending without changing
`AUTORUN_STATE` content.

## Acceptance results

The following commands pass in the isolated Windows worktree:

- `./scripts/validate-windows-x64-control-surfaces.ps1` — 6 matrices, 57
  required rows, x64 target, two distinct claim gates;
- `./scripts/validate-validation-ownership.ps1` — 15 manifest matrices and 3
  profiles;
- `./scripts/validate-contract-clause-disposition.ps1` — 60 clauses, 57 in
  scope and 3 deferred;
- `./scripts/validate-bead-traceability.ps1` — exact current-program route
  graph;
- `./scripts/test-windows-x64-control-surfaces.ps1` — positive fixtures and all
  12 negative mutations.

The new positive validator is part of both governance and truth reconciliation.
The slower mutation suite remains a focused validator-maintenance check.

## Residual truth

Only `WIN-ABI-CARRIER/WAC-TARGET-DEV-ENV` remains assigned to WIN-0 support
bead `bd-59co.3.1.2`. All COM, event, serving, Declare/callback, carrier,
certification, wrapper and genuine native-output rows retain their exact
downstream delivery or certification owners. This support result supplies no
implementation or certification credit.
