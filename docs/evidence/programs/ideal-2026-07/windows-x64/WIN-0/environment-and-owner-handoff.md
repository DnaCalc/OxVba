# WIN-0 Development Environment and Owner Handoff

Date: 2026-07-14
Bead: `bd-59co.3.1.7`
Effect: support only
Clauses: `CONF-MATRIX-001`, `CONF-ORACLE-001`, `DOC-AUTH-001`,
`DOC-TRACE-001`, `PROFILE-WIN-001`

## Outcome and authority boundary

The accepted immutable characterization of the current Windows x64/Office64
development and Excel/VBA oracle host is now published at the exact tracked
controlled root required by the fixture contract:

`artifacts/windows-x64/controlled-environments/v1/win-x64-dev-oracle-2026-07/environment-capture.json`

Its canonical raw/canonical-source SHA-256 is
`sha256:6616a1302f787f77f1acf022315a92f428f425279ef46d5752666c8ff3e1edf1`.
The published bytes are exactly identical to
`docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json`.
The capture remains `release=false`, `certification_authority=false`,
`noncertifying=true`, and `evidence_state=characterized-noncertifying`.

This verifies only the Windows control row
`WIN-ABI-CARRIER/WAC-TARGET-DEV-ENV`: the x64 target and the identity and role
of the current development/oracle host are known and reproducible. It supplies
no COM, native, VM3, JIT, Excel-parity, carrier, artifact, clean-deployment or
release-certification credit.

## Deterministic publication

`scripts/sync-windows-dev-environment.ps1` has one deliberately narrow job. It
validates the accepted capture against `IDEAL_ENVIRONMENT_MANIFEST_V1.csv` and
either creates those exact bytes once at an absent controlled path or checks an
existing byte-identical publication,
the exact schema, environment identity, x64/Office64 facts, reset-policy hash,
noncertifying flags and immutable capture hash. Source validation and the fixed
accepted hash precede any write; a different existing publication is rejected
without replacement, and every existing path component must be non-reparse.
It does not edit capability
matrices or infer capability truth.

The focused mutation suite rejects a missing accepted capture, a missing
controlled publication in check mode, controlled-byte drift, certification
authority on the development host, capture-authority owner drift and canonical
environment-fact drift. It also proves that a bad source and a differing
existing publication leave existing controlled bytes unchanged, and that a
supported junction/reparse route cannot publish outside the controlled root.
The existing environment and fixture validators also
reject duplicate or mis-cased JSON fields, identity/role/image/reset drift,
non-x64 facts, a certifying development capture and a forged environment hash.

## Exact matrix transition

The controller applies the truth transition; the publication script never
mutates canonical matrices.

Exactly the twelve `WINDOWS_ABI_CARRIER_MATRIX_V1.csv` rows whose
`environment_id` is `win-x64-dev-oracle-2026-07` bind the published hash above.
For the eleven capability consumers, this changes only `environment_hash`.
Their `truth_state`, `evidence_state`, compiler/package/VM3/JIT/build states,
`owner_epic`, `evidence_owner_bead`, `residual_disposition` and
`residual_owner_bead` remain exactly as they were before this handoff.

Only `WAC-TARGET-DEV-ENV` additionally transitions as follows:

| field | accepted value |
|---|---|
| `metadata_revision` | `win-x64-dev-oracle-2026-07-capture-v1` |
| `environment_hash` | `sha256:6616a1302f787f77f1acf022315a92f428f425279ef46d5752666c8ff3e1edf1` |
| `evidence_state` | `verified` |
| `truth_state` | `verified` |
| `evidence_owner_bead` | `bd-59co.3.1.2` |
| `residual_disposition` | empty (no residual) |
| `residual_owner_bead` | blank |

