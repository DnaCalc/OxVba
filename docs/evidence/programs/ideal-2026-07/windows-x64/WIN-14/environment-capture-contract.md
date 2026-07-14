# WIN-14 environment capture and certification-plan contract

Date: 2026-07-14
Program: `ideal-2026-07`
Bead: `bd-59co.3.15.5`
Canonical row: `WIN-ABI-CARRIER/WAC-CLEAN-CERT-ENV`
Clauses: `CONF-ORACLE-001`, `CONF-QUALITY-001`, `PROFILE-WIN-001`

## Result

The single WIN-0 producer, `scripts/capture-ideal-environment.ps1` with its
`lib-ideal-environment-capture.ps1` contract library, now has three explicit
and non-interchangeable identity layers:

1. the existing immutable development/oracle-host capture;
2. a versioned certification-environment plan and deterministic plan seal;
3. a future trusted environment capture sealed by pinned-image restore/session
   attestation.

Only the first two layers are available. A plan seal binds intended inputs; it
does not attest that a VM was restored, that the observations came from that
VM, or that the session remained clean. Both the plan and its seal therefore
carry `certification_authority=false`, `noncertifying=true`, and the seal carries
`attestation_state=required-unavailable`. The producer continues to reject a
`certification-vm` capture before Excel or host observation until the trusted
pinned-image restore/session-attestation producer and verifier exist.

This is support-contract progress only. `WAC-CLEAN-CERT-ENV` remains planned,
and capability and release-certification credit remain `none`.

## Versioned plan contract

`oxvba-windows-x64-certification-environment-plan-v1` binds the exact case,
matrix row, controlled capture path/schema, environment identity, Windows
build, Office product/version/build/channel/64-bit identity, an explicit
canonical default locale and a distinct canonical non-default locale,
resolvable ANSI/OEM Windows codepages, immutable image/snapshot
identity, reset policy, fixture manifest/root/recipe/artifact hashes,
owned-process cleanup policy, Excel/VBE UIA modal policy, and the required
attestation schema.

The validator rejects:

- provisional or mutable environment, OS, Office, locale, image, fixture, or
  path identities;
- non-x64 or non-Office64 plans;
- invalid, absent, or positive-but-unsupported Windows codepages;
- placeholder, unsupported, or equal declared-default and observed locales;
- case, fixture, environment, or controlled-artifact-root mismatches;
- reset policies without a pinned image/snapshot reset or revert;
- cleanup policies that do not confine cleanup to recorded owned processes;
- UIA policies without owned Excel/VBE modal interception; and
- any attempt to turn a plan into certification authority.

`oxvba-windows-x64-certification-environment-plan-seal-v1` records a
deterministic SHA-256 over the exact plan object. Its name and fields deliberately
identify it as a *plan seal*, not an environment or capture seal. It remains
noncertifying until a separate trusted attestation is verified by this same
capture authority.

## Development-host dry run

`capture-ideal-environment.ps1 -EnvironmentId win-x64-dev-oracle-2026-07
-DryRun` executes the complete read-only reconstruction and validation path,
including exact OS/Office/locale/toolchain and fixture-authority checks. It does
not publish, replace, or check-write evidence. Its deterministic result is:

```text
environment=win-x64-dev-oracle-2026-07
release=false
certification_authority=false
noncertifying=true
dry_run=true
capture_sha256=sha256:6616a1302f787f77f1acf022315a92f428f425279ef46d5752666c8ff3e1edf1
```

`-Check` and `-DryRun` are mutually exclusive. The existing immutable capture
and report timestamps remain unchanged across a dry run.

## Focused evidence

Commands run from the repository root:

```powershell
./scripts/test-capture-ideal-environment.ps1
./scripts/capture-ideal-environment.ps1 -EnvironmentId win-x64-dev-oracle-2026-07 -DryRun
```

The focused suite passed `11` positive and `32` negative cases. It covers exact
capture reconstruction; idempotent publication; dry-run non-mutation; dev-host
authority rejection; exact certification case, fixture, and path binding;
deterministic plan/seal production; placeholder, image, locale, codepage, fixture hash,
reset, process, UIA, authority, plan-digest, and attestation-state mutations;
an alternate supported codepage changing the sealed plan digest; and the
pre-existing owned timeout/reap proof.

Independent controller review rechecked the schema closure, canonical hashing,
locale/codepage admission, placeholder rejection, noncertifying authority
boundary, dry-run side effects, and the CLI's refusal to cross the missing
attestation boundary. PowerShell parsing, the focused suite, the exact dry run,
documentation and diff checks pass with no remaining finding.

## Six-axis evidence

| axis | evidence |
|---|---|
| result | Dev dry run reconstructs the exact canonical capture hash. Synthetic fully pinned plan inputs validate and produce a stable plan digest/seal; current provisional certification inputs fail closed. |
| full Err | Not applicable: no VBA compile or execution occurs and no VBA `Err` state is created. PowerShell failures identify the rejected contract field. |
| side effects | Dry run writes no evidence, registry, Office, fixture, or certification state. The focused suite observed unchanged capture/report write times. |
| lifecycle/order | Select exact environment/case and controlled path; validate plan inputs; bind deterministic plan seal; require trusted restore/session attestation; only a future attested path may observe and seal a certification capture. |
| transport | Read-only manifest/JSON/PE/registry/locale/toolchain input for the development capture; no Excel, VBE, COM, UIA, fixture execution, or certification VM is started. |
| balance | The existing capture path requires empty Excel PID sets before/after, equal selected-registry hashes, and awaited/disposed owned version-reader children. Dry run introduces no additional resource owner. |

## Residual boundary

`bd-59co.3.15.3` still owns provisioning a clean pinned Windows x64 VM with
actual 64-bit Excel, an explicit non-default locale, immutable fixture inputs,
and a trusted restore/session-attestation producer and verifier. That work must
replace every provisional certification identity, restore the pinned image
before each run, bind the observed session to the restored image, and prove
owned process/dialog/registry/file/handle balance. Only then may this producer
emit a certification-authoritative environment capture and advance
`WAC-CLEAN-CERT-ENV`.

The closed WIN-0 handoff under `bd-59co.3.1.7` publishes the development
capture at its controlled root, but it remains the `dev-oracle`,
`release=false`, noncertifying identity. It is compatible with this contract and
cannot satisfy or substitute for the certification-plan attestation boundary.
