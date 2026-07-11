# WIN-0 Controlled x64 Fixture Inventory

Date: 2026-07-11
Bead: `bd-59co.3.1.4`
Effect: support only
Clauses: `CONF-MATRIX-001`, `PROFILE-WIN-001`

## Outcome and no-credit boundary

`docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv` is the controlled
fixture inventory for all 57 required rows in the six Windows matrices. Every
entry pins an immutable x64 recipe and artifact identity, process shape,
Office bitness, apartment shape, exact signature, execution recipe, owned
cleanup recipe and the six required observable axes.

This result does not implement or certify a Windows capability. Every manifest
row has `capability_credit=none`; all 57 canonical matrix rows retain
`truth_state=planned` and `evidence_state=planned`. Current source/recipe hashes
identify reproducible input bytes only. They are not built-artifact, runtime,
VM3/JIT, Excel/VBA or release-certification evidence.

## Exact coverage

| matrix | rows | current source/recipe | pending source/recipe | pending built artifacts |
|---|---:|---:|---:|---:|
| `WIN-ABI-CARRIER` | 18 | 8 | 10 | 18 |
| `WIN-COM-CLIENT` | 9 | 6 | 3 | 9 |
| `WIN-COM-EVENTS` | 7 | 1 | 6 | 7 |
| `WIN-COM-SERVER` | 7 | 4 | 3 | 7 |
| `WIN-NATIVE-IMPORT` | 8 | 0 | 8 | 8 |
| `WIN-NATIVE-EXPORT` | 8 | 1 | 7 | 8 |
| **total** | **57** | **20** | **37** | **57** |

The manifest key is `matrix_id|row_id`. Row, fixture, recipe and built-artifact
identities are unique. `fixture_id`, `process_shape`, `apartment_shape`,
`exact_signature`, environment identity and all six expectations are copied
from the canonical matrix row and checked against it; the manifest cannot
silently redefine capability truth.

## Recipe and hash semantics

### Source/recipe

Twenty rows reuse controlled current repository sources for bounded fixture or
recipe inputs: the repo-owned in-process COM dispatch/vtable fixture, current
COM matrix source, current wrapped-server source, current runtime carrier
source and the current typelib test-server source. Their
`source_recipe_hash` is SHA-256 over:

1. the immutable row recipe fields and six expectations in fixed order;
2. normalized, ordinal-sorted, deduplicated repo-relative source paths; and
3. each source file's UTF-8 bytes after BOM removal and CRLF/CR-to-LF
   normalization.

This makes source hashes stable across Git checkout EOL policy. The other 37
rows use the exact triple `source_recipe_state=pending`,
`source_recipe_paths=pending`, `source_recipe_hash=pending` and retain their
active matrix residual owner in `source_recipe_owner_bead`.

The validator rejects generated/history/evidence paths and binary extensions as
source/recipe inputs. Current source hashes therefore cannot be fabricated from
an old DLL, TLB, workbook or archived capture.

### Built artifacts

All 57 `built_artifact_state` values are `pending`, with `path=pending`,
`hash=pending` and the exact active matrix residual owner. No historical or
ad-hoc binary is reused as current proof. A future current artifact requires an
immutable x64 artifact identity, a controlled non-historical repo path and a
SHA-256 that recomputes from the file bytes; changing a matrix fixture hash
without adding that controlled path fails generation.

### Environments

All 57 environment hashes remain pending because neither an immutable dev-host
capture nor the clean certification image has landed:

| environment | rows | pending owner | meaning |
|---|---:|---|---|
| `win-x64-dev-oracle-2026-07` | 12 | `bd-59co.3.1.2` | characterized, noncertifying development/oracle host |
| `win-x64-cert-vm-pending-v1` | 45 | `bd-59co.3.15.3` | clean pinned release-certification VM still blocking |

Environment hashes are separate from both source/recipe and built-artifact
hashes. A current environment hash requires the environment owner to supply an
immutable capture path. The later WIN-0 reconciliation bead
`bd-59co.3.1.7` owns the environment/fixture/downstream handoff; this bead does
not edit `IDEAL_ENVIRONMENT_MANIFEST_V1.csv`.

## Six observable axes

Every row carries the matrix's independent expectations for:

| axis | required observable |
|---|---|
| result | structural value/result, not a status tag |
| full Err | number, source, description and applicable help/line state |
| side effects | writeback and externally visible effects |
| lifecycle/order | call, event, reentry, writeback and cleanup order |
| transport | dispatch/vtable/proxy/native/export route and exact signature |
| balance | carrier, reference, pin, callback, registration and process balance |

Execution recipes pin `target=x64`, the exact fixture/process/apartment and a
six-axis capture. Cleanup recipes are category-specific and limit mutation and
cleanup to recorded owned PIDs, interfaces, callbacks, files and HKCU keys.
Blanket dialog, process, registry or filesystem cleanup is not admitted.

## Fail-closed checks

The deterministic sync and validator enforce:

- the exact V1 header, six matrices and 57-row identity set;
- no missing, extra or duplicate row/fixture/recipe/artifact identity;
- immutable versioned fixture identities and explicit x64 recipe/artifact
  identities, with x86/WOW64/ARM64 and mutable `latest/current/head` identities
  rejected;
- normalized sorted deduplicated source paths and LF-stable hash recomputation;
- distinct source/recipe, built-artifact and environment state/hash/owner
  triples;
- an active exact owner for every pending triple;
- nonblank process, apartment, exact-signature, execution, cleanup and six-axis
  fields; and
- `capability_credit=none` on every row.

`scripts/test-windows-fixture-manifest.ps1` proves the clean generated copy, a
legal pending-with-owner population and CRLF checkout stability. Its 15
fail-closed mutations cover missing and duplicate rows, unowned pending source,
artifact and environment records, forged and malformed current hashes,
pending-with-forged-hash, mutable and non-x64 identities, capability credit,
32-bit Office, noncanonical paths, historical binary source reuse and blank
cleanup.

## Acceptance record

The focused acceptance commands pass:

- `./scripts/sync-windows-fixture-manifest.ps1 -Check` — 57 deterministic rows,
  20 current source recipes and 37 pending source recipes;
- `./scripts/validate-windows-fixture-manifest.ps1` — exact six-matrix/57-row x64
  inventory, 57 pending built artifacts, 57 pending environments and no credit;
- `./scripts/test-windows-fixture-manifest.ps1` — three positive observations
  and 15 negative mutations.
- `./scripts/run-truth-reconciliation.ps1` — full check-only reconciliation,
  including the new sync and validator, with no generated/controller rewrite.

The sync check and positive validator are wired into governance and truth
reconciliation. The mutation suite remains a focused validator-maintenance
check.

## Residuals

- Dedicated controlled source/recipe delivery remains pending for 37 rows under
  the exact owners recorded in the manifest.
- Every built fixture remains pending; there are zero current built-artifact
  hashes and therefore zero binary-proof claims.
- Every environment hash remains pending; the dev host is noncertifying and the
  clean certification VM is not provisioned.
- Capability implementation, VM3/JIT parity, real COM/native transport and
  Excel64 certification remain with WIN-1 through WIN-14. This support artifact
  supplies inventory and ownership only.
- The broader `check-governance.ps1` currently stops before this lane at the
  pre-existing stale `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md` check.
  That generated controller surface is outside this bead and was not rewritten;
  the full truth-reconciliation gate and all fixture-specific gates pass.