Its test anchors are the publication sync/mutation scripts and the environment
and fixture validators. Its evidence references are the typed development
environment identity, controlled publication, accepted capture and this
handoff. Its stable logical `fixture_id` remains
`environment-dev-oracle-v1`, while `fixture_hash=n/a`: the environment
publication is not a built capability fixture. In the derived fixture manifest
this one environment-only control row uses
`source_recipe_state=not-applicable` and
`built_artifact_state=not-applicable`, with path/hash/owner fields `n/a`. No
other row may use those states.

Deterministic fixture generation therefore yields twelve `environment_state`
values of `current`, the exact controlled path and hash, and
`environment_owner_bead=n/a`; the other 45 environment entries remain
`pending` under the clean certification owner. Source recipes are 20 current,
36 pending and one not applicable; built artifacts are 56 pending and one not
applicable. Every fixture row continues to carry `capability_credit=none`.

The matching current-stack residual row is now `canonical_truth_state=verified`,
`current_test_state=current-subset`, `gap_kind=control-satisfied` and
`live_residual_owner_bead=n/a`. The other 56 residual rows remain `planned`
with their exact downstream owner. The support owner that captured the facts
remains `.3.1.2`; capture ownership is not rewritten to the reconciliation bead.

## Downstream ownership and release boundary

All capability delivery stays with WIN-1 through WIN-13 and their exact row
owners. Clean deployment, the pinned resettable Windows x64/64-bit Excel VM,
current Excel/VBA oracle certification and profile closure remain under
WIN-14. In particular:

- `WAC-CLEAN-CERT-ENV` stays planned under `bd-59co.3.15.3`;
- the 45 certification-environment hashes stay pending;
- `WCC-EXCEL-AUTHORITY`, `WAC-EXCEL-COM-CERT`,
  `WAC-EXCEL-NATIVE-CERT` and `WAC-RELEASE-CERT` do not consume the
  development host as release authority; and
- no Excel, VBE, UI Automation, COM activation, registry mutation or native
  fixture execution was performed for this handoff.

Owned-resource rules remain those of
`docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md`: record before mutation,
retain exact object/process identity, clean only recorded owned resources and
surface conflicts instead of broad cleanup. Publishing this tracked JSON file
does not grant mutation authority over any external resource.

## Acceptance record

The decisive checks are:

```powershell
./scripts/sync-windows-dev-environment.ps1 -Check
./scripts/test-windows-dev-environment.ps1
./scripts/validate-windows-x64-control-surfaces.ps1
./scripts/validate-windows-fixture-manifest.ps1
./scripts/test-windows-owned-resource-policy.ps1
./scripts/validate-windows-current-stack-residuals.ps1
./scripts/validate-environment-manifest.ps1
./scripts/run-truth-reconciliation.ps1
./scripts/check-governance.ps1
```

The environment publication check and eight focused fail-closed mutations pass
without Excel/VBE launch or external Windows mutation. Full reconciliation and
governance consume the controller-owned canonical transition described above;
the integrating controller recorded the following final result on 2026-07-14:

- the complete fixture suite passed eight positive and 56 fail-closed mutation
  cases in 202.4 seconds;
- the owned-resource policy passed 81 assertions and 65 fail-closed mutations
  over owned Registry64, file and harmless-child resources, with exact teardown;
- current-stack residual and WIN-14 certification-manifest suites passed 11 and
  12 fail-closed mutations respectively;
- the generalized program validator passed 26 negative cases;
- truth reconciliation passed with 193 canonical rows, including exactly two
  verified rows overall and exactly this one verified Windows infrastructure
  row; and
- full governance passed in 126.1 seconds.

Fresh integration review found and repaired the certification-manifest
assumption that every producer remained planned, duplicate trace relationship
semantics, and missing typed six-axis evidence grammar before accepting the
handoff. The certification manifest now records this producer as satisfied but
keeps its certification case blocked on the clean certification environment and
runner. No capability or release credit was introduced.

## Residual

The current host is useful development/oracle infrastructure but cannot prove a
clean start, reset, deployment or release result. Provisioning and attesting the
pinned clean certification VM remains required under `bd-59co.3.15.3`, and all
Windows capability delivery remains open under its existing downstream owners.
