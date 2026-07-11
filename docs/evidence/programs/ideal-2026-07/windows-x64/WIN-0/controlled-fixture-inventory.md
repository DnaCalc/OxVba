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
ad-hoc binary is reused as current proof. Each row now has an explicit artifact
contract rather than a generic future path:

| artifact class | rows | immutable file name | admitted content |
|---|---:|---|---|
| `pe-dll-x64` | 23 | `fixture.dll` | PE32+ AMD64 executable with the DLL characteristic |
| `pe-exe-x64` | 18 | `fixture.exe` | PE32+ AMD64 executable without the DLL characteristic |
| `fixture-bundle-json-v1` | 16 | `fixture-bundle.json` | exact versioned JSON schema plus the row's ordered component-type contract |

The controlled root is exactly
`artifacts/windows-x64/controlled-fixtures/v1/<matrix-id>/<row-id>` with
lower-case immutable matrix and row IDs. A `current` transition must use that
exact root/name pair. The validator rejects traversal, reparse-point escape,
mutable aliases and historical/generated/evidence locations before reading
content.

Direct binaries are parsed as PE files: DOS and PE signatures, AMD64 machine
`0x8664`, PE32+ optional-header magic `0x020B`, section/header bounds,
executable characteristic and DLL-versus-EXE characteristic must all agree
with the row contract. Bundle JSON has an exact property-and-type schema bound
to `matrix_id`, `row_id`, `fixture_id`, `built_artifact_id`, x64 and the bundle
class. Its ordered components use immutable versioned IDs, exact controlled
component paths and raw-byte SHA-256 values; each component is then validated
as x64 DLL/EXE, MSFT typelib or nonblank UTF-8 VBA source as declared. The
top-level artifact hash is recomputed only after content validation.

### Environments

All 57 environment hashes remain pending because neither an immutable dev-host
capture nor the clean certification image has landed:

| environment | rows | pending owner | meaning |
|---|---:|---|---|
| `win-x64-dev-oracle-2026-07` | 12 | `bd-59co.3.1.2` | characterized, noncertifying development/oracle host |
| `win-x64-cert-vm-pending-v1` | 45 | `bd-59co.3.15.3` | clean pinned release-certification VM still blocking |

Environment hashes are separate from both source/recipe and built-artifact
hashes. Every row pins the canonical environment role, profile, target,
Office bitness, evidence state and this exact capture contract:

`artifacts/windows-x64/controlled-environments/v1/<environment-id>/environment-capture.json`

The capture must use schema
`oxvba-windows-x64-environment-capture-v1`, exact JSON properties and types,
and a versioned capture ID. It must reproduce the canonical environment ID,
role, `windows-x64` profile, x64 target, Office64 product/build/channel,
locale, OS build, evidence state, image identity and reset policy from
`IDEAL_ENVIRONMENT_MANIFEST_V1.csv`. The image must be pinned by SHA-256 and
the capture separately hashes the exact reset policy. Development-oracle
captures must remain explicitly noncertifying; certification captures must be
verified, authoritative and bind a pinned resettable snapshot/image. Thus the
current mutable development host and pending certification VM cannot be
promoted merely by hashing arbitrary notes.

The later WIN-0 reconciliation bead
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
- exact per-row artifact classes, roots, names, types and ordered bundle
  component contracts, with structural PE32+/AMD64 and exact JSON admission;
- exact versioned environment capture roots, names and schema, bound field by
  field to the canonical Windows x64/Office64 environment and its authority
  role, image and reset policy;
- normalized sorted deduplicated source paths and LF-stable hash recomputation;
- distinct source/recipe, built-artifact and environment state/hash/owner
  triples;
- an active exact owner for every pending triple;
- nonblank process, apartment, exact-signature, execution, cleanup and six-axis
  fields; and
- `capability_credit=none` on every row.

`scripts/test-windows-fixture-manifest.ps1` proves the clean generated copy, a
legal pending-with-owner population, CRLF checkout stability and successful
full-validator admission of controlled x64 DLL, x64 EXE, exact bundle and
canonical environment-capture samples. Its 31 fail-closed mutations cover the
original row/owner/hash/credit/source-path guards plus source text disguised as
a DLL, mutable and historical artifact aliases, artifact traversal, x86 PE and
wrong bundle schema. Environment mutations cover arbitrary text, mutable
alias, wrong identity, target architecture, Office bitness, role, traversal, mutable image,
dev-oracle certification flags and forged reset-policy binding.

## Acceptance record

The focused acceptance commands pass:

- `./scripts/sync-windows-fixture-manifest.ps1 -Check` — 57 deterministic rows,
  20 current source recipes and 37 pending source recipes;
- `./scripts/validate-windows-fixture-manifest.ps1` — exact six-matrix/57-row x64
  inventory, 57 pending built artifacts, 57 pending environments and no credit;
- `./scripts/test-windows-fixture-manifest.ps1` — six positive observations
  and 31 negative mutations, including full `current` admission probes.
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
